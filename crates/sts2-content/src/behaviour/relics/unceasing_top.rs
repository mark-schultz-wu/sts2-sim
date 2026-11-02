//! `UnceasingTop`: a card drawn whenever the hand empties in its owner's
//! turn.

use sts2_core::{EventHook, HookEffect, TurnPhaseKinds as _};

use crate::behaviour::relics::Number;

/// `CardPileCmd.Draw(choiceContext, player)`, which draws one.
const CARDS: Number = 1;

/// `AfterHandEmptied` for its owner in `AutoPrePlay`, `Play` or
/// `AutoPostPlay`. Before the hand is drawn, or after it is flushed, an
/// auto-played card always leaves it empty, so those phases draw nothing.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::HandEmptied { player, .. } = hook
        && player != owner
    {
        return;
    }
    type Phase = crate::generated::runstate::PlayerTurnPhase;
    if let EventHook::HandEmptied { phase, .. } = hook
        && [Phase::AUTO_PRE_PLAY, Phase::PLAY, Phase::AUTO_POST_PLAY].contains(&phase)
    {
        asked.push(HookEffect::Draw(CARDS));
    }
}
