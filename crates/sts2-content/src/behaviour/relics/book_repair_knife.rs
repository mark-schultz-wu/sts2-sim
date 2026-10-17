//! `BookRepairKnife`: a heal for each enemy Doom kills.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `AfterDiedToDoom`: `CreatureCmd.Heal` on its owner, its heal for each
/// creature besides its owner whose death triggers Fatal, where there is
/// one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::DiedToDoom { fatal } = hook
        && fatal != 0
    {
        let fatal = Number::try_from(fatal).expect("a count of creatures fits the game's numbers");
        asked.push(HookEffect::Heal(V::Relic::BOOK_REPAIR_KNIFE_HEAL * fatal));
    }
}
