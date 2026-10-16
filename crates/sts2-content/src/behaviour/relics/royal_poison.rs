//! `RoyalPoison`: damage as the first turn of a fight starts.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `DamageVar(4, Unblockable | Unpowered)`.
const DAMAGE: Number = 4;

/// `AfterPlayerTurnStart` on its owner's first turn: its damage to its
/// owner, from nobody.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::PlayerTurnStart { turn, .. } if turn <= 1 => {
            asked.push(HookEffect::DamageOwnerInFight {
                amount: DAMAGE,
                unblockable: true,
                self_dealt: false,
            })
        }
        EventHook::PlayerTurnStart { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what RoyalPoison does for {hook:?}"),
    }
}
