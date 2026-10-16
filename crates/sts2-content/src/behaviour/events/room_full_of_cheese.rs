//! Room Full of Cheese: two commons of eight, or a relic for HP.

use crate::ids::{EventIds as _, EventOptionIds as _, RelicIds as _};
use nonmax::NonMaxU8;
use sts2_core::{CardDraw, EventBehavior, EventView, HookEffect, HookEffects, RewardRarity, Room};

/// `DamageVar(14)`, unblockable and unpowered.
const DAMAGE: i32 = 14;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, while what it asked is open.
    chosen: Option<NonMaxU8>,
}

pub struct RoomFullOfCheese;

impl<V: crate::Build> EventBehavior<V> for RoomFullOfCheese {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.act() < 2
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::ROOM_FULL_OF_CHEESE,
            &[
                (V::EventOption::ROOM_FULL_OF_CHEESE_GORGE, false),
                (V::EventOption::ROOM_FULL_OF_CHEESE_SEARCH, false),
            ],
            state.chosen,
        )
    }

    /// `Gorge`: eight commons from the character's pool at uniform odds, and
    /// two of them into the deck. `Search`: the damage, then Chosen Cheese. Each
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
                HookEffect::ChooseCards {
                    draw: CardDraw::Uniform(RewardRarity::Common),
                    count: 8,
                    min: 2,
                    max: 2,
                    card_reward: false,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::DamageOwner(DAMAGE),
                HookEffect::ObtainRelic(V::Relic::CHOSEN_CHEESE),
                HookEffect::FinishEvent,
            ],
        }
    }
}
