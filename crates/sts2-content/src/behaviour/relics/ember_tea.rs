//! `EmberTea`: Strength as each of its fights opens, for five fights.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;
use crate::ids::PowerIds as _;

/// `_combatsLeft`, which the relic is picked up with.
const COMBATS: Number = 5;

/// `PowerVar<StrengthPower>(2)`.
const STRENGTH: PowerAmount<Number> = PowerAmount::Counted(2);

/// `CombatsLeft`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct State {
    combats_left: Number,
}

impl Default for State {
    fn default() -> Self {
        Self {
            combats_left: COMBATS,
        }
    }
}

impl State {
    /// `IsUsedUp`, which is what disables the relic on screen.
    pub(super) fn used_up(self) -> bool {
        self.combats_left <= 0
    }
}

/// `DisplayAmount`: the fights it has left, and none below that.
pub(super) fn display_amount(state: &State) -> Number {
    state.combats_left.max(0)
}

/// `AfterRoomEntered`: Strength on its owner as a fight's room opens, while
/// it has fights left, and one fewer after.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered {
            room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
            ..
        } if !state.used_up() => {
            asked.push(HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, STRENGTH));
            state.combats_left -= 1;
        }
        EventHook::RoomEntered { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what EmberTea does for {hook:?}"),
    }
}
