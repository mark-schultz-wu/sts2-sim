//! `SpikedGauntlets`: one more energy each turn, and a Power costs one more.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyMaxEnergy`: its energy, for its owner. `TryModifyEnergyCostInCombat`:
/// one more on a Power its owner holds. A fight here has one player.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MaxEnergy { .. } => Modify::Added(V::Relic::SPIKED_GAUNTLETS_ENERGY),
        ModifyHook::EnergyCost {
            card: (_, card), ..
        } if <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Power) => Modify::Added(1),
        ModifyHook::EnergyCost { .. } => Modify::Nothing,
        hook => unimplemented!("what SpikedGauntlets does for {hook:?}"),
    }
}
