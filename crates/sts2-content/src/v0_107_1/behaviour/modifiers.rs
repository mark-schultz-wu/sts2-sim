//! What a run modifier does, as ported code. A modifier is asked only about
//! a hook its class overrides.

use sts2_core::{DecisionHook, EventHook, HookEffect, HookResponse, Modify, ModifyHook};

use super::super::V0_107_1 as V;
use super::super::generated::encounters::RoomType;
use super::super::generated::modifiers::Modifier;
use super::super::generated::restsite::RestSiteOption;

/// `NightTerrors._maxHpLoss`.
const NIGHT_TERRORS_LOSS: i32 = 5;

/// What `modifier` decides for `hook`.
///
/// # Panics
///
/// A modifier whose override of `hook` is not ported.
pub(crate) fn decide_hook(modifier: Modifier, hook: DecisionHook<V>) -> HookResponse<bool> {
    match (modifier, hook) {
        // `Flight.ShouldAllowFreeTravel`.
        (Modifier::Flight, DecisionHook::ShouldAllowFreeTravel) => HookResponse::Decision(true),
        (modifier, hook) => unimplemented!("what {modifier:?} decides for {hook:?}"),
    }
}

/// What `modifier` makes of the number `hook` asks about.
///
/// # Panics
///
/// A modifier whose override of `hook` is not ported.
pub(crate) fn modify_hook(modifier: Modifier, hook: ModifyHook<V>) -> Modify<i32> {
    match (modifier, hook) {
        // `NightTerrors.ModifyRestSiteHealAmount`: the whole of the maximum.
        (Modifier::NightTerrors, ModifyHook::RestSiteHealAmount { max_hp }) => {
            Modify::Replaced(max_hp)
        }
        (modifier, hook) => unimplemented!("what {modifier:?} does for {hook:?}"),
    }
}

/// What `modifier` asks for once `hook` is raised.
///
/// # Panics
///
/// A modifier whose override of `hook` is not ported.
pub(crate) fn event_hook(
    modifier: Modifier,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (modifier, hook) {
        // `Vintage.TryModifyRewardsLate`: a relic for each card reward, in a
        // monster's room.
        (Modifier::Vintage, EventHook::ModifyRewardsLate { room }) => {
            if room == Some(RoomType::Monster) {
                asked.push(HookEffect::RelicRewardsForCardRewards);
            }
        }
        // `Midas.TryModifyRewardsLate`: every gold reward at twice its amount.
        (Modifier::Midas, EventHook::ModifyRewardsLate { .. }) => {
            asked.push(HookEffect::DoubleGoldRewards);
        }
        // `Midas.TryModifyRestSiteOptions`: no smithing.
        (Modifier::Midas, EventHook::RestSiteOptions) => {
            asked.push(HookEffect::WithdrawRestOption(RestSiteOption::Smith));
        }
        // `NightTerrors.AfterRestSiteHeal`: five off the maximum, which
        // keeps one, after a rest site's own heal.
        (Modifier::NightTerrors, EventHook::RestSiteHeal { mimicked, max_hp }) => {
            if !mimicked {
                asked.push(HookEffect::LoseMaxHp(NIGHT_TERRORS_LOSS.min(max_hp - 1)));
            }
        }
        (modifier, hook) => unimplemented!("what {modifier:?} does for {hook:?}"),
    }
}

/// Whether every rule of `modifier` is ported.
pub(crate) fn ported(modifier: Modifier) -> bool {
    matches!(
        modifier,
        Modifier::Flight | Modifier::Vintage | Modifier::Midas | Modifier::NightTerrors
    )
}
