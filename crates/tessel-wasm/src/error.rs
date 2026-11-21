//! The core's errors, as the page sees them: a `TesselError` whose `code` names the
//! case (`group:pending-proposals`, `store:unreadable`…), for the app to react to
//! without parsing messages.
//!
//! The codes come from `tessel-api` ([`ApiError`]); this module only builds the JS error.

use std::fmt::Display;

use js_sys::{Error, Reflect};
use tessel_api::ApiError;
use wasm_bindgen::JsValue;

/// A `TesselError` with its code.
pub(crate) fn fail(code: &str, message: impl Display) -> JsValue {
    let error = Error::new(&message.to_string());
    error.set_name("TesselError");
    // Setting a property on a fresh object cannot fail.
    let _ = Reflect::set(&error, &JsValue::from_str("code"), &JsValue::from_str(code));
    error.into()
}

/// `?` on an API or a conversion result, in a function returning `Result<_, JsValue>`.
pub(crate) trait OrJs<T> {
    fn js(self) -> Result<T, JsValue>;
}

impl<T> OrJs<T> for Result<T, ApiError> {
    fn js(self) -> Result<T, JsValue> {
        self.map_err(|error| fail(&error.code, &error.message))
    }
}

/// A value the page handed that does not have the expected shape.
impl<T> OrJs<T> for Result<T, serde_wasm_bindgen::Error> {
    fn js(self) -> Result<T, JsValue> {
        self.map_err(|error| fail("input:malformed", error))
    }
}
