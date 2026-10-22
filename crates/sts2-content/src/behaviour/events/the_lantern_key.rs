//! The Lantern Key: gold for the key, or a fight for it.

use crate::ids::{EncounterIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

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

impl<V: crate::Build> EventBehavior<V> for TheLanternKey {
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
        let options: &[(V::EventOption, bool)] = match state.page {
            Page::Initial => &[
                (V::EventOption::THE_LANTERN_KEY_RETURN_THE_KEY, false),
                (V::EventOption::THE_LANTERN_KEY_KEEP_THE_KEY, false),
            ],
            Page::KeepTheKey => &[(V::EventOption::THE_LANTERN_KEY_KEEP_THE_KEY_FIGHT, false)],
        };
        crate::behaviour::events::page::<V>(V::Event::THE_LANTERN_KEY, options, state.chosen)
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
                    encounter: V::Encounter::MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER,
                    rewards: V::THE_LANTERN_KEY_REWARDS,
                    roster: &[],
                    resume: false,
                    own_rewards: false,
                }]
            }
        }
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// The fight's reward: the Lantern Key.
        const THE_LANTERN_KEY_REWARDS: &'static [sts2_core::CustomReward<$build>] =
            &[sts2_core::CustomReward::SpecialCard(
                <$build as sts2_core::ContentVersion>::Card::LANTERN_KEY,
            )];
    };
}

pub(crate) use lists;
