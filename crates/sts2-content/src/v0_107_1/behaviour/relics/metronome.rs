//! `Metronome`: damage to every enemy on the seventh orb channelled in a
//! fight.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::RoomType;
use super::Number;

/// `DynamicVar("OrbCount", 7)`.
const ORB_COUNT: u8 = 7;

/// `DamageVar(30, ValueProp.Unpowered)`.
const DAMAGE: Number = 30;

/// `OrbsChanneled`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    orbs_channeled: u8,
}

/// `AfterOrbChanneled` counts each orb its owner channels, and the seventh
/// damages every enemy that can be hit. Entering a combat room and the end
/// of a fight start the count again.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::OrbChanneled => {
            state.orbs_channeled = state.orbs_channeled.saturating_add(1);
            if state.orbs_channeled == ORB_COUNT {
                asked.push(HookEffect::DamageOpponents(DAMAGE));
            }
        }
        EventHook::RoomEntered {
            room: RoomType::Monster | RoomType::Elite | RoomType::Boss,
            ..
        }
        | EventHook::CombatEnd { .. } => state.orbs_channeled = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the orbs counted, up to its count.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.orbs_channeled.min(ORB_COUNT))
}
