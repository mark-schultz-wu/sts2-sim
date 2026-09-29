//! Orobas, the ancient the second act opens on.
//!
//! Three relics, one from each of its own pools. A roll adds one relic to the
//! first pool. The third pool holds the relics that inspect the run before
//! they are offered.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, RelicTable as _, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::characters::Character;
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::{Relic, RelicRarity};
use super::super::relics::TRANSCENDENCE;
use super::ancient::{Offered, next_item};

/// `Orobas._prismaticOdds`, which the Sea Glass takes the other side of.
const PRISMATIC: f32 = 0.3333333;

/// `OptionPool1`, before the roll adds a fourth to it.
const POOL_ONE: [Relic; 3] = [Relic::ElectricShrymp, Relic::GlassEye, Relic::SandCastle];

/// `OptionPool2`.
const POOL_TWO: [Relic; 3] = [
    Relic::AlchemicalCoffer,
    Relic::Driftwood,
    Relic::RadiantPearl,
];

pub struct Orobas;

impl EventBehavior<V> for Orobas {
    type State = Offered;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Offered {
        // One of the other characters, drawn whether or not the sea glass that
        // would name it is the option taken. The relic offered is the one
        // relic either way, so which character it was does not show here.
        let others: Vec<Character> = super::super::ALL_CHARACTERS
            .into_iter()
            .filter(|&other| other != run.character())
            .collect();
        let _cut_for = next_item(rng, &others);

        let paired = match (rng.next_double() as f32) < PRISMATIC {
            true => Relic::PrismaticGem,
            false => Relic::SeaGlass,
        };
        let first: Vec<Relic> = POOL_ONE.into_iter().chain([paired]).collect();
        let third = pool_three(run);
        Offered::new(
            [
                next_item(rng, &first),
                next_item(rng, &POOL_TWO),
                next_item(rng, &third),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }

    fn observe(&self, state: &Offered, _run: &impl EventView<V>) -> Room<V> {
        state.observe(Event::Orobas)
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

/// `OptionPool3`, which is the two relics that read the run first.
///
/// `TouchOfOrobas.SetupForPlayer` needs a starter relic to refine, and
/// `ArchaicTooth.SetupForPlayer` a starter card to transform. A run that
/// answers neither is offered a locked option instead, and a pool with
/// nothing in it costs no draw.
fn pool_three(run: &impl EventView<V>) -> Vec<Relic> {
    let mut pool = Vec::new();
    if run
        .relics()
        .any(|held| held.relic.rarity() == RelicRarity::Starter)
    {
        pool.push(Relic::TouchOfOrobas);
    }
    if run
        .deck()
        .iter()
        .any(|held| TRANSCENDENCE.iter().any(|&(from, _)| from == held.card))
    {
        pool.push(Relic::ArchaicTooth);
    }
    pool
}
