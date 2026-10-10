//! Punch Off: a relic for an Injury, or a fight with both constructs for a
//! relic and a potion.

use nonmax::NonMaxU8;
use sts2_core::rng::Span;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::events::{Event, EventOption as Said};

/// The page the event stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Initial,
    /// After `I_CAN_TAKE_THEM`, which offers the fight alone.
    TakeThem,
}

#[derive(Debug, Clone)]
pub struct State {
    page: Page,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct PunchOff;

impl EventBehavior<V> for PunchOff {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.floor() >= 6
    }

    /// `CalculateVars`: `Rng.NextInt(91, 99)` for a `GoldVar` no option
    /// reads. Its `Combat` layout builds the constructs as it opens, which
    /// the run does.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let bound = |n: i32| num_traits::cast(n).expect("a small number fits the RNG integer");
        let span = Span::new(bound(91), bound(99)).expect("the span is ordered");
        rng.next_int_range(span);
        State {
            page: Page::Initial,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: &[(Said, bool)] = match state.page {
            Page::Initial => &[
                (Said::PunchOffNab, false),
                (Said::PunchOffICanTakeThem, false),
            ],
            Page::TakeThem => &[(Said::PunchOffICanTakeThemFight, false)],
        };
        super::page(Event::PunchOff, options, state.chosen)
    }

    /// `Nab`: an Injury into the deck, then a relic reward, which finishes
    /// the event. `TakeThem`: the page that offers the fight. `Fight`: both
    /// constructs in the event's room, with a relic and a potion among the
    /// rewards. It does not come back to the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        match (state.page, option) {
            (Page::Initial, 0) => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![
                    HookEffect::AddToDeck(Cards::Curse(CurseCard::Injury)),
                    HookEffect::OfferRewards(&[CustomReward::RolledRelic]),
                    HookEffect::FinishEvent,
                ]
            }
            (Page::Initial, _) => {
                state.page = Page::TakeThem;
                state.chosen = None;
                HookEffects::new()
            }
            (Page::TakeThem, _) => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![HookEffect::FightInEvent {
                    encounter: Encounter::PunchOffEventEncounter,
                    rewards: &[CustomReward::RolledRelic, CustomReward::Potion],
                    roster: &[],
                    resume: false,
                }]
            }
        }
    }
}
