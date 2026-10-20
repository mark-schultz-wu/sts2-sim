//! `PunchDagger`: one card enchanted with Momentum.

use crate::ids::EnchantmentIds as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `Momentum`'s `DynamicVar(5)`.
const MOMENTUM: u8 = 5;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for one card,
/// then Momentum at five on it.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::MOMENTUM,
                amount: MOMENTUM,
                kind: None,
            },
            count: 1,
        });
    }
}
