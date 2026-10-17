//! The powers whose rules this build ports for itself, which the shared
//! powers hand over through [`crate::build::OwnRules`].
//!
//! The two v0.107.1 ports for itself, Diamond Diadem's and Outbreak's, are
//! gone from this build. Surprise's death is this build's own.

use sts2_core::{EventHook, HookEffect, Modify, ModifyHook, PowerAmount};

use super::super::V0_111_0 as V;
use super::super::generated::powers::Power;
use crate::behaviour::powers::{Held, State};

type Number = i32;

/// A power's `AfterDeath` commands, for one this build ports for itself.
///
/// `SurprisePower.AfterDeath` makes the Fat Gremlin first, which takes the
/// next id and rolls its health. It takes a Heist of the gold each Thievery
/// stole while it stands off the board. The Sneaky Gremlin joins, then the
/// Fat Gremlin. Any gold at all marks the encounter's gold stolen.
pub(crate) fn own_power_death<F: sts2_core::Fight<V>>(
    power: Power,
    holder: sts2_core::CombatId,
    fight: &mut F,
) -> Option<()> {
    use super::super::generated::monsters::Monster;

    if power != Power::SurprisePower {
        return None;
    }
    let fat = fight.create_monster(Monster::FatGremlin);
    let mut stolen = None;
    fight.write_power(holder, Power::ThieveryPower, |state| {
        stolen = Some(*state.stolen_gold());
    });
    if let Some(stolen) = stolen {
        fight.apply_power_from(
            fat,
            Power::HeistPower,
            PowerAmount::Counted(stolen),
            Some(holder),
        );
    }
    fight.summon(Monster::SneakyGremlin, usize::MAX);
    fight.add_created(fat, usize::MAX);
    if stolen.is_some_and(|stolen| stolen > 0) {
        fight.write_encounter(crate::behaviour::encounters::State::mark_gold_stolen);
    }
    Some(())
}

/// Whether `power`'s rule is this build's own.
pub(crate) fn own_power(power: Power) -> bool {
    matches!(power, Power::AmbergrisPower)
}

/// The state a power of this build's own starts with.
pub(crate) fn own_power_state(_power: Power) -> Option<State<V>> {
    None
}

/// `ModifyHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_modify_hook(
    _power: Power,
    _amount: PowerAmount<Number>,
    _held: Held<'_, V>,
    _state: &State<V>,
    _hook: ModifyHook<V>,
) -> Option<Modify<Number>> {
    None
}

/// `EventHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_event_hook(
    power: Power,
    _amount: PowerAmount<Number>,
    _held: Held<'_, V>,
    _state: &mut State<V>,
    hook: EventHook<V>,
) -> Option<Option<HookEffect<V>>> {
    match (power, hook) {
        // `AmbergrisPower.AfterTakingExtraTurn`: one off, for its owner's
        // turn. A fight here has one player, whose turn it is.
        (Power::AmbergrisPower, EventHook::TakingExtraTurn) => Some(Some(HookEffect::Decrement)),
        _ => None,
    }
}

/// A yes-or-no hook for a power of this build's own, or nothing for a hook
/// the shared powers answer for it.
pub(crate) fn own_power_decide_hook(
    power: Power,
    amount: PowerAmount<Number>,
    _held: Held<'_, V>,
    _state: &State<V>,
    hook: sts2_core::DecisionHook<V>,
) -> Option<sts2_core::HookResponse<bool>> {
    match (power, amount, hook) {
        // `AmbergrisPower.ShouldTakeExtraTurn`: yes while it has a stack, for
        // its owner. A fight here has one player.
        (
            Power::AmbergrisPower,
            PowerAmount::Counted(amount),
            sts2_core::DecisionHook::ShouldTakeExtraTurn { .. },
        ) => Some(sts2_core::HookResponse::Decision(amount > 0)),
        (Power::AmbergrisPower, PowerAmount::Endless, _) => {
            unimplemented!("an endless Ambergris")
        }
        _ => None,
    }
}
