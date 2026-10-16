//! `NutritiousSoup`: the deck's Basic Strikes enchanted with Tezcatara's
//! Ember as it is picked up.

use sts2_core::{EventHook, HookEffect};

use crate::generated::cards::CardTag;
use crate::ids::EnchantmentIds as _;

/// `AfterObtained`: `CardCmd.Enchant<TezcatarasEmber>` at one on each deck
/// card of Basic rarity tagged Strike that the enchantment takes.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::EnchantBasicTagged {
            tag: CardTag::Strike,
            enchantment: V::Enchantment::TEZCATARAS_EMBER,
            amount: 1,
        });
    }
}
