//! `FragrantMushroom`: a cut, and two cards upgraded, as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::RunRngType;

/// `AfterObtained`: `CreatureCmd.Damage` on its owner, then the deck's
/// upgradable cards stably shuffled off `RunState.Rng.Niche`, and the first
/// two upgraded.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::DamageOwner(V::Relic::FRAGRANT_MUSHROOM_HP_LOSS),
            HookEffect::UpgradeRandomDeckCards {
                count: (V::Relic::FRAGRANT_MUSHROOM_CARDS as usize),
                stream: sts2_core::Stream::Run(RunRngType::Niche),
                kind: None,
            },
        ]);
    }
}
