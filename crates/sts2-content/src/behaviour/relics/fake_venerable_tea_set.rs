//! `FakeVenerableTeaSet`: an energy in the fight after a rest site.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::encounters::RoomType;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

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
            asked.push(HookEffect::GainEnergy(ENERGY));
        }
        _ => {}
    }
}
