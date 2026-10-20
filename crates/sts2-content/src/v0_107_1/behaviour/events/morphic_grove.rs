//! Morphic Grove: every coin for two transforms, or Max HP.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `MaxHpVar(5)`.
const MAX_HP: i32 = 5;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct MorphicGrove;

impl EventBehavior<V> for MorphicGrove {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.gold() >= 100
            && run
                .deck()
                .iter()
                .filter(|card| !crate::behaviour::enchantments::card_eternal(card))
                .count()
                >= 2
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::MorphicGrove,
            &[
                (Said::MorphicGroveGroup, false),
                (Said::MorphicGroveLoner, false),
            ],
            state.chosen,
        )
    }

    /// `Group`: every coin, then `FromDeckForTransformation` for two cards,
    /// each `TransformToRandom` off the event's stream. `Loner`:
    /// `CreatureCmd.GainMaxHp`. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::LoseGold(run.gold()),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Transform(Stream::Event),
                    count: 2,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![HookEffect::GainMaxHp(MAX_HP), HookEffect::FinishEvent],
        }
    }
}
