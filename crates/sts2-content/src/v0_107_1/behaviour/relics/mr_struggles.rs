//! `MrStruggles`: damage to every enemy as each of its owner's turns starts,
//! as much as the turn's number.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `AfterPlayerTurnStart`: `CreatureCmd.Damage` on every enemy the player can
/// hit, unpowered, for its owner's `TurnNumber`. A relic's owner is the
/// player, and a fight here has one, so the player starting a turn is always
/// its own.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PlayerTurnStart { turn, .. } = hook {
        let damage = Number::try_from(turn).expect("a turn number fits the game's numbers");
        asked.push(HookEffect::DamageOpponents(damage));
    }
}
