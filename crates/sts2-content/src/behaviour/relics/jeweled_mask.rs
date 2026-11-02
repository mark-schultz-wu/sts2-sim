//! `JeweledMask`: a Power from the draw pile, free, as a fight opens.

use sts2_core::{CardKind, EventHook, HookEffect};

/// `BeforeHandDraw` on its owner's first turn: one Power of the draw pile,
/// free this turn, into the hand.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,

    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // `player != Owner`: another player's turn is not its owner's.
    if let EventHook::BeforeHandDraw { player, .. } = hook
        && player != owner
    {
        return;
    }
    match hook {
        EventHook::BeforeHandDraw { turn, .. } if turn <= 1 => {
            asked.push(HookEffect::FreeRandomFromDraw {
                kind: CardKind::Power,
                preferring_without: None,
            })
        }
        EventHook::BeforeHandDraw { .. } | EventHook::Obtained { .. } => {}
        hook => unimplemented!("what JeweledMask does for {hook:?}"),
    }
}
