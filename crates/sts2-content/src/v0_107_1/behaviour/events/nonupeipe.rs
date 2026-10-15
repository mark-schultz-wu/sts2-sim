//! Nonupeipe, one of the ancients the third act opens on.
//!
//! Three relics from one pool, shuffled. A deck with four cards Swift can
//! enchant adds Beautiful Bracelet to it.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::Relic;
use super::ancient::Offered;
use crate::behaviour::enchantments::can_enchant;

/// `OptionPool`, before `BeautifulBraceletEventOption` joins it.
const POOL: [Relic; 9] = [
    Relic::BlessedAntler,
    Relic::BrilliantScarf,
    Relic::DelicateFrond,
    Relic::DiamondDiadem,
    Relic::FurCoat,
    Relic::Glitter,
    Relic::JewelryBox,
    Relic::LoomingFruit,
    Relic::SignetRing,
];

/// How many deck cards Swift must take for the bracelet.
const BRACELET_CARDS: usize = 4;

/// How many options the event offers.
const OFFERED: usize = 3;

pub struct Nonupeipe;

impl EventBehavior<V> for Nonupeipe {
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
            .filter(|card| can_enchant(Enchantment::Swift, card))
            .count()
            >= BRACELET_CARDS
        {
            pool.push(Relic::BeautifulBracelet);
        }
        rng.unstable_shuffle(&mut pool);
        pool.truncate(OFFERED);
        Offered::new(pool)
    }

    fn observe(&self, state: &Offered, _run: &impl EventView<V>) -> Room<V> {
        state.observe(Event::Nonupeipe)
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
