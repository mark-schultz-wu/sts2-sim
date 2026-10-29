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
/// stole, aimed at the Thievery's target, while it stands off the board. The Sneaky Gremlin joins, then the
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
    let stolen = crate::behaviour::powers::heists::<V, _>(fight, holder, fat);
    fight.summon(Monster::SneakyGremlin, usize::MAX);
    fight.add_created(fat, usize::MAX);
    if stolen > 0 {
        fight.write_encounter(crate::behaviour::encounters::State::mark_gold_stolen);
    }
    Some(())
}

/// `TankPower`'s `DynamicVar("DamageIncrease", 1.5m)`, which only this
/// build has.
const TANK_DAMAGE_INCREASE: sts2_core::Fixed = sts2_core::Fixed::new(15, 1);

/// Whether `power`'s rule is this build's own.
pub(crate) fn own_power(power: Power) -> bool {
    matches!(
        power,
        Power::AmbergrisPower
            | Power::TrackingPower
            | Power::FeralPower
            | Power::JugglingPower
            | Power::CorruptionPower
            | Power::PaleBlueDotPower
            | Power::VulnerablePower
            | Power::ConcoctPower
            | Power::TankPower
    )
}

/// The state a power of this build's own starts with.
pub(crate) fn own_power_state(power: Power) -> Option<State<V>> {
    match power {
        Power::PaleBlueDotPower => Some(State::PaleBlueDot { activated: false }),
        _ => None,
    }
}

/// `ModifyHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_modify_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    _state: &State<V>,
    hook: ModifyHook<V>,
) -> Option<Modify<Number>> {
    match (power, amount, hook) {
        // `TrackingPower.ModifyDamageMultiplicative`: one and its amount in
        // hundredths, on a powered hit from a card by its owner or a pet of
        // its owner's, on a target carrying Weak.
        (
            Power::TrackingPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageMultiplicative {
                target: Some(target),
                dealer: Some(dealer),
                card: Some(_),
                powered: true,
            },
        ) if (dealer == held.holder || held.fight.pet_owner(dealer) == Some(held.holder))
            && held.fight.power(target, Power::WeakPower).is_some() =>
        {
            Some(Modify::Scaled(sts2_core::Fixed::new(100 + amount, 2)))
        }
        (Power::TrackingPower, PowerAmount::Endless, ModifyHook::DamageMultiplicative { .. }) => {
            unimplemented!("an endless Tracking")
        }
        (Power::TrackingPower, _, ModifyHook::DamageMultiplicative { .. }) => Some(Modify::Nothing),
        // `TankPower.ModifyDamageMultiplicative`: its `DamageIncrease` on a
        // powered attack's damage to its owner, where v0.107.1's doubled it.
        (
            Power::TankPower,
            _,
            ModifyHook::DamageMultiplicative {
                target: Some(target),
                powered: true,
                ..
            },
        ) if target == held.holder => Some(Modify::Scaled(TANK_DAMAGE_INCREASE)),
        // `VulnerablePower.ModifyDamageMultiplicative` reads the Cruelty of a
        // pet's owner where the pet has none. v0.107.1's read the dealer's
        // alone.
        (
            Power::VulnerablePower,
            _,
            ModifyHook::DamageMultiplicative {
                target: Some(target),
                dealer,
                powered: true,
                ..
            },
        ) if target == held.holder => Some(Modify::Scaled(
            crate::behaviour::powers::vulnerable::<V>(held.fight, held.holder, dealer, true),
        )),
        _ => None,
    }
}

/// `ModifyCardPlayResultLocation` for a power of this build's own, or
/// nothing for one the shared powers answer.
///
/// `FeralPower` leaves a dupe where it goes, where v0.107.1's took it back
/// to the hand as well.
pub(crate) fn own_power_result_pile(
    power: Power,
    _amount: PowerAmount<Number>,
    _holder: sts2_core::CombatId,
    _state: &State<V>,
    hook: sts2_core::ResultPile<V>,
) -> Option<sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)>> {
    match power {
        Power::FeralPower if hook.dupe => Some(sts2_core::HookResponse::Decision((
            hook.pile,
            hook.position,
        ))),
        _ => None,
    }
}

/// `EventHook` for a power of this build's own, or nothing for a hook the
/// shared powers answer for it.
pub(crate) fn own_power_event_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    state: &mut State<V>,
    hook: EventHook<V>,
) -> Option<Option<HookEffect<V>>> {
    match (power, hook) {
        // `PaleBlueDotPower.AfterCardPlayed`: once a turn, when its owner
        // has finished five plays this turn, its stack as
        // `DrawCardsNextTurnPower` on its owner. Another player's card does
        // nothing.
        (Power::PaleBlueDotPower, EventHook::CardPlayed { player, .. }) => {
            let State::PaleBlueDot { activated } = state else {
                unreachable!("a Pale Blue Dot keeps whether it activated")
            };
            if player != held.holder
                || *activated
                || held.history.finished_this_turn().count() < PALE_BLUE_DOT_PLAYS
            {
                return Some(None);
            }
            *activated = true;
            Some(Some(HookEffect::ApplyToOwner(
                Power::DrawCardsNextTurnPower,
                amount,
            )))
        }
        // `AfterSideTurnEnd` on its owner's side clears the turn's
        // activation.
        (Power::PaleBlueDotPower, EventHook::SideTurnEnd { side, .. }) => {
            if side == held.owner {
                *state = State::PaleBlueDot { activated: false };
            }
            Some(None)
        }
        // `CorruptionPower.AfterModifyingCardPlayResultLocation` flashes
        // alone. v0.107.1's overrode no such hook.
        (Power::CorruptionPower, EventHook::ResultPileModified { .. }) => Some(None),
        // `JugglingPower.BeforeCardPlayed`: one more of its owner's attacks,
        // and on the third, its stack of clones of that attack into hand.
        // v0.107.1's counted after the play.
        (
            Power::JugglingPower,
            EventHook::CardPlaying {
                card, id, player, ..
            },
        ) => {
            use sts2_core::Rules as _;

            if player != held.holder || V::card_kind(card) != Some(sts2_core::CardKind::Attack) {
                return Some(None);
            }
            let PowerAmount::Counted(amount) = amount else {
                unimplemented!("an endless Juggling")
            };
            state.count(1);
            Some(
                (state.counted() == JUGGLING_ATTACKS).then_some(HookEffect::CloneIntoHand {
                    card: id,
                    count: amount,
                    writes: &[],
                }),
            )
        }
        // `ConcoctPower.AfterDamageGiven`: its stack of Poison onto whoever a
        // powered hit from its owner took hit points from.
        (
            Power::ConcoctPower,
            EventHook::DamageGiven {
                dealer,
                target,
                unblocked,
                powered,
                ..
            },
        ) => Some(
            (dealer == Some(held.holder) && powered && unblocked > 0).then(|| {
                let PowerAmount::Counted(amount) = amount else {
                    unimplemented!("an endless Concoct")
                };
                HookEffect::ApplyPower {
                    target,
                    power: Power::PoisonPower,
                    amount: PowerAmount::Counted(amount),
                    applier: Some(held.holder),
                }
            }),
        ),
        // `ConcoctPower.AfterSideTurnEnd`: gone as the other side's turn ends.
        (Power::ConcoctPower, EventHook::SideTurnEnd { side, .. }) => {
            Some((side != held.owner).then_some(HookEffect::Remove))
        }
        // `AmbergrisPower.AfterTakingExtraTurn`: one off, for its owner's
        // turn.
        (Power::AmbergrisPower, EventHook::TakingExtraTurn { player }) => {
            Some((player == held.holder).then_some(HookEffect::Decrement))
        }
        _ => None,
    }
}

/// `PaleBlueDotPower`'s `cardPlayThresholdValue`.
const PALE_BLUE_DOT_PLAYS: usize = 5;

/// `JugglingPower`'s third attack of a turn, which it checks for in its body.
const JUGGLING_ATTACKS: Number = 3;

/// A yes-or-no hook for a power of this build's own, or nothing for a hook
/// the shared powers answer for it.
pub(crate) fn own_power_decide_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    _state: &State<V>,
    hook: sts2_core::DecisionHook<V>,
) -> Option<sts2_core::HookResponse<bool>> {
    match (power, amount, hook) {
        // `AmbergrisPower.ShouldTakeExtraTurn`: yes while it has a stack, for
        // its owner.
        (
            Power::AmbergrisPower,
            PowerAmount::Counted(amount),
            sts2_core::DecisionHook::ShouldTakeExtraTurn { player, .. },
        ) => Some(sts2_core::HookResponse::Decision(
            amount > 0 && player == held.holder,
        )),
        (Power::AmbergrisPower, PowerAmount::Endless, _) => {
            unimplemented!("an endless Ambergris")
        }
        _ => None,
    }
}

/// What a temporary power only this build has lends, and `IsPositive`.
///
/// `HyperbeamFocusDownPower`, a `TemporaryFocusPower` that is not positive.
pub(crate) fn own_lends(power: Power) -> Option<(Power, bool)> {
    match power {
        Power::HyperbeamFocusDownPower => Some((Power::FocusPower, false)),
        Power::FadePower => Some((Power::DexterityPower, true)),
        _ => None,
    }
}
