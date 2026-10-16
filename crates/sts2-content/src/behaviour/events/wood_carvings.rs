//! Wood Carvings: a basic card carved into Peck or Toric Toughness, or
//! enchanted with Slither.

use nonmax::NonMaxU8;
use sts2_core::{CardTable as _, DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use crate::behaviour::enchantments::can_enchant;
use crate::generated::cards::CardRarity;
use crate::ids::{CardIds as _, EnchantmentIds as _, EventIds as _, EventOptionIds as _};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct WoodCarvings;

impl<V: crate::Build> EventBehavior<V> for WoodCarvings {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.deck().iter().any(|card| {
            card.card.rarity() == CardRarity::Basic
                && !crate::behaviour::enchantments::card_eternal(card)
        })
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// The snake is locked for a deck with no card Slither takes.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        let slither = run
            .deck()
            .iter()
            .any(|card| can_enchant(V::Enchantment::SLITHER, card));
        crate::behaviour::events::page::<V>(
            V::Event::WOOD_CARVINGS,
            &[
                (V::EventOption::WOOD_CARVINGS_BIRD, false),
                (V::EventOption::WOOD_CARVINGS_SNAKE, !slither),
                (V::EventOption::WOOD_CARVINGS_TORUS, false),
            ],
            state.chosen,
        )
    }

    /// `Bird` and `Torus`: `FromDeckGeneric` for one basic card, then
    /// `CardCmd.TransformTo` Peck or Toric Toughness. `Snake`:
    /// `FromDeckForEnchantment` for one card, then Slither. Each finishes the
    /// event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let act = match option {
            0 => DeckAct::TransformBasicInto(V::Card::PECK),
            1 => DeckAct::Enchant {
                enchantment: V::Enchantment::SLITHER,
                amount: 1,
                kind: None,
            },
            _ => DeckAct::TransformBasicInto(V::Card::TORIC_TOUGHNESS),
        };
        smallvec::smallvec![
            HookEffect::SelectFromDeck { act, count: 1 },
            HookEffect::FinishEvent,
        ]
    }
}
