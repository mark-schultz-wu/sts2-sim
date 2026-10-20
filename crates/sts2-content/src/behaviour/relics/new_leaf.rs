//! `NewLeaf`: one card of the deck transformed as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect, Stream};

use crate::generated::runstate::RunRngType;

/// `AfterObtained`: `CardSelectCmd.FromDeckForTransformation`, then
/// `CardCmd.TransformToRandom` off `RunState.Rng.Niche` for each card it
/// takes.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Transform(Stream::Run(RunRngType::Niche)),
            count: (V::Relic::NEW_LEAF_CARDS as usize),
        });
    }
}
