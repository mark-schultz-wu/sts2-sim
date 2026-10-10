//! `TheBoot`: a powered attack its owner or Osty lands for one to four
//! takes five.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DamageMinimum`.
const MINIMUM: Number = 5;

/// `ModifyHpLostAfterOstyLate`: a loss of at least one and under five, from
/// a powered attack its owner or Osty deals to someone else, becomes five.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::HpLostAfterOstyLate {
            target,
            player,
            dealer: Some(dealer),
            osty,
            powered: true,
        } if (dealer == player || Some(dealer) == osty) && target != player => Modify::RaisedTo {
            floor: MINIMUM,
            from: 1,
        },
        ModifyHook::HpLostAfterOstyLate { .. } => Modify::Nothing,
        hook => unimplemented!("what TheBoot does for {hook:?}"),
    }
}

/// Its `AfterModifyingHpLostAfterOsty` only flashes, and the fight asks it
/// nothing else.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TheBoot does for {hook:?}"),
    }
}
