//! `Astrolabe`: three cards transformed and upgraded as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect, Stream};

use crate::generated::runstate::RunRngType;

/// `AfterObtained`: `CardSelectCmd.FromDeckForTransformation` for three.
/// Each becomes `CreateRandomCardForTransform` off `RunState.Rng.Niche`,
/// upgraded, then `CardCmd.Transform`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::TransformUpgraded(Stream::Run(RunRngType::Niche)),
            count: (V::Relic::ASTROLABE_CARDS as usize),
        });
    }
}
