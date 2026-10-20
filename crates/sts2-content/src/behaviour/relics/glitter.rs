//! `Glitter`: card rewards enchanted with Glam.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::enchantments::can_enchant;
use crate::ids::EnchantmentIds as _;

/// `TryModifyCardRewardOptionsLate`: each reward card Glam can enchant is
/// replaced by a clone enchanted with it at one. A run here has one player.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardOption { card, .. } => {
            if can_enchant(V::Enchantment::GLAM, &card) {
                asked.push(HookEffect::EnchantCard {
                    enchantment: V::Enchantment::GLAM,
                    amount: 1,
                });
            }
        }
        EventHook::Obtained { .. } | EventHook::CardRewardPick { .. } => {}
        hook => unimplemented!("what Glitter does for {hook:?}"),
    }
}
