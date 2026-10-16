//! Tanx, one of the ancients the third act opens on.
//!
//! Three relics from one pool, shuffled. A deck with three cards Instinct can
//! enchant adds Tri-Boomerang to it.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use crate::behaviour::enchantments::can_enchant;
use crate::behaviour::events::ancient::Offered;
use crate::ids::{EnchantmentIds as _, EventIds as _, RelicIds as _};

/// How many deck cards Instinct must take for the boomerang.
const BOOMERANG_CARDS: usize = 3;

/// How many options the event offers.
const OFFERED: usize = 3;

pub struct Tanx;

impl<V: crate::Build> EventBehavior<V> for Tanx {
    type State = Offered<V>;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered<V> {
        let mut pool = V::TANX_POOL.to_vec();
        if run
            .deck()
            .iter()
            .filter(|card| can_enchant(V::Enchantment::INSTINCT, card))
            .count()
            >= BOOMERANG_CARDS
        {
            pool.push(V::Relic::TRI_BOOMERANG);
        }
        rng.unstable_shuffle(&mut pool);
        pool.truncate(OFFERED);
        Offered::new(pool)
    }

    fn observe(&self, state: &Offered<V>, _run: &impl EventView<V>) -> Room<V> {
        state.observe(V::Event::TANX)
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
        /// `BaseOptionPool`, before `TriBoomerangOption` joins it.
        const TANX_POOL: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::CLAWS,
            <$build as sts2_core::ContentVersion>::Relic::CROSSBOW,
            <$build as sts2_core::ContentVersion>::Relic::IRON_CLUB,
            <$build as sts2_core::ContentVersion>::Relic::MEAT_CLEAVER,
            <$build as sts2_core::ContentVersion>::Relic::SAI,
            <$build as sts2_core::ContentVersion>::Relic::SPIKED_GAUNTLETS,
            <$build as sts2_core::ContentVersion>::Relic::TANXS_WHISTLE,
            <$build as sts2_core::ContentVersion>::Relic::THROWING_AXE,
            <$build as sts2_core::ContentVersion>::Relic::WAR_HAMMER,
        ];
    };
}

pub(crate) use lists;
