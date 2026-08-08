//! Where a card is.

use core::fmt::{self, Formatter};

use serde::de::{Error, IntoDeserializer, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The combat-scoped piles, and the only ones a `CombatCard` choice may name.
///
/// `Deck` is run-scoped; `Play` holds a card while its own action resolves.
/// Neither is selectable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatPile {
    Draw,
    Hand,
    Discard,
    Exhaust,
}

/// Where a card is. The game's `Entities.Cards.PileType`, minus its `None`,
/// which Rust spells `Option`. `Ord` to be compatible with BTreeMap.
///
/// Serialises flat — `"hand"`, `"deck"` — because it keys
/// [`Obs::piles`](crate::Obs::piles), and a JSON map key must be a string.
/// Derived impls would nest it and break that, hence the hand-written ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pile {
    Combat(CombatPile),
    /// The master deck: every card in the run.
    Deck,
    /// Played, not yet resolved.
    Play,
}

impl From<CombatPile> for Pile {
    fn from(p: CombatPile) -> Self {
        Pile::Combat(p)
    }
}

impl Serialize for Pile {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Pile::Combat(c) => c.serialize(s),
            Pile::Deck => s.serialize_str("deck"),
            Pile::Play => s.serialize_str("play"),
        }
    }
}

/// Reads a flat pile name, deferring the combat ones to [`CombatPile`]'s own
/// derived impl so the two lists cannot drift apart.
///
/// A visitor rather than `<&str>::deserialize` because `visit_str` is handed a
/// `&str` by every deserializer, borrowing or not, and the name never outlives
/// the call — nothing here needs to own it.
struct Name;

impl Visitor<'_> for Name {
    type Value = Pile;

    fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("a pile name")
    }

    fn visit_str<E: Error>(self, name: &str) -> Result<Pile, E> {
        match name {
            "deck" => Ok(Pile::Deck),
            "play" => Ok(Pile::Play),
            _ => CombatPile::deserialize(name.into_deserializer())
                .map(Pile::Combat)
                .map_err(|_: E| E::invalid_value(Unexpected::Str(name), &"a pile name")),
        }
    }
}

impl<'de> Deserialize<'de> for Pile {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_str(Name)
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn the_wire_form_is_flat() {
        let cases = [
            (Pile::Combat(CombatPile::Hand), r#""hand""#),
            (Pile::Deck, r#""deck""#),
            (Pile::Play, r#""play""#),
        ];
        for (pile, json) in cases {
            assert_eq!(serde_json::to_string(&pile).expect("serialises"), json);
            assert_eq!(serde_json::from_str::<Pile>(json).expect("parses"), pile);
        }
    }

    #[test]
    fn it_keys_a_map_as_a_bare_string() {
        let m = BTreeMap::from([(Pile::Combat(CombatPile::Hand), 1u8), (Pile::Deck, 2)]);
        let s = serde_json::to_string(&m).expect("serialises");
        assert_eq!(s, r#"{"hand":1,"deck":2}"#);
        assert_eq!(
            serde_json::from_str::<BTreeMap<Pile, u8>>(&s).expect("parses"),
            m
        );
    }

    /// A `StringDeserializer` hands over an owned `String`, which is what any
    /// deserializer that cannot lend out its input does. Reading a `&str` would
    /// reject this.
    #[test]
    fn a_name_need_not_be_borrowed() {
        use serde::de::value::{Error as ValueError, StringDeserializer};

        let d: StringDeserializer<ValueError> =
            alloc::string::String::from("exhaust").into_deserializer();
        assert_eq!(
            Pile::deserialize(d).expect("parses"),
            Pile::Combat(CombatPile::Exhaust)
        );
    }

    /// Neither arm may absorb a name it does not define.
    #[test]
    fn an_unknown_pile_name_is_refused() {
        assert!(serde_json::from_str::<Pile>(r#""limbo""#).is_err());
        assert!(serde_json::from_str::<CombatPile>(r#""deck""#).is_err());
    }

    /// Declaration order, which is the order a pile-keyed diff reports in.
    #[test]
    fn piles_sort_in_declaration_order() {
        let mut v = alloc::vec![
            Pile::Play,
            Pile::Deck,
            Pile::Combat(CombatPile::Hand),
            Pile::Combat(CombatPile::Draw)
        ];
        v.sort();
        assert_eq!(
            v,
            [
                Pile::Combat(CombatPile::Draw),
                Pile::Combat(CombatPile::Hand),
                Pile::Deck,
                Pile::Play
            ]
        );
    }
}
