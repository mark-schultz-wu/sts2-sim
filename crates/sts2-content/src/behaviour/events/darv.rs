//! Darv, one of the ancients the second and third acts open on.
//!
//! One relic from each set the run qualifies for, shuffled, and three of
//! them offered. Half the time the third is a Dusty Tome instead.

use sts2_core::rng::GameRng;
use sts2_core::{CardTable as _, EventBehavior, EventView, HookEffect, HookEffects, Room};

use crate::behaviour::events::ancient::{Offered, next_item};
use crate::generated::cards::CardRarity;
use crate::ids::{EventIds as _, RelicIds as _};

/// What one copy of this event offers.
#[derive(Debug, Clone)]
pub struct State<V: crate::Build> {
    offered: Offered<V>,
    /// Whether the third option is the Dusty Tome.
    tome: bool,
    /// `DustyTome.AncientCard`, which `SetupForPlayer` draws.
    ancient: Option<V::Card>,
}

pub struct Darv;

impl<V: crate::Build> EventBehavior<V> for Darv {
    type State = State<V>;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    /// `GenerateInitialOptions`: one `NextItem` over each set the run
    /// qualifies for, in order, then `UnstableShuffle`. `NextBool` then
    /// takes two and the Dusty Tome, or three.
    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State<V> {
        let act: usize = num_traits::cast(run.act()).expect("an act index fits a usize");
        let cleared = (run.modifiers().iter())
            .any(|entry| sts2_core::ModifierTable::clears_deck(entry.modifier));
        let mut source: Vec<V::Relic> = V::DARV_SETS
            .iter()
            .filter(|(wanted, _)| wanted.is_none_or(|wanted| wanted == act))
            .filter(|(_, relics)| !(cleared && relics.contains(&V::Relic::PANDORAS_BOX)))
            .filter_map(|(_, relics)| next_item(rng, relics))
            .collect();
        rng.unstable_shuffle(&mut source);
        let tome = rng.next_bool();
        let offered = match tome {
            true => source
                .into_iter()
                .take(2)
                .chain([V::Relic::DUSTY_TOME])
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
    fn set_up<R: GameRng>(&self, state: &mut State<V>, run: &impl EventView<V>, rewards: &mut R) {
        if !state.tome {
            return;
        }
        let ancient: Vec<V::Card> = <V as crate::build::Tables>::pool_cards(run.character())
            .iter()
            .copied()
            .filter(|&card| card.rarity() == CardRarity::Ancient)
            .filter(|&card| {
                !<V as crate::build::Lists>::ARCHAIC_TOOTH_TRANSCENDENCE
                    .iter()
                    .any(|&(_, into)| into == card)
            })
            .collect();
        state.ancient = next_item(rewards, &ancient);
    }

    fn observe(&self, state: &State<V>, _run: &impl EventView<V>) -> Room<V> {
        state.offered.observe(V::Event::DARV)
    }

    /// `RelicOption`. The Dusty Tome's `AfterObtained` adds its ancient card,
    /// upgraded. Its model comes with nothing written on it, so the card
    /// `SetupForPlayer` named comes with it here.
    fn choose<R: GameRng>(
        &self,
        state: &mut State<V>,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        let relic = state.offered.relic(option);
        let mut asked = state.offered.choose(option);
        if relic == V::Relic::DUSTY_TOME {
            let card = state
                .ancient
                .unwrap_or_else(|| unimplemented!("a Dusty Tome with no ancient card to give"));
            asked.insert(1, HookEffect::AddUpgradedToDeck(card));
        }
        asked
    }
}

/// `SETS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `_validRelicSets`, in order, each with the act it asks for, where it asks
        /// for one. The Pandora's Box set also asks for a run no modifier cleared
        /// the deck of.
        const DARV_SETS: &'static [(
            Option<usize>,
            &[<$build as sts2_core::ContentVersion>::Relic],
        )] = &[
            (
                None,
                &[<$build as sts2_core::ContentVersion>::Relic::ASTROLABE],
            ),
            (
                None,
                &[<$build as sts2_core::ContentVersion>::Relic::BLACK_STAR],
            ),
            (
                None,
                &[<$build as sts2_core::ContentVersion>::Relic::CALLING_BELL],
            ),
            (
                None,
                &[<$build as sts2_core::ContentVersion>::Relic::EMPTY_CAGE],
            ),
            (
                None,
                &[<$build as sts2_core::ContentVersion>::Relic::PANDORAS_BOX],
            ),
            (
                None,
                &[<$build as sts2_core::ContentVersion>::Relic::RUNIC_PYRAMID],
            ),
            (
                None,
                &[<$build as sts2_core::ContentVersion>::Relic::SNECKO_EYE],
            ),
            (
                Some(1),
                &[
                    <$build as sts2_core::ContentVersion>::Relic::ECTOPLASM,
                    <$build as sts2_core::ContentVersion>::Relic::SOZU,
                ],
            ),
            (
                Some(2),
                &[
                    <$build as sts2_core::ContentVersion>::Relic::PHILOSOPHERS_STONE,
                    <$build as sts2_core::ContentVersion>::Relic::VELVET_CHOKER,
                ],
            ),
        ];
    };
}

pub(crate) use lists;
