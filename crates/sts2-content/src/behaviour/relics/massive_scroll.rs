//! `MassiveScroll`: the co-op card its pickup offers.

use sts2_core::{EventHook, HookEffect};

/// How many cards the pickup offers.
const OFFERED: usize = 3;

/// Nothing in a fight. Its one override is `AfterObtained`. That offers
/// three of the cards only a run of several players deals. They come from
/// the colorless pool and then the character's, as a `Concat` lists them.
/// One or none goes into the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(obtained::<V>(true));
    }
}

/// What the pickup asks for, with the colorless pool first or last.
pub(crate) fn obtained<V: crate::Build>(colorless_first: bool) -> HookEffect<V> {
    HookEffect::ChooseMultiplayerCards {
        count: OFFERED,
        colorless_first,
    }
}
