//! `EmotionChip`: every orb's passive at the start of a turn after its owner
//! lost health.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// The player's `TurnNumber`.
type Turn = <V as sts2_core::ContentVersion>::Count;

/// `LostHpInPreviousTurn`, which reads the history's `DamageReceivedEntry`s
/// that were not fully blocked. The last two player turns such a hit landed
/// in, latest first, which is all a turn's start asks about.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    hurt: [Option<Turn>; 2],
}

/// `AfterPlayerTurnStart`: `OrbCmd.Passive` on each orb, where its owner
/// lost health in its last turn, or in the enemies' turn after it.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        // The history it reads is the fight's own, which the next fight
        // starts empty.
        EventHook::CombatEnd { .. } => *state = State::default(),
        EventHook::DamageReceived {
            target,
            player,
            fully_blocked,
            turn,
            ..
        } if target == player && !fully_blocked && state.hurt[0] != Some(turn) => {
            state.hurt = [Some(turn), state.hurt[0]];
        }
        EventHook::PlayerTurnStart { turn, .. }
            if turn
                .checked_sub(1)
                .is_some_and(|last| state.hurt.contains(&Some(last))) =>
        {
            asked.push(HookEffect::EachOrbPassive);
        }
        _ => {}
    }
}
