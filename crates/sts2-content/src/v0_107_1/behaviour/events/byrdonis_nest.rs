//! Byrdonis Nest: Max HP, or the egg.

use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, QuestCard};
use super::super::super::generated::events::{Event, EventOption as Said};

/// `MaxHpVar(7)`.
const MAX_HP: i32 = 7;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<u8>,
}

pub struct ByrdonisNest;

impl EventBehavior<V> for ByrdonisNest {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        !run.relics().any(super::adds_pet)
            && !run
                .deck()
                .iter()
                .any(|card| card.card == Cards::Quest(QuestCard::ByrdonisEgg))
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::ByrdonisNest,
            &[
                (Said::ByrdonisNestEat, false),
                (Said::ByrdonisNestTake, false),
            ],
            state.chosen,
        )
    }

    /// `Eat`: `CreatureCmd.GainMaxHp`. `Take`: Byrdonis Egg into the deck.
    /// Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = Some(option);
        match option {
            0 => smallvec::smallvec![HookEffect::GainMaxHp(MAX_HP), HookEffect::FinishEvent],
            _ => smallvec::smallvec![
                HookEffect::AddToDeck(Cards::Quest(QuestCard::ByrdonisEgg)),
                HookEffect::FinishEvent,
            ],
        }
    }
}
