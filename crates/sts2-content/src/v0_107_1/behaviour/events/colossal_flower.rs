//! Colossal Flower: gold that grows with each dig, or Pollinous Core at the
//! bottom, each dig paid for in HP.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

/// `_prizeCosts`: the gold each dig's prize pays.
const PRIZES: [i32; 3] = [35, 75, 135];

/// `_prizeDamage`: what each dig deals its owner.
const DAMAGE: [i32; 3] = [5, 6, 7];

/// Each page's options, by `NumberOfDigs`.
const PAGES: [[Said; 2]; 3] = [
    [
        Said::ColossalFlowerExtractCurrentPrize1,
        Said::ColossalFlowerReachDeeper1,
    ],
    [
        Said::ColossalFlowerReachDeeper1ExtractCurrentPrize2,
        Said::ColossalFlowerReachDeeper1ReachDeeper2,
    ],
    [
        Said::ColossalFlowerReachDeeper2ExtractInstead,
        Said::ColossalFlowerReachDeeper2PollinousCore,
    ],
];

#[derive(Debug, Clone, Default)]
pub struct State {
    /// `NumberOfDigs`.
    digs: usize,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct ColossalFlower;

impl EventBehavior<V> for ColossalFlower {
    type State = State;

    /// Its HP check, which the shared eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let [extract, deeper] = PAGES[state.digs];
        super::page(
            Event::ColossalFlower,
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
            _ if digs + 1 < PAGES.len() => {
                state.digs += 1;
                state.chosen = None;
                smallvec::smallvec![HookEffect::DamageOwner(DAMAGE[digs])]
            }
            _ => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![
                    HookEffect::DamageOwner(DAMAGE[digs]),
                    HookEffect::ObtainRelic(Relic::PollinousCore),
                    HookEffect::FinishEvent,
                ]
            }
        }
    }
}
