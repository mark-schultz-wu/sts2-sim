//! `GnarledHammer`: up to three cards enchanted with Sharp.

use crate::ids::EnchantmentIds as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `CardsVar(3)`: how many cards it offers to enchant.
const CARDS: usize = 3;

/// `DynamicVar("SharpAmount", 3)`.
const SHARP: u8 = 3;

/// `AfterObtained`: `CardSelectCmd.FromDeckForEnchantment` for none to three,
/// confirmed by hand, then Sharp at three on each.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeckConfirmed {
            act: DeckAct::Enchant {
                enchantment: V::Enchantment::SHARP,
                amount: SHARP,
                kind: None,
            },
            min: 0,
            max: CARDS,
        });
    }
}
