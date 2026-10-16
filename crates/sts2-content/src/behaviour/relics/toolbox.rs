//! `Toolbox`: a colorless card of three, chosen into the first hand.

use sts2_core::{EventHook, HookEffect};

/// `BeforeHandDraw` on its owner's first turn: three distinct colorless
/// cards offered, and the one chosen into hand. Its `CardsVar(3)` is the
/// three a choice offers.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BeforeHandDraw { turn, .. } = hook
        && turn == 1
    {
        asked.push(HookEffect::ChooseColorlessIntoHand);
    }
}
