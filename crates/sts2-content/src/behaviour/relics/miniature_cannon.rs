//! `MiniatureCannon`: more damage on each powered hit from an upgraded card.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyDamageAdditive`: its extra damage on a powered hit from an upgraded
/// card. A fight here has one player, so every card is its owner's.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::DamageAdditive {
            card: Some(source),
            powered: true,
            ..
        } if source.upgraded => Modify::Added(V::Relic::MINIATURE_CANNON_EXTRA_DAMAGE),
        ModifyHook::DamageAdditive { .. } => Modify::Nothing,
        hook => unimplemented!("what MiniatureCannon does for {hook:?}"),
    }
}

/// Its one hook is `ModifyDamageAdditive`, which the fight asks of it rather
/// than tells it. `AfterObtained` reaches every relic, and its own does
/// nothing.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MiniatureCannon does for {hook:?}"),
    }
}
