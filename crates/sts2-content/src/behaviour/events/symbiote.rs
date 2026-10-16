//! Symbiote: one attack Corrupted, or one card transformed.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

use crate::behaviour::enchantments::can_enchant;
use crate::ids::{EnchantmentIds as _, EventIds as _, EventOptionIds as _};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct Symbiote;

impl<V: crate::Build> EventBehavior<V> for Symbiote {
    type State = State;

    /// Whether the run is past the first act, which the shared eligibility
    /// check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// The approach is locked for a deck with no card Corrupted takes.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        let corruptible = run
            .deck()
            .iter()
            .any(|card| can_enchant(V::Enchantment::CORRUPTED, card));
        crate::behaviour::events::page::<V>(
            V::Event::SYMBIOTE,
            &[
                (V::EventOption::SYMBIOTE_APPROACH, !corruptible),
                (V::EventOption::SYMBIOTE_KILL_WITH_FIRE, false),
            ],
            state.chosen,
        )
    }

    /// `Approach`: `FromDeckForEnchantment` for one card, then Corrupted.
    /// `KillWithFire`: `FromDeckForTransformation` for one card, then
    /// `TransformToRandom` off the event's own stream. Each finishes the
    /// event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let act = match option {
            0 => DeckAct::Enchant {
                enchantment: V::Enchantment::CORRUPTED,
                amount: 1,
                kind: None,
            },
            _ => DeckAct::Transform(Stream::Event),
        };
        smallvec::smallvec![
            HookEffect::SelectFromDeck { act, count: 1 },
            HookEffect::FinishEvent,
        ]
    }
}
