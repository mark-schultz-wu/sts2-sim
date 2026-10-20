//! `HeftyTablet`: one of three rares, and an Injury either way.

use sts2_core::{CardDraw, EventHook, HookEffect, RewardRarity};

use crate::ids::CardIds as _;

/// `CardsVar(3)`.
const CARDS: usize = 3;

/// `AfterObtained`: three rares at `Uniform` odds, and a choice of one that
/// can be skipped. The chosen card goes into the deck, then an Injury.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::ChooseCards {
                draw: CardDraw::Uniform(RewardRarity::Rare),
                count: CARDS,
                min: 0,
                max: 1,
                card_reward: false,
            },
            HookEffect::AddToDeck(V::Card::INJURY),
        ]);
    }
}
