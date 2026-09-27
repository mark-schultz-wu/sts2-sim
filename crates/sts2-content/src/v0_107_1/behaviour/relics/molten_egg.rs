//! `MoltenEgg`: every Attack card rewards, merchants and the deck take in comes
//! upgraded.

use sts2_core::{CardKind, EventHook};

use super::super::super::V0_107_1 as V;

/// `TryModifyCardRewardOptionsLate`, `ModifyMerchantCardCreationResults`
/// and `TryModifyCardBeingAddedToDeck`, each over Attacks alone.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    super::egg(CardKind::Attack, hook, asked);
}
