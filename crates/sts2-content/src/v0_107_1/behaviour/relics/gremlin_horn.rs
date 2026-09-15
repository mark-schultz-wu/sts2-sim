//! `Gremlin Horn`: a card and energy after an enemy dies.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// Its death response is delivered after an enemy dies and combat continues.
/// The other lifecycle notifications do nothing.
pub(super) fn event_hook(hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match hook {
        EventHook::AfterDeath => vec![HookEffect::GainEnergy(1), HookEffect::Draw(1)],
        _ => Vec::new(),
    }
}
