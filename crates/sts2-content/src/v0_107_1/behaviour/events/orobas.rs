//! Orobas, the ancient the second act opens on.
//!
//! Three relics, one from each of its own pools. A roll adds one relic to the
//! first pool. The third pool holds the relics that inspect the run before
//! they are offered.

use sts2_core::rng::{GameRng, Span};
use sts2_core::{
    EventBehavior, EventOption, EventOptionKey, EventView, HookEffect, HookEffects,
    RelicTable as _, Room,
};

use super::super::super::EventOptionId;
use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{
    Cards, DefectCard, IroncladCard, NecrobinderCard, RegentCard, SilentCard,
};
use super::super::super::generated::characters::Character;
use super::super::super::generated::events::Event;
use super::super::super::generated::relics::{Relic, RelicRarity};

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

/// The characters `ModelDb.AllCharacters` lists, in its order.
///
/// Not the table's, which is alphabetical. The draw indexes into this one.
const ALL_CHARACTERS: [Character; 5] = [
    Character::Ironclad,
    Character::Silent,
    Character::Regent,
    Character::Necrobinder,
    Character::Defect,
];

/// `ArchaicTooth.TranscendenceUpgrades`, as the cards it is keyed on.
///
/// One per character, and the Regent's is the Falling Star. A deck holding
/// none of them is one the Tooth has nothing to transform, and it is not
/// offered.
const TRANSCENDS: [Cards; 5] = [
    Cards::Ironclad(IroncladCard::Bash),
    Cards::Silent(SilentCard::Neutralize),
    Cards::Necrobinder(NecrobinderCard::Unleash),
    Cards::Regent(RegentCard::FallingStar),
    Cards::Defect(DefectCard::Dualcast),
];

/// What one copy of this event offers.
#[derive(Debug, Clone)]
pub struct State {
    offered: Vec<Relic>,
    /// The option taken.
    chosen: Option<u8>,
}

pub struct Orobas;

impl EventBehavior<V> for Orobas {
    type State = State;

    /// An ancient is the node its act opens on, and is never rolled as an
    /// unknown room.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        // One of the other characters, drawn whether or not the sea glass that
        // would name it is the option taken. The relic offered is the one
        // relic either way, so which character it was does not show here.
        let others: Vec<Character> = ALL_CHARACTERS
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
        State {
            offered: [
                next_item(rng, &first),
                next_item(rng, &POOL_TWO),
                next_item(rng, &third),
            ]
            .into_iter()
            .flatten()
            .collect(),
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options = state
            .offered
            .iter()
            .enumerate()
            .map(|(index, &relic)| EventOption {
                index: index as u8,
                key: EventOptionKey {
                    event: Event::Orobas,
                    option: EventOptionId::Relic(relic),
                },
                title: None,
                text: None,
                locked: false,
                proceed: false,
                chosen: state.chosen == Some(index as u8),
                relic: Some(relic),
            })
            .collect();
        Room::Event {
            event: Event::Orobas,
            finished: false,
            title: None,
            text: None,
            options,
        }
    }

    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        let relic = state.offered[usize::from(option)];
        state.chosen = Some(option);
        smallvec::smallvec![HookEffect::ObtainRelic(relic), HookEffect::FinishEvent]
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
        .any(|relic| relic.rarity() == RelicRarity::Starter)
    {
        pool.push(Relic::TouchOfOrobas);
    }
    if run
        .deck()
        .iter()
        .any(|held| TRANSCENDS.contains(&held.card))
    {
        pool.push(Relic::ArchaicTooth);
    }
    pool
}

/// One item of a list, which is `Rng.NextItem`.
///
/// A two-argument `NextInt`, which is a different draw from the one-argument
/// one even over the same range. An empty list costs no draw at all.
fn next_item<T: Copy, R: GameRng>(rng: &mut R, items: &[T]) -> Option<T> {
    let count: R::Int = num_traits::cast(items.len())?;
    let span = Span::new(num_traits::zero(), count).ok()?;
    let at: usize = num_traits::cast(rng.next_int_range(span))?;
    items.get(at).copied()
}
