//! Spiraling Whirlpool: Spiral on a deck card, or a third of the maximum
//! healed.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::{Event, EventOption as Said};

#[derive(Debug, Clone)]
pub struct State {
    /// `HealVar`, which `CalculateVars` sets to a third of the maximum.
    heal: i32,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct SpiralingWhirlpool;

impl EventBehavior<V> for SpiralingWhirlpool {
    type State = State;

    /// `IsAllowed`, a deck card Spiral can enchant, sits in the event table.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: `MaxHp * 0.33`, which `IntValue` cuts to a whole
    /// number.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            heal: run.max_hp() * 33 / 100,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::SpiralingWhirlpool,
            &[
                (Said::SpiralingWhirlpoolObserve, false),
                (Said::SpiralingWhirlpoolDrink, false),
            ],
            state.chosen,
        )
    }

    /// `ObserveTheSpiral`: Spiral on one deck card the player picks among
    /// those it can enchant. `Drink`: the heal. Each finishes the event.
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
                HookEffect::SelectFromDeck {
                    act: DeckAct::Enchant {
                        enchantment: Enchantment::Spiral,
                        amount: 1,
                        kind: None,
                    },
                    count: 1,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![HookEffect::Heal(state.heal), HookEffect::FinishEvent],
        }
    }
}
