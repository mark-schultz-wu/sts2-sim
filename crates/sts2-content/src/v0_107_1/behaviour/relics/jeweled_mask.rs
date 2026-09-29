//! `JeweledMask`: a Power from the draw pile, free, as a fight opens.

use sts2_core::{CardKind, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;

/// `BeforeHandDraw` on its owner's first turn: one Power of the draw pile,
/// free this turn, into the hand.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::BeforeHandDraw { turn, .. } if turn <= 1 => {
            asked.push(HookEffect::FreeRandomFromDraw {
                kind: CardKind::Power,
            })
        }
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what JeweledMask does for {hook:?}"),
    }
}
