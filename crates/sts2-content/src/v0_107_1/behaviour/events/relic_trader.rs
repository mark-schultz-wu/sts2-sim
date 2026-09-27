//! Relic Trader: up to three of the player's relics, each for one pulled from
//! the bag.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;
use super::super::relics::tradable;

/// The options, top to bottom, one for each relic on offer.
const OPTIONS: [Said; 3] = [
    Said::RelicTraderTop,
    Said::RelicTraderMiddle,
    Said::RelicTraderBottom,
];

#[derive(Debug, Clone, Default)]
pub struct State {
    /// `OwnedRelics`: what the player can give, top first.
    owned: Vec<Relic>,
    /// `NewRelics`: what each gets back, in the same order.
    new: Vec<Relic>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct RelicTrader;

impl EventBehavior<V> for RelicTrader {
    type State = State;

    /// A later act and five tradable relics, which the shared eligibility
    /// check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `OwnedRelics`: the tradable relics, `StableShuffle`d off the event's
    /// stream, the first three. The shuffle sorts them by id first, which is
    /// the order the relic table is declared in.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let mut owned: Vec<Relic> = run.relics().filter(|&relic| tradable(relic)).collect();
        owned.sort_by_key(|&relic| relic as u16);
        rng.unstable_shuffle(&mut owned);
        owned.truncate(OPTIONS.len());
        if owned.is_empty() {
            unimplemented!("a relic trader with nothing to trade");
        }
        State {
            owned,
            new: Vec::new(),
            chosen: None,
        }
    }

    /// `NewRelics`, which `CalculateVars` pulls in full as the event opens.
    fn relics_pulled(&self, _state: &State) -> usize {
        OPTIONS.len()
    }

    fn hand_pulled_relics(&self, state: &mut State, relics: &[Relic]) {
        state.new = relics.to_vec();
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: Vec<(Said, bool)> = OPTIONS[..state.owned.len()]
            .iter()
            .map(|&said| (said, false))
            .collect();
        super::page(Event::RelicTrader, &options, state.chosen)
    }

    /// `Trade`: `RelicCmd.Remove` on the owned relic, then `RelicCmd.Obtain`
    /// on the new one beside it, which finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let at = usize::from(option);
        smallvec::smallvec![
            HookEffect::RemoveRelic(state.owned[at]),
            HookEffect::ObtainRelic(state.new[at]),
            HookEffect::FinishEvent,
        ]
    }
}
