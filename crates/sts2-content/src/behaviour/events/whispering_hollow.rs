//! Whispering Hollow: two potions bought, or a card transformed for HP.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::rng::Span;
use sts2_core::{
    CustomReward, DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream,
};

/// `GoldVar(35)`, before `CalculateVars` moves it.
const GOLD: i32 = 35;

/// `HpLossVar(9)`.
const HP_LOSS: i32 = 9;

#[derive(Debug, Clone)]
pub struct State {
    /// What the potions cost, drawn as the event opens.
    gold: i32,
    /// The option taken, while what it asked is open.
    chosen: Option<NonMaxU8>,
}

pub struct WhisperingHollow;

impl<V: crate::Build> EventBehavior<V> for WhisperingHollow {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.gold() >= 44
    }

    /// `CalculateVars`: `Rng.NextInt(-9, 10)` onto the price.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let bound = |n: i32| num_traits::cast(n).expect("a small number fits the RNG integer");
        let span = Span::new(bound(-9), bound(10)).expect("the span is ordered");
        let moved: i32 =
            num_traits::cast(rng.next_int_range(span)).expect("the draw is inside the span");
        State {
            gold: GOLD + moved,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::WHISPERING_HOLLOW,
            &[
                (V::EventOption::WHISPERING_HOLLOW_GOLD, false),
                (V::EventOption::WHISPERING_HOLLOW_HUG, false),
            ],
            state.chosen,
        )
    }

    /// `Gold`: `PlayerCmd.LoseGold`, then `RewardsCmd.OfferCustom` with two
    /// potion rewards. `Hug`: `FromDeckForTransformation` for one card,
    /// `TransformToRandom` off the event's stream, then the HP. Each finishes
    /// the event.
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
                HookEffect::LoseGold(state.gold),
                HookEffect::OfferRewards(&[CustomReward::Potion, CustomReward::Potion]),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Transform(Stream::Event),
                    count: 1,
                },
                HookEffect::DamageOwner(HP_LOSS),
                HookEffect::FinishEvent,
            ],
        }
    }
}
