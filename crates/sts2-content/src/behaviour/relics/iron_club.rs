//! `IronClub`: a card drawn for every fourth card played.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `CardsPlayed`, its one `[SavedProperty]`, which carries from one fight to
/// the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    cards_played: u32,
}

/// `AfterCardPlayed` counts each card its owner plays, and every fourth
/// draws one while the fight is in progress. A fight here has one player.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { .. } = hook {
        state.cards_played += 1;
        if state
            .cards_played
            .is_multiple_of(V::Relic::IRON_CLUB_CARDS as u32)
        {
            asked.push(HookEffect::Draw(1));
        }
    }
}

/// `DisplayAmount`: the cards counted towards the next draw.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::try_from(state.cards_played % (V::Relic::IRON_CLUB_CARDS as u32))
        .expect("under four fits a game number")
}
