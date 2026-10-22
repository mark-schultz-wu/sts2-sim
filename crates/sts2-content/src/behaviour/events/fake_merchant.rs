//! Fake Merchant: a shop of six fake relics inside an event.

use crate::ids::{EncounterIds as _, RelicIds as _};
use sts2_core::rng::{GameRng, Interval};
use sts2_core::{
    CustomReward, EventBehavior, EventView, HookEffect, HookEffects, RelicTable as _, Room, Stock,
};

type Number = i32;

/// How many of them it stocks, which is `Take(6)`.
const STOCKED: usize = 6;

/// `MerchantRelicEntry.CalcCost`'s spread.
const SPREAD: (f32, f32) = (0.85, 1.15);

/// One shelf: its relic, its price, and whether it is still there.
#[derive(Debug, Clone, Copy)]
struct Shelf<V: crate::Build> {
    relic: V::Relic,
    cost: Number,
    stocked: bool,
}

#[derive(Debug, Clone)]
pub struct State<V: crate::Build> {
    /// `Inventory.RelicEntries`, in the order they were added.
    shelves: Vec<Shelf<V>>,
    /// `StartedFight`: a Foul Potion was thrown, and the merchant's button
    /// is hidden.
    started: bool,
}

pub struct FakeMerchant;

impl<V: crate::Build> EventBehavior<V> for FakeMerchant {
    type State = State<V>;

    /// The second act on, with a hundred gold or a Foul Potion, which the
    /// table of allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `BeforeEventStarted`: the fake relics unstably shuffled off the
    /// event's stream, and the first six shelved. `stock` prices them.
    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State<V> {
        let mut relics = V::FAKE_MERCHANT_INVENTORY.to_vec();
        rng.unstable_shuffle(&mut relics);
        State {
            shelves: relics[..STOCKED]
                .iter()
                .map(|&relic| Shelf {
                    relic,
                    cost: 0,
                    stocked: true,
                })
                .collect(),
            started: false,
        }
    }

    /// `MerchantRelicEntry.CalcCost` for each shelf in turn: its
    /// `MerchantCost` times a draw off the player's shop stream, rounded half
    /// to even.
    fn stock<R: GameRng>(&self, state: &mut State<V>, shops: &mut R) {
        for shelf in &mut state.shelves {
            let base = shelf
                .relic
                .merchant_price()
                .unwrap_or_else(|| unimplemented!("{:?} with no merchant price", shelf.relic));
            let scale = shops.next_f32_range(Interval::new(SPREAD.0, SPREAD.1));
            let cost = (f32::from(base) * scale).round_ties_even();
            shelf.cost = num_traits::cast(cost).expect("a relic's price fits a game number");
        }
    }

    /// The shop its `NFakeMerchant` screen shows.
    fn observe(&self, state: &State<V>, run: &impl EventView<V>) -> Room<V> {
        let gold = run.gold();
        Room::Shop {
            entries: state
                .shelves
                .iter()
                .enumerate()
                .map(|(index, shelf)| sts2_core::Entry {
                    index: u8::try_from(index).expect("six shelves fit a byte"),
                    stock: Stock::Relic(shelf.stocked.then_some(shelf.relic)),
                    cost: shelf.cost,
                    title: None,
                    text: None,
                    stocked: shelf.stocked,
                    affordable: gold >= shelf.cost,
                    blocked: None,
                })
                .collect(),
        }
    }

    /// Nothing to choose: the room is its shop.
    fn choose<R: GameRng>(
        &self,
        _state: &mut State<V>,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        unimplemented!("option {option} of a Fake Merchant, which offers none")
    }

    /// `MerchantEntry.OnTryPurchaseWrapper` over a `MerchantRelicEntry`: the
    /// price, then the relic, then `AfterItemPurchased`. Outside a merchant's
    /// room no entry refills, so the shelf is left empty.
    /// `NFakeMerchant` hides its button once the fight has started, and
    /// shows it until then. Its inventory is never open in a run with no
    /// screen.
    fn merchant(&self, state: &State<V>) -> bool {
        !state.started
    }

    /// `FoulPotionThrown`: `StartedFight`, then the fight inside the event,
    /// which goes on to the map once it is won. Its rewards are the
    /// fight's own and [`EventBehavior::rewards`].
    fn potion_thrown(&self, state: &mut State<V>, _potion: V::Potion) -> HookEffects<V> {
        state.started = true;
        smallvec::smallvec![HookEffect::FightInEvent {
            encounter: V::Encounter::FAKE_MERCHANT_EVENT_ENCOUNTER,
            rewards: &[],
            roster: &[],
            resume: false,
            own_rewards: true,
        }]
    }

    /// The fight's extra rewards: a `RelicReward` of the Fake Merchant's Rug,
    /// then one of each relic still on a shelf, in shelf order.
    fn rewards(&self, state: &State<V>) -> Vec<CustomReward<V>> {
        std::iter::once(V::Relic::FAKE_MERCHANTS_RUG)
            .chain(
                (state.shelves.iter())
                    .filter(|shelf| shelf.stocked)
                    .map(|shelf| shelf.relic),
            )
            .map(CustomReward::SpecificRelic)
            .collect()
    }

    fn buy(&self, state: &mut State<V>, index: u8) -> HookEffects<V> {
        let shelf = &mut state.shelves[usize::from(index)];
        shelf.stocked = false;
        smallvec::smallvec![
            HookEffect::LoseGold(shelf.cost),
            HookEffect::ObtainRelic(shelf.relic),
            HookEffect::ItemPurchased(shelf.cost),
        ]
    }
}

/// `INVENTORY`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `_inventoryRelics`, in the order it lists them.
        const FAKE_MERCHANT_INVENTORY: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::FAKE_ANCHOR,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_BLOOD_VIAL,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_HAPPY_FLOWER,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_LEES_WAFFLE,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_MANGO,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_ORICHALCUM,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_SNECKO_EYE,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_STRIKE_DUMMY,
            <$build as sts2_core::ContentVersion>::Relic::FAKE_VENERABLE_TEA_SET,
        ];
    };
}

pub(crate) use lists;
