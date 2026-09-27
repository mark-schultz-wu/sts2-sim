//! `BookOfFiveRings`: a heal for every fifth card that joins the deck.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(5)`.
const CARDS: u32 = 5;

/// `HealVar(20)`.
const HEAL: Number = 20;

/// `CardsAdded`, its one `[SavedProperty]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    cards_added: u32,
}

/// `AfterCardChangedPiles` for a card now in the deck counts it, and each
/// fifth heals.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::AddedToDeck { .. } = hook {
        state.cards_added += 1;
        if state.cards_added.is_multiple_of(CARDS) {
            asked.push(HookEffect::Heal(HEAL));
        }
    }
}

/// `DisplayAmount`: the cards counted towards the next heal.
pub(super) fn display_amount(state: &State) -> Number {
    Number::try_from(state.cards_added % CARDS).expect("under five fits a game number")
}
