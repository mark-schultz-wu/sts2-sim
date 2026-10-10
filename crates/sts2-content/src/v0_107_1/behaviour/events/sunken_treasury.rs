//! Sunken Treasury: a small chest, or a large one with Greed.

use nonmax::NonMaxU8;
use sts2_core::rng::Bound;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::events::{Event, EventOption as Said};

/// `DynamicVar("SmallChestGold", 60)` and `("LargeChestGold", 333)`, before
/// `CalculateVars` moves them.
const SMALL: i32 = 60;
const LARGE: i32 = 333;

#[derive(Debug, Clone)]
pub struct State {
    /// What each chest holds, drawn as the event opens.
    small: i32,
    large: i32,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct SunkenTreasury;

impl EventBehavior<V> for SunkenTreasury {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: `Rng.NextInt(16) - 8` onto the small chest, then
    /// `Rng.NextInt(61) - 30` onto the large one.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let mut draw = |n: i32| -> i32 {
            let bound = num_traits::cast(n).expect("a small number fits the RNG integer");
            num_traits::cast(rng.next_int(Bound::try_new(bound).expect("a positive bound")))
                .expect("the draw is below the bound")
        };
        let small = SMALL + draw(16) - 8;
        let large = LARGE + draw(61) - 30;
        State {
            small,
            large,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::SunkenTreasury,
            &[
                (Said::SunkenTreasuryFirstChest, false),
                (Said::SunkenTreasurySecondChest, false),
            ],
            state.chosen,
        )
    }

    /// `FirstChest`: the small chest's gold. `SecondChest`: the large one's,
    /// then Greed into the deck. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![HookEffect::GainGold(state.small), HookEffect::FinishEvent],
            _ => smallvec::smallvec![
                HookEffect::GainGold(state.large),
                HookEffect::AddToDeck(Cards::Curse(CurseCard::Greed)),
                HookEffect::FinishEvent,
            ],
        }
    }
}
