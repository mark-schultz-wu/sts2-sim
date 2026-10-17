//! `ChoicesParadox`: one card of five, with Retain, into the first hand.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterPlayerTurnStart` on its owner's first turn: five distinct cards of
/// the character's pool, each with Retain, and the one chosen into hand. A
/// relic's owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::PlayerTurnStart { turn: 1, .. } = hook {
        asked.push(HookEffect::ChooseFromPoolIntoHand {
            count: V::Relic::CHOICES_PARADOX_CARDS,
            writes: V::CHOICES_PARADOX_RETAIN,
        });
    }
}

/// `RETAIN`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `CardCmd.ApplyKeyword(card, Retain)` on each card offered.
        const CHOICES_PARADOX_RETAIN: &'static [(sts2_core::Change<$build>, sts2_core::Lasting)] =
            &[(
                sts2_core::Change::AddKeyword(crate::generated::cards::CardKeyword::Retain),
                sts2_core::Lasting::ThisFight,
            )];
    };
}

pub(crate) use lists;
