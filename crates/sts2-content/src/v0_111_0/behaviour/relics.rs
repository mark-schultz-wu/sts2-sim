//! The relics whose rules this build ports for itself, which the shared
//! relics hand over through [`crate::build::OwnRules`].

use sts2_core::EventHook;

use super::super::V0_111_0 as V;
use crate::behaviour::relics::State;

/// `EventHook` for a relic of this build's own.
pub(crate) fn own_relic_event_hook(
    state: &mut State<V>,
    _hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match state {
        // `DiamondDiadem` blocks and blurs on its owner's first turn here.
        State::DiamondDiadem(_) => unimplemented!("Diamond Diadem in v0.111.0"),
        state => unreachable!("{state:?} is a shared relic"),
    }
}
