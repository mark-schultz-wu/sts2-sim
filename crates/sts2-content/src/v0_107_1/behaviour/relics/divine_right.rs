//! `DivineRight`: the stars the Regent opens every fight with.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `StarsVar(3)`, which is the whole of what it gives.
const STARS: Number = 3;

/// `AfterRoomEntered`, which pays the stars on the way into a fight.
pub(super) fn event_hook(hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match hook {
        EventHook::CombatRoomEntered => vec![HookEffect::GainStars(STARS)],
        _ => Vec::new(),
    }
}
