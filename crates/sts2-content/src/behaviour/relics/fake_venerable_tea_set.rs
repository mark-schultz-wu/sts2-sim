//! `FakeVenerableTeaSet`: an energy in the fight after a rest site.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;

/// `GainEnergyInNextCombat`, the one `[SavedProperty]` it declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    gain_energy: bool,
}

/// `AfterRoomEntered` at a rest site sets it to give. `AfterEnergyReset` then
/// gives its energy, once.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::RoomEntered {
            room: RoomType::RestSite,
            ..
        } => state.gain_energy = true,
        EventHook::EnergyReset { .. } if state.gain_energy => {
            state.gain_energy = false;
            asked.push(HookEffect::GainEnergy(
                V::Relic::FAKE_VENERABLE_TEA_SET_ENERGY,
            ));
        }
        _ => {}
    }
}
