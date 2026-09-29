//! `MiniatureCannon`: more damage on each powered hit from an upgraded card.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DynamicVar("ExtraDamage", 3)`.
const EXTRA_DAMAGE: Number = 3;

/// `ModifyDamageAdditive`: its extra damage on a powered hit from an upgraded
/// card. A fight here has one player, so every card is its owner's.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::DamageAdditive {
            card: Some(source),
            powered: true,
            ..
        } if source.upgraded => Modify::Added(EXTRA_DAMAGE),
        ModifyHook::DamageAdditive { .. } => Modify::Nothing,
        hook => unimplemented!("what MiniatureCannon does for {hook:?}"),
    }
}

/// Its one hook is `ModifyDamageAdditive`, which the fight asks of it rather
/// than tells it. `AfterObtained` reaches every relic, and its own does
/// nothing.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MiniatureCannon does for {hook:?}"),
    }
}
