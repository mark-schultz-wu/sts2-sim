//! `TanxsWhistle`: a Whistle in the deck.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, EventCard};

/// `AfterObtained`: `CardPileCmd.Add` of a Whistle to the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddToDeck(Cards::Event(EventCard::Whistle)));
    }
}
