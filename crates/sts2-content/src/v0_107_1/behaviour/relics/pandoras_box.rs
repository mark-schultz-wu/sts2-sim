//! `PandorasBox`: every basic Strike and Defend transformed as it is picked
//! up.

use sts2_core::{EventHook, HookEffect, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::RunRngType;

/// `AfterObtained`: `CardCmd.Transform` on each removable deck card that
/// `IsBasicStrikeOrDefend`, off `RunState.Rng.Niche`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.push(HookEffect::TransformBasicStrikesAndDefends(Stream::Run(
            RunRngType::Niche,
        )));
    }
}
