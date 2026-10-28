//! `MusicBox`: the first Attack its owner plays each turn, cloned into hand
//! as Ethereal.

use sts2_core::{CardKind, EventHook, HookEffect};

use crate::generated::runstate::CombatSide;

/// `WasUsedThisTurn` and `CardBeingPlayed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State<V: crate::Build> {
    used: bool,
    playing: Option<sts2_core::CardId<V>>,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        Self {
            used: Default::default(),
            playing: Default::default(),
        }
    }
}

/// `BeforeCardPlayed` notes an Attack while none is noted and the turn's
/// clone is unused. `AfterCardPlayed` of that card clones it into hand as
/// Ethereal. `BeforeSideTurnStart` for its owner's side and `AfterCombatEnd`
/// clear the turn's use.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlaying {
            card, id, player, ..
        } if player == owner
            && state.playing.is_none()
            && !state.used
            && V::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.playing = Some(id);
        }
        EventHook::CardPlayed { id, .. } if state.playing == Some(id) => {
            asked.push(HookEffect::CloneIntoHand {
                card: id,
                count: 1,
                writes: V::MUSIC_BOX_ETHEREAL,
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

/// `ETHEREAL`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `CardCmd.ApplyKeyword(card, Ethereal)` on the clone.
        const MUSIC_BOX_ETHEREAL: &'static [(sts2_core::Change<$build>, sts2_core::Lasting)] = &[(
            sts2_core::Change::AddKeyword(crate::generated::cards::CardKeyword::Ethereal),
            sts2_core::Lasting::ThisFight,
        )];
    };
}

pub(crate) use lists;
