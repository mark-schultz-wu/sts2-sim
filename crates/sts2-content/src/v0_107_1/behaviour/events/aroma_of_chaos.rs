//! Aroma of Chaos: a card transformed, or a card upgraded.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, while what it asked is open.
    chosen: Option<NonMaxU8>,
}

pub struct AromaOfChaos;

impl EventBehavior<V> for AromaOfChaos {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::AromaOfChaos,
            &[
                (Said::AromaOfChaosLetGo, false),
                (Said::AromaOfChaosMaintainControl, false),
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
