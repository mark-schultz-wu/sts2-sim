//! `InfusedCore`: Cracked Core refined, three Lightning as each fight opens
//! and a stronger Lightning.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, SideKinds};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::orbs::Orb;
use super::super::super::generated::runstate::CombatSide;
use super::Number;

/// `DynamicVar("Lightning", 3)`: how many it channels.
const LIGHTNING: usize = 3;

/// `DynamicVar("ExtraDamage", 1)`.
const EXTRA_DAMAGE: Number = 1;

/// `AfterSideTurnStart` on its owner's first turn, which channels the
/// Lightning. A relic's owner is the player, and a fight here has one.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::SideTurnStart { side, turn, .. }
            if side == <CombatSide as SideKinds>::PLAYER && turn <= 1 =>
        {
            asked.extend((0..LIGHTNING).map(|_| HookEffect::Channel(Orb::LightningOrb)));
        }
        _ => {}
    }
}

/// `ModifyOrbValue`: its extra damage on its owner's Lightning. The player
/// is the only one with orbs.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::OrbValue {
            orb: Orb::LightningOrb,
            ..
        } => Modify::Added(EXTRA_DAMAGE),
        ModifyHook::OrbValue { .. } => Modify::Nothing,
        hook => unimplemented!("what InfusedCore does for {hook:?}"),
    }
}
