//! Tanx, one of the ancients the third act opens on.
//!
//! Three relics from one pool, shuffled. A deck with three cards Instinct can
//! enchant adds Tri-Boomerang to it.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::Relic;
use super::super::enchantments::can_enchant;
use super::ancient::Offered;

/// `BaseOptionPool`, before `TriBoomerangOption` joins it.
const POOL: [Relic; 9] = [
    Relic::Claws,
    Relic::Crossbow,
    Relic::IronClub,
    Relic::MeatCleaver,
    Relic::Sai,
    Relic::SpikedGauntlets,
    Relic::TanxsWhistle,
    Relic::ThrowingAxe,
    Relic::WarHammer,
];

/// How many deck cards Instinct must take for the boomerang.
const BOOMERANG_CARDS: usize = 3;

/// How many options the event offers.
const OFFERED: usize = 3;

pub struct Tanx;

impl EventBehavior<V> for Tanx {
    type State = Offered;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered {
        let mut pool = POOL.to_vec();
        if run
            .deck()
            .iter()
            .filter(|card| can_enchant(Enchantment::Instinct, card))
            .count()
            >= BOOMERANG_CARDS
        {
            pool.push(Relic::TriBoomerang);
        }
        rng.unstable_shuffle(&mut pool);
        pool.truncate(OFFERED);
        Offered::new(pool)
    }

    fn observe(&self, state: &Offered, _run: &impl EventView<V>) -> Room<V> {
        state.observe(Event::Tanx)
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
