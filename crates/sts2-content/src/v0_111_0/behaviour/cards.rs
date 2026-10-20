//! The cards whose rules this build ports for itself, which the shared cards
//! hand over through [`crate::build::OwnRules`].

use sts2_core::{Fight, Played};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{CardVar, Cards, IroncladCard};

type Number = i32;

/// What a card of this build's own does when it is played, or nothing for a
/// shared one.
pub(crate) fn play<F: Fight<V>>(card: Cards, _fight: &mut F) -> Option<Played<V>> {
    match card {
        // `ExpectAFight` gains its calculated block here.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            unimplemented!("Expect a Fight's play in v0.111.0")
        }
        _ => None,
    }
}

/// A number a card of this build's own works out, or nothing for a shared one.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    _key: CardVar,
    _printed: &P,
    _owner: sts2_core::CardOwner<'_, V>,
) -> Option<Number> {
    match card {
        // `ExpectAFight`: a base, and more for each Strength its owner has.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            unimplemented!("Expect a Fight's calculated block in v0.111.0")
        }
        _ => None,
    }
}
