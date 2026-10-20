//! Aroma of Chaos: a card transformed, or a card upgraded.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, while what it asked is open.
    chosen: Option<NonMaxU8>,
}

pub struct AromaOfChaos;

impl<V: crate::Build> EventBehavior<V> for AromaOfChaos {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::AROMA_OF_CHAOS,
            &[
                (V::EventOption::AROMA_OF_CHAOS_LET_GO, false),
                (V::EventOption::AROMA_OF_CHAOS_MAINTAIN_CONTROL, false),
            ],
            state.chosen,
        )
    }

    /// `LetGo`: `FromDeckForTransformation` for one card, then
    /// `TransformToRandom` off the event's own stream. `MaintainControl`:
    /// `FromDeckForUpgrade` for one card, then `CardCmd.Upgrade`. Each
    /// finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let act = match option {
            0 => DeckAct::Transform(Stream::Event),
            _ => DeckAct::Upgrade,
        };
        smallvec::smallvec![
            HookEffect::SelectFromDeck { act, count: 1 },
            HookEffect::FinishEvent
        ]
    }
}
