//! Lost Wisp: the relic and a Decay, or some gold.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _, RelicIds as _};
use nonmax::NonMaxU8;
use sts2_core::rng::{GameRng, Span};
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `_baseGold`.
const GOLD: i32 = 60;

/// `_goldVariance`: the draw is `NextInt(-15, 16)`.
const VARIANCE: i32 = 15;

#[derive(Debug, Clone)]
pub struct State {
    /// `GoldVar`, as `CalculateVars` left it.
    gold: i32,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct LostWisp;

impl<V: crate::Build> EventBehavior<V> for LostWisp {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: the gold moves by a draw off the event's stream.
    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let span = Span::new(
            num_traits::cast(-VARIANCE).expect("the variance fits the RNG integer"),
            num_traits::cast(VARIANCE + 1).expect("the variance fits the RNG integer"),
        )
        .expect("the variance is a span");
        let moved: i32 = num_traits::cast(rng.next_int_range(span)).expect("a gold draw");
        State {
            gold: GOLD + moved,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::LOST_WISP,
            &[
                (V::EventOption::LOST_WISP_CLAIM, false),
                (V::EventOption::LOST_WISP_SEARCH, false),
            ],
            state.chosen,
        )
    }

    /// `Claim`: a Decay into the deck, then the Lost Wisp. `Search`:
    /// `PlayerCmd.GainGold`. Each finishes the event.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::AddToDeck(V::Card::DECAY),
                HookEffect::ObtainRelic(V::Relic::LOST_WISP),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![HookEffect::GainGold(state.gold), HookEffect::FinishEvent],
        }
    }
}
