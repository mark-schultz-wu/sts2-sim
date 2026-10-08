//! `MusicBox`: the first Attack its owner plays each turn, cloned into hand
//! as Ethereal.

use sts2_core::{CardKind, Change, EventHook, HookEffect, Lasting, Rules as _};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardKeyword;
use super::super::super::generated::runstate::CombatSide;

/// `CardCmd.ApplyKeyword(card, Ethereal)` on the clone.
const ETHEREAL: &[(Change<V>, Lasting)] = &[(
    Change::AddKeyword(CardKeyword::Ethereal),
    Lasting::ThisFight,
)];

/// `WasUsedThisTurn` and `CardBeingPlayed`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    used: bool,
    playing: Option<sts2_core::CardId<V>>,
}

/// `BeforeCardPlayed` notes an Attack while none is noted and the turn's
/// clone is unused. `AfterCardPlayed` of that card clones it into hand as
/// Ethereal. `BeforeSideTurnStart` for its owner's side and `AfterCombatEnd`
/// clear the turn's use. A relic's owner is the player, and a fight here has
/// one, so every card is its owner's.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlaying { card, id, .. }
            if state.playing.is_none()
                && !state.used
                && V::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.playing = Some(id);
        }
        EventHook::CardPlayed { id, .. } if state.playing == Some(id) => {
            asked.push(HookEffect::CloneIntoHand {
                card: id,
                count: 1,
                writes: ETHEREAL,
            });
            state.used = true;
            state.playing = None;
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        }
        | EventHook::CombatEnd { .. } => state.used = false,
        _ => {}
    }
}
