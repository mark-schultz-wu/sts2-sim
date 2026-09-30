//! `WingCharm`: one card of each card reward enchanted with Swift.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `DynamicVar("SwiftAmount", 1)`.
const SWIFT: u8 = 1;

/// `TryModifyCardRewardOptionsLate` over the whole reward: one card Swift can
/// enchant, `NextItem` off `Rng.Niche`, replaced by a clone enchanted with
/// it. A run here has one player.
///
/// # Panics
///
/// A card made for another use than a `CardReward`, which nothing here has
/// shown the game asks it about.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardRewardPick { card_reward: true } => {
            asked.push(HookEffect::EnchantOneRewardCard {
                enchantment: Enchantment::Swift,
                amount: SWIFT,
            });
        }
        EventHook::CardRewardOption {
            card_reward: true, ..
        }
        | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what WingCharm does for {hook:?}"),
    }
}
