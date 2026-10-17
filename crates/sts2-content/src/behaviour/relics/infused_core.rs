//! `InfusedCore`: Cracked Core refined, three Lightning as each fight opens
//! and a stronger Lightning.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, SideKinds};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;
use crate::ids::OrbIds as _;

/// `AfterSideTurnStart` on its owner's first turn, which channels the
/// Lightning. A relic's owner is the player, and a fight here has one.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::SideTurnStart { side, turn, .. }
            if side == <CombatSide as SideKinds>::PLAYER && turn <= 1 =>
        {
            asked.extend(
                (0..(V::Relic::INFUSED_CORE_LIGHTNING as usize))
                    .map(|_| HookEffect::Channel(V::Orb::LIGHTNING_ORB)),
            );
        }
        _ => {}
    }
}

/// `ModifyOrbValue`: its extra damage on its owner's Lightning. The player
/// is the only one with orbs.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::OrbValue { orb: __c0, .. } if __c0 == V::Orb::LIGHTNING_ORB => {
            Modify::Added(V::Relic::INFUSED_CORE_EXTRA_DAMAGE)
        }
        ModifyHook::OrbValue { .. } => Modify::Nothing,
        hook => unimplemented!("what InfusedCore does for {hook:?}"),
    }
}
