//! `SparklingRouge`: Strength and Dexterity as the third turn begins.

use crate::ids::PowerIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `AfterBlockCleared`: Strength then Dexterity on its owner once its block
/// clears on the third turn.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::BlockCleared {
        creature,
        player,
        turn,
    } = hook
        && creature == player
        && turn == 3
    {
        asked.extend([
            HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Relic::SPARKLING_ROUGE_STRENGTH_POWER),
            ),
            HookEffect::ApplyToOwner(
                V::Power::DEXTERITY_POWER,
                PowerAmount::Counted(V::Relic::SPARKLING_ROUGE_STRENGTH_POWER),
            ),
        ]);
    }
}
