//! The hooks a relic is asked on its own rather than raised on the fight.
//!
//! A relic answers as its own model, and writes the state the run carries for
//! it. Every hook the fight raises reaches the belt through the walk.

use sts2_core::{EventHook, Rules};

/// `AfterObtained`, which `RelicCmd.Obtain` gives the relic it just handed
/// over and to no other, once it is on the belt.
pub(crate) fn obtained<V: Rules>(
    (state, belt): (&mut V::RelicState, &[V::Relic]),
    character: V::Character,
    act: usize,
    turn: Option<V::Count>,
) -> sts2_core::HookEffects<V> {
    let mut asked = sts2_core::HookEffects::new();
    V::relic_event_hook(
        state,
        belt,
        EventHook::Obtained {
            character,
            act,
            turn,
        },
        &mut asked,
    );
    asked
}
