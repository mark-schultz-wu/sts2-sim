//! This or That: gold for HP, or a relic and a curse.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::rng::Span;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `HpLossVar(6)`.
const HP_LOSS: i32 = 6;

#[derive(Debug, Clone)]
pub struct State {
    /// What the option pays or costs, drawn as the event opens.
    gold: i32,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct ThisOrThat;

impl<V: crate::Build> EventBehavior<V> for ThisOrThat {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: `Rng.NextInt(41, 69)` is the gold.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let bound = |n: i32| num_traits::cast(n).expect("a small number fits the RNG integer");
        let span = Span::new(bound(41), bound(69)).expect("the span is ordered");
        let drawn: i32 =
            num_traits::cast(rng.next_int_range(span)).expect("the draw is inside the span");
        State {
            gold: drawn,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::THIS_OR_THAT,
            &[
                (V::EventOption::THIS_OR_THAT_PLAIN, false),
                (V::EventOption::THIS_OR_THAT_ORNATE, false),
            ],
            state.chosen,
        )
    }

    /// `WillKillPlayer`, for `Plain`: its hit point loss.
    fn kills(&self, _state: &State, option: u8, run: &impl EventView<V>) -> bool {
        option == 0 && run.hp() <= HP_LOSS
    }

    /// `Plain`: the damage, then the gold. `Ornate`: the next relic from the
    /// front of the bag, then Clumsy. Each finishes the event.
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
                HookEffect::DamageOwner(HP_LOSS),
                HookEffect::GainGold(state.gold),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled),
                HookEffect::AddToDeck(V::Card::CLUMSY),
                HookEffect::FinishEvent,
            ],
        }
    }
}
