//! The powers whose rules this build ports for itself, which the shared
//! powers hand over through [`crate::build::OwnRules`].
//!
//! None yet. The two v0.107.1 ports for itself, Diamond Diadem's and
//! Outbreak's, are gone from this build.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, PowerAmount};

use super::super::V0_111_0 as V;
use super::super::generated::powers::Power;
use crate::behaviour::powers::{Held, State};

type Number = i32;

/// Whether `power`'s rule is this build's own.
pub(crate) fn own_power(_power: Power) -> bool {
    false
}

/// The state a power of this build's own starts with.
pub(crate) fn own_power_state(_power: Power) -> Option<State<V>> {
    None
}

/// `ModifyHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_modify_hook(
    _power: Power,
    _amount: PowerAmount<Number>,
    _held: Held<'_, V>,
    _state: &State<V>,
    _hook: ModifyHook<V>,
) -> Option<Modify<Number>> {
    None
}

/// `EventHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_event_hook(
    _power: Power,
    _amount: PowerAmount<Number>,
    _held: Held<'_, V>,
    _state: &mut State<V>,
    _hook: EventHook<V>,
) -> Option<Option<HookEffect<V>>> {
    None
}
