//! What a card is priced at, in the three states the game actually has.

use serde::de::{Deserializer, Error as _, Unexpected};
use serde::{Deserialize, Serialize};
use sts2_core::Cost;

/// A cost as the game sends it: a number, `"x"`, or `null`.
///
/// Three JSON shapes rather than one integer with a sentinel in it. A front
/// end that must remember `-1` means "not priced" prints `(-1)` the first time
/// somebody forgets.
///
/// `"x"` is its own value because the number is absent, not zero. An X-cost
/// card and a free card were the same integer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(crate) enum WireCost {
    Spelled(&'static str),
    Fixed(u8),
    /// A field the game left out entirely, which is the same as `null`: the
    /// thing is not priced in this pool.
    #[default]
    Absent,
}

impl WireCost {
    /// The cost this build understands, or an error naming what arrived.
    pub(crate) fn cost(self) -> Result<Cost, String> {
        match self {
            WireCost::Absent => Ok(Cost::None),
            WireCost::Spelled("x") => Ok(Cost::X),
            WireCost::Spelled(other) => Err(format!("unknown cost {other:?}")),
            WireCost::Fixed(n) => Ok(Cost::Fixed(n)),
        }
    }
}

impl<'de> Deserialize<'de> for WireCost {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
        match raw {
            None | Some(serde_json::Value::Null) => Ok(WireCost::Absent),
            Some(serde_json::Value::String(s)) if s == "x" => Ok(WireCost::Spelled("x")),
            Some(serde_json::Value::String(s)) => Err(D::Error::invalid_value(
                Unexpected::Str(&s),
                &"a number, \"x\", or null",
            )),
            Some(serde_json::Value::Number(n)) => match n.as_u64() {
                // The sentinel is the mod's to resolve. One arriving here means
                // the mod is older than this client, and reading it as a cost
                // would put `-1` on a card box.
                Some(n) if n <= u64::from(u8::MAX) => Ok(WireCost::Fixed(n as u8)),
                _ => Err(D::Error::invalid_value(
                    Unexpected::Other(&n.to_string()),
                    &"a cost between 0 and 255, \"x\", or null",
                )),
            },
            Some(other) => Err(D::Error::invalid_type(
                Unexpected::Other(&other.to_string()),
                &"a number, \"x\", or null",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> Result<Cost, String> {
        serde_json::from_str::<WireCost>(json)
            .map_err(|e| e.to_string())?
            .cost()
    }

    #[test]
    fn the_three_states_are_three_shapes() {
        assert_eq!(read("2"), Ok(Cost::Fixed(2)));
        assert_eq!(read("0"), Ok(Cost::Fixed(0)));
        assert_eq!(read("\"x\""), Ok(Cost::X));
        assert_eq!(read("null"), Ok(Cost::None));
    }

    /// The bug this type exists for. `-1` is how the game says a card is not
    /// priced in a pool. It used to travel as a number, and get printed.
    #[test]
    fn the_sentinel_is_refused_rather_than_shown() {
        let err = read("-1").expect_err("-1 is not a cost");
        assert!(err.contains("-1"), "the error says what arrived: {err}");
    }

    #[test]
    fn nonsense_is_refused() {
        assert!(read("\"free\"").is_err());
        assert!(read("[]").is_err());
        assert!(read("300").is_err());
    }
}
