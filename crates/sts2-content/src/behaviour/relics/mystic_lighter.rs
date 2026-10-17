//! `MysticLighter`: more damage on each powered hit from an enchanted card.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyDamageAdditive`: its damage on a powered hit from a card with an
/// enchantment. A fight here has one player, so every card is its owner's.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::DamageAdditive {
            card: Some(source),
            powered: true,
            ..
        } if source.enchanted => Modify::Added(V::Relic::MYSTIC_LIGHTER_DAMAGE),
        ModifyHook::DamageAdditive { .. } => Modify::Nothing,
        hook => unimplemented!("what MysticLighter does for {hook:?}"),
    }
}

/// Its one hook the fight asks rather than tells. `AfterObtained` reaches
/// every relic, and its own does nothing.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MysticLighter does for {hook:?}"),
    }
}
