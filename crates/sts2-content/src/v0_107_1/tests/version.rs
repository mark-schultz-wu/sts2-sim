//! The build as a type, and the claim that makes it usable on a wire.
//!
//! `ContentVersion` exists so that a client or a simulator names a build once
//! and every id follows from it. That is only free if a typed id spells exactly
//! what the game spells. Otherwise every call site needs an adapter, and an
//! adapter is a second spelling waiting to disagree with the first.

use super::super::*;
use sts2_core::ContentVersion;

/// Serialized, each id is the game's own string and nothing around it.
///
/// The pool-wrapped `Cards` is the one with room to go wrong. It is an enum of
/// enums, and externally tagged it puts `{"Ironclad": …}` on the wire. It is
/// `#[serde(untagged)]`, so it does not. The ten cards whose ids carry a pool
/// suffix keep it in the rename, so `Strike` in the Ironclad pool still says
/// `STRIKE_IRONCLAD`.
#[test]
fn every_id_spells_itself_the_way_the_game_does() {
    fn wire<T: serde::Serialize>(value: T) -> String {
        serde_json::to_string(&value).unwrap()
    }

    assert_eq!(
        wire(<V0_107_1 as ContentVersion>::Character::Ironclad),
        "\"IRONCLAD\""
    );
    assert_eq!(wire(cards::IroncladCard::Strike), "\"STRIKE_IRONCLAD\"");
    assert_eq!(
        wire(<V0_107_1 as ContentVersion>::Card::Ironclad(
            cards::IroncladCard::Strike
        )),
        "\"STRIKE_IRONCLAD\"",
        "the pool wrapper must not tag the wire form"
    );
    assert_eq!(
        wire(<V0_107_1 as ContentVersion>::Modifier::Draft),
        "\"DRAFT\""
    );
    assert_eq!(
        wire(<V0_107_1 as ContentVersion>::Orb::PlasmaOrb),
        "\"PLASMA_ORB\""
    );
}

/// A typed id round-trips, so a response can be parsed into one.
#[test]
fn an_id_parses_back_from_the_wire() {
    let card: <V0_107_1 as ContentVersion>::Card =
        serde_json::from_str("\"STRIKE_IRONCLAD\"").expect("a real card");
    assert_eq!(card, cards::Cards::Ironclad(cards::IroncladCard::Strike));

    serde_json::from_str::<<V0_107_1 as ContentVersion>::Card>("\"NOT_A_CARD\"")
        .expect_err("an id the tables do not have is not a card");
}

/// The type carries the build it was generated from.
///
/// This is what makes the parameter more than typing. A client can compare the
/// build it is talking to against the build its tables describe, and refuse
/// rather than quietly drive the wrong game.
#[test]
fn the_type_knows_which_build_it_describes() {
    assert_eq!(<V0_107_1 as ContentVersion>::VERSION, VERSION);
    assert_eq!(VERSION.name, "v0.107.1");
    assert_eq!(VERSION.assembly_hash, 1_692_500_715);
}
