//! `SilverCrucible`: the first three card rewards upgraded, and the first
//! chest empty.

use sts2_core::{
    CardTable as _, DecisionHook, EventHook, HookEffect, HookResponse, RoomKinds as _,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::Number;

/// `Cards`: the card rewards it upgrades.
const CARDS: u8 = 3;

/// What it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    /// `TimesUsed`: the card rewards it has upgraded.
    times_used: u8,
    /// `TreasureRoomsEntered`.
    treasure_rooms: u8,
}

/// `TryModifyCardRewardOptionsLate` upgrades each reward card that can be,
/// until it has done so for three rewards, which
/// `AfterModifyingCardRewardOptions` counts. `AfterRoomEntered` counts the
/// treasure rooms.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardOption { card }
            if state.times_used < CARDS && card.upgrade < card.card.max_upgrade() =>
        {
            asked.push(HookEffect::UpgradeCard);
        }
        EventHook::CardRewardOption { .. } => {}
        EventHook::CardRewardOptionsModified => {
            if state.times_used < CARDS {
                state.times_used += 1;
            }
        }
        EventHook::RoomEntered { room, .. } => {
            if room == RoomType::TREASURE {
                state.treasure_rooms += 1;
            }
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what SilverCrucible does for {hook:?}"),
    }
}

/// `ShouldGenerateTreasure`: not until a second treasure room.
pub(super) fn decide_hook(state: &State, hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldGenerateTreasure => HookResponse::Decision(state.treasure_rooms > 1),
        _ => HookResponse::DoesNotImplement,
    }
}

/// `IsUsedUp`: every upgrade given, and a treasure room entered.
pub(super) fn used_up(state: &State) -> bool {
    state.times_used >= CARDS && state.treasure_rooms > 0
}

/// `DisplayAmount`: the card rewards it has left to upgrade.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(CARDS - state.times_used)
}
