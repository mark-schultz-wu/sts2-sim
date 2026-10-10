//! `MawBank`: gold for each room entered, until anything is bought.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `GoldVar(12)`.
const GOLD: Number = 12;

/// `HasItemBeenBought`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    bought: bool,
}

/// `IsUsedUp`, which is `HasItemBeenBought`.
pub(super) fn used_up(state: &State) -> bool {
    state.bought
}

/// `AfterRoomEntered`: its gold for the run's base room, until it is used
/// up. `AfterItemPurchased`: used up by a purchase that spent any gold.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered { base: true, .. } if !state.bought => {
            asked.push(HookEffect::GainGold(GOLD));
        }
        EventHook::ItemPurchased { gold } if gold > 0 => state.bought = true,
        EventHook::RoomEntered { .. } | EventHook::ItemPurchased { .. } => {}
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MawBank does for {hook:?}"),
    }
}
