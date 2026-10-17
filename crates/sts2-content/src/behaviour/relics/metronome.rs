//! `Metronome`: damage to every enemy on the seventh orb channelled in a
//! fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;

/// `OrbsChanneled`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    orbs_channeled: u8,
}

/// `AfterOrbChanneled` counts each orb its owner channels, and the seventh
/// damages every enemy that can be hit. Entering a combat room and the end
/// of a fight start the count again.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::OrbChanneled => {
            state.orbs_channeled = state.orbs_channeled.saturating_add(1);
            if state.orbs_channeled == (V::Relic::METRONOME_ORB_COUNT as u8) {
                asked.push(HookEffect::DamageOpponents(V::Relic::METRONOME_DAMAGE));
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
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::from(
        state
            .orbs_channeled
            .min(V::Relic::METRONOME_ORB_COUNT as u8),
    )
}
