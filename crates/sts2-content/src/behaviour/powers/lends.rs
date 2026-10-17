//! Which powers lend another for a turn, and whether an amount of a power
//! is a debuff.

use crate::ids::PowerIds as _;

/// What a power's amounts are held as.
type Number = i32;

/// What a temporary power lends, and `IsPositive`, for the family that lends
/// Strength, Dexterity or Focus for a turn and takes it back.
///
/// Every subclass of `TemporaryStrengthPower`, `TemporaryDexterityPower` and
/// `TemporaryFocusPower`, which is `InternallyAppliedPower`, and its
/// `IsPositive` override. None of them overrides anything else, so
/// [`temporary`] is the whole of each.
pub(crate) fn lends<V: crate::Build>(power: V::Power) -> Option<(V::Power, bool)> {
    // A temporary power only the build has.
    if let Some(lent) = V::own_lends(power) {
        return Some(lent);
    }
    Some(match power {
        __c0 if __c0 == V::Power::COORDINATE_POWER
            || __c0 == V::Power::FEEDING_FRENZY_POWER
            || __c0 == V::Power::FLEX_POTION_POWER
            || __c0 == V::Power::REPTILE_TRINKET_POWER
            || __c0 == V::Power::SETUP_STRIKE_POWER =>
        {
            (V::Power::STRENGTH_POWER, true)
        }
        __c0 if __c0 == V::Power::CRUSH_UNDER_POWER
            || __c0 == V::Power::DARK_SHACKLES_POWER
            || __c0 == V::Power::DYING_STAR_POWER
            || __c0 == V::Power::ENFEEBLING_TOUCH_POWER
            || __c0 == V::Power::MANGLE_POWER
            || __c0 == V::Power::MOCK_TEMPORARY_STRENGTH_LOSS_POWER
            || __c0 == V::Power::MONARCHS_GAZE_STRENGTH_DOWN_POWER
            || __c0 == V::Power::PIERCING_WAIL_POWER
            || __c0 == V::Power::SHACKLING_POTION_POWER =>
        {
            (V::Power::STRENGTH_POWER, false)
        }
        __c0 if __c0 == V::Power::ANTICIPATE_POWER
            || __c0 == V::Power::HELICAL_DART_POWER
            || __c0 == V::Power::SPEED_POTION_POWER =>
        {
            (V::Power::DEXTERITY_POWER, true)
        }
        __c0 if __c0 == V::Power::FOCUSED_STRIKE_POWER
            || __c0 == V::Power::HOTFIX_POWER
            || __c0 == V::Power::SYNCHRONIZE_POWER =>
        {
            (V::Power::FOCUS_POWER, true)
        }
        _ => return None,
    })
}

/// `PowerModel.GetTypeForAmount`: whether `power` is a debuff at `amount`.
///
/// A counter that allows negatives is a debuff below zero, and a debuff that
/// does not allow them is a buff there.
pub(crate) fn debuff_at<V: crate::Build>(power: V::Power, amount: Number) -> bool {
    use sts2_core::PowerTable as _;

    use crate::generated::powers::{PowerFlag, PowerStackType};

    let data = sts2_core::Table::data(power);
    let debuff = match lends::<V>(power) {
        Some((_, positive)) => !positive,
        None => power
            .debuff()
            .unwrap_or_else(|| unimplemented!("whether {power:?} is a debuff at {amount}")),
    };
    let counter = match data.stack {
        crate::Field::Constant(stack) => stack == Some(PowerStackType::Counter),
        // `ShrinkPower.StackType`: a counter, save a negative one, which is
        // `IsInfinite` and single.
        crate::Field::Computed if power == V::Power::SHRINK_POWER => amount >= 0,
        crate::Field::Computed => unimplemented!("how {power:?} stacks at {amount}"),
    };
    let crate::Field::Constant(negatives) = data.flags.get(PowerFlag::AllowNegative) else {
        unimplemented!("whether {power:?} allows negatives at {amount}")
    };
    match amount < 0 {
        true if counter && negatives => true,
        true if !negatives && debuff => false,
        _ => debuff,
    }
}
