//! Sapphire Seed: a heal and an upgrade, or a card sown.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `HealVar(9)`.
const HEAL: i32 = 9;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct SapphireSeed;

impl EventBehavior<V> for SapphireSeed {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::SapphireSeed,
            &[
                (Said::SapphireSeedEat, false),
                (Said::SapphireSeedPlant, false),
            ],
            state.chosen,
        )
    }

    /// `Eat`: `CreatureCmd.Heal`, then `FromDeckForUpgrade` for one card.
    /// `Plant`: `FromDeckForEnchantment` for one card, then Sown. Each
    /// finishes the event.
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
                HookEffect::Heal(HEAL),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Upgrade,
                    count: 1,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Enchant(Enchantment::Sown),
                    count: 1,
                },
                HookEffect::FinishEvent,
            ],
        }
    }
}
