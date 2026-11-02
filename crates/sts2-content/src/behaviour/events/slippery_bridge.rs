//! Slippery Bridge: lose a card the bridge names, or hold on and lose more
//! hit points each time, with a new card named.

use nonmax::NonMaxU8;
use sts2_core::rng::GameRng;
use sts2_core::{CardIdent, CardTable as _, EventBehavior, EventView};
use sts2_core::{HookEffect, HookEffects, Room};

use crate::generated::cards::CardRarity;
use crate::ids::{EventIds as _, EventOptionIds as _};

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
fn named<V: crate::Build, R: GameRng>(
    deck: &[CardIdent<V>],
    state: Option<&State>,
    rng: &mut R,
) -> usize {
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

impl<V: crate::Build> EventBehavior<V> for SlipperyBridge {
    type State = State;

    /// Past floor six, with a card that can be removed, which the table of
    /// allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        State {
            holds: 0,
            losing: named::<V, _>(&run.deck(), None, rng),
            skipped: Vec::new(),
            chosen: None,
        }
    }

    /// `OVERCOME` keeps the first page's key on every page. Each Hold On key
    /// names the page it stands on and the hold it makes.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let hold = match state.holds {
            0 => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON0,
            1 => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON0_HOLD_ON1,
            2 => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON1_HOLD_ON2,
            3 => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON2_HOLD_ON3,
            4 => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON3_HOLD_ON4,
            5 => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON4_HOLD_ON5,
            6 => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON5_HOLD_ON6,
            NAMED_HOLDS => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON6_HOLD_ON_LOOP,
            _ => V::EventOption::SLIPPERY_BRIDGE_HOLD_ON_LOOP_HOLD_ON_LOOP,
        };
        crate::behaviour::events::page::<V>(
            V::Event::SLIPPERY_BRIDGE,
            &[
                (V::EventOption::SLIPPERY_BRIDGE_OVERCOME, false),
                (hold, false),
            ],
            state.chosen,
        )
    }

    /// `WillKillPlayer`, for `HoldOn`: `CurrentHpLoss`, three and one more for each hold before.
    fn kills(&self, state: &State, option: u8, run: &impl EventView<V>) -> bool {
        option == 1 && run.hp() <= HP_LOSS + i32::from(state.holds)
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
                state.losing = named::<V, _>(&run.deck(), Some(state), rng);
                smallvec::smallvec![HookEffect::DamageOwner(loss)]
            }
        }
    }
}
