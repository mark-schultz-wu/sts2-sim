//! `NewLeaf`: one card of the deck transformed as it is picked up.

use sts2_core::{DeckAct, EventHook, HookEffect, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::RunRngType;

/// `CardsVar(1)`.
const CARDS: usize = 1;

/// `AfterObtained`: `CardSelectCmd.FromDeckForTransformation`, then
/// `CardCmd.TransformToRandom` off `RunState.Rng.Niche` for each card it
/// takes.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Transform(Stream::Run(RunRngType::Niche)),
            count: CARDS,
        });
    }
}
