//! `ElectricShrymp`: a deck card made Imbued as it is picked up.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `AfterObtained`: `FromDeckForEnchantment` for one card, then
/// `CardCmd.Enchant<Imbued>` at one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: Enchantment::Imbued,
                amount: 1,
                kind: None,
            },
            count: 1,
        }),
        hook => unimplemented!("what ElectricShrymp does for {hook:?}"),
    }
}
