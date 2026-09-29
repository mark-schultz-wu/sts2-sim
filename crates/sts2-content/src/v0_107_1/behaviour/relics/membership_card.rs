//! `MembershipCard`: half off the merchant's prices.

use sts2_core::{EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `DynamicVar("Discount", 50)`, as the share of the price that is paid.
const DISCOUNT: f64 = 50.0;

/// `ModifyMerchantPrice`: the price times its discount, for its owner, the
/// one player a run here has, who is the local one.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MerchantPrice => Modify::Scaled(DISCOUNT / 100.0),
        _ => Modify::Nothing,
    }
}

/// It overrides no event but its pickup, which does nothing.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MembershipCard does for {hook:?}"),
    }
}
