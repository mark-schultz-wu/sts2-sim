//! `TriBoomerang`: three attacks enchanted with Instinct.

use crate::ids::EnchantmentIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for three, each
/// then `CardCmd.Enchant<Instinct>` by one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::INSTINCT,
                amount: 1,
                kind: None,
            },
            count: (V::Relic::TRI_BOOMERANG_CARDS as usize),
        });
    }
}
