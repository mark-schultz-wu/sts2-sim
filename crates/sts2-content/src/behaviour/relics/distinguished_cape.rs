//! `DistinguishedCape`: less maximum health, and three Apparitions.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::ids::CardIds as _;

/// `HpLossVar(9)`.
const HP_LOSS: Number = 9;

/// `CardsVar(3)`.
const CARDS: usize = 3;

/// `AfterObtained`: `CreatureCmd.LoseMaxHp`, then three Apparitions into
/// the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::LoseMaxHp(HP_LOSS));
        for _ in 0..CARDS {
            asked.push(HookEffect::AddToDeck(V::Card::APPARITION));
        }
    }
}
