//! `PandorasBox`: every basic Strike and Defend transformed as it is picked
//! up.

use sts2_core::{EventHook, HookEffect, Stream};

use crate::generated::runstate::RunRngType;

/// `AfterObtained`: `CardCmd.Transform` on each removable deck card that
/// `IsBasicStrikeOrDefend`, off `RunState.Rng.Niche`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::TransformBasicStrikesAndDefends(Stream::Run(
            RunRngType::Niche,
        )));
    }
}
