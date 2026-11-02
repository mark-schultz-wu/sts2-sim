//! Spirit Grafter: a heal and a Metamorphosis, or an upgrade for HP.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `HealVar("LetItInHealAmount", 25)`.
const HEAL: i32 = 25;

/// `HpLossVar("RejectionHpLoss", 10)`.
const HP_LOSS: i32 = 10;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct SpiritGrafter;

impl<V: crate::Build> EventBehavior<V> for SpiritGrafter {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::SPIRIT_GRAFTER,
            &[
                (V::EventOption::SPIRIT_GRAFTER_LET_IT_IN, false),
                (V::EventOption::SPIRIT_GRAFTER_REJECTION, false),
            ],
            state.chosen,
        )
    }

    /// `WillKillPlayer`, for `Rejection`: its hit point loss.
    fn kills(&self, _state: &State, option: u8, run: &impl EventView<V>) -> bool {
        option == 1 && run.hp() <= HP_LOSS
    }

    /// `LetItIn`: the heal, then a Metamorphosis into the deck. `Rejection`:
    /// `FromDeckForUpgrade` for one card, then the damage. Each finishes the
    /// event.
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
                HookEffect::Heal(HEAL),
                HookEffect::AddToDeck(V::Card::METAMORPHOSIS),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Upgrade,
                    count: 1,
                },
                HookEffect::DamageOwner(HP_LOSS),
                HookEffect::FinishEvent,
            ],
        }
    }
}
