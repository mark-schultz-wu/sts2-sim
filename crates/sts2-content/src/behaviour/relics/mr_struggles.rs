//! `MrStruggles`: damage to every enemy as each of its owner's turns starts,
//! as much as the turn's number.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `AfterPlayerTurnStart`: `CreatureCmd.Damage` on every enemy the player can
/// hit, unpowered, for its owner's `TurnNumber`, on its owner's turn.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::PlayerTurnStart { player, .. } = hook
        && player != owner
    {
        return;
    }
    if let EventHook::PlayerTurnStart { turn, .. } = hook {
        let damage = Number::try_from(turn).expect("a turn number fits the game's numbers");
        asked.push(HookEffect::DamageOpponents(damage));
    }
}
