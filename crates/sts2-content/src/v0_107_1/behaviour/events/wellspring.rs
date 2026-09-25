//! Wellspring: a potion, or a card removed for a curse.

use nonmax::NonMaxU8;
use sts2_core::{CustomReward, DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::events::{Event, EventOption as Said};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct Wellspring;

impl EventBehavior<V> for Wellspring {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::Wellspring,
            &[
                (Said::WellspringBottle, false),
                (Said::WellspringBathe, false),
            ],
            state.chosen,
        )
    }

    /// `Bottle`: a reward of one potion drawn off the rewards stream.
    /// `Bathe`: `FromDeckForRemoval` for one card, then Guilty. Each finishes
    /// the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::OfferRewards(&[CustomReward::UnlockedPotion]),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Remove,
                    count: 1,
                },
                HookEffect::AddToDeck(Cards::Curse(CurseCard::Guilty)),
                HookEffect::FinishEvent,
            ],
        }
    }
}
