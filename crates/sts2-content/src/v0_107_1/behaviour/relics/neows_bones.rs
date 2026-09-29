//! `NeowsBones`: two of Neow's relics, then a curse.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::relics::Relic;
use super::super::super::generated::runstate::RunRngType;

/// `Relics`.
const RELICS: usize = 2;

/// `Curses`.
const CURSES: usize = 1;

/// `AfterObtained`: a relic reward for each of two of Neow's other relics,
/// shuffled off the rewards stream, which the player cannot skip. Then one
/// curse, `NextItem` off `RunState.Rng.Niche`, into the deck.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferNeowRelics {
            count: RELICS,
            except: Relic::NeowsBones,
        });
        asked.push(HookEffect::AddDistinctToDeck {
            cards: &super::MODIFIER_CURSES,
            count: CURSES,
            stream: RunRngType::Niche,
        });
    }
}
