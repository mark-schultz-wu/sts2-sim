//! Brain Leech: one of five cards, or a colourless card reward for HP.

use nonmax::NonMaxU8;
use sts2_core::{CardDraw, CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `DamageVar("RipHpLoss", 5)`, unblockable and unpowered.
const RIP_HP_LOSS: i32 = 5;

/// `IntVar("FromCardChoiceCount", 5)`.
const CARDS: usize = 5;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct BrainLeech;

impl EventBehavior<V> for BrainLeech {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.act() < 2
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::BrainLeech,
            &[
                (Said::BrainLeechShareKnowledge, false),
                (Said::BrainLeechRip, false),
            ],
            state.chosen,
        )
    }

    /// `ShareKnowledge`: five cards of the character's pool at regular odds,
    /// and one of them into the deck. `Rip`: the damage, then a card reward
    /// of three colourless cards. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::ChooseCards {
                    draw: CardDraw::Regular,
                    count: CARDS,
                    min: 1,
                    max: 1,
                    card_reward: false,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::DamageOwner(RIP_HP_LOSS),
                HookEffect::OfferRewards(&[CustomReward::ColorlessCards(3)]),
                HookEffect::FinishEvent,
            ],
        }
    }
}
