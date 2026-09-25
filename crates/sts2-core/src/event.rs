//! The lifecycle shared by event implementations.

use crate::rng::GameRng;
use crate::{ContentVersion, Question, RewardKind, Room};

/// The read-only run facts event behavior can inspect.
pub trait EventView<V: ContentVersion> {
    fn act(&self) -> V::Count;
    /// `RunState.TotalFloor`: every room the run has entered, across acts.
    fn floor(&self) -> V::Count;
    fn gold(&self) -> V::Number;
    fn hp(&self) -> V::Health;
    fn max_hp(&self) -> V::Health;
    /// The potions in the belt, in slot order.
    fn potions(&self) -> impl Iterator<Item = V::Potion> + '_;
    /// Who the run is being played as.
    ///
    /// Not in an observation, which reports what a run holds rather than who
    /// is holding it. The run itself is what answers an event's questions.
    fn character(&self) -> V::Character;
    /// The relics the run carries, in panel order.
    fn relics(&self) -> impl Iterator<Item = V::Relic> + '_;
    /// The deck as it stands, in deck order.
    fn deck(&self) -> &[crate::CardIdent<V>];
    /// `RelicGrabBag.HasAvailableRelics` on the player's own bag: whether a
    /// relic reward still has a relic to deal.
    fn relics_left(&self) -> bool;
}

/// The run operations an event is allowed to perform.
///
/// Keeping this surface narrow prevents content behavior from depending on a
/// simulator's storage model.
pub trait EventContext<V: ContentVersion> {
    fn gold(&self) -> V::Number;
    fn spend_gold(&mut self, amount: V::Number) -> bool;
    fn obtain_relic(&mut self, relic: V::Relic);
}

/// Where an event choice leaves the run.
#[derive(Debug, Clone)]
pub enum EventFlow<V: ContentVersion> {
    Stay,
    Finished,
    Ask(Question<V>),
    Rewards(Vec<RewardKind<V>>),
    StartCombat(V::Encounter),
}

/// One event's eligibility, state, observation, and choices.
pub trait EventBehavior<V: ContentVersion> {
    type State: Clone + std::fmt::Debug;

    fn allowed(&self, run: &impl EventView<V>) -> bool;

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Self::State;

    fn observe(&self, state: &Self::State, run: &impl EventView<V>) -> Room<V>;

    fn choose<R: GameRng, C: EventContext<V>>(
        &self,
        state: &mut Self::State,
        option: u8,
        run: &mut C,
        rng: &mut R,
    ) -> EventFlow<V>;
}
