//! `LeafyPoultice`: less maximum health, and the first Basic Strike and Defend
//! transformed, as it is picked up.

use crate::vars::RelicVars as _;
use sts2_core::{DeckAct, EventHook, HookEffect, Stream};

use crate::generated::runstate::PlayerRngType;

/// `AfterObtained`: `CreatureCmd.LoseMaxHp` on its owner, then
/// `CardCmd.Transform` on the cards it names, off
/// `PlayerRng.Transformations`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::LoseMaxHp(V::Relic::LEAFY_POULTICE_MAX_HP),
            HookEffect::OnDeckCards {
                act: DeckAct::TransformTogether(Stream::Player(PlayerRngType::Transformations)),
                picks: V::LEAFY_POULTICE_PICKS,
            },
        ]);
    }
}

/// `PICKS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `FirstOrDefault` over the deck's Basic cards, for each tag in turn.
        const LEAFY_POULTICE_PICKS: &'static [sts2_core::DeckPick<$build>] = &[
            sts2_core::DeckPick::FirstBasic(crate::generated::cards::CardTag::Strike),
            sts2_core::DeckPick::FirstBasic(crate::generated::cards::CardTag::Defend),
        ];
    };
}

pub(crate) use lists;
