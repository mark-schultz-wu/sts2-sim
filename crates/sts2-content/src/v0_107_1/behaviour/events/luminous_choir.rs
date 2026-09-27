//! Luminous Choir: two cards removed for a curse, or gold for a relic.

use nonmax::NonMaxU8;
use sts2_core::rng::Span;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::events::{Event, EventOption as Said};

/// `GoldVar(149)`, before `CalculateVars` takes its draw off it.
const GOLD: i32 = 149;

#[derive(Debug, Clone)]
pub struct State {
    /// What the tribute costs, drawn as the event opens.
    gold: i32,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct LuminousChoir;

impl EventBehavior<V> for LuminousChoir {
    type State = State;

    /// Gold enough for the undrawn tribute, and a relic left in the bag, which
    /// the table of allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: `Rng.NextInt(0, 50)` off the gold.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let bound = |n: i32| num_traits::cast(n).expect("a small number fits the RNG integer");
        let span = Span::new(bound(0), bound(50)).expect("the span is ordered");
        let drawn: i32 =
            num_traits::cast(rng.next_int_range(span)).expect("the draw is inside the span");
        State {
            gold: GOLD - drawn,
            chosen: None,
        }
    }

    /// A tribute the purse cannot pay for is locked.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::LuminousChoir,
            &[
                (Said::LuminousChoirReachIntoTheFlesh, false),
                (Said::LuminousChoirOfferTribute, run.gold() < state.gold),
            ],
            state.chosen,
        )
    }

    /// `ReachIntoTheFlesh`: `FromDeckForRemoval` for two cards, then Spore
    /// Mind. `OfferTribute`: the gold, then the next relic from the front of
    /// the bag. Each finishes the event.
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
                    act: DeckAct::Remove,
                    count: 2,
                },
                HookEffect::AddToDeck(Cards::Curse(CurseCard::SporeMind)),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::LoseGold(state.gold),
                HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled),
                HookEffect::FinishEvent,
            ],
        }
    }
}
