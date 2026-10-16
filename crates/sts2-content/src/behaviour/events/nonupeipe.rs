//! Nonupeipe, one of the ancients the third act opens on.
//!
//! Three relics from one pool, shuffled. A deck with four cards Swift can
//! enchant adds Beautiful Bracelet to it.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use crate::behaviour::enchantments::can_enchant;
use crate::behaviour::events::ancient::Offered;
use crate::ids::{EnchantmentIds as _, EventIds as _, RelicIds as _};

/// How many deck cards Swift must take for the bracelet.
const BRACELET_CARDS: usize = 4;

/// How many options the event offers.
const OFFERED: usize = 3;

pub struct Nonupeipe;

impl<V: crate::Build> EventBehavior<V> for Nonupeipe {
    type State = Offered<V>;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered<V> {
        let mut pool = V::NONUPEIPE_POOL.to_vec();
        if run
            .deck()
            .iter()
            .filter(|card| can_enchant(V::Enchantment::SWIFT, card))
            .count()
            >= BRACELET_CARDS
        {
            pool.push(V::Relic::BEAUTIFUL_BRACELET);
        }
        rng.unstable_shuffle(&mut pool);
        pool.truncate(OFFERED);
        Offered::new(pool)
    }

    fn observe(&self, state: &Offered<V>, _run: &impl EventView<V>) -> Room<V> {
        state.observe(V::Event::NONUPEIPE)
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

/// `POOL`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `OptionPool`, before `BeautifulBraceletEventOption` joins it.
        const NONUPEIPE_POOL: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::BLESSED_ANTLER,
            <$build as sts2_core::ContentVersion>::Relic::BRILLIANT_SCARF,
            <$build as sts2_core::ContentVersion>::Relic::DELICATE_FROND,
            <$build as sts2_core::ContentVersion>::Relic::DIAMOND_DIADEM,
            <$build as sts2_core::ContentVersion>::Relic::FUR_COAT,
            <$build as sts2_core::ContentVersion>::Relic::GLITTER,
            <$build as sts2_core::ContentVersion>::Relic::JEWELRY_BOX,
            <$build as sts2_core::ContentVersion>::Relic::LOOMING_FRUIT,
            <$build as sts2_core::ContentVersion>::Relic::SIGNET_RING,
        ];
    };
}

pub(crate) use lists;
