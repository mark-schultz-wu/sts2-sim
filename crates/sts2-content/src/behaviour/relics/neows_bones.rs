//! `NeowsBones`: two of Neow's relics, then a curse.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

use crate::generated::runstate::RunRngType;
use crate::ids::RelicIds as _;

/// `AfterObtained`: a relic reward for each of two of Neow's other relics,
/// shuffled off the rewards stream, which the player cannot skip. Then one
/// curse, `NextItem` off `RunState.Rng.Niche`, into the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OfferNeowRelics {
            count: (V::Relic::NEOWS_BONES_RELICS as usize),
            except: V::Relic::NEOWS_BONES,
        });
        asked.push(HookEffect::AddDistinctToDeck {
            cards: V::RELICS_MODIFIER_CURSES,
            count: (V::Relic::NEOWS_BONES_CURSES as usize),
            stream: RunRngType::Niche,
        });
    }
}
