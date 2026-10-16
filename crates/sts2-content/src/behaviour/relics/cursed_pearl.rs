//! `CursedPearl`: Greed in the deck, and gold for carrying it.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::ids::CardIds as _;

/// `GoldVar(333)`.
const GOLD: Number = 333;

/// `AfterObtained`: `CardPileCmd.AddCurseToDeck<Greed>`, then the gold.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::AddToDeck(V::Card::GREED),
            HookEffect::GainGold(GOLD),
        ]);
    }
}
