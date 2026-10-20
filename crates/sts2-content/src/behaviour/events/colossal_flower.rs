//! Colossal Flower: gold that grows with each dig, or Pollinous Core at the
//! bottom, each dig paid for in HP.

use crate::ids::{EventIds as _, RelicIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `_prizeCosts`: the gold each dig's prize pays.
const PRIZES: [i32; 3] = [35, 75, 135];

/// `_prizeDamage`: what each dig deals its owner.
const DAMAGE: [i32; 3] = [5, 6, 7];

#[derive(Debug, Clone, Default)]
pub struct State {
    /// `NumberOfDigs`.
    digs: usize,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct ColossalFlower;

impl<V: crate::Build> EventBehavior<V> for ColossalFlower {
    type State = State;

    /// Its HP check, which the shared eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let [extract, deeper] = V::COLOSSAL_FLOWER_PAGES[state.digs];
        crate::behaviour::events::page::<V>(
            V::Event::COLOSSAL_FLOWER,
            &[(extract, false), (deeper, false)],
            state.chosen,
        )
    }

    /// The first option takes the current prize's gold and finishes the
    /// event. The second deals this dig's damage and digs once more, or on
    /// the last page deals it and obtains Pollinous Core.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        let digs = state.digs;
        match option {
            0 => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![HookEffect::GainGold(PRIZES[digs]), HookEffect::FinishEvent]
            }
            _ if digs + 1 < V::COLOSSAL_FLOWER_PAGES.len() => {
                state.digs += 1;
                state.chosen = None;
                smallvec::smallvec![HookEffect::DamageOwner(DAMAGE[digs])]
            }
            _ => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![
                    HookEffect::DamageOwner(DAMAGE[digs]),
                    HookEffect::ObtainRelic(V::Relic::POLLINOUS_CORE),
                    HookEffect::FinishEvent,
                ]
            }
        }
    }
}

/// `PAGES`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// Each page's options, by `NumberOfDigs`.
        const COLOSSAL_FLOWER_PAGES: &'static [[<$build as sts2_core::ContentVersion>::EventOption; 2]] = &[
    [
        <$build as sts2_core::ContentVersion>::EventOption::COLOSSAL_FLOWER_EXTRACT_CURRENT_PRIZE1,
        <$build as sts2_core::ContentVersion>::EventOption::COLOSSAL_FLOWER_REACH_DEEPER1,
    ],
    [
        <$build as sts2_core::ContentVersion>::EventOption::COLOSSAL_FLOWER_REACH_DEEPER1_EXTRACT_CURRENT_PRIZE2,
        <$build as sts2_core::ContentVersion>::EventOption::COLOSSAL_FLOWER_REACH_DEEPER1_REACH_DEEPER2,
    ],
    [
        <$build as sts2_core::ContentVersion>::EventOption::COLOSSAL_FLOWER_REACH_DEEPER2_EXTRACT_INSTEAD,
        <$build as sts2_core::ContentVersion>::EventOption::COLOSSAL_FLOWER_REACH_DEEPER2_POLLINOUS_CORE,
    ],
];
    };
}

pub(crate) use lists;
