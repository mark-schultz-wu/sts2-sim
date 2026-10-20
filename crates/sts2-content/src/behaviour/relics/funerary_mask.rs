//! `FuneraryMask`: Souls shuffled into the draw pile as the first hand is
//! drawn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `BeforeHandDraw` for its owner on turn one: `CardsVar` Souls, each
/// `AddGeneratedCardToCombat` into the draw pile at `CardPilePosition.Random`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BeforeHandDraw { turn, .. } = hook
        && turn == 1
    {
        asked.push(HookEffect::AddToDrawAtRandom {
            card: V::Card::SOUL,
            count: V::Relic::FUNERARY_MASK_CARDS,
            by_owner: true,
        });
    }
}
