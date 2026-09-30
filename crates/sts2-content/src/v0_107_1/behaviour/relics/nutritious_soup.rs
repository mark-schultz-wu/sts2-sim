//! `NutritiousSoup`: the deck's Basic Strikes enchanted with Tezcatara's
//! Ember as it is picked up.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardTag;
use super::super::super::generated::enchantments::Enchantment;

/// `AfterObtained`: `CardCmd.Enchant<TezcatarasEmber>` at one on each deck
/// card of Basic rarity tagged Strike that the enchantment takes.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::EnchantBasicTagged {
            tag: CardTag::Strike,
            enchantment: Enchantment::TezcatarasEmber,
            amount: 1,
        });
    }
}
