//! `NeowsTalisman`: the last Basic Strike and Defend upgraded as it is picked up.

use sts2_core::{DeckAct, EventHook, HookEffect};

/// `AfterObtained`: `CardCmd.Upgrade` on each card it names.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::OnDeckCards {
            act: DeckAct::Upgrade,
            picks: V::NEOWS_TALISMAN_PICKS,
        });
    }
}

/// `PICKS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `LastOrDefault` over the deck's Basic cards, for each tag in turn.
        const NEOWS_TALISMAN_PICKS: &'static [sts2_core::DeckPick<$build>] = &[
            sts2_core::DeckPick::LastBasic(crate::generated::cards::CardTag::Strike),
            sts2_core::DeckPick::LastBasic(crate::generated::cards::CardTag::Defend),
        ];
    };
}

pub(crate) use lists;
