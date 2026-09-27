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

use sts2_core::{
    CardTable as _, Modify, ModifyHook, PowerAmount, PowerModel as _, PowerTable as _,
};

use super::super::V0_107_1 as V;
use super::super::generated::afflictions::Affliction;
use super::super::generated::cards::{CardKeyword, CardTag, Cards, ColorlessCard, TokenCard};
use super::super::generated::monsters::{IntentKind, Monster, MoveId};
use super::super::generated::orbs::Orb;
use super::super::generated::powers::Power;
use super::super::generated::relics::Relic;
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
    /// `DarkEmbracePower`'s `etherealCount`.
    DarkEmbrace {
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
    /// `FeralPower`'s `zeroCostAttacksPlayed`.
    Feral {
        played: Number,
    },
    /// `GigantificationPower`'s `commandToModify`: the command, and the card
    /// that made it.
    Gigantification {
        bound: Option<(sts2_core::AttackId, sts2_core::CardId<V>)>,
    },
    /// `AutomationPower`'s `cardsLeft`.
    Automation {
        cards_left: u8,
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
            Power::DarkEmbracePower => Self::DarkEmbrace { counted: 0 },
            Power::ImbalancedPower => Self::Imbalanced { off_balance: false },
            Power::IllusionPower => Self::Illusion { reviving: false },
            Power::ReattachPower => Self::Reattach { reviving: false },
            Power::SwipePower => Self::Swipe { stolen: None },
            Power::VigorPower => Self::Vigor { bound: None },
            Power::FeralPower => Self::Feral { played: 0 },
            Power::GigantificationPower => Self::Gigantification { bound: None },
            Power::AutomationPower => Self::Automation {
                cards_left: AUTOMATION_CARDS,
            },
            power => Self::Stateless(power),
        }
    }

    fn power(&self) -> Power {
        match self {
            Self::Stateless(power) => *power,
            Self::Slow { .. } => Power::SlowPower,
            Self::Tender { .. } => Power::TenderPower,
            Self::DarkEmbrace { .. } => Power::DarkEmbracePower,
            Self::Imbalanced { .. } => Power::ImbalancedPower,
            Self::Illusion { .. } => Power::IllusionPower,
            Self::Reattach { .. } => Power::ReattachPower,
            Self::Swipe { .. } => Power::SwipePower,
            Self::Vigor { .. } => Power::VigorPower,
            Self::Feral { .. } => Power::FeralPower,
            Self::Gigantification { .. } => Power::GigantificationPower,
            Self::Automation { .. } => Power::AutomationPower,
        }
    }

    fn debuff(&self) -> bool {
        use sts2_core::PowerTable as _;

        let power = self.power();
        match lends(power) {
            // `TemporaryStrengthPower.Type`, which is a debuff when it lends
            // a negative amount.
            Some((_, positive)) => !positive,
            // Every power whose `Type` this build computes lends, which
            // `every_computed_type_is_a_lend` holds the table to.
            None => power.debuff().unwrap_or_else(|| {
                unimplemented!(
                    "whether {power:?} is a debuff: its `Type` is computed, and it lends nothing"
                )
            }),
        }
    }
}

/// What a temporary power lends, and `IsPositive`, for the family that lends
/// Strength, Dexterity or Focus for a turn and takes it back.
///
/// Every subclass of `TemporaryStrengthPower`, `TemporaryDexterityPower` and
/// `TemporaryFocusPower`, which is `InternallyAppliedPower`, and its
/// `IsPositive` override. None of them overrides anything else, so
/// [`temporary`] is the whole of each.
fn lends(power: Power) -> Option<(Power, bool)> {
    use Power::{DexterityPower as Dexterity, FocusPower as Focus, StrengthPower as Strength};

    Some(match power {
        Power::CoordinatePower
        | Power::FeedingFrenzyPower
        | Power::FlexPotionPower
        | Power::ReptileTrinketPower
        | Power::SetupStrikePower => (Strength, true),
        Power::CrushUnderPower
        | Power::DarkShacklesPower
        | Power::DyingStarPower
        | Power::EnfeeblingTouchPower
        | Power::ManglePower
        | Power::MockTemporaryStrengthLossPower
        | Power::MonarchsGazeStrengthDownPower
        | Power::PiercingWailPower
        | Power::ShacklingPotionPower => (Strength, false),
        Power::AnticipatePower | Power::HelicalDartPower | Power::SpeedPotionPower => {
            (Dexterity, true)
        }
        Power::FocusedStrikePower | Power::HotfixPower | Power::SynchronizePower => (Focus, true),
        _ => return None,
    })
}

impl State {
    fn counted(&self) -> Number {
        match self {
            Self::Slow { counted } | Self::Tender { counted } | Self::DarkEmbrace { counted } => {
                *counted
            }
            _ => 0,
        }
    }

    fn count(&mut self, amount: Number) {
        match self {
            Self::Slow { counted } | Self::Tender { counted } | Self::DarkEmbrace { counted } => {
                *counted += amount
            }
            state => unreachable!("counted state for {:?}", state.power()),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Slow { counted } | Self::Tender { counted } | Self::DarkEmbrace { counted } => {
                *counted = 0
            }
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

    /// `FeralPower`'s count of the turn's zero-cost attacks it sent back.
    fn feral_played(&self) -> Number {
        match self {
            Self::Feral { played } => *played,
            state => unreachable!("Feral's count for {:?}", state.power()),
        }
    }

    /// `FeralPower.SetZeroCostAttacksPlayed`.
    fn set_feral_played(&mut self, to: Number) {
        match self {
            Self::Feral { played } => *played = to,
            state => unreachable!("Feral's count for {:?}", state.power()),
        }
    }

    /// The command `VigorPower` is bound to, once it is.
    fn bound(&self) -> Option<Bound> {
        match self {
            Self::Vigor { bound } => *bound,
            state => unreachable!("bound state for {:?}", state.power()),
        }
    }

    /// The command `GigantificationPower` is bound to, and the card that made
    /// it.
    fn giant(&self) -> Option<(sts2_core::AttackId, sts2_core::CardId<V>)> {
        match self {
            Self::Gigantification { bound } => *bound,
            state => unreachable!("gigantification state for {:?}", state.power()),
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

/// `VulnerablePower`'s `DamageIncrease`, in hundredths.
///
/// Vulnerable's and Weak's multipliers are sums of the game's decimals, which
/// a binary float adds a hair off. So they are summed in hundredths, which is
/// exact, and turned into a float once. The float nearest a decimal fraction
/// lands every product that is a whole number on that whole number.
const DAMAGE_INCREASE: i32 = 150;

/// `WeakPower`'s `DamageDecrease`, in hundredths.
const DAMAGE_DECREASE: i32 = 75;

/// `PaperPhrog.ModifyVulnerableMultiplier`'s `0.25m`, in hundredths.
const PHROG: i32 = 25;

/// `PaperKrane.ModifyWeakMultiplier`'s `0.15m`, in hundredths.
const KRANE: i32 = 15;

/// `FrailPower.ModifyBlockMultiplicative`'s `0.75m`.
const FRAIL: f64 = 0.75;

/// `ColossusPower`'s `DamageDecrease`.
const COLOSSAL: f64 = 0.5;

/// `AutomationPower._baseCardsLeft`: the cards its owner draws for each
/// payout.
const AUTOMATION_CARDS: u8 = 10;

/// `PaleBlueDotPower.cardPlayThresholdValue`: the plays its owner finishes in
/// one turn for the next turn's hand to grow.
const PALE_BLUE_DOT_PLAYS: usize = 5;

/// `GigantificationPower.ModifyDamageMultiplicative`'s `3m`.
const GIGANTIFIED: f64 = 3.0;

/// `ShrinkPower`'s own `DamageDecrease`, which the power stores as the
/// percentage taken off rather than as what is left.
const SHRUNK: f64 = 1.0 - 30.0 / 100.0;

/// What one card played adds to what `SlowPower`'s owner takes.
///
/// The game works this out in decimal, where a tenth is exact. A binary float
/// holds a tenth a hair high. So every product lands at or above the decimal
/// one, and truncates to the same whole number.
const SLOW_PER_CARD: f64 = 0.1;

/// Whether a power that makes a card free takes `card`. The three `Free`
/// powers take an Attack, a Skill or a Power, and Veilpiercer an Ethereal card.
fn frees(power: Power, card: Cards, ethereal: bool) -> bool {
    use sts2_core::{CardKind, Rules as _};

    let kind = V::card_kind(card);
    match power {
        Power::FreeAttackPower => kind == Some(CardKind::Attack),
        Power::FreeSkillPower => kind == Some(CardKind::Skill),
        Power::FreePowerPower => kind == Some(CardKind::Power),
        Power::VeilpiercerPower => ethereal,
        power => unreachable!("{power:?} makes no card free"),
    }
}

/// `RollingBoulderPower`'s `DamageVar(5)`, what each turn adds to it.
const ROLLING_BOULDER_GROWTH: Number = 5;

/// `DanseMacabrePower`'s `EnergyVar(2)`: the resolved cost it answers from.
const DANSE_MACABRE_ENERGY: Number = 2;

/// `RadiancePower`'s `EnergyVar(1)`.
const RADIANCE_ENERGY: Number = 1;

/// What Vulnerable on `holder` multiplies a powered hit from `dealer` by.
///
/// # Panics
///
/// An endless Cruelty.
fn vulnerable(
    fight: &dyn sts2_core::FightView<V>,
    holder: sts2_core::CombatId,
    dealer: Option<sts2_core::CombatId>,
) -> f64 {
    let mut by = DAMAGE_INCREASE;
    // `if (dealer != null)`, and both answer the dealer hitting itself with
    // nothing.
    if let Some(dealer) = dealer
        && dealer != holder
    {
        if fight.relic(dealer, Relic::PaperPhrog) {
            by += PHROG;
        }
        match fight.power(dealer, Power::CrueltyPower) {
            // `Amount / 100m`, which is its stack in hundredths.
            Some(PowerAmount::Counted(amount)) => by += amount,
            Some(PowerAmount::Endless) => unimplemented!("an endless Cruelty"),
            None => {}
        }
    }
    // `amount + (amount - 1m)`.
    if fight.power(holder, Power::DebilitatePower).is_some() {
        by += by - 100;
    }
    f64::from(by) / 100.0
}

/// What Weak on `holder` multiplies a powered hit it deals `target` by.
fn weak(
    fight: &dyn sts2_core::FightView<V>,
    holder: sts2_core::CombatId,
    target: Option<sts2_core::CombatId>,
) -> f64 {
    let mut by = DAMAGE_DECREASE;
    if target.is_some_and(|target| fight.relic(target, Relic::PaperKrane)) {
        by -= KRANE;
    }
    // `amount - (1m - amount)`.
    if fight.power(holder, Power::DebilitatePower).is_some() {
        by -= 100 - by;
    }
    f64::from(by) / 100.0
}

/// `TemporaryStrengthPower` and `TemporaryDexterityPower`, which lend `lent`
/// and take it back.
///
/// `BeforeApplied` lands the lent power, signed by `IsPositive`, before this
/// power attaches. `AfterSideTurnEnd` for its owner's own side takes this
/// power off and hands the same amount back.
///
/// `positive` is `IsPositive`. Crush Under's is false, so it lands negative
/// Strength and returns it.
fn temporary(
    power: Power,
    lent: Power,
    positive: bool,
    amount: PowerAmount<Number>,
    held: Held<'_>,
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
                unimplemented!("an endless temporary {lent:?} landing")
            };
            asked.push(HookEffect::ApplyToOwner(
                lent,
                PowerAmount::Counted(signed(landing)),
            ))
        }
        (PowerAmount::Counted(amount), EventHook::SideTurnEnd { side }) if side == held.owner => {
            // `PowerCmd.Remove(this)` and then the lent amount back, which is the
            // order the game hands them over in.
            asked.extend([
                HookEffect::Remove,
                HookEffect::ApplyToOwner(lent, PowerAmount::Counted(-signed(amount))),
            ])
        }
        (PowerAmount::Endless, EventHook::SideTurnEnd { .. }) => {
            unimplemented!("an endless temporary {lent:?}")
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
            unimplemented!("a temporary {lent:?} stacked onto its own instance")
        }
        (_, EventHook::PowerAmountChanged { .. }) => {}
        // Asked of every power, and not ones this class overrides.
        (_, EventHook::MonsterMovePerformed { .. }) => {}
        (_, hook)
            if hook
                .power_virtual()
                .is_some_and(|method| !power.overrides_virtual(method)) => {}
        (_, hook) => unimplemented!("what a temporary {lent:?} does for {hook:?}"),
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
    held: Held<'_>,
    state: &State,
    hook: sts2_core::ModifyHook<V>,
) -> Modify<Number> {
    let Held { history, fight, .. } = held;
    let one: Number = num_traits::one();
    debug_assert_eq!(state.power(), power);
    let counted = state.counted();
    let holder = held.holder;
    let owner = held.owner;
    match (power, amount, hook) {
        // `SlowPower.ModifyDamageMultiplicative`, which reads the cards played
        // since its owner's turn began and takes a tenth more for each.
        (
            Power::SlowPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Scaled(1.0 + SLOW_PER_CARD * f64::from(counted)),
        // `VulnerablePower.ModifyDamageMultiplicative`: its `DamageIncrease`,
        // raised by the dealer's Paper Phrog and Cruelty, then by the
        // holder's own Debilitate.
        (
            Power::VulnerablePower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Scaled(vulnerable(fight, holder, dealer)),
        // `WeakPower.ModifyDamageMultiplicative`: its `DamageDecrease`,
        // lowered by the target's Paper Krane, then by the holder's own
        // Debilitate.
        (
            Power::WeakPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) => Modify::Scaled(weak(fight, holder, target)),
        (
            Power::ShrinkPower,
            _,
            ModifyHook::DamageMultiplicative {
                dealer,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) => Modify::Scaled(SHRUNK),
        // `TryModifyEnergyCostInCombatLate`: nothing for a card of its kind
        // that its owner holds or is playing, which is every card whose cost
        // the fight reads. A fight here has one player, so its owner's cards
        // are the player's.
        (
            Power::FreeAttackPower
            | Power::FreeSkillPower
            | Power::FreePowerPower
            | Power::VeilpiercerPower,
            PowerAmount::Counted(_),
            ModifyHook::EnergyCostLate {
                card: (_, card),
                ethereal,
                ..
            },
        ) if owner == CombatSide::Player && frees(power, card, ethereal) => {
            Modify::AtMost(num_traits::zero())
        }
        (
            Power::FreeAttackPower
            | Power::FreeSkillPower
            | Power::FreePowerPower
            | Power::VeilpiercerPower,
            PowerAmount::Endless,
            ModifyHook::EnergyCostLate { .. },
        ) => unimplemented!("an endless {power:?}"),
        (
            Power::FreeAttackPower
            | Power::FreeSkillPower
            | Power::FreePowerPower
            | Power::VeilpiercerPower,
            _,
            ModifyHook::EnergyCostLate { .. },
        ) => Modify::Nothing,
        // `BorrowedTimePower.TryModifyEnergyCostInCombat`: its stack onto
        // every card its owner holds.
        (Power::BorrowedTimePower, PowerAmount::Counted(amount), ModifyHook::EnergyCost { .. })
            if owner == CombatSide::Player =>
        {
            Modify::Added(amount)
        }
        (Power::BorrowedTimePower, PowerAmount::Endless, ModifyHook::EnergyCost { .. }) => {
            unimplemented!("an endless Borrowed Time")
        }
        (Power::BorrowedTimePower, _, ModifyHook::EnergyCost { .. }) => Modify::Nothing,
        // `TangledPower.TryModifyEnergyCostInCombat`: its stack onto a card
        // with Entangled. A fight here has one player, whose cards are its
        // holder's.
        (
            Power::TangledPower,
            PowerAmount::Counted(amount),
            ModifyHook::EnergyCost {
                affliction: Some(Affliction::Entangled),
                ..
            },
        ) => Modify::Added(amount),
        (
            Power::TangledPower,
            PowerAmount::Endless,
            ModifyHook::EnergyCost {
                affliction: Some(Affliction::Entangled),
                ..
            },
        ) => unimplemented!("an endless Tangled"),
        (Power::TangledPower, _, ModifyHook::EnergyCost { .. }) => Modify::Nothing,
        // `IntangiblePower`: a hit on its owner capped at one, and the hit
        // points it loses capped the same, whatever the hit's props.
        (Power::IntangiblePower, _, ModifyHook::DamageCap { target, .. })
            if target == Some(holder) =>
        {
            Modify::AtMost(one)
        }
        (Power::IntangiblePower, _, ModifyHook::HpLostAfterOsty { target }) if target == holder => {
            Modify::AtMost(one)
        }
        (
            Power::IntangiblePower,
            _,
            ModifyHook::DamageCap { .. } | ModifyHook::HpLostAfterOsty { .. },
        ) => Modify::Nothing,
        // `CalcifyPower.ModifyDamageAdditive`: its stack onto a powered hit
        // from Osty, where its holder is the player Osty stands beside.
        (
            Power::CalcifyPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if dealer.and_then(|dealer| fight.monster(dealer)) == Some(Monster::Osty)
            && dealer.and_then(|dealer| fight.pet_owner(dealer)) == Some(holder) =>
        {
            Modify::Added(amount)
        }
        (Power::CalcifyPower, PowerAmount::Endless, ModifyHook::DamageAdditive { dealer, .. })
            if dealer.and_then(|dealer| fight.pet_owner(dealer)) == Some(holder) =>
        {
            unimplemented!("an endless Calcify")
        }
        (Power::CalcifyPower, _, ModifyHook::DamageAdditive { .. }) => Modify::Nothing,
        // `ModifyMaxEnergy`: its stack onto its owner's energy, or off it for
        // Waste Away.
        (
            Power::DemesnePower | Power::PyrePower | Power::FriendshipPower,
            PowerAmount::Counted(amount),
            ModifyHook::MaxEnergy { player },
        ) if player == holder => Modify::Added(amount),
        (Power::WasteAwayPower, PowerAmount::Counted(amount), ModifyHook::MaxEnergy { player })
            if player == holder =>
        {
            Modify::Added(-amount)
        }
        (
            Power::DemesnePower | Power::PyrePower | Power::FriendshipPower | Power::WasteAwayPower,
            PowerAmount::Endless,
            ModifyHook::MaxEnergy { player },
        ) if player == holder => unimplemented!("an endless {power:?}"),
        (
            Power::DemesnePower | Power::PyrePower | Power::FriendshipPower | Power::WasteAwayPower,
            _,
            ModifyHook::MaxEnergy { .. },
        ) => Modify::Nothing,
        // `LethalityPower.ModifyDamageMultiplicative`: its stack in hundredths
        // onto a powered hit from its owner's first attack of the turn. A
        // card being played has started its own play, and a replay starts
        // another, so the first attack's first play is the one that counts
        // one. A fight here has one player, whose cards are its owner's.
        (
            Power::LethalityPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageMultiplicative {
                card: Some(source),
                powered: true,
                ..
            },
        ) if owner == CombatSide::Player => {
            use sts2_core::{CardKind, Rules as _};

            let attacks = history
                .started_this_turn()
                .filter(|started| V::card_kind(started.card) == Some(CardKind::Attack))
                .count();
            match attacks > usize::from(source.playing) {
                true => Modify::Nothing,
                false => Modify::Scaled(f64::from(100 + amount) / 100.0),
            }
        }
        (
            Power::LethalityPower,
            PowerAmount::Endless,
            ModifyHook::DamageMultiplicative { card: Some(_), .. },
        ) => unimplemented!("an endless Lethality"),
        (Power::LethalityPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `ColossusPower.ModifyDamageMultiplicative`: half of a powered hit on
        // its owner, from a dealer that is vulnerable.
        (
            Power::ColossusPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if target == Some(holder)
            && dealer
                .is_some_and(|dealer| fight.power(dealer, Power::VulnerablePower).is_some()) =>
        {
            Modify::Scaled(COLOSSAL)
        }
        (Power::ColossusPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `ConquerorPower.ModifyDamageMultiplicative`: double a powered hit on
        // its owner from a Sovereign Blade.
        (
            Power::ConquerorPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                card:
                    Some(sts2_core::CardSource {
                        card: Cards::Token(TokenCard::SovereignBlade),
                        ..
                    }),
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Scaled(2.0),
        (Power::ConquerorPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `GigantificationPower.ModifyDamageMultiplicative`: a powered hit
        // from a card its owner played, before it binds or from the card it
        // bound to. It reads the card's owner and not the dealer, so Osty's
        // hit from a card counts. A fight here has one player, whose cards
        // are all its holder's.
        (
            Power::GigantificationPower,
            _,
            ModifyHook::DamageMultiplicative {
                card: Some(sts2_core::CardSource { id, .. }),
                powered: true,
                ..
            },
        ) if state.giant().is_none_or(|(_, source)| source == id) => Modify::Scaled(GIGANTIFIED),
        (Power::GigantificationPower, _, ModifyHook::DamageMultiplicative { .. }) => {
            Modify::Nothing
        }
        (
            Power::StrengthPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) => Modify::Added(amount),
        (
            Power::StrengthPower,
            PowerAmount::Endless,
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) => {
            unimplemented!("an endless Strength")
        }
        // `VigorPower.ModifyDamageAdditive`. Once bound, it pays the card
        // that started the command and no other card. A hit from no card
        // is paid either way. The command it bound was its own owner's, so
        // the attacker it names is this dealer.
        (
            Power::VigorPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                card,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) => {
            let other = |bound: Bound| card.is_some_and(|source| Some(source.id) != bound.source);
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
                card: Some(sts2_core::CardSource { card, .. }),
                powered: true,
                ..
            },
        ) if dealer == Some(holder) && card.tagged(CardTag::Shiv) => Modify::Added(amount),
        // `PhantomBladesPower.ModifyDamageAdditive`: the first Shiv its owner
        // plays in a turn, and no other. A play counts once it has finished,
        // so the Shiv being played is not one of them. A fight here has one
        // player, so every play is its owner's.
        (
            Power::PhantomBladesPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                card: Some(sts2_core::CardSource { card, .. }),
                powered: true,
                ..
            },
        ) if dealer == Some(holder)
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
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) => unimplemented!("an endless {power:?}"),
        // `OneTwoPunchPower.ModifyCardPlayCount`: one more play of an attack
        // its owner plays.
        (Power::OneTwoPunchPower, _, ModifyHook::CardPlayCount { player, card })
            if player == holder
                && <V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Attack) =>
        {
            Modify::Added(one)
        }
        (Power::OneTwoPunchPower, _, ModifyHook::CardPlayCount { .. }) => Modify::Nothing,
        // `DuplicationPower.ModifyCardPlayCount`: one more play of any card
        // its owner plays.
        (Power::DuplicationPower, _, ModifyHook::CardPlayCount { player, .. })
            if player == holder =>
        {
            Modify::Added(one)
        }
        (Power::DuplicationPower, _, ModifyHook::CardPlayCount { .. }) => Modify::Nothing,
        // `BurstPower.ModifyCardPlayCount`: one more play of a Skill its
        // owner plays.
        (Power::BurstPower, _, ModifyHook::CardPlayCount { player, card })
            if player == holder
                && <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill) =>
        {
            Modify::Added(one)
        }
        (Power::BurstPower, _, ModifyHook::CardPlayCount { .. }) => Modify::Nothing,
        // `SignalBoostPower.ModifyCardPlayCount`: one more play of a Power
        // card its owner plays.
        (Power::SignalBoostPower, _, ModifyHook::CardPlayCount { player, card })
            if player == holder
                && <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power) =>
        {
            Modify::Added(one)
        }
        (Power::SignalBoostPower, _, ModifyHook::CardPlayCount { .. }) => Modify::Nothing,
        // `EchoFormPower.ModifyCardPlayCount`: one more play of a card its
        // owner plays, while the turn has started fewer first plays of a
        // series than its stack. This play is not started yet. A fight here
        // has one player, whose plays are all its holder's.
        (
            Power::EchoFormPower,
            PowerAmount::Counted(amount),
            ModifyHook::CardPlayCount { player, .. },
        ) if player == holder => {
            let first: Number = history
                .started_this_turn()
                .filter(|started| started.first)
                .count()
                .try_into()
                .expect("a turn's plays fit a game number");
            match first < amount {
                true => Modify::Added(one),
                false => Modify::Nothing,
            }
        }
        (Power::EchoFormPower, PowerAmount::Endless, ModifyHook::CardPlayCount { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Echo Form")
        }
        (Power::EchoFormPower, _, ModifyHook::CardPlayCount { .. }) => Modify::Nothing,
        // `ShadowmeldPower.ModifyBlockMultiplicative`: two to the power of
        // its stack, on its owner's block.
        (
            Power::ShadowmeldPower,
            PowerAmount::Counted(amount),
            ModifyHook::BlockMultiplicative { target, .. },
        ) if target == holder => Modify::Scaled(2f64.powi(amount)),
        (
            Power::ShadowmeldPower,
            PowerAmount::Endless,
            ModifyHook::BlockMultiplicative { target, .. },
        ) if target == holder => {
            unimplemented!("an endless Shadowmeld")
        }
        (Power::ShadowmeldPower, _, ModifyHook::BlockMultiplicative { .. }) => Modify::Nothing,
        // `NoBlockPower.ModifyBlockMultiplicative`: none of its owner's
        // powered block from a card.
        (
            Power::NoBlockPower,
            _,
            ModifyHook::BlockMultiplicative {
                target,
                powered: true,
                card: true,
            },
        ) if target == holder => Modify::Scaled(0.0),
        (Power::NoBlockPower, _, ModifyHook::BlockMultiplicative { .. }) => Modify::Nothing,
        // `BufferPower.ModifyHpLostAfterOstyLate`: its holder loses nothing.
        (Power::BufferPower, _, ModifyHook::HpLostAfterOstyLate { target }) if target == holder => {
            Modify::AtMost(num_traits::zero())
        }
        (Power::BufferPower, _, ModifyHook::HpLostAfterOstyLate { .. }) => Modify::Nothing,
        // `FocusPower.ModifyOrbValue`: its stack onto each number its owner's
        // orbs show, and no lower than nothing.
        (Power::FocusPower, PowerAmount::Counted(amount), ModifyHook::OrbValue { player, .. })
            if player == holder =>
        {
            Modify::Added(amount)
        }
        (Power::FocusPower, PowerAmount::Endless, ModifyHook::OrbValue { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Focus")
        }
        (Power::FocusPower, _, ModifyHook::OrbValue { .. }) => Modify::Nothing,
        // `SlipperyPower.ModifyHpLostAfterOsty`, which hands back one however
        // much its owner was about to lose. A hit that the block took whole
        // loses nothing, and this leaves that where it is.
        (Power::SlipperyPower, _, ModifyHook::HpLostAfterOsty { target }) if target == holder => {
            Modify::AtMost(one)
        }
        // `DexterityPower.ModifyBlockAdditive` pays powered block from a card
        // its owner plays, or powered block its owner gains otherwise. A
        // fight here has one player, whose cards give the player block, so
        // both are the block its holder is gaining.
        (
            Power::DexterityPower,
            PowerAmount::Counted(amount),
            ModifyHook::BlockAdditive {
                target,
                powered: true,
                ..
            },
        ) if target == holder => Modify::Added(amount),
        (
            Power::DexterityPower,
            PowerAmount::Endless,
            ModifyHook::BlockAdditive {
                target,
                powered: true,
                ..
            },
        ) if target == holder => {
            unimplemented!("an endless Dexterity")
        }
        // `FrailPower.ModifyBlockMultiplicative`: a quarter off the block a
        // card or a move gives its owner.
        (
            Power::FrailPower,
            _,
            ModifyHook::BlockMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if target == holder => Modify::Scaled(FRAIL),
        (Power::FrailPower, _, ModifyHook::BlockMultiplicative { .. }) => Modify::Nothing,
        // `ArtifactPower.TryModifyPowerAmountReceived`: nothing of a power
        // that is a debuff at the amount landing on its owner.
        (
            Power::ArtifactPower,
            _,
            ModifyHook::PowerAmountReceived {
                power: landing,
                target,
                amount,
            },
        ) if target == holder && debuff_at(landing, amount) => Modify::Replaced(0),
        (Power::ArtifactPower, _, ModifyHook::PowerAmountReceived { .. }) => Modify::Nothing,
        // `ModifyHandDraw`: its stack more cards for its owner.
        (
            Power::MachineLearningPower
            | Power::TyrannyPower
            | Power::ToolsOfTheTradePower
            | Power::DemesnePower,
            PowerAmount::Counted(amount),
            ModifyHook::HandDraw { player, .. },
        ) if player == holder => Modify::Added(amount),
        // `ClarityPower.ModifyHandDraw`: one more card for its owner, whatever
        // its stack.
        (Power::ClarityPower, _, ModifyHook::HandDraw { player, .. }) if player == holder => {
            Modify::Added(one)
        }
        // `PaleBlueDotPower.ModifyHandDraw`: its stack more cards for its
        // owner, once it finished enough plays in its last turn. A fight here
        // has one player, whose plays are all its holder's.
        (
            Power::PaleBlueDotPower,
            PowerAmount::Counted(amount),
            ModifyHook::HandDraw { player, turn, .. },
        ) if player == holder => {
            match history.finished_last_player_turn(turn).count() >= PALE_BLUE_DOT_PLAYS {
                true => Modify::Added(amount),
                false => Modify::Nothing,
            }
        }
        (Power::PaleBlueDotPower, PowerAmount::Endless, ModifyHook::HandDraw { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Pale Blue Dot")
        }
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
            Power::MachineLearningPower
            | Power::DrawCardsNextTurnPower
            | Power::TyrannyPower
            | Power::ToolsOfTheTradePower
            | Power::DemesnePower,
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
            Power::MachineLearningPower
            | Power::DrawCardsNextTurnPower
            | Power::TyrannyPower
            | Power::ToolsOfTheTradePower
            | Power::DemesnePower
            | Power::PaleBlueDotPower
            | Power::ClarityPower,
            _,
            ModifyHook::HandDraw { .. },
        ) => Modify::Nothing,
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    }
}

/// `PowerModel.GetTypeForAmount`: whether `power` is a debuff at `amount`.
///
/// A counter that allows negatives is a debuff below zero, and a debuff that
/// does not allow them is a buff there.
fn debuff_at(power: Power, amount: Number) -> bool {
    use sts2_core::PowerTable as _;

    use super::super::generated::powers::{PowerFlag, PowerStackType};

    let data = sts2_core::Table::data(power);
    let debuff = match lends(power) {
        Some((_, positive)) => !positive,
        None => power
            .debuff()
            .unwrap_or_else(|| unimplemented!("whether {power:?} is a debuff at {amount}")),
    };
    let counter = match data.stack {
        crate::Field::Constant(stack) => stack == Some(PowerStackType::Counter),
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

/// Whether a card's type is `Status`.
fn status(card: Cards) -> bool {
    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Status)
}

/// Whether a card's type is `Attack`.
fn attack(card: Cards) -> bool {
    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack)
}

/// Answer `ModifyCardPlayResultPileTypeAndPosition` for one power.
pub(crate) fn result_pile(
    power: Power,
    amount: PowerAmount<Number>,
    holder: sts2_core::CombatId,
    state: &State,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::ResultPile<V>,
) -> sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)> {
    use sts2_core::HookResponse;

    let unchanged = HookResponse::Decision((hook.pile, hook.position));
    match (power, amount) {
        // `FeralPower`: a zero-cost attack its owner plays goes back to the
        // top of the hand, up to its stack of them each turn.
        (Power::FeralPower, PowerAmount::Counted(amount)) => {
            match hook.player == holder
                && attack(hook.card)
                && hook.energy == 0
                && state.feral_played() < amount
            {
                true => HookResponse::Decision((
                    Some(sts2_core::Pile::Hand),
                    sts2_core::PilePosition::Top,
                )),
                false => unchanged,
            }
        }
        (Power::FeralPower, PowerAmount::Endless) => unimplemented!("an endless Feral"),
        // `NostalgiaPower`: an attack or skill of its owner's bound for the
        // discard pile goes to the top of the draw pile instead, while the
        // turn has started fewer such plays than its stack. The play itself
        // is not started yet. A fight here has one player.
        (Power::NostalgiaPower, PowerAmount::Counted(amount)) => {
            let counted = |card| {
                matches!(
                    <V as sts2_core::Rules>::card_kind(card),
                    Some(sts2_core::CardKind::Attack | sts2_core::CardKind::Skill)
                )
            };
            let started: Number = history
                .started_this_turn()
                .filter(|started| counted(started.card))
                .count()
                .try_into()
                .expect("a turn's plays fit a game number");
            match hook.player == holder
                && counted(hook.card)
                && hook.pile == Some(sts2_core::Pile::Discard)
                && started < amount
            {
                true => HookResponse::Decision((
                    Some(sts2_core::Pile::Draw),
                    sts2_core::PilePosition::Top,
                )),
                false => unchanged,
            }
        }
        (Power::NostalgiaPower, PowerAmount::Endless) => unimplemented!("an endless Nostalgia"),
        (power, _) => unimplemented!("where {power:?} sends a played card"),
    }
}

/// Answer a yes-or-no hook for one power.
///
/// A hook the fight raises reaches only a power that overrides it. A question
/// the game asks every power about itself reaches all of them, and one with
/// no override does not implement it.
pub(crate) fn decide_hook(
    power: Power,
    held: Held<'_>,
    state: &State,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::{DecisionHook, HookResponse};

    match (power, hook) {
        // `RingingPower.ShouldPlay`: a card with Ringing waits while its owner
        // has started a play this turn. A fight here has one player, whose
        // cards are its holder's.
        (Power::RingingPower, DecisionHook::ShouldPlay { affliction, .. }) => {
            HookResponse::Decision(
                affliction != Some(Affliction::Ringing)
                    || held.history.started_this_turn().next().is_none(),
            )
        }
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
        // `NoDrawPower.ShouldDraw`: its owner draws nothing but the turn's own
        // hand.
        (Power::NoDrawPower, DecisionHook::ShouldDraw { player, hand_draw }) => {
            HookResponse::Decision(hand_draw || player != held.holder)
        }
        // `RetainHandPower.ShouldFlush`, which keeps its own holder's hand.
        (Power::RetainHandPower, DecisionHook::ShouldFlush { player }) => {
            HookResponse::Decision(player != held.holder)
        }
        // `ShouldClearBlock`, which keeps its own holder's. Blur's
        // `AfterPreventingBlockClear` only flashes.
        (
            Power::BurrowedPower | Power::BarricadePower | Power::BlurPower,
            DecisionHook::ShouldClearBlock { creature },
        ) => HookResponse::Decision(creature != held.holder),
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

/// Whose a power is, and what of the fight it can read, as a hook reaching
/// it reads that.
#[derive(Clone, Copy)]
pub(crate) struct Held<'a> {
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
    /// What the fight has done so far.
    pub(crate) history: sts2_core::History<'a, V>,
    /// The fight's creatures, for a power that asks about one other than its
    /// own.
    pub(crate) fight: &'a dyn sts2_core::FightView<V>,
}

/// Dispatch one of the event hooks the simulator currently supports.
///
/// `amount` is the stack the creature carries this power as.
pub(crate) fn event_hook(
    power: Power,
    amount: PowerAmount<Number>,
    held: Held<'_>,
    state: &mut State,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let Held { history, fight, .. } = held;
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

    // A temporary power asks for two things when a turn ends, and the match
    // below spells one. Its subclasses answer there instead.
    if let Some((lent, positive)) = lends(power) {
        return temporary(power, lent, positive, amount, held, hook, asked);
    }

    let one: Number = num_traits::one();

    let effect = match (power, amount, hook) {
        (
            Power::ImbalancedPower,
            _,
            EventHook::DamageGiven {
                dealer,
                fully_blocked: true,
                ..
            },
        ) if dealer == Some(holder) => {
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
            Power::VulnerablePower | Power::WeakPower | Power::FrailPower,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => Some(HookEffect::TickDownDuration),
        // `DebilitatePower.AfterSideTurnEnd`: one off its stack as its owner's
        // turn ends. What it does to Vulnerable and Weak, they read.
        (Power::DebilitatePower, PowerAmount::Counted(_), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::DebilitatePower, PowerAmount::Endless, EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            unimplemented!("an endless Debilitate")
        }
        (Power::DebilitatePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ConquerorPower.AfterSideTurnEnd`: one off its stack as its owner's
        // side's turn ends.
        (Power::ConquerorPower, PowerAmount::Counted(_), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::ConquerorPower, PowerAmount::Endless, EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            unimplemented!("an endless Conqueror")
        }
        (Power::ConquerorPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ArtifactPower.AfterModifyingPowerAmountReceived`: one used.
        (
            Power::ArtifactPower,
            PowerAmount::Counted(_),
            EventHook::PowerAmountReceivedModified { .. },
        ) => Some(HookEffect::Decrement),
        (
            Power::ArtifactPower,
            PowerAmount::Endless,
            EventHook::PowerAmountReceivedModified { .. },
        ) => unimplemented!("an endless Artifact"),
        // `TangledPower.AfterApplied`: Entangled on every Attack its owner has.
        // `RingingPower.AfterApplied`: Ringing on every card its owner has.
        (Power::RingingPower, _, EventHook::Applied) => Some(HookEffect::Afflict {
            to: sts2_core::Writing::Every,
            affliction: Affliction::Ringing,
        }),
        // `RingingPower.AfterCardEnteredCombat`: Ringing on a card that
        // arrives with no affliction.
        (Power::RingingPower, _, EventHook::CardEnteredCombat { at, .. }) => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: Affliction::Ringing,
            })
        }
        // `RingingPower.AfterSideTurnEnd`: gone as its owner's side ends a
        // turn.
        (Power::RingingPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::RingingPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `RingingPower.AfterRemoved`: Ringing off every card that has it.
        (Power::RingingPower, _, EventHook::Removed) => Some(HookEffect::ClearAffliction(
            sts2_core::Writing::EveryAfflictedWith(Affliction::Ringing),
        )),
        (Power::TangledPower, _, EventHook::Applied) => Some(HookEffect::Afflict {
            to: sts2_core::Writing::EveryOfKind(sts2_core::CardKind::Attack),
            affliction: Affliction::Entangled,
        }),
        // `TangledPower.AfterCardEnteredCombat`: Entangled on an Attack that
        // arrives with no affliction.
        (Power::TangledPower, _, EventHook::CardEnteredCombat { at, card, .. })
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: Affliction::Entangled,
            })
        }
        (Power::TangledPower, _, EventHook::CardEnteredCombat { .. }) => None,
        // `TangledPower.AfterSideTurnEnd`: gone as its owner's side ends a
        // turn.
        (Power::TangledPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::TangledPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `TangledPower.AfterRemoved`: Entangled off every card that has it.
        (Power::TangledPower, _, EventHook::Removed) => Some(HookEffect::ClearAffliction(
            sts2_core::Writing::EveryAfflictedWith(Affliction::Entangled),
        )),
        // `DemisePower.AfterSideTurnEnd`: its stack off its owner as its
        // owner's side's turn ends.
        (Power::DemisePower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            Some(HookEffect::DamageOwnerInFight {
                amount,
                unblockable: true,
                self_dealt: false,
            })
        }
        (Power::DemisePower, PowerAmount::Endless, EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            unimplemented!("an endless Demise")
        }
        (Power::DemisePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ConstrictPower.AfterSideTurnEnd`: its stack off its owner, dealt by
        // the owner and unpowered, as its owner's side's turn ends.
        (Power::ConstrictPower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            Some(HookEffect::DamageOwnerInFight {
                amount,
                unblockable: false,
                self_dealt: true,
            })
        }
        (Power::ConstrictPower, PowerAmount::Endless, EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            unimplemented!("an endless Constrict")
        }
        (Power::ConstrictPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ConstrictPower.AfterDeath`: gone with the creature that applied it.
        (
            Power::ConstrictPower,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if applier == Some(creature) => Some(HookEffect::Remove),
        (Power::ConstrictPower, _, EventHook::AfterDeath { .. }) => None,
        // `BeforeCardPlayed`: one off the stack for each card of its kind its
        // owner plays, which stands in the play pile.
        (
            Power::FreeAttackPower
            | Power::FreeSkillPower
            | Power::FreePowerPower
            | Power::VeilpiercerPower,
            _,
            EventHook::CardPlaying { card, ethereal, .. },
        ) => (owner == CombatSide::Player && frees(power, card, ethereal))
            .then_some(HookEffect::Decrement),
        // `IntangiblePower.AfterSideTurnEnd`: one off its stack as the
        // enemies' turn ends.
        (
            Power::IntangiblePower,
            PowerAmount::Counted(_),
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => Some(HookEffect::Decrement),
        (
            Power::IntangiblePower,
            PowerAmount::Endless,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => unimplemented!("an endless Intangible"),
        (Power::IntangiblePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `BorrowedTimePower.AfterSideTurnEnd`: gone once its owner's turn
        // ends.
        (Power::BorrowedTimePower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::BorrowedTimePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `PoisonPower.AfterSideTurnStart`, for its owner's side: `TriggerCount`
        // rounds, which is its stack or one more than the Accelerant its
        // owner's living opponents carry, whichever is fewer.
        (Power::PoisonPower, PowerAmount::Counted(amount), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            let accelerant: Number = fight
                .living_opponents(holder)
                .into_iter()
                .filter_map(|opponent| match fight.power(opponent, Power::AccelerantPower) {
                    Some(PowerAmount::Counted(amount)) => Some(amount),
                    Some(PowerAmount::Endless) => unimplemented!("an endless Accelerant"),
                    None => None,
                })
                .sum();
            Some(HookEffect::DamageSelfByStack {
                times: amount.min(1 + accelerant),
            })
        }
        (Power::PoisonPower, PowerAmount::Endless, EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Poison")
        }
        (Power::PoisonPower, _, EventHook::SideTurnStart { .. }) => None,
        // `NoxiousFumesPower.AfterSideTurnStart`: its stack of Poison onto
        // every enemy that can be hit, from its owner.
        (
            Power::NoxiousFumesPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnStart { side, .. },
        ) if side == owner => Some(HookEffect::ApplyToOpponents(
            Power::PoisonPower,
            PowerAmount::Counted(amount),
        )),
        (Power::NoxiousFumesPower, PowerAmount::Endless, EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Noxious Fumes")
        }
        (Power::NoxiousFumesPower, _, EventHook::SideTurnStart { .. }) => None,
        // `EnvenomPower.AfterDamageGiven`: its stack of Poison onto whoever a
        // powered hit from its owner took hit points from.
        (
            Power::EnvenomPower,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer,
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) && unblocked > 0 => Some(HookEffect::ApplyPower {
            target,
            power: Power::PoisonPower,
            amount: PowerAmount::Counted(amount),
        }),
        (Power::EnvenomPower, PowerAmount::Endless, EventHook::DamageGiven { dealer, .. })
            if dealer == Some(holder) =>
        {
            unimplemented!("an endless Envenom")
        }
        (Power::EnvenomPower, _, EventHook::DamageGiven { .. }) => None,
        // `NoDrawPower.AfterSideTurnEnd`: gone once its owner's turn ends.
        (Power::NoDrawPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::NoDrawPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `HellraiserPower.AfterCardDrawnEarly`: a Strike its owner draws is
        // auto-played. A fight here has one player, whose cards are all its
        // holder's. The cap it keeps on auto-plays counts only against
        // enemies whose hit points are infinite.
        (Power::HellraiserPower, _, EventHook::CardDrawnEarly { card, id, .. }) => card
            .tagged(CardTag::Strike)
            .then_some(HookEffect::AutoPlay(id)),
        // `HellraiserPower.AfterSideTurnEnd` resets that cap, and its
        // `BeforeAttack` sets how an auto-played Strike's hit looks.
        (Power::HellraiserPower, _, EventHook::SideTurnEnd { .. } | EventHook::BeforeAttack { .. }) => {
            None
        }
        // `ColossusPower.AfterSideTurnEnd`: one off its stack as the enemies'
        // turn ends, whichever side its owner is on.
        (
            Power::ColossusPower,
            PowerAmount::Counted(_),
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => Some(HookEffect::Decrement),
        (
            Power::ColossusPower,
            PowerAmount::Endless,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => unimplemented!("an endless Colossus"),
        (Power::ColossusPower, _, EventHook::SideTurnEnd { .. }) => None,
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
            | Power::FrailPower
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
        // `BlurPower.AfterSideTurnStart`: one off, as its owner's side starts
        // a turn.
        (Power::BlurPower, _, EventHook::SideTurnStart { side, .. }) if side == owner => {
            Some(HookEffect::Decrement)
        }
        (Power::BlurPower, _, EventHook::SideTurnStart { .. }) => None,
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
        // `DarkEmbracePower.AfterCardExhausted`: its stack of cards for each
        // of its owner's cards exhausted, and a count of those Ethereal
        // exhausted, which its `AfterSideTurnEnd` draws for once the hand is
        // flushed. A fight here has one player, whose every card is.
        (Power::DarkEmbracePower, _, EventHook::CardExhausted { ethereal: true, .. })
            if owner == CombatSide::Player =>
        {
            state.count(one);
            None
        }
        (Power::DarkEmbracePower, PowerAmount::Counted(amount), EventHook::CardExhausted { .. })
            if owner == CombatSide::Player =>
        {
            Some(HookEffect::Draw(amount))
        }
        (Power::DarkEmbracePower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            let counted = state.counted();
            state.clear();
            Some(HookEffect::Draw(amount * counted))
        }
        (Power::DarkEmbracePower, PowerAmount::Endless, _) => unimplemented!("an endless Dark Embrace"),
        (Power::DarkEmbracePower, _, EventHook::CardExhausted { .. } | EventHook::SideTurnEnd { .. }) => {
            None
        }
        // `FeelNoPainPower.AfterCardExhausted`: its stack as block, unpowered,
        // for each of its owner's cards exhausted.
        (
            Power::FeelNoPainPower,
            PowerAmount::Counted(amount),
            EventHook::CardExhausted { .. },
        ) if owner == CombatSide::Player => Some(HookEffect::GainBlock(amount)),
        (Power::FeelNoPainPower, PowerAmount::Endless, EventHook::CardExhausted { .. }) => {
            unimplemented!("an endless Feel No Pain")
        }
        (Power::FeelNoPainPower, _, EventHook::CardExhausted { .. }) => None,
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
            EventHook::DamageReceived { target, unblocked, .. },
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
        (Power::SlipperyPower, _, EventHook::DamageReceived { target, unblocked, .. })
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
                ..
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
        // `PlowPower.AfterDamageReceived`: a hit that takes hit points off its
        // owner and leaves it at its stack or below takes the Strength it
        // gained, lent or its own, and stuns it. The Ceremonial Beast comes
        // back with Beast Cry. Then Plow goes.
        (
            Power::PlowPower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target, unblocked, ..
            },
        ) if target == holder
            && unblocked > 0
            && fight
                .current_hp(holder)
                .is_some_and(|hp| Number::from(hp) <= amount) =>
        {
            use sts2_core::Table as _;

            asked.extend(
                Power::ALL
                    .iter()
                    .copied()
                    .filter(|&lent| {
                        lends(lent).is_some_and(|(to, _)| to == Power::StrengthPower)
                            && fight.power(holder, lent).is_some()
                    })
                    .map(HookEffect::RemoveFromOwner),
            );
            asked.push(HookEffect::RemoveFromOwner(Power::StrengthPower));
            asked.push(match monster {
                Some(Monster::CeremonialBeast) => {
                    let cry =
                        super::monsters::state_of(Monster::CeremonialBeast, MoveId::BeastCryMove, 0)
                            .unwrap_or_else(|_| {
                                unreachable!("the Ceremonial Beast's graph holds its cry")
                            });
                    stunned(Some(cry))
                }
                _ => stunned(None),
            });
            asked.push(HookEffect::RemoveFromOwner(Power::PlowPower));
            return;
        }
        (Power::PlowPower, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if target == holder =>
        {
            unimplemented!("an endless Plow")
        }
        (Power::PlowPower, _, EventHook::DamageReceived { .. }) => None,
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
                    source: source.map(|(id, _)| id),
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
        // `GigantificationPower.BeforeAttack`: binds to the first powered
        // attack an Attack card of its owner's makes, and keeps that one.
        (
            Power::GigantificationPower,
            PowerAmount::Counted(_),
            EventHook::BeforeAttack {
                command,
                source: Some((id, card)),
                powered: true,
                ..
            },
        ) if state.giant().is_none()
            && <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack) =>
        {
            *state = State::Gigantification {
                bound: Some((command, id)),
            };
            None
        }
        (Power::GigantificationPower, PowerAmount::Endless, EventHook::BeforeAttack { .. }) => {
            unimplemented!("an endless Gigantification")
        }
        (Power::GigantificationPower, _, EventHook::BeforeAttack { .. }) => None,
        // `GigantificationPower.AfterAttack`: once the command it bound is
        // over, it lets go and takes one off its stack.
        (Power::GigantificationPower, _, EventHook::AfterAttack { command, .. }) => {
            match state.giant() {
                Some((bound, _)) if bound == command => {
                    *state = State::Gigantification { bound: None };
                    Some(HookEffect::Decrement)
                }
                _ => None,
            }
        }
        // `VigorPower.AfterAttack`: what it held as the command began comes
        // off once that command is over.
        (Power::VigorPower, _, EventHook::AfterAttack { command, .. }) => state
            .bound()
            .filter(|bound| bound.command == command)
            .map(|bound| HookEffect::ModifyAmount(-bound.amount)),
        // `FlameBarrierPower.AfterDamageReceived`: a powered attack on its
        // owner takes its stack back off the attacker.
        (
            Power::FlameBarrierPower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                dealer: Some(dealer),
                powered: true,
                ..
            },
        ) if target == holder => Some(HookEffect::Damage {
            target: dealer,
            amount,
        }),
        (Power::FlameBarrierPower, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if target == holder =>
        {
            unimplemented!("an endless Flame Barrier")
        }
        (Power::FlameBarrierPower, _, EventHook::DamageReceived { .. }) => None,
        // `ThornsPower.BeforeDamageReceived`: its stack at whoever hits its
        // owner with a powered attack or an Omnislice, unpowered, before the
        // hit meets block.
        (
            Power::ThornsPower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceiving {
                target,
                dealer: Some(dealer),
                powered,
                card,
                ..
            },
        ) if target == holder
            && (powered || card == Some(Cards::Colorless(ColorlessCard::Omnislice))) =>
        {
            Some(HookEffect::Damage {
                target: dealer,
                amount,
            })
        }
        (Power::ThornsPower, PowerAmount::Endless, EventHook::DamageReceiving { target, .. })
            if target == holder =>
        {
            unimplemented!("an endless Thorns")
        }
        (Power::ThornsPower, _, EventHook::DamageReceiving { .. }) => None,
        // `AutomationPower.AfterCardDrawn`: its stack of energy each tenth
        // card its owner draws. A fight here has one player, whose cards are
        // all its holder's.
        (Power::AutomationPower, PowerAmount::Counted(amount), EventHook::CardDrawn { .. }) => {
            let State::Automation { cards_left } = state else {
                unreachable!("cards left for {:?}", state.power())
            };
            *cards_left -= 1;
            match *cards_left {
                0 => {
                    *cards_left = AUTOMATION_CARDS;
                    Some(HookEffect::GainEnergy(amount))
                }
                _ => None,
            }
        }
        (Power::AutomationPower, PowerAmount::Endless, EventHook::CardDrawn { .. }) => {
            unimplemented!("an endless Automation")
        }
        // `ReflectPower.AfterDamageReceived`: what the block took off a
        // powered attack on its owner goes back at the attacker, unpowered.
        (
            Power::ReflectPower,
            _,
            EventHook::DamageReceived {
                target,
                blocked,
                dealer: Some(dealer),
                powered: true,
                ..
            },
        ) if target == holder && blocked > num_traits::zero() => Some(HookEffect::Damage {
            target: dealer,
            amount: blocked,
        }),
        (Power::ReflectPower, _, EventHook::DamageReceived { .. }) => None,
        // `ReflectPower.AfterSideTurnStart`: one off its stack as its owner's
        // side's turn starts.
        (Power::ReflectPower, PowerAmount::Counted(_), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::ReflectPower, PowerAmount::Endless, EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Reflect")
        }
        (Power::ReflectPower, _, EventHook::SideTurnStart { .. }) => None,
        // `ClarityPower.AfterSideTurnStart`: one off its stack as its owner's
        // side's turn starts.
        (Power::ClarityPower, PowerAmount::Counted(_), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::ClarityPower, PowerAmount::Endless, EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Clarity")
        }
        (Power::ClarityPower, _, EventHook::SideTurnStart { .. }) => None,
        // `FlameBarrierPower.AfterSideTurnEnd`, which goes once the other side
        // has had its turn.
        (Power::FlameBarrierPower, _, EventHook::SideTurnEnd { side }) if side != owner => {
            Some(HookEffect::Remove)
        }
        // `DuplicationPower.AfterModifyingCardPlayCount`, and its
        // `AfterSideTurnEnd` for its owner's own side, the same as One-Two
        // Punch's.
        (Power::DuplicationPower, _, EventHook::PlayCountModified { .. }) => {
            Some(HookEffect::Decrement)
        }
        (Power::DuplicationPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::DuplicationPower, _, EventHook::SideTurnEnd { .. }) => None,
        (Power::BurstPower | Power::SignalBoostPower, _, EventHook::PlayCountModified { .. }) => {
            Some(HookEffect::Decrement)
        }
        // `EchoFormPower.AfterModifyingCardPlayCount` does nothing.
        (Power::EchoFormPower, _, EventHook::PlayCountModified { .. }) => None,
        (Power::BurstPower | Power::ShadowmeldPower | Power::CorrosiveWavePower, _, EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            Some(HookEffect::Remove)
        }
        (Power::BurstPower | Power::ShadowmeldPower | Power::CorrosiveWavePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `NoBlockPower.AfterSideTurnEnd`: one off as the enemies' turn
        // ends, whoever holds it.
        (
            Power::NoBlockPower,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
            },
        ) => {
            Some(HookEffect::Decrement)
        }
        (Power::NoBlockPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `CorrosiveWavePower.AfterCardDrawn`: its stack of Poison onto every
        // enemy that can be hit, for each of its owner's cards drawn. A fight
        // here has one player, whose every card is.
        (Power::CorrosiveWavePower, amount, EventHook::CardDrawn { .. })
            if owner == CombatSide::Player =>
        {
            Some(HookEffect::ApplyToOpponents(Power::PoisonPower, amount))
        }
        (Power::CorrosiveWavePower, _, EventHook::CardDrawn { .. }) => None,
        // `SpeedsterPower.AfterCardDrawn`: its stack to every enemy that can
        // be hit, unpowered, for a card of its owner's drawn outside the hand
        // draw and in its owner's own turn.
        (
            Power::SpeedsterPower,
            PowerAmount::Counted(amount),
            EventHook::CardDrawn {
                hand_draw: false,
                side,
                ..
            },
        ) if owner == CombatSide::Player && side == owner => Some(HookEffect::DamageOpponents(amount)),
        (Power::SpeedsterPower, PowerAmount::Endless, EventHook::CardDrawn { hand_draw: false, side, .. })
            if owner == CombatSide::Player && side == owner =>
        {
            unimplemented!("an endless Speedster")
        }
        (Power::SpeedsterPower, _, EventHook::CardDrawn { .. }) => None,
        // `PrepTimePower.AfterSideTurnStart`: its stack of Vigor onto its
        // owner, as its owner's side starts a turn.
        (Power::PrepTimePower, amount, EventHook::SideTurnStart { side, .. }) if side == owner => {
            Some(HookEffect::ApplyToOwner(Power::VigorPower, amount))
        }
        (Power::PrepTimePower, _, EventHook::SideTurnStart { .. }) => None,
        // `DemonFormPower.AfterSideTurnStart`: its stack of Strength onto its
        // owner, as its owner's side starts a turn.
        (Power::DemonFormPower, amount, EventHook::SideTurnStart { side, .. }) if side == owner => {
            Some(HookEffect::ApplyToOwner(Power::StrengthPower, amount))
        }
        (Power::DemonFormPower, _, EventHook::SideTurnStart { .. }) => None,
        // `DanseMacabrePower.BeforeCardPlayed`: its stack as block, unpowered,
        // before each of its owner's cards resolved at two or more.
        (
            Power::DanseMacabrePower,
            PowerAmount::Counted(amount),
            EventHook::CardPlaying { resolved, .. },
        ) if owner == CombatSide::Player && resolved >= DANSE_MACABRE_ENERGY => {
            Some(HookEffect::GainBlock(amount))
        }
        (Power::DanseMacabrePower, PowerAmount::Endless, EventHook::CardPlaying { resolved, .. })
            if owner == CombatSide::Player && resolved >= DANSE_MACABRE_ENERGY =>
        {
            unimplemented!("an endless Danse Macabre")
        }
        (Power::DanseMacabrePower, _, EventHook::CardPlaying { .. }) => None,
        // `TheGambitPower.AfterDamageReceived`: a powered hit that takes hit
        // points from its owner removes it, then kills the owner.
        (
            Power::TheGambitPower,
            _,
            EventHook::DamageReceived {
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if target == holder && unblocked > 0 => {
            asked.extend([HookEffect::Remove, HookEffect::KillOwner]);
            return;
        }
        (Power::TheGambitPower, _, EventHook::DamageReceived { .. }) => None,
        // `MonarchsGazePower.AfterDamageGiven`: its stack of Monarch's Gaze
        // Strength Down onto whoever a powered hit from its owner lands on.
        (
            Power::MonarchsGazePower,
            amount,
            EventHook::DamageGiven {
                dealer: Some(dealer),
                target,
                powered: true,
                ..
            },
        ) if dealer == holder => Some(HookEffect::ApplyPower {
            target,
            power: Power::MonarchsGazeStrengthDownPower,
            amount,
        }),
        (Power::MonarchsGazePower, _, EventHook::DamageGiven { .. }) => None,
        // `RegenPower.AfterSideTurnEnd`: its stack healed, then one off, as
        // its owner's side ends a turn.
        (Power::RegenPower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side })
            if side == owner =>
        {
            asked.extend([HookEffect::Heal(amount), HookEffect::Decrement]);
            return;
        }
        (Power::RegenPower, PowerAmount::Endless, EventHook::SideTurnEnd { side }) if side == owner => {
            unimplemented!("an endless Regen")
        }
        (Power::RegenPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `RitualPower.AfterSideTurnEnd`: its stack of Strength as its
        // owner's side ends a turn. A monster's skips the turn it was applied
        // in, through `AfterApplied`.
        (Power::RitualPower, amount, EventHook::SideTurnEnd { side })
            if side == owner && owner == CombatSide::Player =>
        {
            Some(HookEffect::ApplyToOwner(Power::StrengthPower, amount))
        }
        (Power::RitualPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            unimplemented!("a monster's Ritual, which skips the turn it was applied in")
        }
        (Power::RitualPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `RitualPower.AfterApplied`, which marks a monster's alone.
        (Power::RitualPower, _, EventHook::Applied) if owner == CombatSide::Player => None,
        (Power::RitualPower, _, EventHook::Applied) => {
            unimplemented!("a monster's Ritual, which skips the turn it was applied in")
        }
        // `RadiancePower.AfterEnergyReset`: its `Energy`, then one off.
        (Power::RadiancePower, PowerAmount::Counted(_), EventHook::EnergyReset { player })
            if player == holder =>
        {
            asked.extend([HookEffect::GainEnergy(RADIANCE_ENERGY), HookEffect::Decrement]);
            return;
        }
        (Power::RadiancePower, PowerAmount::Endless, EventHook::EnergyReset { player })
            if player == holder =>
        {
            unimplemented!("an endless Radiance")
        }
        (Power::RadiancePower, _, EventHook::EnergyReset { .. }) => None,
        // `OneTwoPunchPower.AfterModifyingCardPlayCount`, and its
        // `AfterSideTurnEnd` for its owner's own side.
        (Power::OneTwoPunchPower, _, EventHook::PlayCountModified { .. }) => {
            Some(HookEffect::Decrement)
        }
        (Power::OneTwoPunchPower, _, EventHook::SideTurnEnd { side }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::FlameBarrierPower | Power::OneTwoPunchPower, _, EventHook::SideTurnEnd { .. }) => {
            None
        }
        // `AggressionPower.BeforeSideTurnStart`, for its owner's own side.
        (
            Power::AggressionPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnStart { side, .. },
        ) if side == owner => Some(HookEffect::AttacksFromDiscard(amount)),
        (Power::AggressionPower, PowerAmount::Endless, EventHook::BeforeSideTurnStart { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Aggression")
        }
        (Power::AggressionPower, _, EventHook::BeforeSideTurnStart { .. }) => None,
        // `DoomPower.BeforeSideTurnEnd`, for its owner's own side.
        (Power::DoomPower, _, EventHook::BeforeSideTurnEnd { side }) => match side == owner {
            true => Some(HookEffect::DoomKill),
            false => None,
        },
        // `HailstormPower.BeforeSideTurnEnd`: its stack of damage to every enemy
        // while its owner holds a Frost orb.
        (Power::HailstormPower, PowerAmount::Counted(amount), EventHook::BeforeSideTurnEnd { side })
            if side == owner =>
        {
            Some(HookEffect::DamageOpponentsHolding {
                orb: Orb::FrostOrb,
                amount,
            })
        }
        (Power::HailstormPower, PowerAmount::Endless, EventHook::BeforeSideTurnEnd { side })
            if side == owner =>
        {
            unimplemented!("an endless Hailstorm")
        }
        (Power::HailstormPower, _, EventHook::BeforeSideTurnEnd { .. }) => None,
        // `NeurosurgePower.AfterSideTurnStart`: its stack of Doom on its own
        // owner, each of its owner's turns.
        (Power::NeurosurgePower, PowerAmount::Counted(amount), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            Some(HookEffect::ApplyToOwner(
                Power::DoomPower,
                PowerAmount::Counted(amount),
            ))
        }
        // `CountdownPower.AfterSideTurnStart`: its stack of Doom on one enemy
        // that can be hit.
        (Power::CountdownPower, PowerAmount::Counted(amount), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            Some(HookEffect::ApplyToRandomOpponent {
                power: Power::DoomPower,
                amount: PowerAmount::Counted(amount),
            })
        }
        (
            Power::NeurosurgePower | Power::CountdownPower,
            PowerAmount::Endless,
            EventHook::SideTurnStart { side, .. },
        ) if side == owner => unimplemented!("an endless {power:?}"),
        (Power::NeurosurgePower | Power::CountdownPower, _, EventHook::SideTurnStart { .. }) => {
            None
        }
        // `ReaperFormPower.AfterDamageGiven`: a powered hit from its owner, or
        // from its owner's pet, puts Doom on what it hit, its stack for each
        // point the hit dealt.
        (
            Power::ReaperFormPower,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer,
                pet_of,
                target,
                total,
                powered: true,
                ..
            },
        ) if (dealer == Some(holder) || pet_of == Some(holder)) && total > 0 => {
            Some(HookEffect::ApplyPower {
                target,
                power: Power::DoomPower,
                amount: PowerAmount::Counted(total * amount),
            })
        }
        (Power::ReaperFormPower, PowerAmount::Endless, EventHook::DamageGiven { .. }) => {
            unimplemented!("an endless Reaper Form")
        }
        (Power::ReaperFormPower, _, EventHook::DamageGiven { .. }) => None,
        // `BufferPower.AfterModifyingHpLostAfterOsty`: one spent on each hit it
        // took to nothing.
        (Power::BufferPower, _, EventHook::HpLostModified) => Some(HookEffect::Decrement),
        // `IntangiblePower.AfterModifyingHpLostAfterOsty` flashes, which is
        // all it does.
        (Power::IntangiblePower, _, EventHook::HpLostModified) => None,
        // `IterationPower.AfterCardDrawn`: the first Status card its owner
        // draws in a turn draws its stack more. The draw is in the history
        // before the hook, so the first is a count of one. A fight here has
        // one player, so every draw is its owner's.
        (Power::IterationPower, PowerAmount::Counted(amount), EventHook::CardDrawn { card, .. })
            if status(card) =>
        {
            match history.drawn_this_turn().filter(|drawn| status(*drawn)).count() <= 1 {
                true => Some(HookEffect::Draw(amount)),
                false => None,
            }
        }
        (Power::IterationPower, PowerAmount::Endless, EventHook::CardDrawn { card, .. })
            if status(card) =>
        {
            unimplemented!("an endless Iteration")
        }
        (Power::IterationPower, _, EventHook::CardDrawn { .. }) => None,
        // `FeralPower.AfterApplied`: the zero-cost attacks its owner started
        // this turn count against it from the start.
        (Power::FeralPower, _, EventHook::Applied) => {
            let played = history
                .started_this_turn()
                .filter(|started| started.energy == 0 && attack(started.card))
                .count();
            state.set_feral_played(num_traits::cast(played).expect("a turn's plays fit a number"));
            None
        }
        // `FeralPower.AfterModifyingCardPlayResultPileOrPosition` counts the
        // one it sent back, and `AfterSideTurnStart` for its owner's side
        // forgets the count.
        // `NostalgiaPower.AfterModifyingCardPlayResultPileOrPosition` only
        // flashes.
        (Power::NostalgiaPower, _, EventHook::ResultPileModified { .. }) => None,
        (Power::FeralPower, _, EventHook::ResultPileModified { .. }) => {
            state.set_feral_played(state.feral_played() + one);
            None
        }
        (Power::FeralPower, _, EventHook::SideTurnStart { side, .. }) => {
            if side == owner {
                state.set_feral_played(0);
            }
            None
        }
        // `CoolantPower.AfterSideTurnStart`: its stack of block for each kind
        // of orb its owner holds.
        (Power::CoolantPower, PowerAmount::Counted(amount), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            Some(HookEffect::GainBlockPerOrbKind(amount))
        }
        (Power::CoolantPower, PowerAmount::Endless, EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Coolant")
        }
        (Power::CoolantPower, _, EventHook::SideTurnStart { .. }) => None,
        // `LoopPower.AfterPlayerTurnStart`: the first orb's passive, its stack
        // of times.
        (Power::LoopPower, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player })
            if player == holder =>
        {
            Some(HookEffect::FirstOrbPassive(amount))
        }
        (Power::LoopPower, PowerAmount::Endless, EventHook::PlayerTurnStart { player })
            if player == holder =>
        {
            unimplemented!("an endless Loop")
        }
        (Power::LoopPower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `CreativeAiPower.BeforeHandDraw`: its stack of Power cards from the
        // character's pool, one at a time.
        (
            Power::CreativeAiPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => Some(HookEffect::GenerateFromPool {
            filter: sts2_core::PoolFilter::OfKind(sts2_core::CardKind::Power),
            count: amount,
            keyword: None,
        }),
        (Power::CreativeAiPower, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Creative AI")
        }
        (Power::CreativeAiPower, _, EventHook::BeforeHandDraw { .. }) => None,
        // `SpectrumShiftPower.BeforeHandDraw`: its stack of distinct
        // colorless cards.
        (
            Power::SpectrumShiftPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => Some(HookEffect::GenerateDistinctColorless { count: amount }),
        (Power::SpectrumShiftPower, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Spectrum Shift")
        }
        (Power::SpectrumShiftPower, _, EventHook::BeforeHandDraw { .. }) => None,
        // `CallOfTheVoidPower.BeforeHandDraw`: its stack of cards from the
        // character's pool, none of them Basic or Ancient, each made Ethereal.
        (
            Power::CallOfTheVoidPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => Some(HookEffect::GenerateFromPool {
            filter: sts2_core::PoolFilter::NotBasicOrAncient,
            count: amount,
            keyword: Some(CardKeyword::Ethereal),
        }),
        // `SentryModePower.BeforeHandDraw`: its stack of Sweeping Gazes.
        (
            Power::SentryModePower,
            PowerAmount::Counted(amount),
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => Some(HookEffect::GenerateInHand {
            card: Cards::Token(TokenCard::SweepingGaze),
            count: amount,
        }),
        (
            Power::CallOfTheVoidPower | Power::SentryModePower,
            PowerAmount::Endless,
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => unimplemented!("an endless {power:?}"),
        (Power::CallOfTheVoidPower | Power::SentryModePower, _, EventHook::BeforeHandDraw { .. }) => {
            None
        }
        // `PagestormPower.AfterCardDrawn`: an Ethereal card its owner draws
        // draws its stack more. A fight here has one player, so every draw is
        // its owner's.
        (Power::PagestormPower, PowerAmount::Counted(amount), EventHook::CardDrawn { ethereal: true, .. }) => {
            Some(HookEffect::Draw(amount))
        }
        (Power::PagestormPower, PowerAmount::Endless, EventHook::CardDrawn { ethereal: true, .. }) => {
            unimplemented!("an endless Pagestorm")
        }
        (Power::PagestormPower, _, EventHook::CardDrawn { .. }) => None,
        // `SummonNextTurnPower.AfterPlayerTurnStart`: once the stack was there
        // as the turn began, Osty is summoned for it, and the power goes.
        (
            Power::SummonNextTurnPower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player },
        ) if player == holder && held.on_turn_start != PowerAmount::Counted(0) => {
            asked.extend([HookEffect::Summon(amount), HookEffect::Remove]);
            return;
        }
        (Power::SummonNextTurnPower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `GenesisPower.AfterEnergyReset`: its stack of stars, every turn.
        (Power::GenesisPower, PowerAmount::Counted(amount), EventHook::EnergyReset { player })
            if player == holder =>
        {
            Some(HookEffect::GainStars(amount))
        }
        // `LightningRodPower.AfterEnergyReset`: a Lightning orb for its owner,
        // then one off the stack.
        (
            Power::LightningRodPower,
            PowerAmount::Counted(_),
            EventHook::EnergyReset { player },
        ) if player == holder => {
            asked.extend([HookEffect::Channel(Orb::LightningOrb), HookEffect::Decrement]);
            return;
        }
        (Power::LightningRodPower, PowerAmount::Endless, EventHook::EnergyReset { player })
            if player == holder =>
        {
            unimplemented!("an endless LightningRodPower")
        }
        (Power::LightningRodPower, _, EventHook::EnergyReset { .. }) => None,
        // `StarNextTurnPower.AfterEnergyReset`: its stars once, then it goes.
        (
            Power::StarNextTurnPower,
            PowerAmount::Counted(amount),
            EventHook::EnergyReset { player },
        ) if player == holder => {
            asked.extend([HookEffect::GainStars(amount), HookEffect::Remove]);
            return;
        }
        (
            Power::GenesisPower | Power::StarNextTurnPower,
            PowerAmount::Endless,
            EventHook::EnergyReset { player },
        ) if player == holder => unimplemented!("an endless {power:?}"),
        (Power::GenesisPower | Power::StarNextTurnPower, _, EventHook::EnergyReset { .. }) => None,
        // `TyrannyPower.AfterPlayerTurnStart`: its owner picks its stack of
        // cards from hand, and each is exhausted.
        (
            Power::TyrannyPower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player },
        ) if player == holder => Some(HookEffect::SelectFromPile {
            pile: sts2_core::Pile::Hand,
            count: amount,
            act: sts2_core::PileAct::Exhaust,
        }),
        // `ThunderPower.AfterOrbEvoked`: its stack to each creature a
        // Lightning's evoke hit that is still alive, unpowered. A fight here
        // has one player, whose every orb is its owner's.
        (
            Power::ThunderPower,
            PowerAmount::Counted(amount),
            EventHook::OrbEvoked {
                orb: Orb::LightningOrb,
            },
        ) if owner == CombatSide::Player => Some(HookEffect::DamageTargets(amount)),
        (Power::ThunderPower, _, EventHook::OrbEvoked { .. }) => None,
        // `EntropyPower.AfterPlayerTurnStart`: its owner picks its stack of
        // cards from hand, and each is transformed at random.
        (
            Power::EntropyPower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player },
        ) if player == holder => Some(HookEffect::SelectFromPile {
            pile: sts2_core::Pile::Hand,
            count: amount,
            act: sts2_core::PileAct::TransformToRandom,
        }),
        (Power::EntropyPower, PowerAmount::Endless, EventHook::PlayerTurnStart { player })
            if player == holder =>
        {
            unimplemented!("an endless Entropy")
        }
        (Power::EntropyPower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `RollingBoulderPower.AfterPlayerTurnStart`: its stack to each enemy
        // that can be hit, unpowered, then its `DamageVar(5)` more on itself.
        // A headless game runs in `TestMode`, where the damage lands at once.
        (
            Power::RollingBoulderPower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player },
        ) if player == holder => {
            asked.extend([
                HookEffect::DamageOpponents(amount),
                HookEffect::SetAmount(amount + ROLLING_BOULDER_GROWTH),
            ]);
            return;
        }
        (Power::RollingBoulderPower, PowerAmount::Endless, EventHook::PlayerTurnStart { player })
            if player == holder =>
        {
            unimplemented!("an endless Rolling Boulder")
        }
        (Power::RollingBoulderPower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `ToolsOfTheTradePower.AfterPlayerTurnStart`: the same, discarded.
        (
            Power::ToolsOfTheTradePower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player },
        ) if player == holder => Some(HookEffect::SelectFromPile {
            pile: sts2_core::Pile::Hand,
            count: amount,
            act: sts2_core::PileAct::Discard,
        }),
        (
            Power::TyrannyPower | Power::ToolsOfTheTradePower,
            PowerAmount::Endless,
            EventHook::PlayerTurnStart { player },
        ) if player == holder => unimplemented!("an endless {power:?}"),
        (
            Power::TyrannyPower | Power::ToolsOfTheTradePower,
            _,
            EventHook::PlayerTurnStart { .. },
        ) => None,
        // `ForegoneConclusionPower.BeforeHandDraw`: its owner picks its stack
        // of cards from the draw pile into hand, and the power goes.
        (
            Power::ForegoneConclusionPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => {
            asked.extend([
                HookEffect::ShuffleIfNecessary,
                HookEffect::SelectFromPile {
                    pile: sts2_core::Pile::Draw,
                    count: amount,
                    act: sts2_core::PileAct::IntoHand,
                },
                HookEffect::Remove,
            ]);
            return;
        }
        (
            Power::ForegoneConclusionPower,
            PowerAmount::Endless,
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => unimplemented!("an endless Foregone Conclusion"),
        (Power::ForegoneConclusionPower, _, EventHook::BeforeHandDraw { .. }) => None,
        // `InfiniteBladesPower.BeforeHandDraw`: its stack of Shivs into its
        // owner's hand, before the hand is drawn.
        (
            Power::InfiniteBladesPower,
            PowerAmount::Counted(amount),
            EventHook::BeforeHandDraw { player, .. },
        ) if player == holder => Some(HookEffect::GenerateInHand {
            card: Cards::Token(TokenCard::Shiv),
            count: amount,
        }),
        (Power::InfiniteBladesPower, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
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
        (Power::PhantomBladesPower, _, EventHook::CardEnteredCombat { at, card, .. })
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
        // `AfterApplied` and `AfterRemoved` where the class overrides them for
        // what it shows: a name in its text, a size, a sound.
        (
            Power::BarricadePower | Power::ShrinkPower | Power::PlatingPower,
            _,
            EventHook::Applied,
        )
        | (Power::ShrinkPower | Power::SlumberPower, _, EventHook::Removed) => None,
        // `IllusionPower.AfterApplied` applies Minion to its owner, unless it
        // has Minion already. The Fogmog's illusion is the one creature this
        // build puts Illusion on, and it applies Minion right after.
        (Power::IllusionPower, _, EventHook::Applied) => None,
        (Power::SwordSagePower, _, EventHook::Removed) => {
            unimplemented!("the replays Sword Sage takes back off the blades as it goes")
        }
        // `BeforeApplied`, `AfterApplied` and `AfterRemoved` are a power's
        // own, and a move being performed is the move state's. Every power is
        // asked these, and one whose class does not override them does
        // nothing.
        (_, _, EventHook::MonsterMovePerformed { .. }) => None,
        (_, _, hook)
            if hook
                .power_virtual()
                .is_some_and(|method| !power.overrides_virtual(method)) =>
        {
            None
        }
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    };
    asked.extend(effect);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A power whose `Type` the table leaves computed has to lend, or asking
    /// whether it is a debuff panics in the middle of a fight. Hotfix did, on
    /// its first play.
    #[test]
    fn every_computed_type_is_a_lend() {
        use sts2_core::{PowerTable as _, Table as _};

        let unanswered: Vec<Power> = Power::ALL
            .iter()
            .copied()
            .filter(|&power| power.debuff().is_none() && lends(power).is_none())
            .collect();
        assert!(
            unanswered.is_empty(),
            "computed types nothing answers: {unanswered:?}"
        );
    }
}
