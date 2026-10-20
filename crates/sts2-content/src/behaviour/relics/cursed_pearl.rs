//! `CursedPearl`: Greed in the deck, and gold for carrying it.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `AfterObtained`: `CardPileCmd.AddCurseToDeck<Greed>`, then the gold.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::AddToDeck(V::Card::GREED),
            HookEffect::GainGold(V::Relic::CURSED_PEARL_GOLD),
        ]);
    }
}
