//! Darv, one of the ancients the second and third acts open on.
//!
//! One relic from each set the run qualifies for, shuffled, and three of
//! them offered. Half the time the third is a Dusty Tome instead.

use sts2_core::rng::GameRng;
use sts2_core::{CardTable as _, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{CardRarity, Cards, pool_cards};
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::Relic;
use super::super::relics::TRANSCENDENCE;
use super::ancient::{Offered, next_item};

/// `_validRelicSets`, in order, each with the act it asks for, where it asks
/// for one. No run here plays a modifier that clears the deck, so the
/// Pandora's Box set always qualifies.
const SETS: [(Option<usize>, &[Relic]); 9] = [
    (None, &[Relic::Astrolabe]),
    (None, &[Relic::BlackStar]),
    (None, &[Relic::CallingBell]),
    (None, &[Relic::EmptyCage]),
    (None, &[Relic::PandorasBox]),
    (None, &[Relic::RunicPyramid]),
    (None, &[Relic::SneckoEye]),
    (Some(1), &[Relic::Ectoplasm, Relic::Sozu]),
    (Some(2), &[Relic::PhilosophersStone, Relic::VelvetChoker]),
];

/// What one copy of this event offers.
#[derive(Debug, Clone)]
pub struct State {
    offered: Offered,
    /// Whether the third option is the Dusty Tome.
    tome: bool,
    /// `DustyTome.AncientCard`, which `SetupForPlayer` draws.
    ancient: Option<Cards>,
}

pub struct Darv;

impl EventBehavior<V> for Darv {
    type State = State;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    /// `GenerateInitialOptions`: one `NextItem` over each set the run
    /// qualifies for, in order, then `UnstableShuffle`. `NextBool` then
    /// takes two and the Dusty Tome, or three.
    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let act: usize = num_traits::cast(run.act()).expect("an act index fits a usize");
        let mut source: Vec<Relic> = SETS
            .iter()
            .filter(|(wanted, _)| wanted.is_none_or(|wanted| wanted == act))
            .filter_map(|(_, relics)| next_item(rng, relics))
            .collect();
        rng.unstable_shuffle(&mut source);
        let tome = rng.next_bool();
        let offered = match tome {
            true => source
                .into_iter()
                .take(2)
                .chain([Relic::DustyTome])
                .collect(),
            false => source.into_iter().take(3).collect(),
        };
        State {
            offered: Offered::new(offered),
            tome,
            ancient: None,
        }
    }

    /// `DustyTome.SetupForPlayer`: `NextItem` over the character's ancient
    /// cards that no starter card transcends into.
    fn set_up<R: GameRng>(&self, state: &mut State, run: &impl EventView<V>, rewards: &mut R) {
        if !state.tome {
            return;
        }
        let pool = sts2_core::Table::data(run.character()).card_pool;
        let ancient: Vec<Cards> = pool_cards(pool)
            .iter()
            .copied()
            .filter(|&card| card.rarity() == CardRarity::Ancient)
            .filter(|&card| !TRANSCENDENCE.iter().any(|&(_, into)| into == card))
            .collect();
        state.ancient = next_item(rewards, &ancient);
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        state.offered.observe(Event::Darv)
    }

    /// `RelicOption`. The Dusty Tome's `AfterObtained` adds its ancient card,
    /// upgraded. Its model comes with nothing written on it, so the card
    /// `SetupForPlayer` named comes with it here.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        let relic = state.offered.relic(option);
        let mut asked = state.offered.choose(option);
        if relic == Relic::DustyTome {
            let card = state
                .ancient
                .unwrap_or_else(|| unimplemented!("a Dusty Tome with no ancient card to give"));
            asked.insert(1, HookEffect::AddUpgradedToDeck(card));
        }
        asked
    }
}
