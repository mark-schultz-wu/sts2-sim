//! Pael, one of the ancients the second act opens on.
//!
//! Three relics, one from each of its own pools. The second and third pools
//! grow with what the deck and the belt hold.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use crate::behaviour::enchantments::can_enchant;
use crate::behaviour::events::ancient::{Offered, next_item};
use crate::ids::{EnchantmentIds as _, EventIds as _, RelicIds as _};

/// How many deck cards Goopy must take for `PaelsClawOption`.
const CLAW_CARDS: usize = 3;

/// How many removable deck cards `PaelsToothOption` needs.
const TOOTH_CARDS: usize = 5;

pub struct Pael;

impl<V: crate::Build> EventBehavior<V> for Pael {
    type State = Offered<V>;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered<V> {
        let first = next_item(rng, V::PAEL_POOL_ONE);

        // `OptionPool2`, then Claw and Tooth where the deck allows. The list
        // is then added to itself, which doubles each, before Growth joins.
        let mut second = vec![V::Relic::PAELS_WING];
        let deck = run.deck();
        if deck
            .iter()
            .filter(|card| can_enchant(V::Enchantment::GOOPY, card))
            .count()
            >= CLAW_CARDS
        {
            second.push(V::Relic::PAELS_CLAW);
        }
        if deck
            .iter()
            .filter(|card| !crate::behaviour::enchantments::card_eternal(card))
            .count()
            >= TOOTH_CARDS
        {
            second.push(V::Relic::PAELS_TOOTH);
        }
        second.extend_from_within(..);
        second.push(V::Relic::PAELS_GROWTH);
        let second = next_item(rng, &second);

        let mut third = V::PAEL_POOL_THREE.to_vec();
        if !crate::behaviour::events::has_event_pet::<V>(run) {
            third.push(V::Relic::PAELS_LEGION);
        }
        let third = next_item(rng, &third);

        Offered::new([first, second, third].into_iter().flatten().collect())
    }

    fn observe(&self, state: &Offered<V>, _run: &impl EventView<V>) -> Room<V> {
        state.observe(V::Event::PAEL)
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

/// `POOL_ONE`, `POOL_THREE`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `OptionPool1`.
        const PAEL_POOL_ONE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::PAELS_FLESH,
            <$build as sts2_core::ContentVersion>::Relic::PAELS_HORN,
            <$build as sts2_core::ContentVersion>::Relic::PAELS_TEARS,
        ];
        /// `OptionPool3`, before a run with no event pet adds `PaelsLegionOption`.
        const PAEL_POOL_THREE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::PAELS_EYE,
            <$build as sts2_core::ContentVersion>::Relic::PAELS_BLOOD,
        ];
    };
}

pub(crate) use lists;
