//! Grave of the Forgotten: Decay and a card's Exhaust taken off, or a relic.

use crate::ids::{
    CardIds as _, EnchantmentIds as _, EventIds as _, EventOptionIds as _, RelicIds as _,
};
use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, while what it asked is open.
    chosen: Option<NonMaxU8>,
}

pub struct GraveOfTheForgotten;

/// `HasEnchantableCards`: a deck card Souls Power takes.
fn enchantable<V: crate::Build>(run: &impl EventView<V>) -> bool {
    crate::behaviour::events::enchantable::<V>(run, V::Enchantment::SOULS_POWER)
}

impl<V: crate::Build> EventBehavior<V> for GraveOfTheForgotten {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        enchantable::<V>(run)
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// `Confront` locks with no card to enchant.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::GRAVE_OF_THE_FORGOTTEN,
            &[
                (
                    V::EventOption::GRAVE_OF_THE_FORGOTTEN_CONFRONT,
                    !enchantable::<V>(run),
                ),
                (V::EventOption::GRAVE_OF_THE_FORGOTTEN_ACCEPT, false),
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
                HookEffect::AddToDeck(V::Card::DECAY),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Enchant {
                        enchantment: V::Enchantment::SOULS_POWER,
                        amount: 1,
                        kind: None,
                    },
                    count: 1,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::ObtainRelic(V::Relic::FORGOTTEN_SOUL),
                HookEffect::FinishEvent,
            ],
        }
    }
}
