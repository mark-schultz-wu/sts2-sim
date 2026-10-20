//! `PrecariousShears`: two cards out of the deck, and a cut, as it is picked
//! up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardSelectCmd.FromDeckForRemoval`, each card it takes
/// out of the deck, then `CreatureCmd.Damage` on its owner.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::SelectFromDeck {
                act: DeckAct::Remove,
                count: (V::Relic::PRECARIOUS_SHEARS_CARDS as usize),
            },
            HookEffect::DamageOwner(V::Relic::PRECARIOUS_SHEARS_DAMAGE),
        ]);
    }
}
