//! `WingCharm`: one card of each set made for a reward enchanted with Swift.

use crate::ids::EnchantmentIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `TryModifyCardRewardOptionsLate` over the whole set of cards made for a
/// reward, a `CardReward`'s or not. One card Swift can enchant, `NextItem`
/// off `Rng.Niche`, is replaced by a clone enchanted with it. A run here
/// has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardPick { .. } => {
            asked.push(HookEffect::EnchantOneRewardCard {
                enchantment: V::Enchantment::SWIFT,
                amount: (V::Relic::WING_CHARM_SWIFT_AMOUNT as u8),
            });
        }
        EventHook::CardRewardOption { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what WingCharm does for {hook:?}"),
    }
}
