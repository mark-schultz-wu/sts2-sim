//! Tezcatara, one of the ancients the second act opens on.
//!
//! Three relics, one from each of its own pools. A deck with a basic Strike
//! adds Nutritious Soup to the first.

use sts2_core::rng::GameRng;
use sts2_core::{CardTable as _, EventBehavior, EventView, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{CardRarity, CardTag};
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::Relic;
use super::ancient::{Offered, next_item};

/// `OptionPool1`, before a basic Strike adds `NutritiousSoupOption`.
const POOL_ONE: [Relic; 2] = [Relic::VeryHotCocoa, Relic::YummyCookie];

/// `OptionPool2`.
const POOL_TWO: [Relic; 3] = [Relic::BiiigHug, Relic::Storybook, Relic::ToastyMittens];

/// `OptionPool3`.
const POOL_THREE: [Relic; 4] = [
    Relic::GoldenCompass,
    Relic::PumpkinCandle,
    Relic::ToyBox,
    Relic::SealOfGold,
];

pub struct Tezcatara;

impl EventBehavior<V> for Tezcatara {
    type State = Offered;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered {
        let mut first = POOL_ONE.to_vec();
        if run.deck().iter().any(|card| {
            card.card.tagged(CardTag::Strike) && card.card.rarity() == CardRarity::Basic
        }) {
            first.push(Relic::NutritiousSoup);
        }
        Offered::new(
            [
                next_item(rng, &first),
                next_item(rng, &POOL_TWO),
                next_item(rng, &POOL_THREE),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }

    fn observe(&self, state: &Offered, _run: &impl EventView<V>) -> Room<V> {
        state.observe(Event::Tezcatara)
    }

    fn choose<R: GameRng>(
        &self,
        state: &mut Offered,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.choose(option)
    }
}
