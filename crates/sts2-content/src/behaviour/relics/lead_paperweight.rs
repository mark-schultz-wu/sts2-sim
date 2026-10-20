//! `LeadPaperweight`: the colorless card its pickup offers.

use sts2_core::{EventHook, HookEffect};

/// Nothing in a fight. Its one override is `AfterObtained`, which offers two
/// colorless cards and adds the chosen one to the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::ChooseColorlessCards { count: 2 });
    }
}
