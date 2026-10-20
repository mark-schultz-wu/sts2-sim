//! `Kifuda`: up to three cards enchanted with Adroit.

use crate::ids::EnchantmentIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `_adroitAmount`.
const ADROIT: u8 = 3;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for none to three,
/// confirmed by hand, then Adroit at three on each.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeckConfirmed {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::ADROIT,
                amount: ADROIT,
                kind: None,
            },
            min: 0,
            max: (V::Relic::KIFUDA_CARDS as usize),
        });
    }
}
