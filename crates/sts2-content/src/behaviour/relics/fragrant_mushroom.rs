//! `FragrantMushroom`: a cut, and two cards upgraded, as it is picked up.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::RunRngType;

/// `HpLossVar(15)`, dealt unblockable and unpowered.
const DAMAGE: Number = 15;

/// `CardsVar(2)`.
const CARDS: usize = 2;

/// `AfterObtained`: `CreatureCmd.Damage` on its owner, then the deck's
/// upgradable cards stably shuffled off `RunState.Rng.Niche`, and the first
/// two upgraded.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::DamageOwner(DAMAGE),
            HookEffect::UpgradeRandomDeckCards {
                count: CARDS,
                stream: sts2_core::Stream::Run(RunRngType::Niche),
                kind: None,
            },
        ]);
    }
}
