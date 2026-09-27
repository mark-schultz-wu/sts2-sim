//! The lifecycle shared by event implementations.

use crate::rng::GameRng;
use crate::{ContentVersion, Room};

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
    /// The deck as it stands, in deck order, by what each card is.
    fn deck(&self) -> Vec<crate::CardIdent<V>>;
    /// `RelicGrabBag.HasAvailableRelics` on the player's own bag: whether a
    /// relic reward still has a relic to deal.
    fn relics_left(&self) -> bool;
}

/// One event's eligibility, state, observation, and choices.
pub trait EventBehavior<V: ContentVersion> {
    type State: Clone + std::fmt::Debug;

    fn allowed(&self, run: &impl EventView<V>) -> bool;

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Self::State;

    /// The page as it stands. Whether the event has finished is the run's,
    /// which [`HookEffect::FinishEvent`](crate::HookEffect::FinishEvent)
    /// sets, and a finished room offers nothing.
    fn observe(&self, state: &Self::State, run: &impl EventView<V>) -> Room<V>;

    /// Take `option`. The state turns its page or marks the choice. What the
    /// option does to the run comes back, in the order the game does it.
    fn choose<R: GameRng>(
        &self,
        state: &mut Self::State,
        option: u8,
        run: &impl EventView<V>,
        rng: &mut R,
    ) -> crate::HookEffects<V>;
}
