//! `DistinguishedCape`: less maximum health, and three Apparitions.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::ids::CardIds as _;

/// `HpLossVar(9)`.
const HP_LOSS: Number = 9;

/// `AfterObtained`: `CreatureCmd.LoseMaxHp`, then three Apparitions into
/// the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::LoseMaxHp(HP_LOSS));
        for _ in 0..(V::Relic::DISTINGUISHED_CAPE_CARDS as usize) {
            asked.push(HookEffect::AddToDeck(V::Card::APPARITION));
        }
    }
}
