//! `HistoryCourse`: each turn after the first, a dupe of the last Attack or
//! Skill its owner played the turn before, auto-played.

use sts2_core::{CardKind, EventHook, HookEffect, HookEffects};

/// `CardType` Attack and Skill, which `(uint)(type - 1) <= 1` keeps.
const KINDS: &[CardKind] = &[CardKind::Attack, CardKind::Skill];

/// `AfterAutoPrePlayPhaseEntered` from its owner's second turn on. A fight
/// here has one player, so every card played was its owner's.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut HookEffects<V>,
) {
    match hook {
        EventHook::AutoPrePlayPhase { player, turn } if player == owner && turn > 1 => {
            asked.push(HookEffect::AutoPlayDupeOfLastPlay { kinds: KINDS });
        }
        EventHook::AutoPrePlayPhase { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what HistoryCourse does for {hook:?}"),
    }
}
