//! `ElectricShrymp`: a deck card made Imbued as it is picked up.

use crate::ids::EnchantmentIds as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `FromDeckForEnchantment` for one card, then
/// `CardCmd.Enchant<Imbued>` at one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::IMBUED,
                amount: 1,
                kind: None,
            },
            count: 1,
        }),
        hook => unimplemented!("what ElectricShrymp does for {hook:?}"),
    }
}
