//! `Whetstone`: two Attacks upgraded, once.

use sts2_core::{CardKind, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::RunRngType;

/// `CardsVar(2)`.
const CARDS: usize = 2;

/// `AfterObtained`: the deck's upgradable Attacks stably shuffled off
/// `RunState.Rng.Niche`, and the first two upgraded.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::UpgradeRandomDeckCards {
            count: CARDS,
            stream: sts2_core::Stream::Run(RunRngType::Niche),
            kind: Some(CardKind::Attack),
        });
    }
}
