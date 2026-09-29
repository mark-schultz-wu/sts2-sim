//! `DistinguishedCape`: less maximum health, and three Apparitions.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, EventCard};
use super::Number;

/// `HpLossVar(9)`.
const HP_LOSS: Number = 9;

/// `CardsVar(3)`.
const CARDS: usize = 3;

/// `AfterObtained`: `CreatureCmd.LoseMaxHp`, then three Apparitions into
/// the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::LoseMaxHp(HP_LOSS));
        for _ in 0..CARDS {
            asked.push(HookEffect::AddToDeck(Cards::Event(EventCard::Apparition)));
        }
    }
}
