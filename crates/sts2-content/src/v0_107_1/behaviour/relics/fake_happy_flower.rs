//! `FakeHappyFlower`: an energy every fifth turn.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `DynamicVar("Turns", 5)`.
const TURNS: Number = 5;

/// `TurnsSeen`, the one `[SavedProperty]` it declares, which carries from
/// one fight to the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    turns_seen: Number,
}

/// `AfterSideTurnStart`: one more turn of its owner's counted, and every
/// fifth an energy.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::SideTurnStart {
        side: CombatSide::Player,
        ..
    } = hook
    {
        state.turns_seen = (state.turns_seen + 1) % TURNS;
        if state.turns_seen == 0 {
            asked.push(HookEffect::GainEnergy(ENERGY));
        }
    }
}

/// `DisplayAmount`: the turns seen. `IsActivating` shows the whole count
/// while the relic's visuals play, which a run without a screen never does.
pub(super) fn display_amount(state: &State) -> Number {
    state.turns_seen
}
