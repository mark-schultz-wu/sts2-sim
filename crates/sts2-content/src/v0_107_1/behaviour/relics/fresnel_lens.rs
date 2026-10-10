//! `FresnelLens`: every card rewards, merchants and the deck take in comes
//! with Nimble, where Nimble can enchant it.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
/// `NimbleAmount`.
const NIMBLE: u8 = 2;

/// `TryModifyCardRewardOptionsLate`, `ModifyMerchantCardCreationResults`
/// and `TryModifyCardBeingAddedToDeck`, each over a card `Nimble.CanEnchant`
/// takes. A card a reward already enchanted is refused as it joins the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::CardRewardOption { card, .. }
        | EventHook::MerchantCardCreated { card }
        | EventHook::CardBeingAddedToDeck { card }
            if super::super::enchantments::can_enchant(Enchantment::Nimble, &card) =>
        {
            asked.push(HookEffect::EnchantCard {
                enchantment: Enchantment::Nimble,
                amount: NIMBLE,
            });
        }
        _ => {}
    }
}
