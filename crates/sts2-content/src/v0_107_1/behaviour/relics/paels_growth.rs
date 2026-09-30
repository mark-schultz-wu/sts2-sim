//! `PaelsGrowth`: a deck card made Clone as it is picked up, and a rest site
//! option that copies each Clone card.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::restsite::RestSiteOption;

/// `CardCmd.Enchant<Clone>`'s amount.
const CLONE_AMOUNT: u8 = 4;

/// `AfterObtained`: `FromDeckForEnchantment` for one card, then
/// `CardCmd.Enchant<Clone>` at four. `TryModifyRestSiteOptions`: `CLONE`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: Enchantment::Clone,
                amount: CLONE_AMOUNT,
                kind: None,
            },
            count: 1,
        }),
        EventHook::RestSiteOptions => {
            asked.push(HookEffect::OfferRestOption(RestSiteOption::Clone));
        }
        hook => unimplemented!("what PaelsGrowth does for {hook:?}"),
    }
}
