//! `SparklingRouge`: Strength and Dexterity as the third turn begins.

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;

/// `PowerVar<StrengthPower>(1)` and `PowerVar<DexterityPower>(1)`.
const ONE: PowerAmount<super::Number> = PowerAmount::Counted(1);

/// `AfterBlockCleared`: Strength then Dexterity on its owner once its block
/// clears on the third turn.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::BlockCleared {
        creature,
        player,
        turn,
    } = hook
        && creature == player
        && turn == 3
    {
        asked.extend([
            HookEffect::ApplyToOwner(Power::StrengthPower, ONE),
            HookEffect::ApplyToOwner(Power::DexterityPower, ONE),
        ]);
    }
}
