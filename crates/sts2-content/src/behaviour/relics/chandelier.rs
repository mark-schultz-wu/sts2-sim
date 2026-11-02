//! `Chandelier`: energy on the third turn of each fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `_energyTurn`.
const TURN: u32 = 3;

/// `AfterSideTurnStart` on its owner's side: its energy on the owner's third
/// turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        participants, turn, ..
    } = hook
        && participants.contains(owner)
        && turn == TURN
    {
        asked.push(HookEffect::GainEnergy(V::Relic::CHANDELIER_ENERGY));
    }
}
