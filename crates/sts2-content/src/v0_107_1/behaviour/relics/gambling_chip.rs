//! `GamblingChip`: on the first turn, any cards of the hand discarded and as
//! many drawn.

use sts2_core::{EventHook, HookEffect, Pile, PileAct, Selection};

use super::super::super::V0_107_1 as V;

/// `CardSelectorPrefs(prompt, 0, 999999999)`: up to every card of the hand.
const PICKS: usize = 999_999_999;

/// `AfterPlayerTurnStart` on its owner's first turn: `FromHandForDiscard`
/// for any number of cards, then `CardCmd.DiscardAndDraw` of those picked
/// with as many drawn. Nothing, for none picked.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::PlayerTurnStart { turn, .. } = hook
        && turn <= 1
    {
        asked.push(HookEffect::SelectFromPile {
            pile: Pile::Hand,
            selection: Selection::UpTo(
                nonmax::NonMaxUsize::new(PICKS).expect("the pick limit is no usize maximum"),
            ),
            act: PileAct::DiscardAndDraw,
        });
    }
}
