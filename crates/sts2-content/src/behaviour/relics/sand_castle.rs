//! `SandCastle`: the upgrades its pickup hands out, and nothing after.

use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::RunRngType;

/// `SandCastle.CardsVar`.
const CARDS: usize = 6;

/// Nothing in a fight. Its one override is `AfterObtained`, which upgrades six
/// of the deck's cards, and the run is what carries that out.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::UpgradeRandomDeckCards {
            count: CARDS,
            stream: sts2_core::Stream::Run(RunRngType::Niche),
            kind: None,
        });
    }
}
