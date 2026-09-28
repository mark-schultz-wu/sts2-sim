//! Vakuu, one of the ancients the third act opens on.
//!
//! Three relics, the first of each of its own pools once all three are
//! shuffled.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::Relic;
use super::ancient::Offered;

/// `Pool1`.
const POOL_ONE: [Relic; 3] = [
    Relic::BloodSoakedRose,
    Relic::WhisperingEarring,
    Relic::Fiddle,
];

/// `Pool2`. Distinguished Cape's `ThatDecreasesMaxHp` only colours its
/// button.
const POOL_TWO: [Relic; 3] = [
    Relic::PreservedFog,
    Relic::SereTalon,
    Relic::DistinguishedCape,
];

/// `Pool3`.
const POOL_THREE: [Relic; 4] = [
    Relic::ChoicesParadox,
    Relic::MusicBox,
    Relic::LordsParasol,
    Relic::JeweledMask,
];

pub struct Vakuu;

impl EventBehavior<V> for Vakuu {
    type State = Offered;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    /// Each pool is shuffled in turn, and each gives its first.
    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> Offered {
        let mut first = POOL_ONE;
        let mut second = POOL_TWO;
        let mut third = POOL_THREE;
        rng.unstable_shuffle(&mut first);
        rng.unstable_shuffle(&mut second);
        rng.unstable_shuffle(&mut third);
        Offered::new(vec![first[0], second[0], third[0]])
    }

    fn observe(&self, state: &Offered, _run: &impl EventView<V>) -> Room<V> {
        state.observe(Event::Vakuu)
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
