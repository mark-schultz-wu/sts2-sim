//! Punch Off: a relic for an Injury, or a fight with both constructs for a
//! relic and a potion.

use crate::ids::{CardIds as _, EncounterIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::rng::Span;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

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

impl<V: crate::Build> EventBehavior<V> for PunchOff {
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
        let options: &[(V::EventOption, bool)] = match state.page {
            Page::Initial => &[
                (V::EventOption::PUNCH_OFF_NAB, false),
                (V::EventOption::PUNCH_OFF_ICAN_TAKE_THEM, false),
            ],
            Page::TakeThem => &[(V::EventOption::PUNCH_OFF_ICAN_TAKE_THEM_FIGHT, false)],
        };
        crate::behaviour::events::page::<V>(V::Event::PUNCH_OFF, options, state.chosen)
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
                    HookEffect::AddToDeck(V::Card::INJURY),
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
                    encounter: V::Encounter::PUNCH_OFF_EVENT_ENCOUNTER,
                    rewards: &[CustomReward::RolledRelic, CustomReward::Potion],
                    roster: &[],
                    resume: false,
                    own_rewards: false,
                }]
            }
        }
    }
}
