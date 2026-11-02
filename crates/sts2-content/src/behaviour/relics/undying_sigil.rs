//! `UndyingSigil`: an enemy whose Doom has caught up with its hit points
//! deals its owner half damage.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, FightView, Modify, ModifyHook, PowerAmount};

use crate::behaviour::relics::Number;
use crate::ids::PowerIds as _;

/// `ModifyDamageMultiplicative`: half a powered attack on its owner, from
/// another creature whose hit points are no more than its Doom.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    fight: Option<&dyn FightView<V>>,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        ModifyHook::DamageMultiplicative {
            target: Some(target),
            dealer: Some(dealer),
            powered: true,
            ..
        } => {
            let fight = fight.expect("a hit is dealt in a fight");
            if target != owner || dealer == owner {
                return Modify::Nothing;
            }
            let doom = match fight.power(dealer, V::Power::DOOM_POWER) {
                None => 0,
                Some(PowerAmount::Counted(doom)) => doom,
                Some(PowerAmount::Endless) => unimplemented!("an endless Doom"),
            };
            let hp = fight.current_hp(dealer).expect("a dealer is in the fight");
            match hp > doom {
                true => Modify::Nothing,
                false => Modify::Scaled(V::Relic::UNDYING_SIGIL_DAMAGE_DECREASE),
            }
        }
        ModifyHook::DamageMultiplicative { .. } => Modify::Nothing,
        hook => unimplemented!("what UndyingSigil does for {hook:?}"),
    }
}

/// It answers no event, but its obtaining.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what UndyingSigil does for {hook:?}"),
    }
}
