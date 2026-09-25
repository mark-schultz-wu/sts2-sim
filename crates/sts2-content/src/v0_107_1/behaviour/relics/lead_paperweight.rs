//! `LeadPaperweight`: the colorless card its pickup offers.

use sts2_core::EventHook;

use super::super::super::V0_107_1 as V;

/// Nothing in a fight. Its one override is `AfterObtained`, which offers two
/// colorless cards and adds the chosen one to the deck.
///
/// # Panics
///
/// Obtained anywhere but Neow's blessing, which is the one path that runs the
/// offer. The relic reaching the belt by another route skips the offer.
pub(super) fn event_hook(hook: EventHook<V>) {
    if let EventHook::Obtained = hook {
        unimplemented!("Lead Paperweight obtained outside the blessing");
    }
}
