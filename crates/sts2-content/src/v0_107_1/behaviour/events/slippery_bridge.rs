//! Slippery Bridge: lose a card the bridge names, or hold on and lose more
//! hit points each time, with a new card named.

use nonmax::NonMaxU8;
use sts2_core::rng::GameRng;
use sts2_core::{CardIdent, CardTable as _, EventBehavior, EventView};
use sts2_core::{HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardRarity;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `_initialHpLoss`: what the first hold costs, and each after it one more.
const HP_LOSS: i32 = 3;

/// The holds whose Hold On keys name their own page. From here on the page
/// and the option are both `LOOP`.
const NAMED_HOLDS: u8 = 7;

#[derive(Debug, Clone)]
pub struct State {
    /// `NumberOfHoldOns`.
    holds: u8,
    /// `RandomCardToLose`, by its place in the deck, which the event does not
    /// change until it removes it.
    losing: usize,
    /// `SkippedRemovals`: the cards named before, by place.
    skipped: Vec<usize>,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct SlipperyBridge;

/// `GetNewRandomCard`: `NextItem` over the deck's cards that can be removed,
/// skipping those named before. The first time it takes cards that are not
/// Basic. After that it takes cards of another kind than the one named last.
/// With none left it takes any card that can be removed.
fn named<R: GameRng>(deck: &[CardIdent<V>], state: Option<&State>, rng: &mut R) -> usize {
    let removable = |card: &CardIdent<V>| !crate::behaviour::enchantments::card_eternal(card);
    let mut places: Vec<usize> = deck
        .iter()
        .enumerate()
        .filter(|&(at, card)| {
            let offered = match state {
                None => card.card.rarity() != CardRarity::Basic,
                Some(state) => card.card != deck[state.losing].card,
            };
            offered && removable(card) && state.is_none_or(|state| !state.skipped.contains(&at))
        })
        .map(|(at, _)| at)
        .collect();
    if places.is_empty() {
        places = (0..deck.len()).filter(|&at| removable(&deck[at])).collect();
    }
    *rng.choose(&places)
        .unwrap_or_else(|_| unreachable!("the event is allowed only with a card to remove"))
}

impl EventBehavior<V> for SlipperyBridge {
    type State = State;

    /// Past floor six, with a card that can be removed, which the table of
    /// allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        State {
            holds: 0,
            losing: named(&run.deck(), None, rng),
            skipped: Vec::new(),
            chosen: None,
        }
    }

    /// `OVERCOME` keeps the first page's key on every page. Each Hold On key
    /// names the page it stands on and the hold it makes.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let hold = match state.holds {
            0 => Said::SlipperyBridgeHoldOn0,
            1 => Said::SlipperyBridgeHoldOn0HoldOn1,
            2 => Said::SlipperyBridgeHoldOn1HoldOn2,
            3 => Said::SlipperyBridgeHoldOn2HoldOn3,
            4 => Said::SlipperyBridgeHoldOn3HoldOn4,
            5 => Said::SlipperyBridgeHoldOn4HoldOn5,
            6 => Said::SlipperyBridgeHoldOn5HoldOn6,
            NAMED_HOLDS => Said::SlipperyBridgeHoldOn6HoldOnLoop,
            _ => Said::SlipperyBridgeHoldOnLoopHoldOnLoop,
        };
        super::page(
            Event::SlipperyBridge,
            &[(Said::SlipperyBridgeOvercome, false), (hold, false)],
            state.chosen,
        )
    }

    /// `Overcome`: the named card leaves the deck, which finishes the event.
    /// `HoldOn`: its hit points, unblockable, then a new card named.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        match option {
            0 => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![
                    HookEffect::RemoveDeckCardAt(state.losing),
                    HookEffect::FinishEvent,
                ]
            }
            _ => {
                let loss = HP_LOSS + i32::from(state.holds);
                state.holds = state.holds.saturating_add(1);
                state.skipped.push(state.losing);
                state.losing = named(&run.deck(), Some(state), rng);
                smallvec::smallvec![HookEffect::DamageOwner(loss)]
            }
        }
    }
}
