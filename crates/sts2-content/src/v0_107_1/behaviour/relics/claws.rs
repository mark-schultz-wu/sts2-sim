//! `Claws`: up to six cards transformed into Maul.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, EventCard};

/// `CardsVar(6)`.
const CARDS: usize = 6;

/// `AfterObtained`: `CardSelectCmd.FromDeckForTransformation` for none to
/// six, confirmed by hand, then each transformed into a Maul.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::SelectFromDeckConfirmed {
            act: DeckAct::TransformInto(Cards::Event(EventCard::Maul)),
            min: 0,
            max: CARDS,
        });
    }
}
