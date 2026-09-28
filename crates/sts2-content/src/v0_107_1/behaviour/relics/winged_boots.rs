//! `WingedBoots`: three steps off the map's lines.

use sts2_core::{DecisionHook, EventHook, HookResponse};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `_roomCount`, the steps it is picked up with.
const ROOMS: Number = 3;

/// `TimesUsed`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    times_used: Number,
}

impl State {
    /// `IsUsedUp`, once every step is spent.
    pub(super) fn used_up(self) -> bool {
        self.times_used >= ROOMS
    }
}

/// `AfterRoomEntered`: a room reached off the map's lines spends a step.
///
/// The game returns early for a used-up relic, for the first point of an act,
/// and for a point the one before leads on to. `strayed` is none of those.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) {
    if let EventHook::RoomEntered { strayed: true, .. } = hook
        && !state.used_up()
    {
        state.times_used += 1;
    }
}

/// `ShouldAllowFreeTravel`, while a step is left.
pub(super) fn decide_hook(state: &State, hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldAllowFreeTravel => HookResponse::Decision(!state.used_up()),
        hook => unimplemented!("what WingedBoots decides for {hook:?}"),
    }
}

/// `DisplayAmount`, which is the steps left.
pub(super) fn display_amount(state: &State) -> Number {
    ROOMS - state.times_used
}
