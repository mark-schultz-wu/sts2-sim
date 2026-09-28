//! `BookRepairKnife`: a heal for each enemy Doom kills.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `HealVar(3)`.
const HEAL: Number = 3;

/// `AfterDiedToDoom`: `CreatureCmd.Heal` on its owner, its heal for each
/// creature besides its owner whose death triggers Fatal, where there is
/// one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::DiedToDoom { fatal } = hook
        && fatal != 0
    {
        let fatal = Number::try_from(fatal).expect("a count of creatures fits the game's numbers");
        asked.push(HookEffect::Heal(HEAL * fatal));
    }
}
