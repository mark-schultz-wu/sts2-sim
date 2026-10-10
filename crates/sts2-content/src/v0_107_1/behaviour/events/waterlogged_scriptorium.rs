//! Waterlogged Scriptorium: Max HP, or gold for Steady on deck cards.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `MaxHpVar(6)`.
const MAX_HP: i32 = 6;

/// `GoldVar(55)`, the Tentacle Quill's price.
const QUILL: i32 = 55;

/// `GoldVar("PricklySpongeGold", 99)`.
const SPONGE: i32 = 99;

/// `CardsVar(2)`, which the Prickly Sponge enchants.
const SPONGE_CARDS: usize = 2;

#[derive(Debug, Clone)]
pub struct State {
    /// Whether the quill and the sponge are locked. `GenerateInitialOptions`
    /// reads the gold once, as the event opens, so a purchase leaves the page
    /// as it was.
    quill_locked: bool,
    sponge_locked: bool,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct WaterloggedScriptorium;

impl EventBehavior<V> for WaterloggedScriptorium {
    type State = State;

    /// `IsAllowed`, gold enough for the quill, sits in the event table.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// The quill and the sponge each lock where the player's gold is short of
    /// its price.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            quill_locked: run.gold() < QUILL,
            sponge_locked: run.gold() < SPONGE,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::WaterloggedScriptorium,
            &[
                (Said::WaterloggedScriptoriumBloodyInk, false),
                (
                    Said::WaterloggedScriptoriumTentacleQuill,
                    state.quill_locked,
                ),
                (
                    Said::WaterloggedScriptoriumPricklySponge,
                    state.sponge_locked,
                ),
            ],
            state.chosen,
        )
    }

    /// `BloodyInk`: Max HP. `TentacleQuill` and `PricklySponge`: their gold
    /// spent, then Steady on one deck card or on two, which the player picks
    /// among those it can enchant. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let steady = |count| HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: Enchantment::Steady,
                amount: 1,
                kind: None,
            },
            count,
        };
        match option {
            0 => smallvec::smallvec![HookEffect::GainMaxHp(MAX_HP), HookEffect::FinishEvent],
            1 => smallvec::smallvec![
                HookEffect::LoseGold(QUILL),
                steady(1),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::LoseGold(SPONGE),
                steady(SPONGE_CARDS),
                HookEffect::FinishEvent,
            ],
        }
    }
}
