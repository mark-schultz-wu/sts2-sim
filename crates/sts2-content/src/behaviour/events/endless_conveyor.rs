//! Endless Conveyor: a dish off the belt for gold, as many as the player
//! likes, or an upgrade for watching the chef.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::rng::Interval;
use sts2_core::{
    CustomReward, DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream,
};

/// `GoldVar(40)`, what each dish costs.
const PRICE: i32 = 40;

/// `GoldVar("GoldenFyshGold", 75)`.
const GOLDEN_FYSH: i32 = 75;

/// `HealVar("ClamRollHeal", 10)`.
const CLAM_ROLL: i32 = 10;

/// `MaxHpVar("CaviarMaxHp", 4)`.
const CAVIAR: i32 = 4;

/// A dish on the belt, as `RollDish` builds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dish {
    Caviar,
    SpicySnappy,
    JellyLiver,
    FriedEel,
    SuspiciousCondiment,
    ClamRoll,
    GoldenFysh,
    SeapunkSalad,
}

impl Dish {
    /// Its weight in the roll.
    fn weight(self) -> f32 {
        match self {
            Dish::Caviar | Dish::ClamRoll => 6.0,
            Dish::SpicySnappy | Dish::JellyLiver | Dish::FriedEel | Dish::SuspiciousCondiment => {
                3.0
            }
            Dish::GoldenFysh => 1.0,
            Dish::SeapunkSalad => 0.0,
        }
    }

    /// Its option on the belt.
    fn option<V: crate::Build>(self) -> V::EventOption {
        match self {
            Dish::Caviar => V::EventOption::ENDLESS_CONVEYOR_ALL_CAVIAR,
            Dish::SpicySnappy => V::EventOption::ENDLESS_CONVEYOR_ALL_SPICY_SNAPPY,
            Dish::JellyLiver => V::EventOption::ENDLESS_CONVEYOR_ALL_JELLY_LIVER,
            Dish::FriedEel => V::EventOption::ENDLESS_CONVEYOR_ALL_FRIED_EEL,
            Dish::SuspiciousCondiment => V::EventOption::ENDLESS_CONVEYOR_ALL_SUSPICIOUS_CONDIMENT,
            Dish::ClamRoll => V::EventOption::ENDLESS_CONVEYOR_ALL_CLAM_ROLL,
            Dish::GoldenFysh => V::EventOption::ENDLESS_CONVEYOR_ALL_GOLDEN_FYSH,
            Dish::SeapunkSalad => V::EventOption::ENDLESS_CONVEYOR_ALL_SEAPUNK_SALAD,
        }
    }

    /// What eating it does.
    fn eaten<V: crate::Build>(self) -> HookEffects<V> {
        match self {
            Dish::Caviar => smallvec::smallvec![HookEffect::GainMaxHp(CAVIAR)],
            // `NextItem` over the deck's upgradable cards, off the event's
            // stream.
            Dish::SpicySnappy => smallvec::smallvec![HookEffect::UpgradeRandomDeckCard {
                stream: Stream::Event,
            }],
            Dish::JellyLiver => smallvec::smallvec![HookEffect::SelectFromDeck {
                act: DeckAct::Transform(Stream::Event),
                count: 1,
            }],
            // `ForNonCombatWithDefaultOdds` over the colorless pool, one card,
            // which goes straight into the deck.
            Dish::FriedEel => {
                smallvec::smallvec![HookEffect::AddRolledToDeck(CustomReward::ColorlessCards(1))]
            }
            Dish::SuspiciousCondiment => {
                smallvec::smallvec![HookEffect::OfferRewards(V::ENDLESS_CONVEYOR_CONDIMENT)]
            }
            Dish::ClamRoll => smallvec::smallvec![HookEffect::Heal(CLAM_ROLL)],
            Dish::GoldenFysh => smallvec::smallvec![HookEffect::GainGold(GOLDEN_FYSH)],
            Dish::SeapunkSalad => {
                smallvec::smallvec![HookEffect::AddToDeck(V::Card::FEEDING_FRENZY)]
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct State {
    /// `NumOfGrabs`, which counts each roll.
    grabs: u32,
    /// `_lastDishId`, which the next roll leaves out.
    last: Option<Dish>,
    /// `CurrentDish`.
    current: Dish,
    /// Whether the player took a dish, which turns the page from
    /// `ObserveChef` to `Leave`.
    grabbed: bool,
    /// Whether the dish is locked: the player's gold, short of its price as
    /// the page was built.
    locked: bool,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct EndlessConveyor;

impl EndlessConveyor {
    /// `RollDish`. Every fifth roll is the Seapunk Salad. Otherwise one
    /// `NextFloat` over the dishes' weights picks among those that apply,
    /// less the dish before.
    fn roll<V: crate::Build, R: sts2_core::rng::GameRng>(
        state: &mut State,
        run: &impl EventView<V>,
        rng: &mut R,
    ) {
        state.grabs += 1;
        if state.grabs.is_multiple_of(5) {
            state.last = Some(Dish::SeapunkSalad);
            state.current = Dish::SeapunkSalad;
            return;
        }
        let mut dishes = vec![
            Dish::Caviar,
            Dish::SpicySnappy,
            Dish::JellyLiver,
            Dish::FriedEel,
        ];
        if run.open_potion_slot() {
            dishes.push(Dish::SuspiciousCondiment);
        }
        if run.hp() != run.max_hp() {
            dishes.push(Dish::ClamRoll);
        }
        if state.grabs > 1 {
            dishes.push(Dish::GoldenFysh);
        }
        dishes.retain(|&dish| Some(dish) != state.last);
        let total: f32 = dishes.iter().map(|dish| dish.weight()).sum();
        let unit = Interval::try_new(0.0, 1.0).expect("the unit interval is ordered");
        let drawn = rng.next_f32_range(unit) * total;
        let mut reached = 0.0;
        for dish in dishes {
            reached += dish.weight();
            if drawn < reached {
                state.last = Some(dish);
                state.current = dish;
                return;
            }
        }
    }
}

impl<V: crate::Build> EventBehavior<V> for EndlessConveyor {
    type State = State;

    /// `IsAllowed`, a hundred and twenty gold, sits in the event table.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars` rolls the first dish.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let mut state = State {
            grabs: 0,
            last: None,
            current: Dish::Caviar,
            grabbed: false,
            locked: run.gold() < PRICE,
            chosen: None,
        };
        Self::roll(&mut state, run, rng);
        state
    }

    /// The dish, or `LOCKED` where the gold falls short, then `ObserveChef`
    /// before any grab and `Leave` after.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let dish = match state.locked {
            true => (V::EventOption::ENDLESS_CONVEYOR_ALL_LOCKED, true),
            false => (state.current.option::<V>(), false),
        };
        let other = match state.grabbed {
            false => V::EventOption::ENDLESS_CONVEYOR_OBSERVE_CHEF,
            true => V::EventOption::ENDLESS_CONVEYOR_GRAB_SOMETHING_OFF_THE_BELT_LEAVE,
        };
        crate::behaviour::events::page::<V>(
            V::Event::ENDLESS_CONVEYOR,
            &[dish, (other, false)],
            state.chosen,
        )
    }

    /// `GrabSomethingOffTheBelt`: the price unless it is the Golden Fysh, then
    /// the dish, then a new roll once that is done. `ObserveChef`: one
    /// upgradable deck card upgraded, drawn off the event's stream.
    /// `Leave`. Each of those two finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        if option != 0 {
            return match state.grabbed {
                false => smallvec::smallvec![
                    HookEffect::UpgradeRandomDeckCard {
                        stream: Stream::Event,
                    },
                    HookEffect::FinishEvent,
                ],
                true => smallvec::smallvec![HookEffect::FinishEvent],
            };
        }
        let mut effects = HookEffects::new();
        if state.current != Dish::GoldenFysh {
            effects.push(HookEffect::LoseGold(PRICE));
        }
        effects.extend(state.current.eaten::<V>());
        effects.push(HookEffect::ResumeEvent);
        effects
    }

    /// The roll after a dish is eaten, and the page after a grab.
    fn resume<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        Self::roll(state, run, rng);
        state.grabbed = true;
        state.locked = run.gold() < PRICE;
        state.chosen = None;
        HookEffects::new()
    }
}

/// `CONDIMENT`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// Suspicious Condiment's one `PotionReward` over the unlocked potions.
        const ENDLESS_CONVEYOR_CONDIMENT: &'static [sts2_core::CustomReward<$build>] =
            &[sts2_core::CustomReward::UnlockedPotion];
    };
}

pub(crate) use lists;
