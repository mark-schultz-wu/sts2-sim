//! `JossPaper`: a card drawn for every fifth card exhausted.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `DynamicVar("ExhaustAmount", 5)`.
const EXHAUST: Number = 5;

/// `CardsExhausted`, which it saves, and `EtherealCount`, the exhausts
/// Ethereal made as a turn ended, counted once the hand is flushed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    cards_exhausted: Number,
    ethereal: Number,
}

impl State {
    /// `DrawIfThresholdMet`: a card for every whole five. The count keeps to
    /// what is left once the draw is over.
    fn draw<V: crate::Build>(&self, asked: &mut sts2_core::HookEffects<V>) {
        if self.cards_exhausted >= EXHAUST {
            asked.push(HookEffect::Draw(self.cards_exhausted / EXHAUST));
        }
    }
}

/// `AfterCardExhausted` counts each card its owner exhausts, and one that
/// Ethereal exhausted waits for `AfterSideTurnEnd` on its owner's side.
/// `AfterCombatEnd` drops what waits. Its own draw over, the count keeps
/// to what is left.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardExhausted { ethereal: true, .. } => state.ethereal += 1,
        EventHook::CardExhausted { .. } => {
            state.cards_exhausted += 1;
            state.draw::<V>(asked);
        }
        EventHook::SideTurnEnd {
            side: CombatSide::Player,
            ..
        } => {
            state.cards_exhausted += state.ethereal;
            state.ethereal = 0;
            state.draw::<V>(asked);
        }
        EventHook::CombatEnd { .. } => state.ethereal = 0,
        EventHook::OwnCommandOver => state.cards_exhausted %= EXHAUST,
        _ => {}
    }
}

/// `DisplayAmount`: the cards counted towards the next draw.
pub(super) fn display_amount(state: &State) -> Number {
    state.cards_exhausted
}
