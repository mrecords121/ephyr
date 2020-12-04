//! Custom [`serde`] serialization/deserialization functions for [`Duration`]
//! in a whole seconds format.

use std::time::Duration;

use serde::{Deserialize as _, Deserializer, Serializer};

/// Serializes [`Duration`] as whole seconds.
///
/// # Errors
///
/// Never errors.
#[inline]
pub fn serialize<S>(dur: &Duration, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ser.serialize_u64(dur.as_secs())
}

/// Deserializes [`Duration`] from whole seconds.
///
/// # Errors
///
/// If an input is not a 64-bits unsigned integer number ([`u64`]).
pub fn deserialize<'a, D>(de: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'a>,
{
    let secs = u64::deserialize(de)?;
    Ok(Duration::from_secs(secs))
}

#[cfg(test)]
mod spec {
    use std::time::Duration;

    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    struct Test(#[serde(with = "super")] Duration);

    #[test]
    fn serializes_correctly() {
        for (input, expected) in &[
            (0, "0"),
            (1, "1"),
            (60, "60"),
            (3600, "3600"),
            (86400, "86400"),
            (10 * 86400 + 67, "864067"),
        ] {
            let input = Test(Duration::from_secs(*input));
            let actual =
                serde_json::to_string(&input).expect("Failed to serialize");

            assert_eq!(actual, expected.to_string());
        }
    }

    #[test]
    fn deserializes_correctly() {
        for (input, expected) in &[
            ("0", 0),
            ("1", 1),
            ("60", 60),
            ("3600", 3600),
            ("86400", 86400),
            ("864067", 10 * 86400 + 67),
        ] {
            let actual: Test =
                serde_json::from_str(*input).expect("Failed to deserialize");

            assert_eq!(actual.0, Duration::from_secs(*expected));
        }
    }
}
