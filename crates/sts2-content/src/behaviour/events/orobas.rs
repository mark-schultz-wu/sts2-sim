//! Orobas, the ancient the second act opens on.
//!
//! Three relics, one from each of its own pools. A roll adds one relic to the
//! first pool. The third pool holds the relics that inspect the run before
//! they are offered.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, RelicTable as _, Room};

use crate::behaviour::events::ancient::{Offered, next_item};
use crate::generated::relics::RelicRarity;
use crate::ids::{EventIds as _, RelicIds as _};

/// `Orobas._prismaticOdds`, which the Sea Glass takes the other side of.
const PRISMATIC: f32 = 0.3333333;

pub struct Orobas;

/// What Orobas offers, and the character a Sea Glass it offers names.
#[derive(Debug, Clone)]
pub struct State<V: crate::Build> {
    offered: Offered<V>,
    sea_glass: V::Character,
}

impl<V: crate::Build> EventBehavior<V> for Orobas {
    type State = State<V>;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State<V> {
        // One of the other characters, drawn whether or not the Sea Glass that
        // names it is offered. The player's own where there is no other.
        let others: Vec<V::Character> = <V as crate::build::Lists>::ALL_CHARACTERS
            .iter()
            .copied()
            .filter(|&other| other != run.character())
            .collect();
        let sea_glass = next_item(rng, &others).unwrap_or(run.character());

        let paired = match (rng.next_double() as f32) < PRISMATIC {
            true => V::Relic::PRISMATIC_GEM,
            false => V::Relic::SEA_GLASS,
        };
        let first: Vec<V::Relic> = V::OROBAS_POOL_ONE.iter().copied().chain([paired]).collect();
        let third = pool_three::<V>(run);
        let offered = Offered::new(
            [
                next_item(rng, &first),
                next_item(rng, V::OROBAS_POOL_TWO),
                next_item(rng, &third),
            ]
            .into_iter()
            .flatten()
            .collect(),
        );
        State { offered, sea_glass }
    }

    fn observe(&self, state: &State<V>, _run: &impl EventView<V>) -> Room<V> {
        state.offered.observe(V::Event::OROBAS)
    }

    /// `RelicOption`, with the Sea Glass set up for its character first.
    fn choose<R: GameRng>(
        &self,
        state: &mut State<V>,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        let mut effects = state.offered.choose(option);
        if state.offered.relic(option) == V::Relic::SEA_GLASS {
            effects[0] = sts2_core::HookEffect::ObtainRelicFor {
                relic: V::Relic::SEA_GLASS,
                character: state.sea_glass,
            };
        }
        effects
    }
}

/// `OptionPool3`, which is the two relics that read the run first.
///
/// `TouchOfOrobas.SetupForPlayer` needs a starter relic to refine, and
/// `ArchaicTooth.SetupForPlayer` a starter card to transform. A run that
/// answers neither is offered a locked option instead, and a pool with
/// nothing in it costs no draw.
fn pool_three<V: crate::Build>(run: &impl EventView<V>) -> Vec<V::Relic> {
    let mut pool = Vec::new();
    if run
        .relics()
        .any(|held| held.relic.rarity() == RelicRarity::Starter)
    {
        pool.push(V::Relic::TOUCH_OF_OROBAS);
    }
    if run.deck().iter().any(|held| {
        <V as crate::build::Lists>::ARCHAIC_TOOTH_TRANSCENDENCE
            .iter()
            .any(|&(from, _)| from == held.card)
    }) {
        pool.push(V::Relic::ARCHAIC_TOOTH);
    }
    pool
}

/// `POOL_ONE`, `POOL_TWO`, written for one build's own types.
///
/// Each build's game fills them its own way, so the build's module picks the
/// arm.
macro_rules! lists {
    ($build:ty, v0_107_1) => {
        /// `OptionPool1`, before the roll adds a fourth to it.
        const OROBAS_POOL_ONE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::ELECTRIC_SHRYMP,
            <$build as sts2_core::ContentVersion>::Relic::GLASS_EYE,
            <$build as sts2_core::ContentVersion>::Relic::SAND_CASTLE,
        ];
        /// `OptionPool2`.
        const OROBAS_POOL_TWO: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::ALCHEMICAL_COFFER,
            <$build as sts2_core::ContentVersion>::Relic::DRIFTWOOD,
            <$build as sts2_core::ContentVersion>::Relic::RADIANT_PEARL,
        ];
    };
    // Sand Castle moved to the second pool.
    ($build:ty, v0_111_0) => {
        /// `OptionPool1`, before the roll adds a third to it.
        const OROBAS_POOL_ONE: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::ELECTRIC_SHRYMP,
            <$build as sts2_core::ContentVersion>::Relic::GLASS_EYE,
        ];
        /// `OptionPool2`.
        const OROBAS_POOL_TWO: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::ALCHEMICAL_COFFER,
            <$build as sts2_core::ContentVersion>::Relic::DRIFTWOOD,
            <$build as sts2_core::ContentVersion>::Relic::RADIANT_PEARL,
            <$build as sts2_core::ContentVersion>::Relic::SAND_CASTLE,
        ];
    };
}

pub(crate) use lists;
