//! `GnarledHammer`: up to three cards enchanted with Sharp.

use crate::ids::EnchantmentIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for none to three,
/// confirmed by hand, then Sharp at three on each.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeckConfirmed {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::SHARP,
                amount: (V::Relic::GNARLED_HAMMER_SHARP_AMOUNT as u8),
                kind: None,
            },
            min: 0,
            max: (V::Relic::GNARLED_HAMMER_CARDS as usize),
        });
    }
}
