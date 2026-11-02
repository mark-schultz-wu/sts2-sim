//! `TheBoot`: a powered attack its owner or Osty lands for one to four
//! takes five.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyHpLostAfterOstyLate`: a loss of at least one and under five
/// becomes five. That is for a powered attack on someone else, which its
/// owner or its owner's Osty deals.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    fight: Option<&dyn sts2_core::FightView<V>>,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    // `Owner.Osty`: the Osty its owner summoned.
    let owners_osty = |dealer| {
        fight.is_some_and(|fight| {
            fight.pet_owner(dealer) == Some(owner)
                && fight.monster(dealer) == Some(<V::Monster as sts2_core::MonsterKinds>::OSTY)
        })
    };
    match hook {
        ModifyHook::HpLostAfterOstyLate {
            target,
            dealer: Some(dealer),
            powered: true,
        } if (dealer == owner || owners_osty(dealer)) && target != owner => Modify::RaisedTo {
            floor: V::Relic::THE_BOOT_DAMAGE_MINIMUM,
            from: 1,
        },
        ModifyHook::HpLostAfterOstyLate { .. } => Modify::Nothing,
        hook => unimplemented!("what TheBoot does for {hook:?}"),
    }
}

/// Its `AfterModifyingHpLostAfterOsty` only flashes, and the fight asks it
/// nothing else.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TheBoot does for {hook:?}"),
    }
}
