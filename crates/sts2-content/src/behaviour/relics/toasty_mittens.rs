//! `ToastyMittens`: a card burned and a point of Strength each turn. In
//! v0.107.1 the top card of the draw pile before every hand, and in
//! v0.111.0 a card of the hand picked once it is drawn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Pile, PileAct, PowerAmount, Selection};

use crate::ids::PowerIds as _;

/// `BeforeHandDraw`: `ShuffleIfNecessary`, then `CardCmd.Exhaust` on the top
/// card of the draw pile. The first turn passes over the innate ones. Then
/// its Strength goes on its owner. A relic's owner is the player, and a
/// fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::BeforeHandDraw { turn, .. } => asked.extend([
            HookEffect::ShuffleIfNecessary,
            HookEffect::ExhaustFromDrawTop {
                passing_innate: turn == 1,
            },
            HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Relic::TOASTY_MITTENS_STRENGTH_POWER),
            ),
        ]),
        // v0.111.0's `AfterPlayerTurnStart`: `CardSelectCmd.FromHand` for
        // one card, `CardCmd.Exhaust` on it, then its Strength.
        EventHook::PlayerTurnStart { .. } => asked.extend([
            HookEffect::SelectFromPile {
                pile: Pile::Hand,
                selection: Selection::Exactly(
                    nonmax::NonMaxUsize::new(1).expect("one is no usize maximum"),
                ),
                act: PileAct::Exhaust,
            },
            HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Relic::TOASTY_MITTENS_STRENGTH_POWER),
            ),
        ]),
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what ToastyMittens does for {hook:?}"),
    }
}
