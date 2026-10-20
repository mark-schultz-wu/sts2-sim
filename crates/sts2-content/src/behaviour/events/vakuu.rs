//! Vakuu, one of the ancients the third act opens on.
//!
//! Three relics, the first of each of its own pools once all three are
//! shuffled.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use crate::behaviour::events::ancient::Offered;
use crate::ids::EventIds as _;

pub struct Vakuu;

impl<V: crate::Build> EventBehavior<V> for Vakuu {
    type State = Offered<V>;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    /// Each pool is shuffled in turn, and each gives its first.
    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> Offered<V> {
        let mut first = V::VAKUU_POOL_ONE.to_vec();
        let mut second = V::VAKUU_POOL_TWO.to_vec();
        let mut third = V::VAKUU_POOL_THREE.to_vec();
        rng.unstable_shuffle(&mut first);
        rng.unstable_shuffle(&mut second);
        rng.unstable_shuffle(&mut third);
        Offered::new(vec![first[0], second[0], third[0]])
    }

    fn observe(&self, state: &Offered<V>, _run: &impl EventView<V>) -> Room<V> {
        state.observe(V::Event::VAKUU)
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
        /// `Pool1`.
        const VAKUU_POOL_ONE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::BLOOD_SOAKED_ROSE,
            <$build as sts2_core::ContentVersion>::Relic::WHISPERING_EARRING,
            <$build as sts2_core::ContentVersion>::Relic::FIDDLE,
        ];
        /// `Pool2`. Distinguished Cape's `ThatDecreasesMaxHp` only colours its
        /// button.
        const VAKUU_POOL_TWO: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::PRESERVED_FOG,
            <$build as sts2_core::ContentVersion>::Relic::SERE_TALON,
            <$build as sts2_core::ContentVersion>::Relic::DISTINGUISHED_CAPE,
        ];
        /// `Pool3`.
        const VAKUU_POOL_THREE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::CHOICES_PARADOX,
            <$build as sts2_core::ContentVersion>::Relic::MUSIC_BOX,
            <$build as sts2_core::ContentVersion>::Relic::LORDS_PARASOL,
            <$build as sts2_core::ContentVersion>::Relic::JEWELED_MASK,
        ];
    };
}

pub(crate) use lists;
