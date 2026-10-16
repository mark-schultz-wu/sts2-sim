//! `TheCourier`: a fifth off the merchant's prices, and shelves that restock
//! once bought from.

use sts2_core::{DecisionHook, EventHook, HookResponse, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `DynamicVar("Discount", 20)`, off every price.
const DISCOUNT: i32 = 20;

/// `ModifyMerchantPrice`: the price less its discount, for its owner, the one
/// player a run here has.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MerchantPrice => Modify::Scaled(sts2_core::Fixed::new(100 - DISCOUNT, 2)),
        _ => Modify::Nothing,
    }
}

/// `ShouldRefillMerchantEntry`: yes, for its owner.
pub(super) fn decide_hook<V: crate::Build>(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldRefillMerchantEntry => HookResponse::Decision(true),
        _ => HookResponse::DoesNotImplement,
    }
}

/// It overrides no event but its pickup, which does nothing.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TheCourier does for {hook:?}"),
    }
}
