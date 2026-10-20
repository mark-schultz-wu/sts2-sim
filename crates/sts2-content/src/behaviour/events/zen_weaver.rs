//! Zen Weaver: two Enlightenments, or one or two cards removed, for gold.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `DynamicVar("BreathingTechniquesCost", 50)`.
const BREATHING_COST: i32 = 50;

/// `DynamicVar("EmotionalAwarenessCost", 125)`.
const EMOTIONAL_COST: i32 = 125;

/// `DynamicVar("ArachnidAcupunctureCost", 250)`.
const ARACHNID_COST: i32 = 250;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct ZenWeaver;

impl<V: crate::Build> EventBehavior<V> for ZenWeaver {
    type State = State;

    /// The gold for Emotional Awareness, which the shared eligibility check
    /// answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// Each removal the purse cannot pay for is the locked option instead.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        let gold = run.gold();
        let paid = |cost: i32, said: V::EventOption| {
            if gold < cost {
                (V::EventOption::ZEN_WEAVER_LOCKED, true)
            } else {
                (said, false)
            }
        };
        crate::behaviour::events::page::<V>(
            V::Event::ZEN_WEAVER,
            &[
                (V::EventOption::ZEN_WEAVER_BREATHING_TECHNIQUES, false),
                paid(
                    EMOTIONAL_COST,
                    V::EventOption::ZEN_WEAVER_EMOTIONAL_AWARENESS,
                ),
                paid(
                    ARACHNID_COST,
                    V::EventOption::ZEN_WEAVER_ARACHNID_ACUPUNCTURE,
                ),
            ],
            state.chosen,
        )
    }

    /// `BreathingTechniques`: the gold, then two Enlightenments into the
    /// deck. `EmotionalAwareness` and `ArachnidAcupuncture`:
    /// `FromDeckForRemoval` for one or two, then the gold. Each finishes the
    /// event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let enlightenment = V::Card::ENLIGHTENMENT;
        match option {
            0 => smallvec::smallvec![
                HookEffect::LoseGold(BREATHING_COST),
                HookEffect::AddToDeck(enlightenment),
                HookEffect::AddToDeck(enlightenment),
                HookEffect::FinishEvent,
            ],
            1 => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Remove,
                    count: 1,
                },
                HookEffect::LoseGold(EMOTIONAL_COST),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Remove,
                    count: 2,
                },
                HookEffect::LoseGold(ARACHNID_COST),
                HookEffect::FinishEvent,
            ],
        }
    }
}
