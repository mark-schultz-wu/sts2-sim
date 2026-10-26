//! The lifecycle shared by event implementations.

use crate::rng::GameRng;
use crate::{ContentVersion, Room};

/// A relic the run carries, as an event reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarriedRelic<V: ContentVersion> {
    pub relic: V::Relic,
    /// `RelicModel.IsUsedUp`, which some relics work out from what they
    /// saved.
    pub used_up: bool,
    /// `RelicModel.IsMelted`: Toy Box melted it, and it stays on the belt.
    pub melted: bool,
}

/// The read-only run facts event behavior can inspect.
pub trait EventView<V: ContentVersion> {
    fn act(&self) -> V::Count;
    /// `RunState.TotalFloor`: every room the run has entered, across acts.
    fn floor(&self) -> V::Count;
    fn gold(&self) -> V::Number;
    fn hp(&self) -> V::Health;
    fn max_hp(&self) -> V::Health;
    /// The potions in the belt, in slot order.
    /// `Player.HasOpenPotionSlots`: a slot of the belt that holds no potion.
    fn open_potion_slot(&self) -> bool;

    fn potions(&self) -> impl Iterator<Item = V::Potion> + '_;
    /// Who the run is being played as.
    ///
    /// Not in an observation, which reports what a run holds rather than who
    /// is holding it. The run itself is what answers an event's questions.
    fn character(&self) -> V::Character;
    /// Whether the run has several players, which the card pools an event
    /// draws from deal the co-op cards from.
    fn multiplayer(&self) -> bool;
    /// The runs the profile had won with the character as this one started.
    /// See [`Start::wins`](crate::Start::wins).
    fn wins(&self) -> u32;
    /// `RunState.Modifiers`, in the order the run was started with them.
    fn modifiers(&self) -> &[crate::RunModifier<V>];
    /// The relics the run carries, in panel order.
    fn relics(&self) -> impl Iterator<Item = CarriedRelic<V>> + '_;
    /// The deck as it stands, in deck order, by what each card is.
    fn deck(&self) -> Vec<crate::CardIdent<V>>;
    /// `RelicGrabBag.HasAvailableRelics` on the player's own bag: whether a
    /// relic reward still has a relic to deal.
    fn relics_left(&self) -> bool;
    /// `RunState.CurrentRoom`'s `RoomType`, for a room no fight opens in: a
    /// shop, an event, a treasure room or a rest site. Nothing on the map,
    /// and in or after a fight.
    fn room(&self) -> Option<V::RoomType>;
}

/// One event's eligibility, state, observation, and choices.
pub trait EventBehavior<V: ContentVersion> {
    type State: Clone + std::fmt::Debug;

    fn allowed(&self, run: &impl EventView<V>) -> bool;

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> Self::State;

    /// `SetupForPlayer` on what the options offer, which draws off the
    /// player's rewards stream as the options generate. An event whose
    /// options set nothing up does nothing here.
    fn set_up<R: GameRng>(
        &self,
        _state: &mut Self::State,
        _run: &impl EventView<V>,
        _rewards: &mut R,
    ) {
    }

    /// `BeforeEventStarted` stocking a merchant inventory of the event's own,
    /// each entry priced off the player's shop stream as `CalcCost` prices
    /// it. An event with no inventory does nothing here.
    fn stock<R: GameRng>(&self, _state: &mut Self::State, _shops: &mut R) {}

    /// `MerchantEntry.OnTryPurchaseWrapper` on the event's entry at `index`,
    /// which the run has checked is stocked and affordable. What the purchase
    /// does to the run comes back, in the order the game does it.
    fn buy(&self, _state: &mut Self::State, index: u8) -> crate::HookEffects<V> {
        unimplemented!("buying entry {index} from an event")
    }

    /// The relics the event pulls from the front of the player's bag as it
    /// opens, after [`EventBehavior::set_up`], in the order pulled.
    fn relics_pulled(&self, _state: &Self::State) -> &'static [crate::RelicPull<V>] {
        &[]
    }

    /// The relics [`EventBehavior::relics_pulled`] asked for, in the order
    /// pulled.
    fn hand_pulled_relics(&self, _state: &mut Self::State, _relics: &[V::Relic]) {}

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

    /// The question the event parks on after
    /// [`HookEffect::AskEvent`](crate::HookEffect::AskEvent), as it stands.
    fn question(&self, _state: &Self::State) -> Option<crate::Question<V>> {
        None
    }

    /// Take the answer to [`EventBehavior::question`]. What it does to the
    /// run comes back, in the order the game does it.
    fn answer(&self, _state: &mut Self::State, answer: &crate::Answer) -> crate::HookEffects<V> {
        unimplemented!("an event answered with {answer:?}")
    }

    /// The rest of an option's body once a pickup it offered has closed,
    /// which [`HookEffect::ResumeEvent`](crate::HookEffect::ResumeEvent)
    /// asks for. What it does to the run comes back, in the order the game
    /// does it.
    fn resume<R: GameRng>(
        &self,
        _state: &mut Self::State,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> crate::HookEffects<V> {
        unimplemented!("an event resumed after its pickup")
    }

    /// `EventModel.Resume(room)`, once the fight the event entered is over
    /// and its rewards' proceed goes back to the event. `escaped` is whether
    /// an enemy escaped that fight. What it does to the run comes back, in
    /// the order the game does it.
    fn resume_from_fight(&self, _state: &mut Self::State, escaped: bool) -> crate::HookEffects<V> {
        unimplemented!("an event resumed after its fight, escaped {escaped}")
    }

    /// The rewards
    /// [`HookEffect::OfferEventRewards`](crate::HookEffect::OfferEventRewards)
    /// offers, in the order the game makes them.
    fn rewards(&self, _state: &Self::State) -> Vec<crate::CustomReward<V>> {
        unimplemented!("an event with rewards of its own")
    }

    /// Whether `FoulPotion.GetFoulPotionMerchantTarget` finds a merchant's
    /// button on the event's screen, which a Foul Potion can be thrown at.
    fn merchant(&self, _state: &Self::State) -> bool {
        false
    }

    /// What a potion thrown at the event's merchant does, in the order the
    /// game does it.
    fn potion_thrown(&self, _state: &mut Self::State, potion: V::Potion) -> crate::HookEffects<V> {
        unimplemented!("{potion:?} thrown at an event")
    }
}
