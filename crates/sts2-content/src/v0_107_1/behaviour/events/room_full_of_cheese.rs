//! Room Full of Cheese: two commons of eight, or a relic for HP.

use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, RewardRarity, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

/// `DamageVar(14)`, unblockable and unpowered.
const DAMAGE: i32 = 14;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, while what it asked is open.
    chosen: Option<u8>,
}

pub struct RoomFullOfCheese;

impl EventBehavior<V> for RoomFullOfCheese {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.act() < 2
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::RoomFullOfCheese,
            &[
                (Said::RoomFullOfCheeseGorge, false),
                (Said::RoomFullOfCheeseSearch, false),
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
        state.chosen = Some(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::ChooseUniformCards {
                    count: 8,
                    rarity: RewardRarity::Common,
                    min: 2,
                    max: 2,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::DamageOwner(DAMAGE),
                HookEffect::ObtainRelic(Relic::ChosenCheese),
                HookEffect::FinishEvent,
            ],
        }
    }
}
