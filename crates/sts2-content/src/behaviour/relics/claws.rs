//! `Claws`: up to six cards transformed into Maul.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect};

use crate::ids::CardIds as _;

/// `AfterObtained`: `CardSelectCmd.FromDeckForTransformation` for none to
/// six, confirmed by hand, then each transformed into a Maul.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeckConfirmed {
            act: DeckAct::TransformInto(V::Card::MAUL),
            min: 0,
            max: (V::Relic::CLAWS_CARDS as usize),
        });
    }
}
