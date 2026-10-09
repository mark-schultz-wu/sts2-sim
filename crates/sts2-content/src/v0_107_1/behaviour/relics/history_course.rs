//! `HistoryCourse`: each turn after the first, a dupe of the last Attack or
//! Skill its owner played the turn before, auto-played.

use sts2_core::{CardKind, EventHook, HookEffect, HookEffects};

use super::super::super::V0_107_1 as V;

/// `CardType` Attack and Skill, which `(uint)(type - 1) <= 1` keeps.
const KINDS: &[CardKind] = &[CardKind::Attack, CardKind::Skill];

/// `AfterAutoPrePlayPhaseEntered` from its owner's second turn on. A relic's
/// owner is the player, and a fight here has one, so the turn is its owner's
/// and every card played was.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut HookEffects<V>) {
    match hook {
        EventHook::AutoPrePlayPhase { turn } if turn > 1 => {
            asked.push(HookEffect::AutoPlayDupeOfLastPlay { kinds: KINDS });
        }
        EventHook::AutoPrePlayPhase { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what HistoryCourse does for {hook:?}"),
    }
}
