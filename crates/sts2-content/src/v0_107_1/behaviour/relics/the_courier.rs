//! `TheCourier`: a fifth off the merchant's prices, and shelves that restock
//! once bought from.

use sts2_core::{DecisionHook, EventHook, HookResponse, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DynamicVar("Discount", 20)`, off every price.
const DISCOUNT: f64 = 20.0;

/// `ModifyMerchantPrice`: the price less its discount, for its owner, the one
/// player a run here has.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MerchantPrice => Modify::Scaled(1.0 - DISCOUNT / 100.0),
        _ => Modify::Nothing,
    }
}

/// `ShouldRefillMerchantEntry`: yes, for its owner.
pub(super) fn decide_hook(hook: DecisionHook<V>) -> HookResponse<bool> {
    match hook {
        DecisionHook::ShouldRefillMerchantEntry => HookResponse::Decision(true),
        _ => HookResponse::DoesNotImplement,
    }
}

/// It overrides no event but its pickup, which does nothing.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what TheCourier does for {hook:?}"),
    }
}
