//! `SereTalon`: two random curses, and three Wishes.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, EventCard};
use super::super::super::generated::runstate::RunRngType;

/// `Curses`.
const CURSES: usize = 2;

/// `Wishes`.
const WISHES: usize = 3;

/// `AfterObtained`: two curses, each `NextItem` off `RunState.Rng.Niche`
/// over those not drawn already, into the deck. Then three Wishes.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddDistinctToDeck {
            cards: &super::MODIFIER_CURSES,
            count: CURSES,
            stream: RunRngType::Niche,
        });
        for _ in 0..WISHES {
            asked.push(HookEffect::AddToDeck(Cards::Event(EventCard::Wish)));
        }
    }
}
