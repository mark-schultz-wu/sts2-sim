//! The Legends Were True: the Spoils Map, or a potion for HP.

use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, QuestCard};
use super::super::super::generated::events::{Event, EventOption as Said};

/// `DamageVar(8)`, unblockable and unpowered.
const DAMAGE: i32 = 8;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<u8>,
}

pub struct TheLegendsWereTrue;

impl EventBehavior<V> for TheLegendsWereTrue {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.act() == 0 && !run.deck().is_empty() && run.hp() >= 10
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::TheLegendsWereTrue,
            &[
                (Said::TheLegendsWereTrueNabTheMap, false),
                (Said::TheLegendsWereTrueSlowlyFindAnExit, false),
            ],
            state.chosen,
        )
    }

    /// `NabTheMap`: Spoils Map into the deck. `SlowlyFindAnExit`: the damage,
    /// then a reward of one potion drawn off the rewards stream. Each
    /// finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = Some(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::AddToDeck(Cards::Quest(QuestCard::SpoilsMap)),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::DamageOwner(DAMAGE),
                HookEffect::OfferRewards(&[CustomReward::UnlockedPotion]),
                HookEffect::FinishEvent,
            ],
        }
    }
}
