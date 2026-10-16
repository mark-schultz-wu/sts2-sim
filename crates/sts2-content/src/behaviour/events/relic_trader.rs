//! Relic Trader: up to three of the player's relics, each for one pulled from
//! the bag.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use crate::behaviour::relics::tradable;
use crate::ids::EventIds as _;

#[derive(Debug, Clone)]
pub struct State<V: crate::Build> {
    /// `OwnedRelics`: what the player can give, top first.
    owned: Vec<V::Relic>,
    /// `NewRelics`: what each gets back, in the same order.
    new: Vec<V::Relic>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        Self {
            owned: Default::default(),
            new: Default::default(),
            chosen: Default::default(),
        }
    }
}

pub struct RelicTrader;

impl<V: crate::Build> EventBehavior<V> for RelicTrader {
    type State = State<V>;

    /// A later act and five tradable relics, which the shared eligibility
    /// check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `OwnedRelics`: the tradable relics, `StableShuffle`d off the event's
    /// stream, the first three. The shuffle sorts them by id first, which is
    /// the order the relic table is declared in.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State<V> {
        let mut owned: Vec<V::Relic> = run
            .relics()
            .filter(|&held| tradable(held))
            .map(|held| held.relic)
            .collect();
        owned.sort_by_key(|&relic| <V as crate::build::Tables>::relic_index(relic));
        rng.unstable_shuffle(&mut owned);
        owned.truncate(V::RELIC_TRADER_OPTIONS.len());
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
    /// One for each option, each at a rolled rarity.
    fn relics_pulled(&self, _state: &State<V>) -> &'static [sts2_core::RelicPull<V>] {
        V::RELIC_TRADER_PULLED
    }

    fn hand_pulled_relics(&self, state: &mut State<V>, relics: &[V::Relic]) {
        state.new = relics.to_vec();
    }

    fn observe(&self, state: &State<V>, _run: &impl EventView<V>) -> Room<V> {
        let options: Vec<(V::EventOption, bool)> = V::RELIC_TRADER_OPTIONS[..state.owned.len()]
            .iter()
            .map(|&said| (said, false))
            .collect();
        crate::behaviour::events::page::<V>(V::Event::RELIC_TRADER, &options, state.chosen)
    }

    /// `Trade`: `RelicCmd.Remove` on the owned relic, then `RelicCmd.Obtain`
    /// on the new one beside it, which finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State<V>,
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

/// `PULLED`, `OPTIONS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// The options, top to bottom, one for each relic on offer.
        /// One rolled pull for each of [`OPTIONS`].
        const RELIC_TRADER_PULLED: &'static [sts2_core::RelicPull<$build>] =
            &[sts2_core::RelicPull::Rolled; 3];
        const RELIC_TRADER_OPTIONS:
            &'static [<$build as sts2_core::ContentVersion>::EventOption] = &[
            <$build as sts2_core::ContentVersion>::EventOption::RELIC_TRADER_TOP,
            <$build as sts2_core::ContentVersion>::EventOption::RELIC_TRADER_MIDDLE,
            <$build as sts2_core::ContentVersion>::EventOption::RELIC_TRADER_BOTTOM,
        ];
    };
}

pub(crate) use lists;
