//! The enchantments whose rules this build ports for itself, which the
//! shared enchantments hand over through [`crate::build::OwnRules`].

use sts2_core::{EnchantHook, Enchanted, Modify};

use super::super::V0_111_0 as V;
use super::super::generated::enchantments::Enchantment;

/// Whether `enchantment`'s rule is this build's own for some hook.
pub(crate) fn own_enchantment(enchantment: Enchantment) -> bool {
    matches!(enchantment, Enchantment::Inky)
}

/// An enchantment's `EnchantDamageAdditive` and its kin, for one this build
/// ports for itself.
///
/// `Inky` overrides none of them here. v0.107.1's added its `DamageVar(1)` to
/// a powered hit.
pub(crate) fn own_modify(enchanted: Enchanted<V>, _hook: EnchantHook) -> Option<Modify<i32>> {
    match enchanted.enchantment {
        Enchantment::Inky => Some(Modify::Nothing),
        _ => None,
    }
}
