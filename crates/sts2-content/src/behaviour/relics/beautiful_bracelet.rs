//! `BeautifulBracelet`: three cards enchanted with Swift.

use crate::ids::EnchantmentIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for three, each
/// then `CardCmd.Enchant<Swift>` by three.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::SWIFT,
                amount: 3,
                kind: None,
            },
            count: (V::Relic::BEAUTIFUL_BRACELET_CARDS as usize),
        });
    }
}
