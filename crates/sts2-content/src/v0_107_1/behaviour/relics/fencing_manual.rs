//! `FencingManual`: a Forge as a fight's first turn starts.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, TokenCard};
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `ForgeVar(10)`.
const FORGE: Number = 10;

/// `AfterSideTurnStart` on its owner's side, while the player's turn number
/// is one or less: `ForgeCmd.Forge` of its Forge.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::SideTurnStart {
            side: CombatSide::Player,
            turn,
            ..
        } if turn <= 1 => asked.push(HookEffect::Forge {
            card: Cards::Token(TokenCard::SovereignBlade),
            amount: FORGE,
        }),
        EventHook::SideTurnStart { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what FencingManual does for {hook:?}"),
    }
}
