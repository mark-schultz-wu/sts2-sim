//! The Legends Were True: the Spoils Map, or a potion for HP.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `DamageVar(8)`, unblockable and unpowered.
const DAMAGE: i32 = 8;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct TheLegendsWereTrue;

impl<V: crate::Build> EventBehavior<V> for TheLegendsWereTrue {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.act() == 0 && !run.deck().is_empty() && run.hp() >= 10
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::THE_LEGENDS_WERE_TRUE,
            &[
                (V::EventOption::THE_LEGENDS_WERE_TRUE_NAB_THE_MAP, false),
                (
                    V::EventOption::THE_LEGENDS_WERE_TRUE_SLOWLY_FIND_AN_EXIT,
                    false,
                ),
            ],
            state.chosen,
        )
    }

    /// `WillKillPlayer`, for `SlowlyFindAnExit`: its damage.
    fn kills(&self, _state: &State, option: u8, run: &impl EventView<V>) -> bool {
        option == 1 && run.hp() <= DAMAGE
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
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::AddToDeck(V::Card::SPOILS_MAP),
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
