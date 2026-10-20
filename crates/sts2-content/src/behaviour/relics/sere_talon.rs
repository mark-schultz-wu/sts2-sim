//! `SereTalon`: two random curses, and three Wishes.

use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::RunRngType;
use crate::ids::CardIds as _;

/// `Curses`.
const CURSES: usize = 2;

/// `Wishes`.
const WISHES: usize = 3;

/// `AfterObtained`: two curses, each `NextItem` off `RunState.Rng.Niche`
/// over those not drawn already, into the deck. Then three Wishes.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::AddDistinctToDeck {
            cards: V::RELICS_MODIFIER_CURSES,
            count: CURSES,
            stream: RunRngType::Niche,
        });
        for _ in 0..WISHES {
            asked.push(HookEffect::AddToDeck(V::Card::WISH));
        }
    }
}
