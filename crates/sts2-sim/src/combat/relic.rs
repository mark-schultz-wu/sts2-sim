//! The hooks a relic is asked on its own rather than raised on the fight.
//!
//! A relic answers as its own model, and writes the state the run carries for
//! it. Every hook the fight raises reaches the belt through the walk.

use sts2_core::{EventHook, Rules};

use super::live::HeardEvent;
use crate::run::HeldRelic;

/// `AfterObtained`, which `RelicCmd.Obtain` gives the relic it just handed
/// over and to no other.
pub(crate) fn obtained<V: Rules>(held: &mut HeldRelic<V>) -> sts2_core::HookEffects<V> {
    let mut asked = sts2_core::HookEffects::new();
    V::event_hook(
        HeardEvent::<V>::Relic(&mut held.state),
        EventHook::Obtained,
        &mut asked,
    );
    asked
}
