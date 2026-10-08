//! `RoyalStamp`: one card enchanted with Royally Approved.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `CardsVar(1)`.
const CARDS: usize = 1;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for one, over the
/// cards Royally Approved can enchant shuffled off `Rng.Niche`, then
/// `CardCmd.Enchant<RoyallyApproved>` by one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromShuffledDeck {
            act: DeckAct::Enchant {
                enchantment: Enchantment::RoyallyApproved,
                amount: 1,
                kind: None,
            },
            count: CARDS,
        });
    }
}
