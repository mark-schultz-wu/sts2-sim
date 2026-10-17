//! `FresnelLens`: every card rewards, merchants and the deck take in comes
//! with Nimble, where Nimble can enchant it.

use crate::ids::EnchantmentIds as _;
use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `TryModifyCardRewardOptionsLate`, `ModifyMerchantCardCreationResults`
/// and `TryModifyCardBeingAddedToDeck`, each over a card `Nimble.CanEnchant`
/// takes. A card a reward already enchanted is refused as it joins the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardOption { card, .. }
        | EventHook::MerchantCardCreated { card }
        | EventHook::CardBeingAddedToDeck { card }
            if crate::behaviour::enchantments::can_enchant(V::Enchantment::NIMBLE, &card) =>
        {
            asked.push(HookEffect::EnchantCard {
                enchantment: V::Enchantment::NIMBLE,
                amount: (V::Relic::FRESNEL_LENS_NIMBLE_AMOUNT as u8),
            });
        }
        _ => {}
    }
}
