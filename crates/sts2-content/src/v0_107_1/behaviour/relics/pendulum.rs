//! `Pendulum`: a card drawn every third turn.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(1)`.
const CARDS: Number = 1;

/// `DynamicVar("Turns", 3)`.
const TURNS: Number = 3;

/// `TurnsSeen`, its one `[SavedProperty]`, which carries from one fight to
/// the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turns_seen: Number,
}

/// `AfterPlayerTurnStart` for its owner counts the turn, and every third
/// draws its card. A fight here has one player.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::PlayerTurnStart { .. } = hook {
        state.turns_seen = (state.turns_seen + 1) % TURNS;
        if state.turns_seen == 0 {
            asked.push(HookEffect::Draw(CARDS));
        }
    }
}

/// `DisplayAmount`: the turns seen. `IsActivating` shows the whole count
/// while the relic's visuals play, which a run without a screen never does.
pub(super) fn display_amount(state: &State) -> Number {
    state.turns_seen
}
