//! `PaelsBlood`: one more card in every hand.

use crate::vars::RelicVars as _;
use sts2_core::{Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyHandDraw`: one more card for its owner each turn.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::HandDraw { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::HandDraw { .. } => Modify::Added(V::Relic::PAELS_BLOOD_CARDS),
        hook => unimplemented!("what PaelsBlood does for {hook:?}"),
    }
}
