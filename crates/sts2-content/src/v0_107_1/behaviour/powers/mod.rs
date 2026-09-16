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
use super::super::generated::cards::{Cards, TokenCard};
use super::super::generated::monsters::Monster;
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

/// What one card played adds to what `SlowPower`'s owner takes.
///
/// The game works this out in decimal, where a tenth is exact. A binary float
/// holds a tenth a hair high. So every product lands at or above the decimal
/// one, and truncates to the same whole number.
const SLOW_PER_CARD: f64 = 0.1;

/// Apply a value-producing hook for one power.
///
/// `TemporaryStrengthPower`, which is Strength that gives itself back.
///
/// `BeforeApplied` lands the Strength, signed by `IsPositive`, before this
/// power attaches. `AfterSideTurnEnd` for its owner's own side takes this
/// power off and hands the same Strength back.
///
/// `positive` is `IsPositive`. Crush Under's is false, so it lands negative
/// Strength and returns it.
fn temporary_strength(
    positive: bool,
    amount: PowerAmount<Number>,
    owner: CombatSide,
    hook: sts2_core::EventHook<V>,
) -> Vec<sts2_core::HookEffect<V>> {
    use sts2_core::{EventHook, HookEffect};

    // `Sign`, which every amount this power moves is multiplied by.
    let signed = |amount: Number| match positive {
        true => amount,
        false => -amount,
    };
    match (amount, hook) {
        (_, EventHook::BeforeApplied { landing }) => {
            let PowerAmount::Counted(landing) = landing else {
                unimplemented!("an endless temporary Strength landing")
            };
            vec![HookEffect::ApplyToOwner(
                Power::StrengthPower,
                PowerAmount::Counted(signed(landing)),
            )]
        }
        (PowerAmount::Counted(amount), EventHook::SideTurnEnd { side }) if side == owner => {
            // `PowerCmd.Remove(this)` and then the Strength back, which is the
            // order the game hands them over in.
            vec![
                HookEffect::Remove,
                HookEffect::ApplyToOwner(
                    Power::StrengthPower,
                    PowerAmount::Counted(-signed(amount)),
                ),
            ]
        }
        (PowerAmount::Endless, EventHook::SideTurnEnd { .. }) => {
            unimplemented!("an endless temporary Strength")
        }
        _ => Vec::new(),
    }
}

/// `amount` is the stack the creature carries this power as, which is what an
/// override reads off itself.
pub(crate) fn modify_hook(
    power: Power,
    amount: PowerAmount<Number>,
    _owner: CombatSide,
    counted: Number,
    hook: sts2_core::ModifyHook<V>,
) -> Modify<Number> {
    let one: Number = num_traits::one();
    match (power, amount, hook) {
        // `SlowPower.ModifyDamageMultiplicative`, which reads the cards played
        // since its owner's turn began and takes a tenth more for each.
        (
            Power::SlowPower,
            _,
            ModifyHook::Damage {
                holding: Holding::Target,
            },
        ) => Modify::Scaled(1.0 + SLOW_PER_CARD * f64::from(counted)),
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
            | Power::EscapeArtistPower
            | Power::TerritorialPower
            | Power::InfestedPower
            | Power::IllusionPower
            | Power::MinionPower
            | Power::DexterityPower
            | Power::SlowPower
            | Power::SlipperyPower
            | Power::BurrowedPower
            | Power::SwipePower
            | Power::SwordSagePower
            | Power::TenderPower,
            _,
            ModifyHook::Damage { .. },
        ) => Modify::Nothing,
        // `SlipperyPower.ModifyHpLostAfterOsty`, which hands back one however
        // much its owner was about to lose. A hit that the block took whole
        // loses nothing, and this leaves that where it is.
        (Power::SlipperyPower, _, ModifyHook::HpLost) => Modify::AtMost(one),
        // No other ported power reads what a hit takes off the hit points.
        (_, _, ModifyHook::HpLost) => Modify::Nothing,
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
        // A `TemporaryStrengthPower` moves no number itself. The Strength it
        // lands is what a hit reads.
        (Power::CrushUnderPower, _, _) => Modify::Nothing,
        (
            Power::VulnerablePower
            | Power::WeakPower
            | Power::ShrinkPower
            | Power::BlockNextTurnPower
            | Power::EnergyNextTurnPower
            | Power::StrengthPower
            | Power::TerritorialPower
            | Power::IllusionPower
            | Power::MinionPower
            | Power::SlowPower
            | Power::SlipperyPower
            | Power::BurrowedPower
            | Power::SwipePower
            | Power::SwordSagePower
            | Power::TenderPower,
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
    counted: Number,
    hook: sts2_core::EventHook<V>,
) -> Vec<sts2_core::HookEffect<V>> {
    use super::super::generated::monsters::{IntentKind, MoveId};
    use sts2_core::{EventHook, HookEffect};

    // `TemporaryStrengthPower` asks for two things when a turn ends, and the
    // match below spells one. Its subclasses answer there instead.
    if let Power::CrushUnderPower = power {
        return temporary_strength(false, amount, owner, hook);
    }

    let one: Number = num_traits::one();

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
            Power::EscapeArtistPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnEnd { side },
        ) if side == owner && amount > one => Some(HookEffect::Decrement),
        (Power::EscapeArtistPower, PowerAmount::Endless, EventHook::SideTurnEnd { .. }) => {
            unimplemented!("an endless Escape Artist")
        }
        (Power::EscapeArtistPower, _, EventHook::SideTurnEnd { .. }) => None,
        (
            Power::VulnerablePower
            | Power::WeakPower
            | Power::EnergyNextTurnPower
            | Power::BlockNextTurnPower
            | Power::StrengthPower
            | Power::InfestedPower
            | Power::IllusionPower
            | Power::MinionPower
            // `PowerStackType.Counter` with no `AfterSideTurnEnd`, so they
            // stay for the fight.
            | Power::DexterityPower
            | Power::SlowPower
            | Power::SlipperyPower
            | Power::BurrowedPower
            | Power::SwipePower
            | Power::SwordSagePower,
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
            | Power::DexterityPower
            | Power::SwordSagePower
            | Power::TenderPower,
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
        (
            Power::InfestedPower,
            PowerAmount::Counted(amount),
            EventHook::AfterDeath,
        ) => Some(HookEffect::SummonAfterOwnerDeath {
            monster: Monster::Wriggler,
            count: amount,
            opening: sts2_core::NextMove {
                id: MoveId::SpawnedMove,
                intents: [sts2_core::Intent {
                    kind: IntentKind::Stun,
                    attack: None,
                }]
                .into_iter()
                .collect(),
            },
        }),
        (Power::InfestedPower, PowerAmount::Endless, EventHook::AfterDeath) => {
            unimplemented!("an endless Infested power")
        }
        // `TenderPower.AfterCardPlayed`, which counts only its owner's own
        // cards. A fight here has one player, so every card played is theirs.
        // Each one takes a point of Strength and of Dexterity, and the count
        // is what gives them back.
        (Power::TenderPower, _, EventHook::CardPlayed { .. }) => {
            return vec![
                HookEffect::Count(one),
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(-one)),
                HookEffect::ApplyToOwner(Power::DexterityPower, PowerAmount::Counted(-one)),
            ];
        }
        // `TenderPower.AfterSideTurnEnd` for the side its owner is among the
        // participants of. Everything taken this turn comes back, and the
        // count starts again.
        (Power::TenderPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            return vec![
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(counted)),
                HookEffect::ApplyToOwner(Power::DexterityPower, PowerAmount::Counted(counted)),
                HookEffect::ClearCount,
            ];
        }
        (Power::TenderPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `SlowPower.AfterCardPlayed` counts every card anyone plays, and its
        // `AfterSideTurnStart` forgets the count when the side its owner is
        // among takes its turn.
        (Power::SlowPower, _, EventHook::CardPlayed { .. }) => Some(HookEffect::Count(one)),
        (Power::SlowPower, _, EventHook::SideTurnStart { side, .. }) if side == owner => {
            Some(HookEffect::ClearCount)
        }
        (Power::SlowPower, _, EventHook::SideTurnStart { .. }) => None,
        // `SlipperyPower.AfterDamageReceived`, which spends an instance on
        // every hit that reaches its owner's hit points.
        (Power::SlipperyPower, _, EventHook::DamageReceived { unblocked })
            if unblocked >= one =>
        {
            Some(HookEffect::Decrement)
        }
        // `SwordSagePower.AfterPowerAmountChanged`, which answers only its own
        // kind. Every Sovereign Blade its owner holds gets one more replay per
        // point applied.
        //
        // A fight here has one player, so a Sword Sage instance and the cards
        // it writes on belong to the same player, which is what
        // `TryAddReplays` checks.
        (
            Power::SwordSagePower,
            _,
            EventHook::PowerAmountChanged {
                power: Power::SwordSagePower,
                delta,
            },
        ) => Some(HookEffect::AddReplays {
            to: sts2_core::Replaying::EveryCopyOf(Cards::Token(TokenCard::SovereignBlade)),
            amount: delta,
        }),
        // `SwordSagePower.AfterCardEnteredCombat`, which gives a blade that
        // joins later the replays the ones already here were given.
        (
            Power::SwordSagePower,
            PowerAmount::Counted(amount),
            EventHook::CardEnteredCombat {
                at,
                card: Cards::Token(TokenCard::SovereignBlade),
            },
        ) => Some(HookEffect::AddReplays {
            to: sts2_core::Replaying::One(at),
            amount,
        }),
        (Power::SwordSagePower, PowerAmount::Endless, EventHook::CardEnteredCombat { .. }) => {
            unimplemented!("an endless Sword Sage")
        }
        // Each of these is an override on an empty base, so a power that does
        // not have one does nothing when the fight raises it.
        (
            _,
            _,
            EventHook::BlockCleared
            | EventHook::AfterDeath
            | EventHook::BeforeApplied { .. }
            | EventHook::CardPlayed { .. }
            | EventHook::SideTurnStart { .. }
            | EventHook::DamageReceived { .. }
            | EventHook::PowerAmountChanged { .. }
            | EventHook::CardEnteredCombat { .. },
        ) => None,
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    };
    effect.into_iter().collect()
}
