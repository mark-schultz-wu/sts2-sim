//! `FuneraryMask`: Souls shuffled into the draw pile as the first hand is
//! drawn.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, TokenCard};
use super::Number;

/// `CardsVar(3)`.
const CARDS: Number = 3;

/// `BeforeHandDraw` for its owner on turn one: `CardsVar` Souls, each
/// `AddGeneratedCardToCombat` into the draw pile at `CardPilePosition.Random`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::BeforeHandDraw { turn, .. } = hook
        && turn == 1
    {
        asked.push(HookEffect::AddToDrawAtRandom {
            card: Cards::Token(TokenCard::Soul),
            count: CARDS,
        });
    }
}
