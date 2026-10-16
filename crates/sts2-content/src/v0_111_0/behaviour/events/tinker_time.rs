//! Tinker Time: a Mad Science of a chosen type, with a chosen rider.

use nonmax::NonMaxU8;
use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_111_0 as V;
use super::super::super::generated::cards::{Cards, EventCard};
use super::super::super::generated::events::{Event, EventOption as Said};

/// `ChooseCardType`'s options, in the order `TakeRandom` is handed them.
const TYPES: [Said; 3] = [
    Said::TinkerTimeChooseCardTypeAttack,
    Said::TinkerTimeChooseCardTypeSkill,
    Said::TinkerTimeChooseCardTypePower,
];

/// `ChooseRiderEffect`'s riders for each type, in `TYPES` order, each with
/// its option and the Mad Science it makes.
const RIDERS: [[(Said, EventCard); 3]; 3] = [
    [
        (
            Said::TinkerTimeChooseRiderSapping,
            EventCard::MadScienceSapping,
        ),
        (
            Said::TinkerTimeChooseRiderViolence,
            EventCard::MadScienceViolence,
        ),
        (
            Said::TinkerTimeChooseRiderChoking,
            EventCard::MadScienceChoking,
        ),
    ],
    [
        (
            Said::TinkerTimeChooseRiderEnergized,
            EventCard::MadScienceEnergized,
        ),
        (
            Said::TinkerTimeChooseRiderWisdom,
            EventCard::MadScienceWisdom,
        ),
        (Said::TinkerTimeChooseRiderChaos, EventCard::MadScienceChaos),
    ],
    [
        (
            Said::TinkerTimeChooseRiderExpertise,
            EventCard::MadScienceExpertise,
        ),
        (
            Said::TinkerTimeChooseRiderCurious,
            EventCard::MadScienceCurious,
        ),
        (
            Said::TinkerTimeChooseRiderImprovement,
            EventCard::MadScienceImprovement,
        ),
    ],
];

/// The page the event stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Initial,
    /// Two of the three types, as places in `TYPES`.
    Type([usize; 2]),
    /// Two of the chosen type's riders, as places in its row of `RIDERS`.
    Rider {
        kind: usize,
        offered: [usize; 2],
    },
}

#[derive(Debug, Clone)]
pub struct State {
    page: Page,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct TinkerTime;

/// `TakeRandom(2, Rng)` over three things, as their places.
fn two_of_three<R: GameRng>(rng: &mut R) -> [usize; 2] {
    rng.take_random(&mut [0, 1, 2])
        .expect("three things hold two")
}

impl EventBehavior<V> for TinkerTime {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            page: Page::Initial,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: Vec<(Said, bool)> = match state.page {
            Page::Initial => vec![(Said::TinkerTimeChooseCardType, false)],
            Page::Type(offered) => offered.iter().map(|&at| (TYPES[at], false)).collect(),
            Page::Rider { kind, offered } => offered
                .iter()
                .map(|&at| (RIDERS[kind][at].0, false))
                .collect(),
        };
        crate::behaviour::events::page::<V>(Event::TinkerTime, &options, state.chosen)
    }

    /// `ChooseCardType`: two of the types, drawn off the event's stream. A
    /// type: two of its riders, drawn the same way. A rider: the Mad Science
    /// it makes into the deck, which finishes the event.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        let option = usize::from(option);
        match state.page {
            Page::Initial => {
                state.page = Page::Type(two_of_three(rng));
                HookEffects::new()
            }
            Page::Type(offered) => {
                state.page = Page::Rider {
                    kind: offered[option],
                    offered: two_of_three(rng),
                };
                HookEffects::new()
            }
            Page::Rider { kind, offered } => {
                state.chosen = u8::try_from(option).ok().and_then(NonMaxU8::new);
                let (_, card) = RIDERS[kind][offered[option]];
                smallvec::smallvec![
                    HookEffect::AddToDeck(Cards::Event(card)),
                    HookEffect::FinishEvent,
                ]
            }
        }
    }
}
