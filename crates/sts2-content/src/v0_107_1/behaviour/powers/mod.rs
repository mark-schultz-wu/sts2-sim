//! What a power does, ported.
//!
//! One function per hook, matching on the power the way `PowerModel`'s
//! subclasses override one. A power gets an arm when something in a fight can
//! apply it.
//!
//! A power nobody has ported panics naming itself. One is only in a fight
//! because ported behaviour put it there. So an unported power reaching here
//! is a gap in the port, and not a state a run can reach.

use sts2_core::{Holding, Modify, ModifyHook, PowerAmount};

use super::super::V0_107_1 as V;
use super::super::generated::powers::Power;
use super::super::generated::runstate::CombatSide;
type Number = <V as sts2_core::ContentVersion>::Number;

/// `VulnerablePower`'s `DamageIncrease`.
const DAMAGE_INCREASE: f64 = 1.5;

/// `WeakPower`'s `DamageDecrease`.
const DAMAGE_DECREASE: f64 = 0.75;

/// `ShrinkPower`'s own `DamageDecrease`, which the power stores as the
/// percentage taken off rather than as what is left.
const SHRUNK: f64 = 1.0 - 30.0 / 100.0;

/// Apply a value-producing hook for one power.
///
/// `amount` is the stack the creature carries this power as, which is what an
/// override reads off itself.
pub(crate) fn modify_hook(
    power: Power,
    amount: PowerAmount<Number>,
    _owner: CombatSide,
    hook: sts2_core::ModifyHook<V>,
) -> Modify<Number> {
    match (power, amount, hook) {
        (
            Power::StrengthPower,
            PowerAmount::Counted(amount),
            ModifyHook::Damage {
                holding: Holding::Dealer,
            },
        ) => Modify::Added(amount),
        (
            Power::StrengthPower,
            PowerAmount::Endless,
            ModifyHook::Damage {
                holding: Holding::Dealer,
            },
        ) => unimplemented!("an endless Strength"),
        (Power::StrengthPower, _, ModifyHook::Damage { .. }) => Modify::Nothing,
        (
            Power::VulnerablePower,
            _,
            ModifyHook::Damage {
                holding: Holding::Target,
            },
        ) => Modify::Scaled(DAMAGE_INCREASE),
        (
            Power::WeakPower,
            _,
            ModifyHook::Damage {
                holding: Holding::Dealer,
            },
        ) => Modify::Scaled(DAMAGE_DECREASE),
        (
            Power::ShrinkPower,
            _,
            ModifyHook::Damage {
                holding: Holding::Dealer,
            },
        ) => Modify::Scaled(SHRUNK),
        (
            Power::VulnerablePower
            | Power::WeakPower
            | Power::ShrinkPower
            | Power::BlockNextTurnPower
            | Power::EnergyNextTurnPower
            | Power::TerritorialPower
            | Power::IllusionPower
            | Power::MinionPower
            | Power::DexterityPower,
            _,
            ModifyHook::Damage { .. },
        ) => Modify::Nothing,
        // `ModifyBlockAdditive`, which pays only a powered card's block or a
        // monster move's. Every block this hook is asked about is one of
        // those, so the condition is the call site and not an arm here. An
        // unpowered source, such as a relic's, will need the props that say
        // so.
        (Power::DexterityPower, PowerAmount::Counted(amount), ModifyHook::Block) => {
            Modify::Added(amount)
        }
        (Power::DexterityPower, PowerAmount::Endless, ModifyHook::Block) => {
            unimplemented!("an endless Dexterity")
        }
        (
            Power::VulnerablePower
            | Power::WeakPower
            | Power::ShrinkPower
            | Power::BlockNextTurnPower
            | Power::EnergyNextTurnPower
            | Power::StrengthPower
            | Power::TerritorialPower
            | Power::IllusionPower
            | Power::MinionPower,
            _,
            ModifyHook::Block,
        ) => Modify::Nothing,
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    }
}

/// Answer a yes-or-no hook for one power.
pub(crate) fn decide_hook(
    power: Power,
    hook: sts2_core::DecisionHook,
) -> sts2_core::HookResponse<bool> {
    match (power, hook) {
        (Power::IllusionPower, sts2_core::DecisionHook::ShouldRemoveCreatureAfterDeath) => {
            sts2_core::HookResponse::Decision(false)
        }
        (Power::MinionPower, sts2_core::DecisionHook::ShouldCountForCombatEnd) => {
            sts2_core::HookResponse::Decision(false)
        }
        _ => sts2_core::HookResponse::DoesNotImplement,
    }
}

/// Dispatch one of the event hooks the simulator currently supports.
///
/// `amount` is the stack the creature carries this power as.
pub(crate) fn event_hook(
    power: Power,
    amount: PowerAmount<Number>,
    owner: CombatSide,
    hook: sts2_core::EventHook<V>,
) -> Vec<sts2_core::HookEffect<V>> {
    use super::super::generated::monsters::{IntentKind, MoveId};
    use sts2_core::{EventHook, HookEffect};

    let effect = match (power, amount, hook) {
        (
            Power::VulnerablePower | Power::WeakPower,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => Some(HookEffect::TickDownDuration),
        (Power::ShrinkPower, _, EventHook::SideTurnEnd { .. }) => {
            unimplemented!("what a counted Shrink does when a turn ends")
        }
        // `AfterSideTurnEnd` for the side its owner is among the participants
        // of, which is its owner's own side.
        (
            Power::TerritorialPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnEnd { side },
        ) if side == owner => Some(HookEffect::ApplyToOwner(
            Power::StrengthPower,
            PowerAmount::Counted(amount),
        )),
        (Power::TerritorialPower, PowerAmount::Endless, EventHook::SideTurnEnd { .. }) => {
            unimplemented!("an endless Territorial")
        }
        (Power::TerritorialPower, _, EventHook::SideTurnEnd { .. }) => None,
        (
            Power::VulnerablePower
            | Power::WeakPower
            | Power::EnergyNextTurnPower
            | Power::BlockNextTurnPower
            | Power::StrengthPower
            | Power::IllusionPower
            | Power::MinionPower
            // `PowerStackType.Counter` with no `AfterSideTurnEnd`, so it
            // stays for the fight.
            | Power::DexterityPower,
            _,
            EventHook::SideTurnEnd { .. },
        ) => None,
        (Power::EnergyNextTurnPower, PowerAmount::Counted(amount), EventHook::EnergyReset) => {
            Some(HookEffect::GainEnergyAndRemove(amount))
        }
        (Power::EnergyNextTurnPower, PowerAmount::Endless, EventHook::EnergyReset) => {
            unimplemented!("an endless EnergyNextTurnPower handing over energy")
        }
        (
            Power::VulnerablePower
            | Power::WeakPower
            | Power::ShrinkPower
            | Power::BlockNextTurnPower
            | Power::StrengthPower
            | Power::IllusionPower
            | Power::MinionPower
            | Power::DexterityPower,
            _,
            EventHook::EnergyReset,
        ) => None,
        (Power::BlockNextTurnPower, PowerAmount::Counted(amount), EventHook::BlockCleared) => {
            Some(HookEffect::GainBlockAndRemove(amount))
        }
        (Power::IllusionPower, _, EventHook::AfterDeath) => {
            Some(HookEffect::InstallMove(sts2_core::NextMove {
                id: MoveId::ReviveMove,
                intents: [sts2_core::Intent {
                    kind: IntentKind::Heal,
                    attack: None,
                }]
                .into_iter()
                .collect(),
            }))
        }
        (_, _, EventHook::BlockCleared | EventHook::AfterDeath) => None,
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    };
    effect.into_iter().collect()
}
