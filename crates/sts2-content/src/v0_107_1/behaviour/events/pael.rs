//! Pael, one of the ancients the second act opens on.
//!
//! Three relics, one from each of its own pools. The second and third pools
//! grow with what the deck and the belt hold.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::Relic;
use super::super::enchantments::can_enchant;
use super::ancient::{Offered, next_item};

/// `OptionPool1`.
const POOL_ONE: [Relic; 3] = [Relic::PaelsFlesh, Relic::PaelsHorn, Relic::PaelsTears];

/// `OptionPool3`, before a run with no event pet adds `PaelsLegionOption`.
const POOL_THREE: [Relic; 2] = [Relic::PaelsEye, Relic::PaelsBlood];

/// How many deck cards Goopy must take for `PaelsClawOption`.
const CLAW_CARDS: usize = 3;

/// How many removable deck cards `PaelsToothOption` needs.
const TOOTH_CARDS: usize = 5;

pub struct Pael;

impl EventBehavior<V> for Pael {
    type State = Offered;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered {
        let first = next_item(rng, &POOL_ONE);

        // `OptionPool2`, then Claw and Tooth where the deck allows. The list
        // is then added to itself, which doubles each, before Growth joins.
        let mut second = vec![Relic::PaelsWing];
        let deck = run.deck();
        if deck
            .iter()
            .filter(|card| can_enchant(Enchantment::Goopy, card))
            .count()
            >= CLAW_CARDS
        {
            second.push(Relic::PaelsClaw);
        }
        if deck
            .iter()
            .filter(|card| !super::super::enchantments::card_eternal(card))
            .count()
            >= TOOTH_CARDS
        {
            second.push(Relic::PaelsTooth);
        }
        second.extend_from_within(..);
        second.push(Relic::PaelsGrowth);
        let second = next_item(rng, &second);

        let mut third = POOL_THREE.to_vec();
        if !super::has_event_pet(run) {
            third.push(Relic::PaelsLegion);
        }
        let third = next_item(rng, &third);

        Offered::new([first, second, third].into_iter().flatten().collect())
    }

    fn observe(&self, state: &Offered, _run: &impl EventView<V>) -> Room<V> {
        state.observe(Event::Pael)
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
