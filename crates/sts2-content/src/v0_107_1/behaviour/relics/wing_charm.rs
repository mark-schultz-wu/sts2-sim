//! `WingCharm`: one card of each set made for a reward enchanted with Swift.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `DynamicVar("SwiftAmount", 1)`.
const SWIFT: u8 = 1;

/// `TryModifyCardRewardOptionsLate` over the whole set of cards made for a
/// reward, a `CardReward`'s or not. One card Swift can enchant, `NextItem`
/// off `Rng.Niche`, is replaced by a clone enchanted with it. A run here
/// has one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardRewardPick { .. } => {
            asked.push(HookEffect::EnchantOneRewardCard {
                enchantment: Enchantment::Swift,
                amount: SWIFT,
            });
        }
        EventHook::CardRewardOption { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what WingCharm does for {hook:?}"),
    }
}
