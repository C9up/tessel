//! The core's errors, as the page sees them: a `TesselError` whose `code` names the
//! case (`group:pending-proposals`, `store:unreadable`…), for the app to react to
//! without parsing messages.
//!
//! Here they are an [`ApiError`]: its code and its message. Each binding turns it into
//! its platform's error (a JS `TesselError` for the web).

use std::fmt::{self, Display};

use serde::Serialize;
use tessel_core::{
    ClientError, CoreError, EnvelopeError, GroupError, IdentityError, Ignored, KeyPackageError,
    LinkError, MediaError, MessageError, ProfileError, SafetyError, StoreError,
};

/// An error with its stable code (`identity:bad-signature`, `input:malformed`, ...) and a
/// message for humans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

/// An error with its code.
pub fn fail(code: &str, message: impl Display) -> ApiError {
    ApiError {
        code: code.to_owned(),
        message: message.to_string(),
    }
}

/// Bytes handed by the page that are not what the call expects.
pub fn malformed(what: &str) -> ApiError {
    fail("input:malformed", format_args!("malformed {what}"))
}

/// The stable code of an error.
pub trait Code: Display {
    fn code(&self) -> String;

    fn into_api(self) -> ApiError
    where
        Self: Sized,
    {
        fail(&self.code(), &self)
    }
}

/// `?` on a core result, in a function returning `Result<_, ApiError>`.
pub(crate) trait OrApi<T> {
    fn api(self) -> Result<T, ApiError>;
}

impl<T, E: Code> OrApi<T> for Result<T, E> {
    fn api(self) -> Result<T, ApiError> {
        self.map_err(Code::into_api)
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

impl Code for LinkError {
    fn code(&self) -> String {
        let case = match self {
            Self::Identity(error) => return error.code(),
            Self::Store(error) => return error.code(),
            Self::MalformedQr => "malformed-qr",
            Self::BadGrant => "bad-grant",
            Self::WrongCertificate => "wrong-certificate",
            Self::NoAccountKey => "no-account-key",
            Self::Crypto => "crypto",
        };
        format!("link:{case}")
    }
}

impl Code for ProfileError {
    fn code(&self) -> String {
        let case = match self {
            Self::Store(error) => return error.code(),
            Self::Invalid(_) => "invalid",
            Self::Altered => "altered",
            Self::UnknownKey => "unknown-key",
            Self::UnsupportedVersion(_) => "unsupported-version",
            Self::Crypto => "crypto",
        };
        format!("profile:{case}")
    }
}

impl Code for EnvelopeError {
    fn code(&self) -> String {
        match self {
            Self::Malformed => "envelope:malformed".into(),
        }
    }
}

/// The web binding throws these as a plain `Error` (the page codes it `host:Error`);
/// other bindings get a code of their own.
impl Code for CoreError {
    fn code(&self) -> String {
        let case = match self {
            Self::UnsupportedCiphersuite => "unsupported-ciphersuite",
            Self::NoRandomness => "no-randomness",
        };
        format!("core:{case}")
    }
}
