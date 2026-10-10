//! The Lantern Key: gold for the key, or a fight for it.

use nonmax::NonMaxU8;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, QuestCard};
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `GoldVar(100)`.
const GOLD: i32 = 100;

/// The page the event stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Initial,
    /// After keeping the key, which offers the fight alone.
    KeepTheKey,
}

#[derive(Debug, Clone)]
pub struct State {
    page: Page,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct TheLanternKey;

impl EventBehavior<V> for TheLanternKey {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// Its `Combat` layout builds the knight as it opens, which the run does.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            page: Page::Initial,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: &[(Said, bool)] = match state.page {
            Page::Initial => &[
                (Said::TheLanternKeyReturnTheKey, false),
                (Said::TheLanternKeyKeepTheKey, false),
            ],
            Page::KeepTheKey => &[(Said::TheLanternKeyKeepTheKeyFight, false)],
        };
        super::page(Event::TheLanternKey, options, state.chosen)
    }

    /// `ReturnTheKey`: the gold, which finishes the event. `KeepTheKey`: the
    /// page that offers the fight. `Fight`: the knight, fought in the event's
    /// room, with a Lantern Key among the rewards.
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
                smallvec::smallvec![HookEffect::GainGold(GOLD), HookEffect::FinishEvent]
            }
            (Page::Initial, _) => {
                state.page = Page::KeepTheKey;
                state.chosen = None;
                HookEffects::new()
            }
            (Page::KeepTheKey, _) => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![HookEffect::FightInEvent {
                    encounter: Encounter::MysteriousKnightEventEncounter,
                    rewards: &[CustomReward::SpecialCard(Cards::Quest(
                        QuestCard::LanternKey
                    ))],
                    roster: &[],
                    resume: false,
                }]
            }
        }
    }
}
