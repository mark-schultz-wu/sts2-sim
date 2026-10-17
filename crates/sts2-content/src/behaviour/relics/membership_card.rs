//! `MembershipCard`: half off the merchant's prices.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `ModifyMerchantPrice`: the price times its discount, for its owner, the
/// one player a run here has, who is the local one.
pub(super) fn modify_hook<V: crate::Build>(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::MerchantPrice => {
            Modify::Scaled(sts2_core::Fixed::new(V::Relic::MEMBERSHIP_CARD_DISCOUNT, 2))
        }
        _ => Modify::Nothing,
    }
}

/// It overrides no event but its pickup, which does nothing.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what MembershipCard does for {hook:?}"),
    }
}
