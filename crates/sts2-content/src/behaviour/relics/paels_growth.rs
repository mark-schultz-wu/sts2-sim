//! `PaelsGrowth`: a deck card made Clone as it is picked up, and a rest site
//! option that copies each Clone card.

use crate::ids::{EnchantmentIds as _, RestSiteOptionIds as _};
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `CardCmd.Enchant<Clone>`'s amount.
const CLONE_AMOUNT: u8 = 4;

/// `AfterObtained`: `FromDeckForEnchantment` for one card, then
/// `CardCmd.Enchant<Clone>` at four. `TryModifyRestSiteOptions`: `CLONE`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::CLONE,
                amount: CLONE_AMOUNT,
                kind: None,
            },
            count: 1,
        }),
        EventHook::RestSiteOptions => {
            asked.push(HookEffect::OfferRestOption(V::RestSiteOption::CLONE));
        }
        hook => unimplemented!("what PaelsGrowth does for {hook:?}"),
    }
}
