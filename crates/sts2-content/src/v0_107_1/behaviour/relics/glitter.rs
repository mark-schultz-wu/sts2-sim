//! `Glitter`: card rewards enchanted with Glam.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::enchantments::can_enchant;

/// `TryModifyCardRewardOptionsLate`: each reward card Glam can enchant is
/// replaced by a clone enchanted with it at one. A run here has one player.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardRewardOption { card, .. } => {
            if can_enchant(Enchantment::Glam, &card) {
                asked.push(HookEffect::EnchantCard {
                    enchantment: Enchantment::Glam,
                    amount: 1,
                });
            }
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what Glitter does for {hook:?}"),
    }
}
