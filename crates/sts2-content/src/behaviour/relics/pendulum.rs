//! `Pendulum`: a card drawn every third turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `TurnsSeen`, its one `[SavedProperty]`, which carries from one fight to
/// the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turns_seen: Number,
}

/// v0.107.1's `AfterPlayerTurnStart` for its owner counts the turn, and
/// every third draws its card. v0.111.0's `BeforeHandDraw` counts it, and
/// its `ModifyHandDraw` adds the card to that turn's hand. Each build's class
/// overrides one pair, so it hears only that pair. A fight here has one
/// player.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::PlayerTurnStart { .. } => {
            state.turns_seen = (state.turns_seen + 1) % V::Relic::PENDULUM_TURNS;
            if state.turns_seen == 0 {
                asked.push(HookEffect::Draw(V::Relic::PENDULUM_CARDS));
            }
        }
        EventHook::BeforeHandDraw { .. } => {
            state.turns_seen = (state.turns_seen + 1) % V::Relic::PENDULUM_TURNS;
        }
        _ => {}
    }
}

/// v0.111.0's `ModifyHandDraw`: its card in the hand of a turn the count
/// came round on.
pub(super) fn modify_hook<V: crate::Build>(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HandDraw { .. } if state.turns_seen == 0 => {
            Modify::Added(V::Relic::PENDULUM_CARDS)
        }
        ModifyHook::HandDraw { .. } => Modify::Nothing,
        hook => unimplemented!("what Pendulum does for {hook:?}"),
    }
}

/// `DisplayAmount`: the turns seen. `IsActivating` shows the whole count
/// while the relic's visuals play, which a run without a screen never does.
pub(super) fn display_amount(state: &State) -> Number {
    state.turns_seen
}
