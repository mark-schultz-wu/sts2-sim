//! `TeaOfDiscourtesy`: two Dazed shuffled into the draw pile, for one fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, StatusCard};
use super::Number;

/// `_combatsLeft`, which the relic is picked up with.
const COMBATS: Number = 1;

/// `DynamicVar("DazedCount", 2)`.
const DAZED: Number = 2;

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

/// `BeforeCombatStart`: its Dazed into the draw pile, each at a place drawn
/// at random, while it has fights left, and one fewer after.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatStarting { .. } => {
            if state.combats_left > 0 {
                asked.push(HookEffect::AddToDrawAtRandom {
                    card: Cards::Status(StatusCard::Dazed),
                    count: DAZED,
                    by_owner: true,
                });
                state.combats_left -= 1;
            }
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TeaOfDiscourtesy does for {hook:?}"),
    }
}
