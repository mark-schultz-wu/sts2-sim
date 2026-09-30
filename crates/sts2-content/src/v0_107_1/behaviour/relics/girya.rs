//! `Girya`: a rest site offers to lift, three times a run, and each lift is
//! a Strength in every fight.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::super::super::generated::restsite::RestSiteOption;
use super::Number;

/// `maxLifts`.
const LIFTS: u8 = 3;

/// `TimesLifted`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    times_lifted: u8,
}

/// `AfterRoomEntered` for a fight's room: a Strength for each lift.
/// `TryModifyRestSiteOptions`: `LIFT`, until it has lifted three times.
/// `LiftRestSiteOption.OnSelect` counts a lift.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered { room, .. }
            if state.times_lifted > 0 && super::combat_room(room) =>
        {
            asked.push(HookEffect::ApplyToOwner(
                Power::StrengthPower,
                PowerAmount::Counted(Number::from(state.times_lifted)),
            ));
        }
        EventHook::RestSiteOptions if state.times_lifted < LIFTS => {
            asked.push(HookEffect::OfferRestOption(RestSiteOption::Lift));
        }
        EventHook::RestOptionTaken => state.times_lifted += 1,
        EventHook::RoomEntered { .. } | EventHook::RestSiteOptions | EventHook::Obtained { .. } => {
        }
        hook => unimplemented!("what Girya does for {hook:?}"),
    }
}

/// `DisplayAmount`: the lifts so far.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.times_lifted)
}
