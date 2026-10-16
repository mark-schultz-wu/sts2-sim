//! Amalgamator: two basic Strikes or two basic Defends, fused into one card.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_111_0 as V;
use super::super::super::generated::cards::{CardTag, Cards, ColorlessCard};
use super::super::super::generated::events::{Event, EventOption as Said};

/// `CardSelectorPrefs(RemoveSelectionPrompt, 2)`.
const FUSED: usize = 2;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct Amalgamator;

impl EventBehavior<V> for Amalgamator {
    type State = State;

    /// Whether the deck holds two of each that `IsValid` takes, which the
    /// shared eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            Event::Amalgamator,
            &[
                (Said::AmalgamatorCombineStrikes, false),
                (Said::AmalgamatorCombineDefends, false),
            ],
            state.chosen,
        )
    }

    /// `CombineStrikes` or `CombineDefends`: `FromDeckForRemoval` for two of
    /// the basic, removable cards with that tag, then the fused card into
    /// the deck. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let (tag, fused) = match option {
            0 => (CardTag::Strike, ColorlessCard::UltimateStrike),
            _ => (CardTag::Defend, ColorlessCard::UltimateDefend),
        };
        smallvec::smallvec![
            HookEffect::SelectFromDeck {
                act: DeckAct::RemoveBasicTagged(tag),
                count: FUSED,
            },
            HookEffect::AddToDeck(Cards::Colorless(fused)),
            HookEffect::FinishEvent,
        ]
    }
}
