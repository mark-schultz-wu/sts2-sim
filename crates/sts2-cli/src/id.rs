//! Reading and writing the names generated content carries on the wire.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde::de::value::{Error, StrDeserializer};

/// Reads one generated id from the spelling the game uses.
pub fn parse<T: DeserializeOwned>(name: &str) -> Result<T, Error> {
    serde::Deserialize::deserialize(StrDeserializer::new(name))
}

/// Returns the game spelling of a generated id, if it has the expected shape.
pub fn spelling<T: Serialize>(id: &T) -> Option<String> {
    match serde_json::to_value(id).ok()? {
        serde_json::Value::String(name) => Some(name),
        _ => None,
    }
}

/// Displays a generated id without making rendering fail on an unknown shape.
pub fn display<T: Serialize + std::fmt::Debug>(id: &T) -> String {
    spelling(id).unwrap_or_else(|| format!("{id:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1;

    type Card = <V0_107_1 as sts2_core::ContentVersion>::Card;

    #[test]
    fn an_id_round_trips_through_its_game_spelling() {
        let card: Card = parse("BASH").expect("a card");
        assert_eq!(spelling(&card).as_deref(), Some("BASH"));
        assert_eq!(display(&card), "BASH");
    }

    #[test]
    fn a_name_outside_the_generated_set_is_refused() {
        assert!(parse::<Card>("NOT_A_CARD").is_err());
    }
}
