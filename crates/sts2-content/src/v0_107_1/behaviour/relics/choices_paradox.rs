//! `ChoicesParadox`: one card of five, with Retain, into the first hand.

use sts2_core::{Change, EventHook, HookEffect, Lasting};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardKeyword;
use super::Number;

/// `CardsVar(5)`.
const CARDS: Number = 5;

/// `CardCmd.ApplyKeyword(card, Retain)` on each card offered.
const RETAIN: &[(Change<V>, Lasting)] =
    &[(Change::AddKeyword(CardKeyword::Retain), Lasting::ThisFight)];

/// `AfterPlayerTurnStart` on its owner's first turn: five distinct cards of
/// the character's pool, each with Retain, and the one chosen into hand. A
/// relic's owner is the player, and a fight here has one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PlayerTurnStart { turn: 1, .. } = hook {
        asked.push(HookEffect::ChooseFromPoolIntoHand {
            count: CARDS,
            writes: RETAIN,
        });
    }
}
