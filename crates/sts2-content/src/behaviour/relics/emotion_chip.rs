//! `EmotionChip`: every orb's passive at the start of a turn after its owner
//! lost health.

use sts2_core::{EventHook, HookEffect};

/// The player's `TurnNumber`.
type Turn = u32;

/// `LostHpInPreviousTurn`, which reads the history's `DamageReceivedEntry`s
/// that were not fully blocked. The last two player turns such a hit landed
/// in, latest first, which is all a turn's start asks about.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    hurt: [Option<Turn>; 2],
}

/// `AfterPlayerTurnStart`: `OrbCmd.Passive` on each orb, where its owner
/// lost health in its last turn, or in the enemies' turn after it.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::PlayerTurnStart { player, .. } = hook
        && player != owner
    {
        return;
    }
    match hook {
        // The history it reads is the fight's own, which the next fight
        // starts empty.
        EventHook::CombatEnd { .. } => *state = State::default(),
        EventHook::DamageReceived {
            target,
            fully_blocked,
            turn,
            in_progress: true,
            ..
        } if target == owner && !fully_blocked && state.hurt[0] != Some(turn) => {
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
