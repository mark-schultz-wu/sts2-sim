//! Stone of All Time: a potion for Max HP, or HP for Vigorous on an attack.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::enchantments::can_enchant;
use super::ancient::next_item;

/// `DynamicVar("DrinkMaxHpGain", 10)`.
const MAX_HP: i32 = 10;

/// `DynamicVar("PushHpLoss", 6)`.
const HP_LOSS: i32 = 6;

/// `DynamicVar("PushVigorousAmount", 8)`.
const VIGOROUS: u8 = 8;

/// The `Rng.NextInt(100)` each option draws once it is done, for the line it
/// shows.
const FLAVOUR: u32 = 100;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// `DrinkAndLiftPotion`, by its place among the potions held.
    potion: Option<usize>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct StoneOfAllTime;

impl EventBehavior<V> for StoneOfAllTime {
    type State = State;

    /// The second act and a potion, which the shared eligibility check
    /// answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `GenerateInitialOptions`: a potion held, with `NextItem` off the
    /// event's stream.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let held: Vec<usize> = (0..run.potions().count()).collect();
        State {
            potion: next_item(rng, &held),
            chosen: None,
        }
    }

    /// `Lift` is locked with no potion, and `Push` with no card Vigorous
    /// takes.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        let vigorous = run
            .deck()
            .iter()
            .any(|card| can_enchant(Enchantment::Vigorous, card));
        super::page(
            Event::StoneOfAllTime,
            &[
                (Said::StoneOfAllTimeLift, state.potion.is_none()),
                (Said::StoneOfAllTimePush, !vigorous),
            ],
            state.chosen,
        )
    }

    /// `Lift`: the potion discarded, then the Max HP. `Push`: the damage,
    /// then `FromDeckForEnchantment` for one card, then Vigorous at eight.
    /// Each then draws its line and finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let flavour = HookEffect::DrawFromStream {
            stream: Stream::Event,
            below: FLAVOUR,
        };
        match option {
            0 => {
                let Some(potion) = state.potion else {
                    unimplemented!("lifting the stone with no potion");
                };
                smallvec::smallvec![
                    HookEffect::DiscardPotion(potion),
                    HookEffect::GainMaxHp(MAX_HP),
                    flavour,
                    HookEffect::FinishEvent,
                ]
            }
            _ => smallvec::smallvec![
                HookEffect::DamageOwner(HP_LOSS),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Enchant {
                        enchantment: Enchantment::Vigorous,
                        amount: VIGOROUS,
                        kind: None,
                    },
                    count: 1,
                },
                flavour,
                HookEffect::FinishEvent,
            ],
        }
    }
}
