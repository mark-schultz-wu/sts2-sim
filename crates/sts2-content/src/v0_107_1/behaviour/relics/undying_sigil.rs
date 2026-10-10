//! `UndyingSigil`: an enemy whose Doom has caught up with its hit points
//! deals its owner half damage.

use sts2_core::{EventHook, FightView, Fixed, Modify, ModifyHook, PowerAmount};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::Number;

/// `DamageDecrease`.
const DECREASE: Fixed = Fixed::new(5, 1);

/// `ModifyDamageMultiplicative`: half a powered attack on its owner, from
/// another creature whose hit points are no more than its Doom.
pub(super) fn modify_hook(fight: Option<&dyn FightView<V>>, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::DamageMultiplicative {
            target: Some(target),
            dealer: Some(dealer),
            powered: true,
            ..
        } => {
            let fight = fight.expect("a hit is dealt in a fight");
            let player = fight.player();
            if target != player || dealer == player {
                return Modify::Nothing;
            }
            let doom = match fight.power(dealer, Power::DoomPower) {
                None => 0,
                Some(PowerAmount::Counted(doom)) => doom,
                Some(PowerAmount::Endless) => unimplemented!("an endless Doom"),
            };
            let hp = fight.current_hp(dealer).expect("a dealer is in the fight");
            match hp > doom {
                true => Modify::Nothing,
                false => Modify::Scaled(DECREASE),
            }
        }
        ModifyHook::DamageMultiplicative { .. } => Modify::Nothing,
        hook => unimplemented!("what UndyingSigil does for {hook:?}"),
    }
}

/// It answers no event, but its obtaining.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what UndyingSigil does for {hook:?}"),
    }
}
