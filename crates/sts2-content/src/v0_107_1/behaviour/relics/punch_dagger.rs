//! `PunchDagger`: one card enchanted with Momentum.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `Momentum`'s `DynamicVar(5)`.
const MOMENTUM: u8 = 5;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for one card,
/// then Momentum at five on it.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: Enchantment::Momentum,
                amount: MOMENTUM,
                kind: None,
            },
            count: 1,
        });
    }
}
