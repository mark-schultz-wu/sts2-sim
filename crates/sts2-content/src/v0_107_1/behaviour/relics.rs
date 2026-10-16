//! The relics whose rules this build ports for itself, which the shared
//! relics hand over through [`crate::build::OwnRules`].

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::V0_107_1 as V;
use super::super::generated;
use crate::behaviour::relics::State;
use crate::generated::runstate::CombatSide;

/// `EventHook` for a relic of this build's own.
pub(crate) fn own_relic_event_hook(
    state: &mut State<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match state {
        State::DiamondDiadem(state) => diamond_diadem(state, hook, asked),
        state => unreachable!("{state:?} is a shared relic"),
    }
}

/// `AfterCardPlayed` counts each card its owner plays. `BeforeSideTurnEnd`
/// on its owner's side applies Diamond Diadem where the count stayed at the
/// threshold or under, then starts again. `AfterCombatEnd` starts again too.
/// A fight here has one player.
pub(crate) fn diamond_diadem(
    state: &mut crate::behaviour::relics::diamond_diadem::State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { .. } => state.cards_played += 1,
        EventHook::BeforeSideTurnEnd {
            side: CombatSide::Player,
            ..
        } => {
            if state.cards_played <= crate::behaviour::relics::diamond_diadem::THRESHOLD {
                asked.push(HookEffect::ApplyToOwner(
                    generated::powers::Power::DiamondDiademPower,
                    PowerAmount::Counted(1),
                ));
            }
            state.cards_played = 0;
        }
        EventHook::CombatEnd { .. } => state.cards_played = 0,
        _ => {}
    }
}
