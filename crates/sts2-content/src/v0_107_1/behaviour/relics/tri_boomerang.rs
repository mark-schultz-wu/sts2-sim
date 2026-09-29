//! `TriBoomerang`: three attacks enchanted with Instinct.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `CardsVar(3)`.
const CARDS: usize = 3;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for three, each
/// then `CardCmd.Enchant<Instinct>` by one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeck {
            act: DeckAct::Enchant {
                enchantment: Enchantment::Instinct,
                amount: 1,
                kind: None,
            },
            count: CARDS,
        });
    }
}
