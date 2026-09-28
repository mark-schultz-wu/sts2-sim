//! Grave of the Forgotten: Decay and a card's Exhaust taken off, or a relic.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, while what it asked is open.
    chosen: Option<NonMaxU8>,
}

pub struct GraveOfTheForgotten;

/// `HasEnchantableCards`: a deck card Souls Power takes.
fn enchantable(run: &impl EventView<V>) -> bool {
    super::enchantable(run, Enchantment::SoulsPower)
}

impl EventBehavior<V> for GraveOfTheForgotten {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        enchantable(run)
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// `Confront` locks with no card to enchant.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::GraveOfTheForgotten,
            &[
                (Said::GraveOfTheForgottenConfront, !enchantable(run)),
                (Said::GraveOfTheForgottenAccept, false),
            ],
            state.chosen,
        )
    }

    /// `Confront`: Decay, then `FromDeckForEnchantment` for one card, which
    /// Souls Power enchants by one. `Accept`: Forgotten Soul. Each finishes
    /// the event.
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
                HookEffect::AddToDeck(Cards::Curse(CurseCard::Decay)),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Enchant {
                        enchantment: Enchantment::SoulsPower,
                        amount: 1,
                        kind: None,
                    },
                    count: 1,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::ObtainRelic(Relic::ForgottenSoul),
                HookEffect::FinishEvent,
            ],
        }
    }
}
