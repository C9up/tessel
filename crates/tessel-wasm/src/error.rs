//! The core's errors, as the page sees them: a `TesselError` whose `code` names the
//! case (`group:pending-proposals`, `store:unreadable`…), for the app to react to
//! without parsing messages.

use std::fmt::Display;

use js_sys::{Error, Reflect};
use tessel_core::{
    ClientError, GroupError, IdentityError, Ignored, KeyPackageError, MediaError, MessageError,
    SafetyError, StoreError,
};
use wasm_bindgen::JsValue;

/// A `TesselError` with its code.
pub(crate) fn fail(code: &str, message: impl Display) -> JsValue {
    let error = Error::new(&message.to_string());
    error.set_name("TesselError");
    // Setting a property on a fresh object cannot fail.
    let _ = Reflect::set(&error, &JsValue::from_str("code"), &JsValue::from_str(code));
    error.into()
}

/// Bytes handed by the page that are not what the call expects.
pub(crate) fn malformed(what: &str) -> JsValue {
    fail("input:malformed", format_args!("malformed {what}"))
}

/// The stable code of an error.
pub(crate) trait Code: Display {
    fn code(&self) -> String;

    fn into_js(self) -> JsValue
    where
        Self: Sized,
    {
        fail(&self.code(), &self)
    }
}

/// `?` on a core result, in a function returning `Result<_, JsValue>`.
pub(crate) trait OrJs<T> {
    fn js(self) -> Result<T, JsValue>;
}

impl<T, E: Code> OrJs<T> for Result<T, E> {
    fn js(self) -> Result<T, JsValue> {
        self.map_err(Code::into_js)
    }
}

pub(crate) fn ignored(reason: Ignored) -> &'static str {
    match reason {
        Ignored::NotAuthor => "not-author",
        Ignored::UnknownTarget => "unknown-target",
        Ignored::AlreadyDeleted => "already-deleted",
        Ignored::DuplicateId => "duplicate-id",
    }
}

impl Code for StoreError {
    fn code(&self) -> String {
        let case = match self {
            Self::NoRandomness => "no-randomness",
            Self::InvalidKey => "invalid-key",
            Self::Unreadable => "unreadable",
            Self::UnsupportedFormat(_) => "unsupported-format",
            Self::Malformed => "malformed",
            Self::Missing(_) => "missing",
            Self::WrongPartition => "wrong-partition",
            Self::Crypto => "crypto",
            Self::UnsupportedSchema(_) => "unsupported-schema",
            Self::Backend(_) => "backend",
        };
        format!("store:{case}")
    }
}

impl Code for IdentityError {
    fn code(&self) -> String {
        let case = match self {
            Self::InvalidAccount(_) => "invalid-account",
            Self::Crypto => "crypto",
            Self::InvalidKey => "invalid-key",
            Self::MalformedCertificate => "malformed-certificate",
            Self::BadSignature => "bad-signature",
            Self::KeyMismatch => "key-mismatch",
            Self::NotBasicCredential => "not-basic-credential",
            Self::MalformedChallenge => "malformed-challenge",
        };
        format!("identity:{case}")
    }
}

impl Code for MessageError {
    fn code(&self) -> String {
        let case = match self {
            Self::UnsupportedVersion(_) => "unsupported-version",
            Self::UnknownKind(_) => "unknown-kind",
            Self::Malformed => "malformed",
            Self::Invalid(_) => "invalid",
            Self::NoRandomness => "no-randomness",
        };
        format!("message:{case}")
    }
}

impl Code for GroupError {
    fn code(&self) -> String {
        let case = match self {
            Self::Identity(e) => return e.code(),
            Self::Message(e) => return e.code(),
            Self::Store(e) => return e.code(),
            Self::Ignored(reason) => return format!("group:ignored:{}", ignored(*reason)),
            Self::UnknownGroup => "unknown-group",
            Self::Malformed => "malformed",
            Self::WrongCiphersuite => "wrong-ciphersuite",
            Self::PendingCommit => "pending-commit",
            Self::NoPendingCommit => "no-pending-commit",
            Self::PendingProposals => "pending-proposals",
            Self::OwnMessage => "own-message",
            Self::NothingToCommit => "nothing-to-commit",
            Self::Mls(_) => "mls",
        };
        format!("group:{case}")
    }
}

impl Code for KeyPackageError {
    fn code(&self) -> String {
        let case = match self {
            Self::Identity(e) => return e.code(),
            Self::Creation => "creation",
            Self::Malformed => "malformed",
            Self::WrongCiphersuite => "wrong-ciphersuite",
            Self::Invalid(_) => "invalid",
        };
        format!("key-package:{case}")
    }
}

impl Code for MediaError {
    fn code(&self) -> String {
        let case = match self {
            Self::NoRandomness => "no-randomness",
            Self::UnsupportedFormat(_) => "unsupported-format",
            Self::Truncated => "truncated",
            Self::Altered => "altered",
            Self::TooLarge => "too-large",
        };
        format!("media:{case}")
    }
}

impl Code for SafetyError {
    fn code(&self) -> String {
        let case = match self {
            Self::Store(e) => return e.code(),
            Self::Malformed => "malformed",
            Self::UnsupportedVersion(_) => "unsupported-version",
            Self::Mismatch => "mismatch",
            Self::KeyChanged => "key-changed",
            Self::NotVerified => "not-verified",
        };
        format!("safety:{case}")
    }
}

impl Code for ClientError {
    fn code(&self) -> String {
        match self {
            Self::Identity(e) => e.code(),
            Self::Store(e) => e.code(),
        }
    }
}

impl Code for serde_wasm_bindgen::Error {
    fn code(&self) -> String {
        "input:malformed".to_owned()
    }
}
