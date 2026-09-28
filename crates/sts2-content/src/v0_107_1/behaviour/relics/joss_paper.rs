//! `JossPaper`: a card drawn for every fifth card exhausted.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

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
    /// `DrawIfThresholdMet`: a card for every whole five, and the count kept
    /// to what is left.
    fn draw(&mut self, asked: &mut sts2_core::HookEffects<V>) {
        if self.cards_exhausted >= EXHAUST {
            asked.push(HookEffect::Draw(self.cards_exhausted / EXHAUST));
            self.cards_exhausted %= EXHAUST;
        }
    }
}

/// `AfterCardExhausted` counts each card its owner exhausts, and one that
/// Ethereal exhausted waits for `AfterSideTurnEnd` on its owner's side.
/// `AfterCombatEnd` drops what waits.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardExhausted { ethereal: true, .. } => state.ethereal += 1,
        EventHook::CardExhausted { .. } => {
            state.cards_exhausted += 1;
            state.draw(asked);
        }
        EventHook::SideTurnEnd {
            side: CombatSide::Player,
            ..
        } => {
            state.cards_exhausted += state.ethereal;
            state.ethereal = 0;
            state.draw(asked);
        }
        EventHook::CombatEnd { .. } => state.ethereal = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the cards counted towards the next draw.
pub(super) fn display_amount(state: &State) -> Number {
    state.cards_exhausted
}
