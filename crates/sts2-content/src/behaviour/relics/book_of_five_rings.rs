//! `BookOfFiveRings`: a heal for every fifth card that joins the deck.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `CardsAdded`, its one `[SavedProperty]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    cards_added: u32,
}

/// `AfterCardChangedPiles` for a card now in the deck counts it, and each
/// fifth heals.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::AddedToDeck { player, .. } = hook
        && player == owner
    {
        state.cards_added += 1;
        if state
            .cards_added
            .is_multiple_of(V::Relic::BOOK_OF_FIVE_RINGS_CARDS as u32)
        {
            asked.push(HookEffect::Heal(V::Relic::BOOK_OF_FIVE_RINGS_HEAL));
        }
    }
}

/// `DisplayAmount`: the cards counted towards the next heal.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::try_from(state.cards_added % (V::Relic::BOOK_OF_FIVE_RINGS_CARDS as u32))
        .expect("under five fits a game number")
}
