//! Ranwid the Elder: a potion, gold or a relic, each for relics from the bag.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use crate::behaviour::events::ancient::next_item;
use crate::behaviour::relics::tradable;
use crate::ids::{EventIds as _, EventOptionIds as _};

/// `GoldVar(100)`.
const GOLD: i32 = 100;

#[derive(Debug, Clone)]
pub struct State<V: crate::Build> {
    /// The potion asked for, by its place among the potions held.
    potion: usize,
    /// The relic asked for.
    relic: V::Relic,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct RanwidTheElder;

impl<V: crate::Build> EventBehavior<V> for RanwidTheElder {
    type State = State<V>;

    /// A later act, a tradable relic, a potion and the gold, which the shared
    /// eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `GenerateInitialOptions`: a potion held with `NextItem`, then a
    /// tradable relic the same way, both off the event's stream.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State<V> {
        let held: Vec<usize> = (0..run.potions().count()).collect();
        let Some(potion) = next_item(rng, &held) else {
            unimplemented!("Ranwid with no potion to ask for");
        };
        let relics: Vec<V::Relic> = run
            .relics()
            .filter(|&held| tradable(held))
            .map(|held| held.relic)
            .collect();
        let Some(relic) = next_item(rng, &relics) else {
            unimplemented!("Ranwid with no relic to ask for");
        };
        State {
            potion,
            relic,
            chosen: None,
        }
    }

    fn observe(&self, state: &State<V>, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::RANWID_THE_ELDER,
            &[
                (V::EventOption::RANWID_THE_ELDER_POTION, false),
                (V::EventOption::RANWID_THE_ELDER_GOLD, false),
                (V::EventOption::RANWID_THE_ELDER_RELIC, false),
            ],
            state.chosen,
        )
    }

    /// `GivePotion`: the potion discarded, then a relic pulled from the front
    /// of the bag. `GiveGold`: the gold, then the same. `GiveRelic`: the relic
    /// removed, then two pulled. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State<V>,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::DiscardPotion(state.potion),
                HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled),
                HookEffect::FinishEvent,
            ],
            1 => smallvec::smallvec![
                HookEffect::LoseGold(GOLD),
                HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::RemoveRelic(state.relic),
                HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled),
                HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled),
                HookEffect::FinishEvent,
            ],
        }
    }
}
