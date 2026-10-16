//! `Byrdpip`: the Byrdonis Egg hatched, fighting beside the player.

use sts2_core::{EventHook, HookEffect};

use crate::ids::{CardIds as _, MonsterIds as _};

/// `AfterObtained` turns each Byrdonis Egg in the deck into a Byrd Swoop, and
/// `BeforeCombatStart` adds the Byrdpip as a pet. The skin it picks draws off
/// its own `Rng`, which no other stream shares.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::TransformInDeck {
            from: V::Card::BYRDONIS_EGG,
            into: V::Card::BYRD_SWOOP,
        }),
        EventHook::CombatStarting { .. } => asked.push(HookEffect::AddPet(V::Monster::BYRDPIP)),
        hook => unimplemented!("what Byrdpip does for {hook:?}"),
    }
}
