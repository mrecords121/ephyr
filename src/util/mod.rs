//! Utils and helpers.

pub mod serde;

use std::any::Any;

/// Interprets given [panic payload][1] as displayable message.
///
/// [1]: std::panic::PanicInfo::payload
pub fn display_panic<'a>(err: &'a (dyn Any + Send + 'static)) -> &'a str {
    if let Some(s) = err.downcast_ref::<&str>() {
        return s;
    }
    if let Some(s) = err.downcast_ref::<String>() {
        return s.as_str();
    }
    "Box<Any>"
}
