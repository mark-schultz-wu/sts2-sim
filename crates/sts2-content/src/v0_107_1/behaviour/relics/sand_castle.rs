//! `SandCastle`: the upgrades its pickup hands out, and nothing after.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::runstate::RunRngType;

/// `SandCastle.CardsVar`.
const CARDS: usize = 6;

/// Nothing in a fight. Its one override is `AfterObtained`, which upgrades six
/// of the deck's cards, and the run is what carries that out.
pub(super) fn event_hook(hook: EventHook<V>) -> Vec<HookEffect<V>> {
    match hook {
        EventHook::Obtained => vec![HookEffect::UpgradeRandomDeckCards {
            count: CARDS,
            stream: RunRngType::Niche,
        }],
        _ => Vec::new(),
    }
}
