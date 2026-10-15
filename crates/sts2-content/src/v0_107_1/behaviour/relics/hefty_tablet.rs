//! `HeftyTablet`: one of three rares, and an Injury either way.

use sts2_core::{CardDraw, EventHook, HookEffect, RewardRarity};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};

/// `CardsVar(3)`.
const CARDS: usize = 3;

/// `AfterObtained`: three rares at `Uniform` odds, and a choice of one that
/// can be skipped. The chosen card goes into the deck, then an Injury.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::ChooseCards {
                draw: CardDraw::Uniform(RewardRarity::Rare),
                count: CARDS,
                min: 0,
                max: 1,
                card_reward: false,
            },
            HookEffect::AddToDeck(Cards::Curse(CurseCard::Injury)),
        ]);
    }
}
