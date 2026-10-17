//! `Whetstone`: two Attacks upgraded, once.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

use crate::generated::runstate::RunRngType;

/// `AfterObtained`: the deck's upgradable Attacks stably shuffled off
/// `RunState.Rng.Niche`, and the first two upgraded.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::UpgradeRandomDeckCards {
            count: (V::Relic::WHETSTONE_CARDS as usize),
            stream: sts2_core::Stream::Run(RunRngType::Niche),
            kind: Some(CardKind::Attack),
        });
    }
}
