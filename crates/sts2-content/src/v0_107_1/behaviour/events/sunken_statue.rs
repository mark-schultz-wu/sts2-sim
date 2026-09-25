//! Sunken Statue: Sword of Stone, or gold for HP.

use sts2_core::rng::Span;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

/// `GoldVar(111)`, before `CalculateVars` moves it.
const GOLD: i32 = 111;

/// `DynamicVar("HpLoss", 7)`.
const HP_LOSS: i32 = 7;

#[derive(Debug, Clone)]
pub struct State {
    /// What the option pays or costs, drawn as the event opens.
    gold: i32,
    /// The option taken.
    chosen: Option<u8>,
}

pub struct SunkenStatue;

impl EventBehavior<V> for SunkenStatue {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: `Rng.NextInt(-10, 11)` onto the gold.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let bound = |n: i32| num_traits::cast(n).expect("a small number fits the RNG integer");
        let span = Span::new(bound(-10), bound(11)).expect("the span is ordered");
        let drawn: i32 =
            num_traits::cast(rng.next_int_range(span)).expect("the draw is inside the span");
        State {
            gold: GOLD + drawn,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::SunkenStatue,
            &[
                (Said::SunkenStatueGrabSword, false),
                (Said::SunkenStatueDiveIntoWater, false),
            ],
            state.chosen,
        )
    }

    /// `GrabSword`: Sword of Stone. `DiveIntoWater`: the gold, then the
    /// damage. Each finishes the event.
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
                HookEffect::ObtainRelic(Relic::SwordOfStone),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::GainGold(state.gold),
                HookEffect::DamageOwner(HP_LOSS),
                HookEffect::FinishEvent,
            ],
        }
    }
}
