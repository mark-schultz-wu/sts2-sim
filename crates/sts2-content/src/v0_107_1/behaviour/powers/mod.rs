//! What a power does, ported.
//!
//! One function per hook, matching on the power the way `PowerModel`'s
//! subclasses override one. A power gets an arm when something in a fight can
//! apply it.
//!
//! A power nobody has ported panics naming itself. One is only in a fight
//! because ported behaviour put it there. So an unported power reaching here
//! is a gap in the port, and not a state a run can reach.

use std::sync::LazyLock;

use sts2_core::{CardTable as _, Modify, ModifyHook, PowerAmount, PowerModel as _};

use super::super::V0_107_1 as V;
use super::super::generated::cards::{CardKeyword, CardTag, Cards, TokenCard};
use super::super::generated::monsters::{IntentKind, Monster, MoveId};
use super::super::generated::powers::Power;
use super::super::generated::runstate::CombatSide;
type Number = <V as sts2_core::ContentVersion>::Number;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Stateless(Power),
    Slow {
        counted: Number,
    },
    Tender {
        counted: Number,
    },
    Imbalanced {
        off_balance: bool,
    },
    /// `IllusionPower`'s `isReviving`.
    Illusion {
        reviving: bool,
    },
    /// `ReattachPower`'s `isReviving`.
    Reattach {
        reviving: bool,
    },
    /// `SwipePower.StolenCard`.
    Swipe {
        stolen: Option<sts2_core::CardIdent<V>>,
    },
    /// `VigorPower`'s `commandToModify`, which stays bound once the command
    /// is over.
    Vigor {
        bound: Option<Bound>,
    },
}

/// The attack command `VigorPower` bound to, and what it held as that began.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bound {
    command: sts2_core::AttackId,
    /// The command's `ModelSource`: the card, or nothing for a monster's.
    source: Option<sts2_core::CardId<V>>,
    /// `amountWhenAttackStarted`.
    amount: Number,
}

impl sts2_core::PowerModel<V> for State {
    fn new(power: Power) -> Self {
        match power {
            Power::SlowPower => Self::Slow { counted: 0 },
            Power::TenderPower => Self::Tender { counted: 0 },
            Power::ImbalancedPower => Self::Imbalanced { off_balance: false },
            Power::IllusionPower => Self::Illusion { reviving: false },
            Power::ReattachPower => Self::Reattach { reviving: false },
            Power::SwipePower => Self::Swipe { stolen: None },
            Power::VigorPower => Self::Vigor { bound: None },
            power => Self::Stateless(power),
        }
    }

    fn power(&self) -> Power {
        match self {
            Self::Stateless(power) => *power,
            Self::Slow { .. } => Power::SlowPower,
            Self::Tender { .. } => Power::TenderPower,
            Self::Imbalanced { .. } => Power::ImbalancedPower,
            Self::Illusion { .. } => Power::IllusionPower,
            Self::Reattach { .. } => Power::ReattachPower,
            Self::Swipe { .. } => Power::SwipePower,
            Self::Vigor { .. } => Power::VigorPower,
        }
    }
}

impl State {
    fn counted(&self) -> Number {
        match self {
            Self::Slow { counted } | Self::Tender { counted } => *counted,
            _ => 0,
        }
    }

    fn count(&mut self, amount: Number) {
        match self {
            Self::Slow { counted } | Self::Tender { counted } => *counted += amount,
            state => unreachable!("counted state for {:?}", state.power()),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Slow { counted } | Self::Tender { counted } => *counted = 0,
            state => unreachable!("counted state for {:?}", state.power()),
        }
    }

    fn set_off_balance(&mut self, off_balance: bool) {
        let Self::Imbalanced {
            off_balance: current,
        } = self
        else {
            unreachable!("off-balance state for {:?}", self.power())
        };
        *current = off_balance;
    }

    pub(crate) fn is_off_balance(&self) -> bool {
        matches!(self, Self::Imbalanced { off_balance: true })
    }

    /// `isReviving`, which a reviving power sets as its owner dies and clears
    /// as it brings the owner back.
    pub(crate) fn set_reviving(&mut self, to: bool) {
        match self {
            Self::Illusion { reviving } | Self::Reattach { reviving } => *reviving = to,
            state => unreachable!("reviving state for {:?}", state.power()),
        }
    }

    fn reviving(&self) -> bool {
        matches!(
            self,
            Self::Illusion { reviving: true } | Self::Reattach { reviving: true }
        )
    }

    /// The command `VigorPower` is bound to, once it is.
    fn bound(&self) -> Option<Bound> {
        match self {
            Self::Vigor { bound } => *bound,
            state => unreachable!("bound state for {:?}", state.power()),
        }
    }

    /// `SwipePower.Steal`, which remembers the card taken.
    pub(crate) fn steal(&mut self, card: sts2_core::CardIdent<V>) {
        let Self::Swipe { stolen } = self else {
            unreachable!("stolen state for {:?}", self.power())
        };
        *stolen = Some(card);
    }
}

/// `CreatureCmd.Stun`'s own state, which advertises a stunned turn.
fn stunned(then: Option<u8>) -> sts2_core::HookEffect<V> {
    sts2_core::HookEffect::StunOwner {
        stun: &STUNNED,
        then,
    }
}

/// A state a power builds: one move, drawing one intent with no number.
fn built(id: MoveId, kind: IntentKind) -> sts2_core::NextMove<V> {
    sts2_core::NextMove {
        id,
        intents: [sts2_core::Intent { kind, attack: None }]
            .into_iter()
            .collect(),
    }
}

/// `Creature.Stun`'s state.
static STUNNED: LazyLock<sts2_core::NextMove<V>> =
    LazyLock::new(|| built(MoveId::Stunned, IntentKind::Stun));

/// `IllusionPower`'s revive, a heal.
static REVIVE: LazyLock<sts2_core::NextMove<V>> =
    LazyLock::new(|| built(MoveId::ReviveMove, IntentKind::Heal));

/// The stunned turn a Wriggler that `InfestedPower` spawns opens on.
static SPAWNED: LazyLock<sts2_core::NextMove<V>> =
    LazyLock::new(|| built(MoveId::SpawnedMove, IntentKind::Stun));

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

/// `TemporaryStrengthPower`, which is Strength that gives itself back.
///
/// `BeforeApplied` lands the Strength, signed by `IsPositive`, before this
/// power attaches. `AfterSideTurnEnd` for its owner's own side takes this
/// power off and hands the same Strength back.
///
/// `positive` is `IsPositive`. Crush Under's is false, so it lands negative
/// Strength and returns it.
fn temporary_strength(
    power: Power,
    positive: bool,
    amount: PowerAmount<Number>,
    held: Held,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
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
            asked.push(HookEffect::ApplyToOwner(
                Power::StrengthPower,
                PowerAmount::Counted(signed(landing)),
            ))
        }
        (PowerAmount::Counted(amount), EventHook::SideTurnEnd { side }) if side == held.owner => {
            // `PowerCmd.Remove(this)` and then the Strength back, which is the
            // order the game hands them over in.
            asked.extend([
                HookEffect::Remove,
                HookEffect::ApplyToOwner(
                    Power::StrengthPower,
                    PowerAmount::Counted(-signed(amount)),
                ),
            ])
        }
        (PowerAmount::Endless, EventHook::SideTurnEnd { .. }) => {
            unimplemented!("an endless temporary Strength")
        }
        (_, EventHook::SideTurnEnd { .. }) => {}
        // `AfterPowerAmountChanged` on its own instance. The instance being
        // put on is `BeforeApplied`'s, and changes by the whole of its
        // amount, which the override passes over.
        (
            PowerAmount::Counted(total),
            EventHook::PowerAmountChanged {
                creature,
                power: changed,
                delta,
            },
        ) if creature == held.holder && changed == power && delta != total => {
            unimplemented!("a temporary Strength stacked onto its own instance")
        }
        (_, EventHook::PowerAmountChanged { .. }) => {}
        // Asked of every power, and not ones this class overrides.
        (_, EventHook::MonsterMovePerformed { .. } | EventHook::Removed | EventHook::Applied) => {}
        (_, hook) => unimplemented!("what a temporary Strength does for {hook:?}"),
    }
}

/// Apply a value-producing hook for one power.
///
/// `amount` is the stack the creature carries this power as, which is what an
/// override reads off itself. Each override reads whether the creature the
/// hook names is its own holder.
///
/// None of these reads `ValueProp`. Every hit and every block a fight asks
/// about here is a powered attack or a powered card's, which is what each
/// override pays for.
pub(crate) fn modify_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held,
    state: &State,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::ModifyHook<V>,
) -> Modify<Number> {
    let one: Number = num_traits::one();
    debug_assert_eq!(state.power(), power);
    let counted = state.counted();
    let holder = held.holder;
    match (power, amount, hook) {
        // `SlowPower.ModifyDamageMultiplicative`, which reads the cards played
        // since its owner's turn began and takes a tenth more for each.
        (Power::SlowPower, _, ModifyHook::DamageMultiplicative { target, .. })
            if target == Some(holder) =>
        {
            Modify::Scaled(1.0 + SLOW_PER_CARD * f64::from(counted))
        }
        (Power::VulnerablePower, _, ModifyHook::DamageMultiplicative { target, .. })
            if target == Some(holder) =>
        {
            Modify::Scaled(DAMAGE_INCREASE)
        }
        (Power::WeakPower, _, ModifyHook::DamageMultiplicative { dealer, .. })
            if dealer == holder =>
        {
            Modify::Scaled(DAMAGE_DECREASE)
        }
        (Power::ShrinkPower, _, ModifyHook::DamageMultiplicative { dealer, .. })
            if dealer == holder =>
        {
            Modify::Scaled(SHRUNK)
        }
        (
            Power::StrengthPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive { dealer, .. },
        ) if dealer == holder => Modify::Added(amount),
        (Power::StrengthPower, PowerAmount::Endless, ModifyHook::DamageAdditive { dealer, .. })
            if dealer == holder =>
        {
            unimplemented!("an endless Strength")
        }
        // `VigorPower.ModifyDamageAdditive`. Once bound, it pays the card
        // that started the command and no other card. A hit from no card
        // is paid either way. The command it bound was its own owner's, so
        // the attacker it names is this dealer.
        (
            Power::VigorPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive { dealer, card, .. },
        ) if dealer == holder => {
            let other = |bound: Bound| card.is_some_and(|(id, _)| Some(id) != bound.source);
            match state.bound().is_some_and(other) {
                true => Modify::Nothing,
                false => Modify::Added(amount),
            }
        }
        // `AccuracyPower.ModifyDamageAdditive`, for a Shiv its owner plays.
        (
            Power::AccuracyPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                card: Some((_, card)),
                ..
            },
        ) if dealer == holder && card.tagged(CardTag::Shiv) => Modify::Added(amount),
        // `PhantomBladesPower.ModifyDamageAdditive`: the first Shiv its owner
        // plays in a turn, and no other. A play counts once it has finished,
        // so the Shiv being played is not one of them. A fight here has one
        // player, so every play is its owner's.
        (
            Power::PhantomBladesPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                card: Some((_, card)),
                ..
            },
        ) if dealer == holder
            && card.tagged(CardTag::Shiv)
            && !history
                .finished_this_turn()
                .any(|played| played.tagged(CardTag::Shiv)) =>
        {
            Modify::Added(amount)
        }
        (
            Power::VigorPower | Power::AccuracyPower | Power::PhantomBladesPower,
            PowerAmount::Endless,
            ModifyHook::DamageAdditive { dealer, .. },
        ) if dealer == holder => unimplemented!("an endless {power:?}"),
        // `SlipperyPower.ModifyHpLostAfterOsty`, which hands back one however
        // much its owner was about to lose. A hit that the block took whole
        // loses nothing, and this leaves that where it is.
        (Power::SlipperyPower, _, ModifyHook::HpLostAfterOsty { target }) if target == holder => {
            Modify::AtMost(one)
        }
        // `DexterityPower.ModifyBlockAdditive` pays block from a card its
        // owner plays, or block its owner gains otherwise. A fight here has
        // one player, whose cards give the player block, so both are the
        // block its holder is gaining.
        (
            Power::DexterityPower,
            PowerAmount::Counted(amount),
            ModifyHook::BlockAdditive { target },
        ) if target == holder => Modify::Added(amount),
        (Power::DexterityPower, PowerAmount::Endless, ModifyHook::BlockAdditive { target })
            if target == holder =>
        {
            unimplemented!("an endless Dexterity")
        }
        // `MachineLearningPower.ModifyHandDraw`: its stack more cards for its
        // owner.
        (
            Power::MachineLearningPower,
            PowerAmount::Counted(amount),
            ModifyHook::HandDraw { player, .. },
        ) if player == holder => Modify::Added(amount),
        // `DrawCardsNextTurnPower.ModifyHandDraw`, which waits for the stack
        // to have been there as its owner's turn began.
        (
            Power::DrawCardsNextTurnPower,
            PowerAmount::Counted(amount),
            ModifyHook::HandDraw { player, .. },
        ) if player == holder && held.on_turn_start != PowerAmount::Counted(0) => {
            Modify::Added(amount)
        }
        (
            Power::MachineLearningPower | Power::DrawCardsNextTurnPower,
            PowerAmount::Endless,
            ModifyHook::HandDraw { .. },
        ) => unimplemented!("an endless {power:?}"),
        // The overrides above, for a number some other creature is party to.
        (
            Power::SlowPower | Power::VulnerablePower | Power::WeakPower | Power::ShrinkPower,
            _,
            ModifyHook::DamageMultiplicative { .. },
        )
        | (
            Power::StrengthPower
            | Power::VigorPower
            | Power::AccuracyPower
            | Power::PhantomBladesPower,
            _,
            ModifyHook::DamageAdditive { .. },
        )
        | (Power::SlipperyPower, _, ModifyHook::HpLostAfterOsty { .. })
        | (Power::DexterityPower, _, ModifyHook::BlockAdditive { .. })
        | (
            Power::MachineLearningPower | Power::DrawCardsNextTurnPower,
            _,
            ModifyHook::HandDraw { .. },
        ) => Modify::Nothing,
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    }
}

/// Answer a yes-or-no hook for one power.
///
/// A hook the fight raises reaches only a power that overrides it. A question
/// the game asks every power about itself reaches all of them, and one with
/// no override does not implement it.
pub(crate) fn decide_hook(
    power: Power,
    held: Held,
    state: &State,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::{DecisionHook, HookResponse};

    match (power, hook) {
        // `ShouldCreatureBeRemovedFromCombatAfterDeath`, which keeps its own
        // holder and no other.
        (
            Power::IllusionPower | Power::ReattachPower | Power::DieForYouPower,
            DecisionHook::ShouldRemoveCreatureAfterDeath { creature },
        ) => HookResponse::Decision(creature != held.holder),
        // `OwnerIsSecondaryEnemy`.
        (Power::MinionPower, DecisionHook::ShouldCountForCombatEnd) => {
            HookResponse::Decision(false)
        }
        // `ShouldPowerBeRemovedAfterOwnerDeath`. A power that keeps its owner
        // in the fight keeps itself, or the creature would come back without
        // whatever brought it back.
        (
            Power::ReattachPower | Power::MinionPower | Power::DieForYouPower,
            DecisionHook::ShouldRemoveSelfAfterOwnerDeath,
        ) => HookResponse::Decision(false),
        // `IllusionPower.ShouldPowerBeRemovedOnDeath`, which reads the power
        // and not whose it is: a debuff goes, and anything else stays. An
        // `ITemporaryPower` is a debuff that goes regardless, and none is
        // ported, so a debuff here is a debuff.
        (Power::IllusionPower, DecisionHook::ShouldRemovePowerOnDeath { power, .. }) => {
            use sts2_core::PowerTable as _;

            let debuff = power.debuff().unwrap_or_else(|| {
                unimplemented!("whether {power:?} is a debuff on a dying creature")
            });
            HookResponse::Decision(debuff)
        }
        // `ShouldAllowHitting`, which refuses its own holder while it revives.
        (
            Power::IllusionPower | Power::ReattachPower,
            DecisionHook::ShouldAllowHitting { creature, .. },
        ) => HookResponse::Decision(creature != held.holder || !state.reviving()),
        // `DieForYouPower.ShouldAllowHitting`, which refuses any creature that
        // is dead, whoever holds it.
        (Power::DieForYouPower, DecisionHook::ShouldAllowHitting { alive, .. }) => {
            HookResponse::Decision(alive)
        }
        // `ModifyUnblockedDamageTarget`, which takes a powered attack on the
        // pet's owner onto the pet while it lives.
        (
            Power::DieForYouPower,
            DecisionHook::TakesUnblockedDamage {
                target,
                powered,
                pet_of,
                alive,
            },
        ) => HookResponse::Decision(pet_of == Some(target) && alive && powered),
        // `RetainHandPower.ShouldFlush`, which keeps its own holder's hand.
        (Power::RetainHandPower, DecisionHook::ShouldFlush { player }) => {
            HookResponse::Decision(player != held.holder)
        }
        // `BurrowedPower.ShouldClearBlock`, which keeps its own holder's.
        (Power::BurrowedPower, DecisionHook::ShouldClearBlock { creature }) => {
            HookResponse::Decision(creature != held.holder)
        }
        // `InfestedPower.ShouldStopCombatFromEnding`, for as long as it is on
        // a creature in the fight.
        (Power::InfestedPower, DecisionHook::ShouldStopCombatFromEnding) => {
            HookResponse::Decision(true)
        }
        (power, hook) if hook.model_hook().is_some() => {
            unimplemented!("what {power:?} decides for {hook:?}")
        }
        _ => HookResponse::DoesNotImplement,
    }
}

/// Whose a power is, as a hook reaching it reads that.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Held {
    /// The side of the creature carrying it.
    pub(crate) owner: CombatSide,
    /// The creature carrying it, which is `PowerModel.Owner`.
    pub(crate) holder: sts2_core::CombatId,
    /// `PowerModel.Applier`.
    pub(crate) applier: Option<sts2_core::CombatId>,
    /// `Owner.Monster`, which is nothing for a power on the player.
    pub(crate) monster: Option<Monster>,
    /// `PowerModel.AmountOnTurnStart`.
    pub(crate) on_turn_start: PowerAmount<Number>,
}

/// Dispatch one of the event hooks the simulator currently supports.
///
/// `amount` is the stack the creature carries this power as.
pub(crate) fn event_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held,
    state: &mut State,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use sts2_core::{EventHook, HookEffect};

    debug_assert_eq!(state.power(), power);
    let counted = state.counted();
    let Held {
        owner,
        holder,
        applier,
        monster,
        ..
    } = held;

    // `TemporaryStrengthPower` asks for two things when a turn ends, and the
    // match below spells one. Its subclasses answer there instead.
    if let Power::CrushUnderPower = power {
        return temporary_strength(power, false, amount, held, hook, asked);
    }

    let one: Number = num_traits::one();

    let effect = match (power, amount, hook) {
        (
            Power::ImbalancedPower,
            _,
            EventHook::DamageGiven {
                dealer,
                fully_blocked: true,
            },
        ) if dealer == holder => {
            state.set_off_balance(true);
            asked.push(stunned(None));
            return;
        }
        (
            Power::ImbalancedPower,
            _,
            EventHook::MonsterMovePerformed {
                move_id: MoveId::Stunned,
            },
        ) => {
            state.set_off_balance(false);
            None
        }
        (
            Power::VulnerablePower | Power::WeakPower,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => Some(HookEffect::TickDownDuration),
        // `ShrinkPower.AfterSideTurnEnd`, for the side its owner is among the
        // participants of. An infinite Shrink, the one the game gives a
        // negative amount, lasts the fight.
        (Power::ShrinkPower, PowerAmount::Counted(_), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::ShrinkPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ShrinkPower.AfterDeath`, which goes with the creature that applied
        // it.
        (Power::ShrinkPower, _, EventHook::AfterDeath { creature, .. })
            if applier == Some(creature) =>
        {
            Some(HookEffect::Remove)
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
            | Power::SwordSagePower
            | Power::ImbalancedPower
            // `PlatingPower` re-plates on `BeforeSideTurnEndEarly`, which is
            // its own hook and not this one.
            | Power::PlatingPower
            | Power::FurnacePower
            | Power::ReattachPower,
            _,
            EventHook::SideTurnEnd { .. },
        ) => None,
        (
            Power::EnergyNextTurnPower,
            PowerAmount::Counted(amount),
            EventHook::EnergyReset { player },
        ) if player == holder => {
            Some(HookEffect::GainEnergyAndRemove(amount))
        }
        (Power::EnergyNextTurnPower, PowerAmount::Endless, EventHook::EnergyReset { player })
            if player == holder =>
        {
            unimplemented!("an endless EnergyNextTurnPower handing over energy")
        }
        (
            Power::BlockNextTurnPower,
            PowerAmount::Counted(amount),
            EventHook::BlockCleared { creature },
        ) if creature == holder => {
            Some(HookEffect::GainBlockAndRemove(amount))
        }
        // `ReattachPower.AfterDeath`, which sends the segment to the dead move
        // its own graph holds. The reattach that follows it is the graph's, so
        // nothing here installs one.
        //
        // A segment whose row is otherwise dead is left where it lies. The
        // fight is over by then, and the game spends that branch on the fade
        // every segment plays.
        //
        // `isReviving` is set too. The segment that dies with the rest of its
        // row sets nothing in the game, and the fight ends with it.
        (Power::ReattachPower, _, EventHook::AfterDeath { creature, .. }) if creature == holder => {
            state.set_reviving(true);
            Some(HookEffect::EnterMove(MoveId::DeadMove))
        }
        (Power::IllusionPower, _, EventHook::AfterDeath { creature, .. }) if creature == holder => {
            state.set_reviving(true);
            Some(HookEffect::InstallMove(&REVIVE))
        }
        (
            Power::InfestedPower,
            PowerAmount::Counted(amount),
            EventHook::AfterDeath { creature, .. },
        ) if creature == holder => Some(HookEffect::SummonAfterOwnerDeath {
            monster: Monster::Wriggler,
            count: amount,
            opening: &SPAWNED,
        }),
        (Power::InfestedPower, PowerAmount::Endless, EventHook::AfterDeath { creature, .. })
            if creature == holder =>
        {
            unimplemented!("an endless Infested power")
        }
        // `TenderPower.AfterCardPlayed`, which counts only its owner's own
        // cards. A fight here has one player, so every card played is theirs.
        // Each one takes a point of Strength and of Dexterity, and the count
        // is what gives them back.
        (Power::TenderPower, _, EventHook::CardPlayed { .. }) => {
            state.count(one);
            asked.extend([
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(-one)),
                HookEffect::ApplyToOwner(Power::DexterityPower, PowerAmount::Counted(-one)),
            ]);
            return;
        }
        // `TenderPower.AfterSideTurnEnd` for the side its owner is among the
        // participants of. Everything taken this turn comes back, and the
        // count starts again.
        (Power::TenderPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            state.clear();
            asked.extend([
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(counted)),
                HookEffect::ApplyToOwner(Power::DexterityPower, PowerAmount::Counted(counted))
            ]);
            return;
        }
        (Power::TenderPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `SlumberPower.AfterSideTurnEnd` for the side its owner is among. The
        // beetle sleeps through its own turns and wakes when the count is
        // spent, which `PowerCmd.Decrement` taking the last instance off is.
        (Power::SlumberPower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            if amount == one {
                asked.extend([
                    HookEffect::Decrement,
                    HookEffect::RemoveFromOwner(Power::PlatingPower),
                ]);
                return;
            }
            Some(HookEffect::Decrement)
        }
        (Power::SlumberPower, _, EventHook::SideTurnEnd { .. }) => None,
        (Power::FurnacePower, PowerAmount::Counted(amount), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            Some(HookEffect::Forge {
                card: Cards::Token(TokenCard::SovereignBlade),
                key: super::super::generated::cards::CardVar::Damage,
                amount,
            })
        }
        (Power::FurnacePower, PowerAmount::Endless, EventHook::SideTurnStart { .. }) => {
            unimplemented!("an endless Furnace")
        }
        // `SlowPower.AfterCardPlayed` counts every card anyone plays, and its
        // `AfterSideTurnStart` forgets the count when the side its owner is
        // among takes its turn.
        (Power::SlowPower, _, EventHook::CardPlayed { .. }) => { state.count(one); None },
        (Power::SlowPower, _, EventHook::SideTurnStart { side, .. }) if side == owner => {
            { state.clear(); None }
        }
        (Power::SlowPower, _, EventHook::SideTurnStart { .. }) => None,
        (
            Power::SlumberPower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived { target, unblocked },
        ) if target == holder && unblocked >= one => {
            if amount == one {
                asked.extend([
                    HookEffect::Decrement,
                    HookEffect::RemoveFromOwner(Power::PlatingPower),
                    stunned(None),
                ]);
                return;
            }
            Some(HookEffect::Decrement)
        }
        (Power::SlumberPower, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if target == holder =>
        {
            unimplemented!("an endless Slumber power")
        }
        // `SlipperyPower.AfterDamageReceived`, which spends an instance on
        // every hit that reaches its owner's hit points.
        (Power::SlipperyPower, _, EventHook::DamageReceived { target, unblocked })
            if target == holder && unblocked >= one =>
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
                creature,
                power: Power::SwordSagePower,
                delta,
            },
        ) if creature == holder => Some(HookEffect::AddReplays {
            to: sts2_core::Writing::EveryCopyOf(Cards::Token(TokenCard::SovereignBlade)),
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
            to: sts2_core::Writing::One(at),
            amount,
        }),
        (Power::SwordSagePower, PowerAmount::Endless, EventHook::CardEnteredCombat { .. }) => {
            unimplemented!("an endless Sword Sage")
        }
        (
            Power::PlatingPower,
            PowerAmount::Counted(_),
            EventHook::SideTurnStart { side, turn, .. },
        ) if owner == CombatSide::Enemy
            && side == owner
            && turn > <V as sts2_core::ContentVersion>::Count::from(1u8) =>
        {
            Some(HookEffect::Decrement)
        }
        // `PlatingPower.BeforeSideTurnStart`: a monster that opens the fight
        // with Plating opens it behind that much block. It answers the
        // player's first turn opening and no other, and only on a monster.
        (
            Power::PlatingPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnStart { side, turn },
        ) if side == CombatSide::Player
            && owner == CombatSide::Enemy
            && turn == <V as sts2_core::ContentVersion>::Count::from(1u8) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (
            Power::PlatingPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnEndEarly { side },
        ) if side == owner => Some(HookEffect::GainBlock(amount)),
        // `PlatingPower.AfterSideTurnStart` on a player: one comes off each
        // of its turns but the first, by `PowerCmd.Decrement`.
        (Power::PlatingPower, PowerAmount::Counted(_), EventHook::SideTurnStart { side, turn, .. })
            if owner != CombatSide::Enemy
                && side == owner
                && turn > <V as sts2_core::ContentVersion>::Count::from(1u8) =>
        {
            Some(HookEffect::Decrement)
        }
        // `SwipePower.BeforeDeath`, which hands the stolen card back as its
        // owner dies.
        (Power::SwipePower, _, EventHook::BeforeDeath { creature }) if creature == holder => {
            match state {
                State::Swipe { stolen } => stolen.map(HookEffect::ReturnCard),
                state => unreachable!("stolen state for {:?}", state.power()),
            }
        }
        // `BurrowedPower.AfterBlockBroken`: a Tunneler whose block breaks is
        // stunned, comes back with a bite, and stops burrowing.
        (Power::BurrowedPower, _, EventHook::BlockBroken { creature })
            if creature == holder && monster == Some(Monster::Tunneler) =>
        {
            let bite = super::monsters::state_of(Monster::Tunneler, MoveId::BiteMove, 0)
                .unwrap_or_else(|_| unreachable!("the Tunneler's graph holds its bite"));
            asked.extend([
                stunned(Some(bite)),
                HookEffect::RemoveFromOwner(Power::BurrowedPower),
            ]);
            return;
        }
        // `BurrowedPower.AfterRemoved`, which takes all of its owner's block.
        (Power::BurrowedPower, _, EventHook::Removed) => Some(HookEffect::LoseBlock),
        // `IllusionPower.AfterCombatEnd` plays the dead owner's animation,
        // which is all it does.
        (Power::IllusionPower, _, EventHook::CombatEnd { .. }) => None,
        // `VigorPower.BeforeAttack`, which binds to the first powered attack
        // its owner makes and keeps that one.
        (
            Power::VigorPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeAttack {
                command,
                attacker,
                source,
                powered: true,
            },
        ) if attacker == holder && state.bound().is_none() => {
            *state = State::Vigor {
                bound: Some(Bound {
                    command,
                    source,
                    amount,
                }),
            };
            None
        }
        (Power::VigorPower, PowerAmount::Endless, EventHook::BeforeAttack { attacker, .. })
            if attacker == holder =>
        {
            unimplemented!("an endless Vigor")
        }
        (Power::VigorPower, _, EventHook::BeforeAttack { .. }) => None,
        // `VigorPower.AfterAttack`: what it held as the command began comes
        // off once that command is over.
        (Power::VigorPower, _, EventHook::AfterAttack { command, .. }) => state
            .bound()
            .filter(|bound| bound.command == command)
            .map(|bound| HookEffect::ModifyAmount(-bound.amount)),
        // `InfiniteBladesPower.BeforeHandDraw`: its stack of Shivs into its
        // owner's hand, before the hand is drawn.
        (
            Power::InfiniteBladesPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeHandDraw { player },
        ) if player == holder => Some(HookEffect::GenerateInHand {
            card: Cards::Token(TokenCard::Shiv),
            count: amount,
        }),
        (Power::InfiniteBladesPower, PowerAmount::Endless, EventHook::BeforeHandDraw { player })
            if player == holder =>
        {
            unimplemented!("an endless Infinite Blades")
        }
        (Power::InfiniteBladesPower, _, EventHook::BeforeHandDraw { .. }) => None,
        // `PhantomBladesPower.AfterApplied`: every Shiv its owner holds, in
        // every pile, retains.
        (Power::PhantomBladesPower, _, EventHook::Applied) => Some(HookEffect::AddKeyword {
            to: sts2_core::Writing::EveryTagged(CardTag::Shiv),
            keyword: CardKeyword::Retain,
        }),
        // `PhantomBladesPower.AfterCardEnteredCombat`, which gives a Shiv that
        // joins later the same. A fight here has one player, so the Shiv is
        // its owner's.
        (Power::PhantomBladesPower, _, EventHook::CardEnteredCombat { at, card })
            if card.tagged(CardTag::Shiv) =>
        {
            Some(HookEffect::AddKeyword {
                to: sts2_core::Writing::One(at),
                keyword: CardKeyword::Retain,
            })
        }
        (Power::PhantomBladesPower, _, EventHook::CardEnteredCombat { .. }) => None,
        // `RoyaltiesPower.AfterCombatEnd`: a gold reward of its stack.
        (Power::RoyaltiesPower, PowerAmount::Counted(amount), EventHook::CombatEnd { .. }) => {
            Some(HookEffect::AddGoldReward(amount))
        }
        (Power::RoyaltiesPower, PowerAmount::Endless, EventHook::CombatEnd { .. }) => {
            unimplemented!("an endless Royalties")
        }
        // `DrawCardsNextTurnPower.AfterSideTurnStart` goes once the turn it
        // drew for has begun.
        (Power::DrawCardsNextTurnPower, _, EventHook::SideTurnStart { side, .. })
            if side == owner && held.on_turn_start != PowerAmount::Counted(0) =>
        {
            Some(HookEffect::Remove)
        }
        (Power::DrawCardsNextTurnPower, _, EventHook::SideTurnStart { .. }) => None,
        // `RetainHandPower.AfterSideTurnEnd`, for the side its owner is among.
        (Power::RetainHandPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            Some(HookEffect::Decrement)
        }
        (Power::RetainHandPower, _, EventHook::SideTurnEnd { .. }) => None,
        // The overrides above that answer for their own holder, or for their
        // own side's turn, and leave every other event alone.
        (Power::BlockNextTurnPower, PowerAmount::Counted(_), EventHook::BlockCleared { .. })
        | (Power::EnergyNextTurnPower, _, EventHook::EnergyReset { .. })
        | (Power::FurnacePower, PowerAmount::Counted(_), EventHook::SideTurnStart { .. })
        | (
            Power::IllusionPower
            | Power::InfestedPower
            | Power::ReattachPower
            | Power::ShrinkPower,
            _,
            EventHook::AfterDeath { .. },
        )
        | (Power::ImbalancedPower, _, EventHook::DamageGiven { .. })
        | (Power::SwipePower, _, EventHook::BeforeDeath { .. })
        | (Power::BurrowedPower, _, EventHook::BlockBroken { .. })
        | (Power::SlipperyPower | Power::SlumberPower, _, EventHook::DamageReceived { .. })
        | (
            Power::PlatingPower,
            PowerAmount::Counted(_),
            EventHook::SideTurnStart { .. }
            | EventHook::BeforeSideTurnStart { .. }
            | EventHook::BeforeSideTurnEndEarly { .. },
        )
        | (
            Power::SwordSagePower,
            _,
            EventHook::PowerAmountChanged { .. } | EventHook::CardEnteredCombat { .. },
        ) => None,
        // `BeforeApplied`, `AfterApplied` and `AfterRemoved` are a power's
        // own, and a move being performed is the move state's. Every power is asked these,
        // and one whose class does not override them does nothing.
        (
            _,
            _,
            EventHook::BeforeApplied { .. }
            | EventHook::Applied
            | EventHook::Removed
            | EventHook::MonsterMovePerformed { .. },
        ) => None,
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    };
    asked.extend(effect);
}
