//! Ranwid the Elder: a potion, gold or a relic, each for relics from the bag.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;
use super::super::relics::tradable;
use super::ancient::next_item;

/// `GoldVar(100)`.
const GOLD: i32 = 100;

#[derive(Debug, Clone)]
pub struct State {
    /// The potion asked for, by its place among the potions held.
    potion: usize,
    /// The relic asked for.
    relic: Relic,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct RanwidTheElder;

impl EventBehavior<V> for RanwidTheElder {
    type State = State;

    /// A later act, a tradable relic, a potion and the gold, which the shared
    /// eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `GenerateInitialOptions`: a potion held with `NextItem`, then a
    /// tradable relic the same way, both off the event's stream.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let held: Vec<usize> = (0..run.potions().count()).collect();
        let Some(potion) = next_item(rng, &held) else {
            unimplemented!("Ranwid with no potion to ask for");
        };
        let relics: Vec<Relic> = run.relics().filter(|&relic| tradable(relic)).collect();
        let Some(relic) = next_item(rng, &relics) else {
            unimplemented!("Ranwid with no relic to ask for");
        };
        State {
            potion,
            relic,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::RanwidTheElder,
            &[
                (Said::RanwidTheElderPotion, false),
                (Said::RanwidTheElderGold, false),
                (Said::RanwidTheElderRelic, false),
            ],
            state.chosen,
        )
    }

    /// `GivePotion`: the potion discarded, then a relic pulled from the front
    /// of the bag. `GiveGold`: the gold, then the same. `GiveRelic`: the relic
    /// removed, then two pulled. Each finishes the event.
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
                HookEffect::DiscardPotion(state.potion),
                HookEffect::ObtainNextRelic,
                HookEffect::FinishEvent,
            ],
            1 => smallvec::smallvec![
                HookEffect::LoseGold(GOLD),
                HookEffect::ObtainNextRelic,
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::RemoveRelic(state.relic),
                HookEffect::ObtainNextRelic,
                HookEffect::ObtainNextRelic,
                HookEffect::FinishEvent,
            ],
        }
    }
}
