//! `NeowsTalisman`: the last Basic Strike and Defend upgraded as it is picked up.

use sts2_core::{DeckAct, DeckPick, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardTag;

/// `LastOrDefault` over the deck's Basic cards, for each tag in turn.
const PICKS: &[DeckPick<V>] = &[
    DeckPick::LastBasic(CardTag::Strike),
    DeckPick::LastBasic(CardTag::Defend),
];

/// `AfterObtained`: `CardCmd.Upgrade` on each card it names.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OnDeckCards {
            act: DeckAct::Upgrade,
            picks: PICKS,
        });
    }
}
