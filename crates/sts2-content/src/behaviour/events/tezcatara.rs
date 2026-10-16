//! Tezcatara, one of the ancients the second act opens on.
//!
//! Three relics, one from each of its own pools. A deck with a basic Strike
//! adds Nutritious Soup to the first.

use sts2_core::rng::GameRng;
use sts2_core::{CardTable as _, EventBehavior, EventView, HookEffects, Room};

use crate::behaviour::events::ancient::{Offered, next_item};
use crate::generated::cards::{CardRarity, CardTag};
use crate::ids::{EventIds as _, RelicIds as _};

pub struct Tezcatara;

impl<V: crate::Build> EventBehavior<V> for Tezcatara {
    type State = Offered<V>;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered<V> {
        let mut first = V::TEZCATARA_POOL_ONE.to_vec();
        if run.deck().iter().any(|card| {
            card.card.tagged(CardTag::Strike) && card.card.rarity() == CardRarity::Basic
        }) {
            first.push(V::Relic::NUTRITIOUS_SOUP);
        }
        Offered::new(
            [
                next_item(rng, &first),
                next_item(rng, V::TEZCATARA_POOL_TWO),
                next_item(rng, V::TEZCATARA_POOL_THREE),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }

    fn observe(&self, state: &Offered<V>, _run: &impl EventView<V>) -> Room<V> {
        state.observe(V::Event::TEZCATARA)
    }

    fn choose<R: GameRng>(
        &self,
        state: &mut Offered<V>,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.choose(option)
    }
}

/// `POOL_ONE`, `POOL_TWO`, `POOL_THREE`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `OptionPool1`, before a basic Strike adds `NutritiousSoupOption`.
        const TEZCATARA_POOL_ONE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::VERY_HOT_COCOA,
            <$build as sts2_core::ContentVersion>::Relic::YUMMY_COOKIE,
        ];
        /// `OptionPool2`.
        const TEZCATARA_POOL_TWO: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::BIIIG_HUG,
            <$build as sts2_core::ContentVersion>::Relic::STORYBOOK,
            <$build as sts2_core::ContentVersion>::Relic::TOASTY_MITTENS,
        ];
        /// `OptionPool3`.
        const TEZCATARA_POOL_THREE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::GOLDEN_COMPASS,
            <$build as sts2_core::ContentVersion>::Relic::PUMPKIN_CANDLE,
            <$build as sts2_core::ContentVersion>::Relic::TOY_BOX,
            <$build as sts2_core::ContentVersion>::Relic::SEAL_OF_GOLD,
        ];
    };
}

pub(crate) use lists;
