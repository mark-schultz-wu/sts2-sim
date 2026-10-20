//! What an affliction does in a fight, as a listener on the card it is on.

use sts2_core::{CardId, EventHook, HookEffects};

use crate::ids::AfflictionIds as _;

/// What `affliction` on a copy asks for when `hook` is raised.
///
/// # Panics
///
/// An affliction and hook nobody has ported.
pub(crate) fn event_hook<V: crate::Build>(
    affliction: V::Affliction,
    _id: CardId<V>,
    hook: EventHook<V>,
    _asked: &mut HookEffects<V>,
) {
    match (affliction, hook) {
        // `Hexed.AfterCardEnteredCombat`: Hexed comes off its own card when
        // the card's owner has no `HexPower`. `HexPower` takes Hexed off
        // every card as it goes, so a card carries Hexed only while its
        // owner has the power, and this clears nothing.
        (__c0, EventHook::CardEnteredCombat { .. }) if __c0 == V::Affliction::HEXED => {}
        (affliction, hook) => unimplemented!("what {affliction:?} asks for at {hook:?}"),
    }
}
