//! `RegalPillow`: more from each rest site's heal.

use crate::vars::RelicVars as _;
use sts2_core::{Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyRestSiteHealAmount`: its heal on top, for its owner. A run here has
/// one player. Its `AfterRoomEntered` and `AfterRestSiteHeal` set only how
/// the relic is drawn.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::RestSiteHealAmount { .. } => Modify::Added(V::Relic::REGAL_PILLOW_HEAL),
        hook => unimplemented!("what RegalPillow does for {hook:?}"),
    }
}
