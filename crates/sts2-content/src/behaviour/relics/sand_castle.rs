//! `SandCastle`: the upgrades its pickup hands out, and nothing after.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::RunRngType;

/// Nothing in a fight. Its one override is `AfterObtained`, which upgrades six
/// of the deck's cards, and the run is what carries that out.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::UpgradeRandomDeckCards {
            count: (V::Relic::SAND_CASTLE_CARDS as usize),
            stream: sts2_core::Stream::Run(RunRngType::Niche),
            kind: None,
        });
    }
}
