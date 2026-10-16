//! `Kusarigama`: damage to a random enemy for every third attack in a turn.

use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `CardsVar(3)`.
const CARDS: u8 = 3;

/// `DamageVar(6, ValueProp.Unpowered)`.
const DAMAGE: Number = 6;

/// `AttacksPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacks_played_this_turn: u8,
}

/// `AfterCardPlayed` counts each attack its owner plays, and every third
/// damages an enemy drawn off `CombatTargets`. `BeforeCombatStart` and the
/// end of its owner's turn start the count again.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, .. }
            if <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.attacks_played_this_turn += 1;
            if state.attacks_played_this_turn.is_multiple_of(CARDS) {
                asked.push(HookEffect::DamageRandomOpponent {
                    amount: DAMAGE,
                    unblockable: false,
                    from_owner: true,
                });
            }
        }
        EventHook::CombatStarting { .. }
        | EventHook::SideTurnEnd {
            side: CombatSide::Player,
            ..
        } => state.attacks_played_this_turn = 0,
        _ => {}
    }
}

/// `DisplayAmount`: the attacks counted towards the next hit.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.attacks_played_this_turn % CARDS)
}
