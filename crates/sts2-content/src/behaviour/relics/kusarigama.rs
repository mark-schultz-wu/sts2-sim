//! `Kusarigama`: damage to a random enemy for every third attack in a turn.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `AttacksPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacks_played_this_turn: u8,
}

/// `AfterCardPlayed` counts each attack its owner plays, and every third
/// damages an enemy drawn off `CombatTargets`. `BeforeCombatStart` and the
/// end of its owner's turn start the count again.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, player, .. }
            if player == owner
                && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.attacks_played_this_turn += 1;
            if state
                .attacks_played_this_turn
                .is_multiple_of(V::Relic::KUSARIGAMA_CARDS as u8)
            {
                asked.push(HookEffect::DamageRandomOpponent {
                    amount: V::Relic::KUSARIGAMA_DAMAGE,
                    unblockable: false,
                    from_owner: true,
                });
            }
        }
        EventHook::CombatStarting { .. } => state.attacks_played_this_turn = 0,
        EventHook::SideTurnEnd { participants, .. } if participants.contains(owner) => {
            state.attacks_played_this_turn = 0
        }
        _ => {}
    }
}

/// `DisplayAmount`: the attacks counted towards the next hit.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::from(state.attacks_played_this_turn % (V::Relic::KUSARIGAMA_CARDS as u8))
}
