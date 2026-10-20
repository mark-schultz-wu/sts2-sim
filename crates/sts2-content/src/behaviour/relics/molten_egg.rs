//! `MoltenEgg`: every Attack card rewards, merchants and the deck take in comes
//! upgraded.

use sts2_core::{CardKind, EventHook};

/// `TryModifyCardRewardOptionsLate`, `ModifyMerchantCardCreationResults`
/// and `TryModifyCardBeingAddedToDeck`, each over Attacks alone.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::relics::egg::<V>(CardKind::Attack, hook, asked);
}
