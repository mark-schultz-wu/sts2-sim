//! One card instance, and the two enums the game classifies cards by.

use alloc::string::String;

use serde::{Deserialize, Serialize};

/// The game's `Entities.Cards.CardType`, minus its `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Attack,
    Skill,
    Power,
    Status,
    Curse,
    Quest,
}

/// The game's `Entities.Cards.CardRarity`, minus its `None`.
///
/// Overlaps [`CardType`] on `Status`, `Curse` and `Quest`, because the game
/// treats those as rarities too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardRarity {
    Basic,
    Common,
    Uncommon,
    Rare,
    Ancient,
    Event,
    Token,
    Status,
    Curse,
    Quest,
}

/// One card instance.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    pub id: String,
    /// Stable per-instance identity, so a card can be followed between piles.
    ///
    /// Not the game's own handle: STS1's `uuid` is `UUID.randomUUID()`, which
    /// is process-local and so uncomparable across implementations. The
    /// determinism mod substitutes a construction counter. STS2 exposes no
    /// per-instance handle, so it is absent there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<i32>,
    /// How many times this card has been upgraded. Zero means unupgraded.
    ///
    /// A count rather than a flag: the game's `CardModel.CurrentUpgradeLevel`,
    /// and STS1's Searing Blow upgrades without bound.
    ///
    /// Enchantment is a separate axis (`CardModel.Enchantment`) and has no
    /// field yet.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub upgrade_level: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CardType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rarity: Option<CardRarity>,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_is_a_level_not_a_flag() {
        let once = Card {
            id: "Searing Blow".into(),
            upgrade_level: 1,
            ..Card::default()
        };
        let thrice = Card {
            upgrade_level: 3,
            ..once.clone()
        };
        assert_ne!(once, thrice);

        let fresh = Card {
            upgrade_level: 0,
            ..once.clone()
        };
        let s = serde_json::to_string(&fresh).expect("serialises");
        assert!(!s.contains("upgrade_level"), "zero must be omitted: {s}");
    }
}
