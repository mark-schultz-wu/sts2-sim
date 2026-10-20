//! `RoyalStamp`: one card enchanted with Royally Approved.

use crate::ids::EnchantmentIds as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `CardsVar(1)`.
const CARDS: usize = 1;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for one, over the
/// cards Royally Approved can enchant shuffled off `Rng.Niche`, then
/// `CardCmd.Enchant<RoyallyApproved>` by one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromShuffledDeck {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::ROYALLY_APPROVED,
                amount: 1,
                kind: None,
            },
            count: CARDS,
        });
    }
}
