//! `CursedPearl`: Greed in the deck, and gold for carrying it.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::Number;

/// `GoldVar(333)`.
const GOLD: Number = 333;

/// `AfterObtained`: `CardPileCmd.AddCurseToDeck<Greed>`, then the gold.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.extend([
            HookEffect::AddToDeck(Cards::Curse(CurseCard::Greed)),
            HookEffect::GainGold(GOLD),
        ]);
    }
}
