//! Lost Wisp: the relic and a Decay, or some gold.

use nonmax::NonMaxU8;
use sts2_core::rng::{GameRng, Span};
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

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

impl EventBehavior<V> for LostWisp {
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
        super::page(
            Event::LostWisp,
            &[(Said::LostWispClaim, false), (Said::LostWispSearch, false)],
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
                HookEffect::AddToDeck(Cards::Curse(CurseCard::Decay)),
                HookEffect::ObtainRelic(Relic::LostWisp),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![HookEffect::GainGold(state.gold), HookEffect::FinishEvent],
        }
    }
}
