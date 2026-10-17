//! `PaelsTooth`: five upgradable cards taken from the deck as it is picked
//! up, and one handed back, upgraded, after each fight.

use crate::vars::RelicVars as _;
use std::sync::Arc;

use sts2_core::{DeckAct, DeckCard, EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::ids::RelicIds as _;

/// `SerializableCards`, which it saves: the deck cards it keeps, whole.
///
/// Shared, because every branch of a search carries the same list until one
/// hands a card back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State<V: crate::Build> {
    cards: Arc<[DeckCard<V>]>,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        State {
            cards: Arc::from([]),
        }
    }
}

impl<V: crate::Build> State<V> {
    /// The cards it keeps.
    pub(super) fn held(&self) -> &[DeckCard<V>] {
        &self.cards
    }

    /// `SerializableCards.Add` of each of `cards`.
    pub(super) fn hold(&mut self, cards: &[DeckCard<V>]) {
        self.cards = self.cards.iter().chain(cards).copied().collect();
    }

    /// `SerializableCards.Remove` of the one at `at`, handed back.
    pub(super) fn release(&mut self, at: usize) -> DeckCard<V> {
        let card = self.cards[at];
        self.cards = (self.cards.iter().enumerate())
            .filter(|&(other, _)| other != at)
            .map(|(_, &card)| card)
            .collect();
        card
    }
}

/// `AfterObtained`: `FromDeckForRemoval` for five upgradable cards, each
/// kept and then removed. `AfterCombatEnd` hands one back while it keeps any.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::RemoveOntoRelic(V::Relic::PAELS_TOOTH),
            count: (V::Relic::PAELS_TOOTH_CARDS as usize),
        }),
        EventHook::CombatEnd { .. } => {
            if !state.cards.is_empty() {
                asked.push(HookEffect::ReturnHeldCard(V::Relic::PAELS_TOOTH));
            }
        }
        hook => unimplemented!("what PaelsTooth does for {hook:?}"),
    }
}

/// `DisplayAmount`: the cards it keeps.
pub(super) fn display_amount<V: crate::Build>(state: &State<V>) -> Number {
    Number::try_from(state.cards.len()).expect("five cards fit the game's numbers")
}
