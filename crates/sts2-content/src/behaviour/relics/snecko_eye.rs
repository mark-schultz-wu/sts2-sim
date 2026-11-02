//! `SneckoEye`: Confused as each fight opens, and two more cards in each
//! hand.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, PowerAmount};

use crate::behaviour::relics::Number;
use crate::ids::PowerIds as _;

/// `PowerCmd.Apply<ConfusedPower>(…, 1m, …)`.
const CONFUSED: PowerAmount<Number> = PowerAmount::Counted(1);

/// `BeforeCombatStart`: Confused on its owner. `AfterObtained` does the same
/// inside a fight, and a run picks it up only outside one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatStarting { .. } => {
            asked.push(HookEffect::ApplyToOwner(V::Power::CONFUSED_POWER, CONFUSED));
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what SneckoEye does for {hook:?}"),
    }
}

/// `ModifyHandDraw`: two more cards on every turn.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::HandDraw { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::HandDraw { .. } => Modify::Added(V::Relic::SNECKO_EYE_CARDS),
        hook => unimplemented!("what SneckoEye does for {hook:?}"),
    }
}
