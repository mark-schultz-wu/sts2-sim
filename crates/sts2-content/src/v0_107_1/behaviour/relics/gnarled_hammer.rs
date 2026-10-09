//! `GnarledHammer`: up to three cards enchanted with Sharp.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `CardsVar(3)`: how many cards it offers to enchant.
const CARDS: usize = 3;

/// `DynamicVar("SharpAmount", 3)`.
const SHARP: u8 = 3;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for none to three,
/// confirmed by hand, then Sharp at three on each.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeckConfirmed {
            act: DeckAct::Enchant {
                enchantment: Enchantment::Sharp,
                amount: SHARP,
                kind: None,
            },
            min: 0,
            max: CARDS,
        });
    }
}
