//! `LeafyPoultice`: less maximum health, and the first Basic Strike and Defend
//! transformed, as it is picked up.

use sts2_core::{DeckAct, EventHook, HookEffect, Stream};

use crate::behaviour::relics::Number;
use crate::generated::runstate::PlayerRngType;

/// `MaxHpVar(12)`.
const MAX_HP: Number = 12;

/// `AfterObtained`: `CreatureCmd.LoseMaxHp` on its owner, then
/// `CardCmd.Transform` on the cards it names, off
/// `PlayerRng.Transformations`.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::LoseMaxHp(MAX_HP),
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
