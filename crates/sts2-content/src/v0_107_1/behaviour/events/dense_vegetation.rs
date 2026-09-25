//! Dense Vegetation: gold for HP, or a rest that the wrigglers interrupt.

use nonmax::NonMaxU8;
use sts2_core::rng::Span;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `HpLossVar(8)`.
const HP_LOSS: i32 = 8;

/// The page the event stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Initial,
    /// After the rest, which offers the fight alone.
    Rest,
}

#[derive(Debug, Clone)]
pub struct State {
    /// What trudging on pays, drawn as the event opens.
    gold: i32,
    page: Page,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct DenseVegetation;

impl EventBehavior<V> for DenseVegetation {
    type State = State;

    /// Its HP check is for a run of more than one player.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: `Rng.NextInt(61, 100)` is the gold. The heal it
    /// shows reads the run and draws nothing.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let bound = |n: i32| num_traits::cast(n).expect("a small number fits the RNG integer");
        let span = Span::new(bound(61), bound(100)).expect("the span is ordered");
        let gold: i32 =
            num_traits::cast(rng.next_int_range(span)).expect("the draw is inside the span");
        State {
            gold,
            page: Page::Initial,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: &[(Said, bool)] = match state.page {
            Page::Initial => &[
                (Said::DenseVegetationTrudgeOn, false),
                (Said::DenseVegetationRest, false),
            ],
            Page::Rest => &[(Said::DenseVegetationRestFight, false)],
        };
        super::page(Event::DenseVegetation, options, state.chosen)
    }

    /// `TrudgeOn`: the damage, then the gold, which finishes the event.
    /// `Rest`: `PlayerCmd.MimicRestSiteHeal`, then the page that offers the
    /// fight. `Fight`: the event's encounter, fought in its room.
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
                    HookEffect::DamageOwner(HP_LOSS),
                    HookEffect::GainGold(state.gold),
                    HookEffect::FinishEvent,
                ]
            }
            (Page::Initial, _) => {
                state.page = Page::Rest;
                state.chosen = None;
                smallvec::smallvec![HookEffect::MimicRestSiteHeal]
            }
            (Page::Rest, _) => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![HookEffect::FightInEvent(
                    Encounter::DenseVegetationEventEncounter
                )]
            }
        }
    }
}
