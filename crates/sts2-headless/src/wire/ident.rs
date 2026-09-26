//! Unpacking the three things the oracle packs into one string each.
//!
//! `sts2_core` owns [`Coord`], [`CardIdent`] and [`ModelId`] and carries no
//! `serde`. So this is where `"3,11"`, `"STRIKE_IRONCLAD+1/SMOG*ADROIT:1"` and
//! `"MONSTER/BYRDPIP"` live, on the same rule the rest of this module follows.
//!
//! Every one of these refuses input the oracle never sends. The crate README
//! says why, and why a coordinate having two spellings is fine.

use serde::de::DeserializeOwned;
use serde::de::value::{Error as ValueError, StrDeserializer};
use sts2_core::{CardIdent, Coord, ModelId};

/// Why a packed string is not the thing it was read as.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Not the shape at all: a coordinate with no comma, an id with no slash.
    #[error("{text:?} is not {want}")]
    Shape { text: String, want: &'static str },
    /// The right shape, with something in a number's place that is not one.
    #[error("{text:?} has no {part} number")]
    Number { text: String, part: &'static str },
    /// A model category this build has no type for.
    #[error("{text:?} names the {category} category, which has no type here")]
    Category { text: String, category: String },
    /// A name that is not one of this build's, for the kind it was read as.
    #[error("{text:?} names no {kind} in this build")]
    Entry { text: String, kind: &'static str },
}

/// A map coordinate, as the observation writes it: `col,row`.
pub fn coord(text: &str) -> Result<Coord, Error> {
    let shape = || Error::Shape {
        text: text.to_string(),
        want: "a `col,row` coordinate",
    };
    let number = |part| Error::Number {
        text: text.to_string(),
        part,
    };

    let (col, row) = text.split_once(',').ok_or_else(shape)?;
    if row.contains(',') {
        return Err(shape());
    }

    Ok(Coord {
        col: col.parse().map_err(|_| number("column"))?,
        row: row.parse().map_err(|_| number("row"))?,
    })
}

/// A card's identity: `CARD`, and then each of `+2`, `/AFFLICTION` and
/// `*ENCHANTMENT:AMOUNT` that the card has, in that order.
///
/// Each part is present only when `Observation.Ident` writes it. A literal `+0`
/// is refused rather than read as unupgraded, because the oracle produces
/// none.
///
/// Read right to left, because that is the order that cannot be confused.
/// Taking the last separator first leaves each step one to find.
pub fn card_ident<V: sts2_content_wire::WireContentVersion>(
    text: &str,
) -> Result<CardIdent<V>, Error> {
    let (head, enchantment) = match text.split_once('*') {
        Some((head, enchanted)) => {
            let (name, amount) = enchanted.split_once(':').ok_or_else(|| Error::Shape {
                text: text.to_string(),
                want: "a card identity — an enchantment carries its `:AMOUNT`",
            })?;
            let amount = amount.parse().map_err(|_| Error::Number {
                text: text.to_string(),
                part: "enchantment amount",
            })?;
            let enchantment = entry(text, "enchantment", name)?;
            (
                head,
                Some(sts2_core::Enchanted {
                    enchantment,
                    amount,
                }),
            )
        }
        None => (text, None),
    };

    let (head, affliction) = match head.split_once('/') {
        Some((head, name)) => (head, Some(entry(text, "affliction", name)?)),
        None => (head, None),
    };

    let (name, upgrade) = match head.split_once('+') {
        Some((name, level)) => {
            let level: u8 = level.parse().map_err(|_| Error::Number {
                text: text.to_string(),
                part: "upgrade",
            })?;
            if level == 0 {
                return Err(Error::Shape {
                    text: text.to_string(),
                    want: "a card identity — an unupgraded card carries no `+0`",
                });
            }
            (name, level)
        }
        None => (head, 0),
    };

    Ok(CardIdent {
        card: entry(text, "card", name)?,
        upgrade,
        affliction,
        enchantment,
    })
}

/// A model id, as the observation writes it: `CATEGORY/ENTRY`.
///
/// The categories are the game's own. `ModelId.Category` is the slugified name
/// of the direct `AbstractModel` subclass, so `MonsterModel` is `MONSTER` and
/// `CardPoolModel` is `CARD_POOL`. The thirteen below are the ones
/// [`ContentVersion`] names a type for. The six it does not are refused by
/// name.
pub fn model_id<V: sts2_content_wire::WireContentVersion>(text: &str) -> Result<ModelId<V>, Error> {
    let (category, name) = text.split_once('/').ok_or_else(|| Error::Shape {
        text: text.to_string(),
        want: "a `CATEGORY/ENTRY` model id",
    })?;

    Ok(match category {
        "ACT" => ModelId::Act(entry(text, "act", name)?),
        "AFFLICTION" => ModelId::Affliction(entry(text, "affliction", name)?),
        "CARD" => ModelId::Card(entry(text, "card", name)?),
        "CHARACTER" => ModelId::Character(entry(text, "character", name)?),
        "ENCHANTMENT" => ModelId::Enchantment(entry(text, "enchantment", name)?),
        "ENCOUNTER" => ModelId::Encounter(entry(text, "encounter", name)?),
        "EVENT" => ModelId::Event(entry(text, "event", name)?),
        "MODIFIER" => ModelId::Modifier(entry(text, "modifier", name)?),
        "MONSTER" => ModelId::Monster(entry(text, "monster", name)?),
        "ORB" => ModelId::Orb(entry(text, "orb", name)?),
        "POTION" => ModelId::Potion(entry(text, "potion", name)?),
        "POWER" => ModelId::Power(entry(text, "power", name)?),
        "RELIC" => ModelId::Relic(entry(text, "relic", name)?),
        other => {
            return Err(Error::Category {
                text: text.to_string(),
                category: other.to_string(),
            });
        }
    })
}

/// One name, read as the generated id it names, or the failure that names the
/// kind it was read as.
fn entry<T: DeserializeOwned>(text: &str, kind: &'static str, name: &str) -> Result<T, Error> {
    named(name).ok_or_else(|| Error::Entry {
        text: text.to_string(),
        kind,
    })
}

/// One generated id, read from its own name.
///
/// The ids spell themselves through serde and nowhere else. So this goes
/// through the same rename a wire message uses, rather than through a second
/// lookup free to disagree with it. No JSON: a `StrDeserializer` is the name
/// itself, with nothing parsed and nothing allocated.
pub(crate) fn named<T: DeserializeOwned>(name: &str) -> Option<T> {
    T::deserialize(StrDeserializer::<ValueError>::new(name)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1;

    type V = V0_107_1;
    type Card = <V as sts2_core::ContentVersion>::Card;
    type Affliction = <V as sts2_core::ContentVersion>::Affliction;
    type Enchantment = <V as sts2_core::ContentVersion>::Enchantment;

    /// One id, read the way a generated table spells it.
    fn id<T: DeserializeOwned>(name: &str) -> T {
        entry(name, "test", name).expect("a name this build has")
    }

    #[test]
    fn a_coordinate_is_a_column_and_a_row_in_that_order() {
        assert_eq!(coord("3,11").unwrap(), Coord::new(3, 11));
        assert_eq!(coord("0,0").unwrap(), Coord::new(0, 0));

        // Column first. The two are only distinguishable when they differ, and
        // getting them backwards is a bug that hides on a square map.
        let c = coord("1,7").unwrap();
        assert_eq!((c.col, c.row), (1, 7));

        // A byte at each end, which is exactly what `MapCoord.Serialize`
        // writes.
        assert_eq!(coord("255,255").unwrap(), Coord::new(255, 255));
    }

    /// Anything the oracle never writes is refused, not repaired.
    ///
    /// The list is the shapes a tolerant parser quietly accepts. Each one means
    /// something changed: the mod's format string, or who is on the other end.
    /// A coordinate invented from one is a legal move to a square that was
    /// never offered.
    #[test]
    fn a_coordinate_refuses_what_the_oracle_never_sends() {
        for text in ["", "3", "3;11", "3,11,0", "3, 11", " 3,11", "3,11 "] {
            assert!(coord(text).is_err(), "{text:?} should not parse");
        }

        // Out of range at either end, and the sign the game has no room for.
        for text in ["-1,0", "256,0", "0,256"] {
            assert!(matches!(coord(text), Err(Error::Number { .. })), "{text}");
        }

        assert_eq!(
            coord("3").unwrap_err().to_string(),
            "\"3\" is not a `col,row` coordinate"
        );
    }

    /// The three things a card carries into a pile, in every combination.
    ///
    /// Exhaustive on purpose. Each part is written only when the card has
    /// something to say, so there are eight shapes. The parser has to read all
    /// of them off one string.
    #[test]
    fn a_card_identity_carries_the_upgrade_the_affliction_and_the_enchantment() {
        let card: Card = id("STRIKE_IRONCLAD");
        let smog: Affliction = id("SMOG");
        let adroit = sts2_core::Enchanted::<V> {
            enchantment: id::<Enchantment>("ADROIT"),
            amount: 2,
        };

        let cases = [
            ("STRIKE_IRONCLAD", 0, None, None),
            ("STRIKE_IRONCLAD+1", 1, None, None),
            ("STRIKE_IRONCLAD/SMOG", 0, Some(smog), None),
            ("STRIKE_IRONCLAD*ADROIT:2", 0, None, Some(adroit)),
            ("STRIKE_IRONCLAD+2/SMOG", 2, Some(smog), None),
            ("STRIKE_IRONCLAD+2*ADROIT:2", 2, None, Some(adroit)),
            ("STRIKE_IRONCLAD/SMOG*ADROIT:2", 0, Some(smog), Some(adroit)),
            (
                "STRIKE_IRONCLAD+2/SMOG*ADROIT:2",
                2,
                Some(smog),
                Some(adroit),
            ),
        ];

        for (text, upgrade, affliction, enchantment) in cases {
            assert_eq!(
                card_ident::<V>(text).unwrap(),
                CardIdent {
                    card,
                    upgrade,
                    affliction,
                    enchantment
                },
                "{text}"
            );
        }

        assert_eq!(
            card_ident::<V>("STRIKE_IRONCLAD").unwrap(),
            CardIdent::new(card)
        );

        // All eight are different cards, which is the whole point of carrying
        // more than the id. Two decks that differ in any one of these are two
        // decks, and until the mod wrote the enchantment out the last four
        // collapsed onto the first four.
        let mut seen: Vec<CardIdent<V>> = Vec::new();
        for (text, ..) in cases {
            let ident = card_ident::<V>(text).unwrap();
            assert!(!seen.contains(&ident), "{text} is a repeat");
            seen.push(ident);
        }

        // The upgrade is a count. Two upgrades is not one upgrade, and a `bool`
        // here would have made them the same card.
        assert_ne!(
            card_ident::<V>("STRIKE_IRONCLAD+1").unwrap(),
            card_ident::<V>("STRIKE_IRONCLAD+2").unwrap()
        );
    }

    #[test]
    fn a_card_identity_refuses_a_shape_the_oracle_does_not_write() {
        // `Observation.Ident` writes the level only when it is above zero, so a
        // literal `+0` is not an unupgraded card — it is a sign that the rule
        // changed.
        assert!(matches!(
            card_ident::<V>("STRIKE_IRONCLAD+0"),
            Err(Error::Shape { .. })
        ));

        assert!(matches!(
            card_ident::<V>("STRIKE_IRONCLAD+x"),
            Err(Error::Number {
                part: "upgrade",
                ..
            })
        ));
        assert!(matches!(
            card_ident::<V>("NOT_A_CARD"),
            Err(Error::Entry { kind: "card", .. })
        ));
        assert!(matches!(
            card_ident::<V>("STRIKE_IRONCLAD/NOT_AN_AFFLICTION"),
            Err(Error::Entry {
                kind: "affliction",
                ..
            })
        ));

        assert!(matches!(
            card_ident::<V>("STRIKE_IRONCLAD*NOT_AN_ENCHANTMENT:1"),
            Err(Error::Entry {
                kind: "enchantment",
                ..
            })
        ));

        // A card id is not an affliction id, even though both are names.
        assert!(card_ident::<V>("STRIKE_IRONCLAD/BURN").is_err());

        // The order is fixed. `Observation.Ident` appends the affliction and
        // then the enchantment, so the reverse is not a card identity written
        // another way — it is a string nothing produces.
        assert!(card_ident::<V>("STRIKE_IRONCLAD*ADROIT:1/SMOG").is_err());

        // An enchantment always says how much of it the card carries.
        assert!(card_ident::<V>("STRIKE_IRONCLAD*ADROIT").is_err());
    }

    /// Every category [`ContentVersion`] has a type for, once each.
    #[test]
    fn a_model_id_is_a_category_and_an_entry() {
        assert_eq!(
            model_id::<V>("ACT/OVERGROWTH").unwrap(),
            ModelId::Act(id("OVERGROWTH"))
        );
        assert_eq!(
            model_id::<V>("AFFLICTION/SMOG").unwrap(),
            ModelId::Affliction(id("SMOG"))
        );
        assert_eq!(
            model_id::<V>("CARD/BURN").unwrap(),
            ModelId::Card(id("BURN"))
        );
        assert_eq!(
            model_id::<V>("CHARACTER/DEFECT").unwrap(),
            ModelId::Character(id("DEFECT"))
        );
        assert_eq!(
            model_id::<V>("ENCHANTMENT/ADROIT").unwrap(),
            ModelId::Enchantment(id("ADROIT"))
        );
        assert_eq!(
            model_id::<V>("ENCOUNTER/AEONGLASS_BOSS").unwrap(),
            ModelId::Encounter(id("AEONGLASS_BOSS"))
        );
        assert_eq!(
            model_id::<V>("EVENT/LOST_WISP").unwrap(),
            ModelId::Event(id("LOST_WISP"))
        );
        assert_eq!(
            model_id::<V>("MODIFIER/ALL_STAR").unwrap(),
            ModelId::Modifier(id("ALL_STAR"))
        );
        assert_eq!(
            model_id::<V>("MONSTER/BYRDPIP").unwrap(),
            ModelId::Monster(id("BYRDPIP"))
        );
        assert_eq!(
            model_id::<V>("ORB/DARK_ORB").unwrap(),
            ModelId::Orb(id("DARK_ORB"))
        );
        assert_eq!(
            model_id::<V>("POTION/ASHWATER").unwrap(),
            ModelId::Potion(id("ASHWATER"))
        );
        assert_eq!(
            model_id::<V>("POWER/SLOTH_POWER").unwrap(),
            ModelId::Power(id("SLOTH_POWER"))
        );
        assert_eq!(
            model_id::<V>("RELIC/BYRDPIP").unwrap(),
            ModelId::Relic(id("BYRDPIP"))
        );
    }

    /// The reason a model id carries both halves.
    ///
    /// `BYRDPIP` is a monster and a relic, and `LOST_WISP` an event and a
    /// relic. The entry alone is two different pieces of content wearing one
    /// name, and both are reachable in an observation. A creature's model is a
    /// monster, and either a monster or a relic can be what stopped a card.
    #[test]
    fn an_entry_that_two_categories_both_have() {
        assert_ne!(
            model_id::<V>("MONSTER/BYRDPIP").unwrap(),
            model_id::<V>("RELIC/BYRDPIP").unwrap()
        );
        assert_ne!(
            model_id::<V>("EVENT/LOST_WISP").unwrap(),
            model_id::<V>("RELIC/LOST_WISP").unwrap()
        );

        // And the cross pairings are not content at all.
        assert!(model_id::<V>("MONSTER/LOST_WISP").is_err());
        assert!(model_id::<V>("EVENT/BYRDPIP").is_err());
    }

    /// The six categories the build has and this does not.
    ///
    /// Every direct subclass of `AbstractModel` is a category, and
    /// [`ContentVersion`] names a type for thirteen of the nineteen. The rest
    /// are refused by name rather than read as something else, so an
    /// observation carrying one says which kind is missing.
    #[test]
    fn a_category_with_no_type_is_refused_by_name() {
        for category in [
            "ACHIEVEMENT",
            "BADGE",
            "CARD_POOL",
            "POTION_POOL",
            "RELIC_POOL",
            "SINGLETON",
        ] {
            let text = format!("{category}/WHATEVER");
            match model_id::<V>(&text) {
                Err(Error::Category {
                    category: named, ..
                }) => assert_eq!(named, category),
                other => panic!("{text} should name its category: {other:?}"),
            }
        }

        // A lowercase category is not the same category: `ModelId.Category` is
        // `Slugify`d, which upper-cases, so the mod never sends one.
        assert!(matches!(
            model_id::<V>("monster/BYRDPIP"),
            Err(Error::Category { .. })
        ));

        assert!(matches!(model_id::<V>("BYRDPIP"), Err(Error::Shape { .. })));
    }
}
