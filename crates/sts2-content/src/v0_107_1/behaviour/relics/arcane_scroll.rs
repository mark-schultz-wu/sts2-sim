//! `ArcaneScroll`: a random rare card into the deck as it is picked up.

use sts2_core::{EventHook, HookEffect, RewardRarity};

use super::super::super::V0_107_1 as V;

/// `AfterObtained`: `CardFactory.CreateForReward` for one rare from the
/// character's pool, at `Uniform` odds with `NoUpgradeRoll`, into the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.push(HookEffect::AddUniformCard {
            rarity: RewardRarity::Rare,
        });
    }
}
