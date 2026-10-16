//! `FestivePopper`: damage to every enemy on the first turn.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `DamageVar(9, Unpowered)`.
const DAMAGE: Number = 9;

/// `AfterPlayerTurnStart`: its damage to every hittable enemy on its owner's
/// first turn. A fight here has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::PlayerTurnStart { turn, .. } = hook
        && turn == 1
    {
        asked.push(HookEffect::DamageOpponents(DAMAGE));
    }
}
