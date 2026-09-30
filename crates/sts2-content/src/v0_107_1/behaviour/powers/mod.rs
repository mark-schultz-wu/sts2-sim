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
    CardTable as _, Fixed, Modify, ModifyHook, PowerAmount, PowerModel as _, PowerTable as _,
};

use super::super::V0_107_1 as V;
use super::super::generated::afflictions::Affliction;
use super::super::generated::cards::{
    CardKeyword, CardTag, Cards, ColorlessCard, NecrobinderCard, StatusCard, TokenCard,
};
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
    /// `JugglingPower`'s `attacksPlayedThisTurn`.
    Juggling {
        counted: Number,
    },
    Imbalanced {
        off_balance: bool,
    },
    /// `RitualPower`'s `WasJustAppliedByEnemy`.
    Ritual {
        just_applied: bool,
    },
    /// `PossessStrengthPower`'s `StolenStrength` and `PossessSpeedPower`'s
    /// `StolenDexterity`, for the one player a fight here has.
    Possess {
        power: Power,
        stolen: Number,
    },
    /// `IllusionPower`'s `isReviving`.
    Illusion {
        reviving: bool,
    },
    /// `ReattachPower`'s `isReviving`.
    Reattach {
        reviving: bool,
    },
    /// `AdaptablePower`'s `isReviving`.
    Adaptable {
        reviving: bool,
    },
    /// `NemesisPower._shouldApplyIntangible`.
    Nemesis {
        intangible: bool,
    },
    /// `ToricToughnessPower`'s `BlockVar`, which `SetBlock` writes.
    ToricToughness {
        block: Number,
    },
    /// The `SelfDamage` of `InfernoPower` or `CrimsonMantlePower`, which
    /// `IncrementSelfDamage` raises.
    SelfDamage {
        power: Power,
        self_damage: Number,
    },
    /// `NightmarePower`'s `selectedCard`, a clone the fight keeps, which
    /// `SetSelectedCard` sets just after the power lands.
    Nightmare {
        card: Option<sts2_core::CardId<V>>,
    },
    /// `WitheringPresencePower`'s `CardsLeft`.
    WitheringPresence {
        cards_left: u8,
    },
    /// `ChainsOfBindingPower`'s `boundCardPlayed`.
    ChainsOfBinding {
        bound_played: bool,
    },
    /// `SwipePower.StolenCard`'s deck version.
    Swipe {
        stolen: Option<sts2_core::DeckCardId>,
    },
    /// `VigorPower`'s `commandToModify`, which stays bound once the command
    /// is over.
    Vigor {
        bound: Option<Bound>,
    },
    /// `OutbreakPower`'s `timesPoisoned`.
    Outbreak {
        poisoned: u8,
    },
    /// `TheBombPower`'s `DamageVar`, which `SetDamage` writes.
    TheBomb {
        damage: Number,
    },
    /// `VoidFormPower`'s `cardsPlayedThisTurn`.
    VoidForm {
        played: u32,
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
    /// `PanachePower`'s `CardsLeft`, and its `alreadyApplied`, which passes
    /// over the play that applied it.
    Panache {
        cards_left: u8,
        applied: bool,
    },
    /// `AutomationPower`'s `cardsLeft`.
    Automation {
        cards_left: u8,
    },
    /// `CurlUpPower`'s `playedCard`: the copy whose play it waits on.
    CurlUp {
        played: Option<sts2_core::CardId<V>>,
    },
    /// `SurroundedPower.Facing`: whether its owner faces left, where it
    /// starts facing right.
    Surrounded {
        facing_left: bool,
    },
    /// `SlothPower._cardsPlayedThisTurn`.
    Sloth {
        played: Number,
    },
    /// `MonologuePower`: its own `amountsForPlayedCards`, and
    /// `StrengthApplied`, the Strength it has lent this turn.
    Monologue {
        playing: [Option<(sts2_core::CardId<V>, Number)>; NOTED_AT_ONCE],
        applied: Number,
    },
    /// `amountsForPlayedCards`, which Strangle, Oblivion, Afterimage and
    /// Storm keep: each card started and not finished, with the stack as it
    /// began. Rupture's `playedCards` keeps the Strength each play has owed.
    Noted {
        power: Power,
        playing: [Option<(sts2_core::CardId<V>, Number)>; NOTED_AT_ONCE],
    },
}

/// `TheBombPower`'s `DamageVar(40)`, before `SetDamage` writes its own.
const THE_BOMB_DAMAGE: Number = 40;

/// `VoidFormPower.HideTemporaryZeroCostVisual`'s count, which leaves no
/// card free for the rest of the turn.
const VOID_FORM_SPENT: u32 = 999_999_999;

/// `OutbreakPower.poisonThreshold`: every third Poison deals its damage.
const OUTBREAK_EVERY: u8 = 3;

/// How many plays `amountsForPlayedCards` keeps at once: one, and one played
/// inside it.
const NOTED_AT_ONCE: usize = 2;

/// `amountsForPlayedCards.Add`: `id`'s play noted with the stack as it began.
fn note(state: &mut State, id: sts2_core::CardId<V>, amount: Number) {
    let (State::Noted { playing, .. } | State::Monologue { playing, .. }) = state else {
        unreachable!("plays noted for {:?}", state.power())
    };
    let Some(free) = playing.iter_mut().find(|slot| slot.is_none()) else {
        unimplemented!(
            "{:?} noting more than {NOTED_AT_ONCE} plays at once",
            state.power()
        )
    };
    *free = Some((id, amount));
}

/// `playedCards[id] += amount`, where `id`'s play was noted. Whether it
/// was.
fn add_noted(state: &mut State, id: sts2_core::CardId<V>, amount: Number) -> bool {
    let (State::Noted { playing, .. } | State::Monologue { playing, .. }) = state else {
        unreachable!("plays noted for {:?}", state.power())
    };
    match playing.iter_mut().flatten().find(|(noted, _)| *noted == id) {
        Some((_, owed)) => {
            *owed += amount;
            true
        }
        None => false,
    }
}

/// `amountsForPlayedCards.Remove`: the stack noted as `id`'s play began, if
/// it was noted.
/// `CardSelectorPrefs(prompt, count)`: exactly `count` cards picked.
fn exactly(count: Number) -> sts2_core::Selection {
    usize::try_from(count)
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .map(sts2_core::Selection::Exactly)
        .unwrap_or_else(|| unimplemented!("a selection of {count} cards"))
}

fn take_noted(state: &mut State, id: sts2_core::CardId<V>) -> Option<Number> {
    let (State::Noted { playing, .. } | State::Monologue { playing, .. }) = state else {
        unreachable!("plays noted for {:?}", state.power())
    };
    playing
        .iter_mut()
        .find(|slot| slot.is_some_and(|(noted, _)| noted == id))
        .and_then(Option::take)
        .map(|(_, amount)| amount)
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

/// `JugglingPower`'s third attack in a turn.
const JUGGLING_ATTACKS: Number = 3;

/// Where a power keeps the clone of a card the fight holds for it.
///
/// # Panics
///
/// A power that keeps no card.
pub(crate) fn kept_card(state: &mut State, kept: sts2_core::CardId<V>) {
    match state {
        // `NightmarePower.SetSelectedCard`.
        State::Nightmare { card } => *card = Some(kept),
        state => unimplemented!("{:?} keeping a card", state.power()),
    }
}

/// What a card's note does to the power it just applied.
///
/// # Panics
///
/// A power no card writes on.
pub(crate) fn noted(state: &mut State, note: Number) {
    match state {
        // `ToricToughnessPower.SetBlock`.
        State::ToricToughness { block } => *block = note,
        // `IncrementSelfDamage`, once for each note of one.
        State::SelfDamage { self_damage, .. } => *self_damage += note,
        state => unimplemented!("a card writing {note} on {:?}", state.power()),
    }
}

impl sts2_core::PowerModel<V> for State {
    fn new(power: Power) -> Self {
        match power {
            Power::SlowPower => Self::Slow { counted: 0 },
            Power::TenderPower => Self::Tender { counted: 0 },
            Power::DarkEmbracePower => Self::DarkEmbrace { counted: 0 },
            Power::JugglingPower => Self::Juggling { counted: 0 },
            Power::ImbalancedPower => Self::Imbalanced { off_balance: false },
            Power::RitualPower => Self::Ritual {
                just_applied: false,
            },
            Power::PossessStrengthPower | Power::PossessSpeedPower => {
                Self::Possess { power, stolen: 0 }
            }
            Power::IllusionPower => Self::Illusion { reviving: false },
            Power::ReattachPower => Self::Reattach { reviving: false },
            Power::AdaptablePower => Self::Adaptable { reviving: false },
            Power::NemesisPower => Self::Nemesis { intangible: false },
            Power::ToricToughnessPower => Self::ToricToughness { block: 0 },
            Power::InfernoPower | Power::CrimsonMantlePower => Self::SelfDamage {
                power,
                self_damage: 0,
            },
            Power::NightmarePower => Self::Nightmare { card: None },
            Power::WitheringPresencePower => Self::WitheringPresence {
                cards_left: WITHERING_CARDS,
            },
            Power::ChainsOfBindingPower => Self::ChainsOfBinding {
                bound_played: false,
            },
            Power::SwipePower => Self::Swipe { stolen: None },
            Power::VigorPower => Self::Vigor { bound: None },
            Power::FeralPower => Self::Feral { played: 0 },
            Power::OutbreakPower => Self::Outbreak { poisoned: 0 },
            Power::VoidFormPower => Self::VoidForm { played: 0 },
            Power::TheBombPower => Self::TheBomb {
                damage: THE_BOMB_DAMAGE,
            },
            Power::GigantificationPower => Self::Gigantification { bound: None },
            Power::PanachePower => Self::Panache {
                cards_left: PANACHE_CARDS,
                applied: false,
            },
            Power::AutomationPower => Self::Automation {
                cards_left: AUTOMATION_CARDS,
            },
            Power::CurlUpPower => Self::CurlUp { played: None },
            Power::SurroundedPower => Self::Surrounded { facing_left: false },
            Power::SlothPower => Self::Sloth { played: 0 },
            Power::MonologuePower => Self::Monologue {
                playing: [None; NOTED_AT_ONCE],
                applied: 0,
            },
            Power::StranglePower
            | Power::OblivionPower
            | Power::AfterimagePower
            | Power::SubroutinePower
            | Power::StormPower
            | Power::CalamityPower
            | Power::RupturePower
            | Power::SerpentFormPower => Self::Noted {
                power,
                playing: [None; NOTED_AT_ONCE],
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
            Self::Juggling { .. } => Power::JugglingPower,
            Self::Imbalanced { .. } => Power::ImbalancedPower,
            Self::Ritual { .. } => Power::RitualPower,
            Self::Possess { power, .. } => *power,
            Self::Illusion { .. } => Power::IllusionPower,
            Self::Reattach { .. } => Power::ReattachPower,
            Self::Adaptable { .. } => Power::AdaptablePower,
            Self::Nemesis { .. } => Power::NemesisPower,
            Self::ToricToughness { .. } => Power::ToricToughnessPower,
            Self::SelfDamage { power, .. } => *power,
            Self::Nightmare { .. } => Power::NightmarePower,
            Self::WitheringPresence { .. } => Power::WitheringPresencePower,
            Self::ChainsOfBinding { .. } => Power::ChainsOfBindingPower,
            Self::Swipe { .. } => Power::SwipePower,
            Self::Outbreak { .. } => Power::OutbreakPower,
            Self::VoidForm { .. } => Power::VoidFormPower,
            Self::TheBomb { .. } => Power::TheBombPower,
            Self::Vigor { .. } => Power::VigorPower,
            Self::Feral { .. } => Power::FeralPower,
            Self::Gigantification { .. } => Power::GigantificationPower,
            Self::Automation { .. } => Power::AutomationPower,
            Self::Panache { .. } => Power::PanachePower,
            Self::CurlUp { .. } => Power::CurlUpPower,
            Self::Surrounded { .. } => Power::SurroundedPower,
            Self::Sloth { .. } => Power::SlothPower,
            Self::Noted { power, .. } => *power,
            Self::Monologue { .. } => Power::MonologuePower,
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
pub(crate) fn lends(power: Power) -> Option<(Power, bool)> {
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
            Self::Slow { counted }
            | Self::Tender { counted }
            | Self::DarkEmbrace { counted }
            | Self::Juggling { counted } => *counted,
            _ => 0,
        }
    }

    /// `SurroundedPower.Facing`, as whether it faces left.
    fn facing_left(&self) -> bool {
        match self {
            Self::Surrounded { facing_left } => *facing_left,
            state => unreachable!("facing state for {:?}", state.power()),
        }
    }

    /// `SurroundedPower.UpdateDirection`: facing right, a target with
    /// Back Attack Left turns its owner left, and the other way round.
    fn turn_toward(&mut self, fight: &dyn sts2_core::FightView<V>, target: sts2_core::CombatId) {
        let Self::Surrounded { facing_left } = self else {
            unreachable!("facing state for {:?}", self.power())
        };
        let behind = match *facing_left {
            false => Power::BackAttackLeftPower,
            true => Power::BackAttackRightPower,
        };
        if fight.power(target, behind).is_some() {
            *facing_left = !*facing_left;
        }
    }

    /// `CurlUpPower`'s `playedCard`.
    fn curl_played(&mut self) -> &mut Option<sts2_core::CardId<V>> {
        match self {
            Self::CurlUp { played } => played,
            state => unreachable!("curl state for {:?}", state.power()),
        }
    }

    fn count(&mut self, amount: Number) {
        match self {
            Self::Slow { counted }
            | Self::Tender { counted }
            | Self::DarkEmbrace { counted }
            | Self::Juggling { counted } => *counted += amount,
            state => unreachable!("counted state for {:?}", state.power()),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Slow { counted }
            | Self::Tender { counted }
            | Self::DarkEmbrace { counted }
            | Self::Juggling { counted } => *counted = 0,
            state => unreachable!("counted state for {:?}", state.power()),
        }
    }

    /// The stolen stack a Possess power keeps, to write on.
    fn stolen_mut(&mut self) -> &mut Number {
        let Self::Possess { stolen, .. } = self else {
            unreachable!("stolen state for {:?}", self.power())
        };
        stolen
    }

    /// `WasJustAppliedByEnemy`, set to `just_applied`, and what it held.
    fn swap_just_applied(&mut self, just_applied: bool) -> bool {
        let Self::Ritual {
            just_applied: current,
        } = self
        else {
            unreachable!("just-applied state for {:?}", self.power())
        };
        std::mem::replace(current, just_applied)
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
            Self::Illusion { reviving }
            | Self::Reattach { reviving }
            | Self::Adaptable { reviving } => *reviving = to,
            state => unreachable!("reviving state for {:?}", state.power()),
        }
    }

    fn reviving(&self) -> bool {
        matches!(
            self,
            Self::Illusion { reviving: true }
                | Self::Reattach { reviving: true }
                | Self::Adaptable { reviving: true }
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

    /// `TheBombPower.SetDamage`.
    pub(crate) fn set_bomb_damage(&mut self, to: Number) {
        let Self::TheBomb { damage } = self else {
            unreachable!("bomb damage for {:?}", self.power())
        };
        *damage = to;
    }

    /// `SwipePower.Steal`, which remembers the deck card taken.
    pub(crate) fn steal(&mut self, card: sts2_core::DeckCardId) {
        let Self::Swipe { stolen } = self else {
            unreachable!("stolen state for {:?}", self.power())
        };
        *stolen = Some(card);
    }
}

/// `CreatureCmd.Stun`'s own state, which advertises a stunned turn.
fn stunned(then: sts2_core::StunResume) -> sts2_core::HookEffect<V> {
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
pub(crate) static STUNNED: LazyLock<sts2_core::NextMove<V>> =
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
/// are summed here in hundredths.
const DAMAGE_INCREASE: i32 = 150;

/// `WeakPower`'s `DamageDecrease`, in hundredths.
const DAMAGE_DECREASE: i32 = 75;

/// `PaperPhrog.ModifyVulnerableMultiplier`'s `0.25m`, in hundredths.
const PHROG: i32 = 25;

/// `PaperKrane.ModifyWeakMultiplier`'s `0.15m`, in hundredths.
const KRANE: i32 = 15;

/// `SurroundedPower`'s `1.5m` on a hit from behind.
const SURROUNDED: Fixed = Fixed::new(15, 1);

/// `CrabRagePower`'s `PowerVar<StrengthPower>(6m)`.
const CRAB_RAGE: Number = 6;

/// `CrabRagePower`'s `BlockVar(99m, ValueProp.Unpowered)`.
const CRAB_BLOCK: Number = 99;

/// `FlutterPower`'s `DamageDecrease` of 50, as the share left.
const FLUTTER: Fixed = Fixed::new(5, 1);

/// `FrailPower.ModifyBlockMultiplicative`'s `0.75m`.
const FRAIL: Fixed = Fixed::new(75, 2);

/// `ColossusPower`'s `DamageDecrease`.
const COLOSSAL: Fixed = Fixed::new(5, 1);

/// `PanachePower._baseCardsLeft`: the plays for each payout.
const PANACHE_CARDS: u8 = 5;

/// `AutomationPower._baseCardsLeft`: the cards its owner draws for each
/// payout.
const AUTOMATION_CARDS: u8 = 10;

/// `PaleBlueDotPower.cardPlayThresholdValue`: the plays its owner finishes in
/// one turn for the next turn's hand to grow.
const PALE_BLUE_DOT_PLAYS: usize = 5;

/// `MonologuePower`'s Strength, which Monologue sets to its own
/// `DynamicVar("Power", 1)`, and which no upgrade changes.
const MONOLOGUE_STRENGTH: Number = 1;

/// `ConfusedPower.NextEnergyCost`'s `NextInt(4)`.
const CONFUSED_COSTS: u8 = 4;

/// `GigantificationPower.ModifyDamageMultiplicative`'s `3m`.
const GIGANTIFIED: Fixed = Fixed::new(3, 0);

/// `ShrinkPower`'s own `DamageDecrease` of 30, which the power stores as the
/// percentage taken off, as the share left.
const SHRUNK: Fixed = Fixed::new(70, 2);

/// What one card played adds to what `SlowPower`'s owner takes, in tenths.
const SLOW_TENTHS: i32 = 1;

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
) -> Fixed {
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
    Fixed::new(by, 2)
}

/// What Weak on `holder` multiplies a powered hit it deals `target` by.
fn weak(
    fight: &dyn sts2_core::FightView<V>,
    holder: sts2_core::CombatId,
    target: Option<sts2_core::CombatId>,
) -> Fixed {
    let mut by = DAMAGE_DECREASE;
    if target.is_some_and(|target| fight.relic(target, Relic::PaperKrane)) {
        by -= KRANE;
    }
    // `amount - (1m - amount)`.
    if fight.power(holder, Power::DebilitatePower).is_some() {
        by -= 100 - by;
    }
    Fixed::new(by, 2)
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
        (PowerAmount::Counted(amount), EventHook::SideTurnEnd { side, .. })
            if side == held.owner =>
        {
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
        // `AfterPowerAmountChanged` on its own instance: the lent power by the
        // change, signed. The instance being put on is `BeforeApplied`'s, and
        // changes by the whole of its amount, which the override passes over.
        (
            PowerAmount::Counted(total),
            EventHook::PowerAmountChanged {
                creature,
                power: changed,
                delta,
                ..
            },
        ) if creature == held.holder && changed == power && delta != total => asked.push(
            HookEffect::ApplyToOwner(lent, PowerAmount::Counted(signed(delta))),
        ),
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
        ) if target == Some(holder) => Modify::Scaled(Fixed::new(10 + SLOW_TENTHS * counted, 1)),
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
        // `HangPower.ModifyDamageMultiplicative`: its stack as a multiplier on
        // a hit on its owner from a Hang.
        (
            Power::HangPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageMultiplicative {
                target: Some(target),
                card:
                    Some(sts2_core::CardSource {
                        card: Cards::Necrobinder(NecrobinderCard::Hang),
                        ..
                    }),
                ..
            },
        ) if target == holder => Modify::Scaled(Fixed::new(amount, 0)),
        (Power::HangPower, PowerAmount::Endless, ModifyHook::DamageMultiplicative { .. }) => {
            unimplemented!("an endless Hang")
        }
        (Power::HangPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `DoubleDamagePower.ModifyDamageMultiplicative`: double a powered
        // hit from a card, by its owner or one of its pets.
        (
            Power::DoubleDamagePower,
            _,
            ModifyHook::DamageMultiplicative {
                dealer: Some(dealer),
                card: Some(_),
                powered: true,
                ..
            },
        ) if dealer == holder || fight.pet_owner(dealer) == Some(holder) => {
            Modify::Scaled(Fixed::new(2, 0))
        }
        (Power::DoubleDamagePower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `TrackingPower.ModifyDamageMultiplicative`: its stack as a
        // multiplier on a powered hit from a card, by its owner or one of its
        // pets, on a target carrying Weak.
        (
            Power::TrackingPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageMultiplicative {
                target: Some(target),
                dealer: Some(dealer),
                card: Some(_),
                powered: true,
            },
        ) if (dealer == holder || fight.pet_owner(dealer) == Some(holder))
            && fight.power(target, Power::WeakPower).is_some() =>
        {
            Modify::Scaled(Fixed::new(amount, 0))
        }
        (Power::TrackingPower, PowerAmount::Endless, ModifyHook::DamageMultiplicative { .. }) => {
            unimplemented!("an endless Tracking")
        }
        (Power::TrackingPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `SoarPower.ModifyDamageMultiplicative`: its `DamageDecrease`, half, of
        // a powered hit on its holder.
        (
            Power::SoarPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Scaled(Fixed::new(5, 1)),
        (Power::SoarPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `DiamondDiademPower.ModifyDamageMultiplicative`: half of a powered
        // hit on its holder.
        (
            Power::DiamondDiademPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Scaled(Fixed::new(5, 1)),
        (Power::DiamondDiademPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `CorruptionPower.TryModifyEnergyCostInCombatLate`: nothing for a
        // Skill its owner holds or is playing. A fight here has one player.
        (
            Power::CorruptionPower,
            _,
            ModifyHook::EnergyCostLate {
                card: (_, card), ..
            },
        ) if owner == CombatSide::Player
            && <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill) =>
        {
            Modify::AtMost(num_traits::zero())
        }
        (Power::CorruptionPower, _, ModifyHook::EnergyCostLate { .. }) => Modify::Nothing,
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
        // `CuriousPower.TryModifyEnergyCostInCombat`: its stack off a Power
        // its owner holds. A cost it would take below nothing is nothing,
        // which the fold's own clamp gives.
        (
            Power::CuriousPower,
            PowerAmount::Counted(amount),
            ModifyHook::EnergyCost {
                card: (_, card), ..
            },
        ) if owner == CombatSide::Player
            && <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power) =>
        {
            Modify::Added(-amount)
        }
        (Power::CuriousPower, PowerAmount::Endless, ModifyHook::EnergyCost { .. }) => {
            unimplemented!("an endless Curious")
        }
        (Power::CuriousPower, _, ModifyHook::EnergyCost { .. }) => Modify::Nothing,
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
        (Power::IntangiblePower, _, ModifyHook::HpLostAfterOsty { target, .. })
            if target == holder =>
        {
            Modify::AtMost(one)
        }
        (
            Power::IntangiblePower,
            _,
            ModifyHook::DamageCap { .. } | ModifyHook::HpLostAfterOsty { .. },
        ) => Modify::Nothing,
        // `FlutterPower.ModifyDamageMultiplicative`: its `DamageDecrease` off
        // a powered attack on its owner.
        (
            Power::FlutterPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Scaled(FLUTTER),
        (Power::FlutterPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `SurroundedPower.ModifyDamageMultiplicative`: half again on a hit
        // on its owner from behind, which is from a dealer with Back Attack
        // Left while it faces right, and the other way round.
        (
            Power::SurroundedPower,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer: Some(dealer),
                ..
            },
        ) if target == Some(holder) => {
            let behind = match state.facing_left() {
                false => Power::BackAttackLeftPower,
                true => Power::BackAttackRightPower,
            };
            match fight.power(dealer, behind) {
                Some(_) => Modify::Scaled(SURROUNDED),
                None => Modify::Nothing,
            }
        }
        (Power::SurroundedPower, _, ModifyHook::DamageMultiplicative { .. }) => Modify::Nothing,
        // `TaintedPower.ModifyDamageAdditive`: its stack onto a powered attack
        // on its owner.
        (
            Power::TaintedPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                target,
                powered: true,
                ..
            },
        ) if target == Some(holder) => Modify::Added(amount),
        (
            Power::TaintedPower,
            PowerAmount::Endless,
            ModifyHook::DamageAdditive {
                target,
                powered: true,
                ..
            },
        ) if target == Some(holder) => unimplemented!("an endless Tainted"),
        (Power::TaintedPower, _, ModifyHook::DamageAdditive { .. }) => Modify::Nothing,
        // `HardToKillPower.ModifyDamageCap`: a hit on its owner capped at its
        // stack, whatever the hit's props.
        (
            Power::HardToKillPower,
            PowerAmount::Counted(amount),
            ModifyHook::DamageCap { target, .. },
        ) if target == Some(holder) => Modify::AtMost(amount),
        (Power::HardToKillPower, PowerAmount::Endless, ModifyHook::DamageCap { target, .. })
            if target == Some(holder) =>
        {
            unimplemented!("an endless Hard To Kill")
        }
        (Power::HardToKillPower, _, ModifyHook::DamageCap { .. }) => Modify::Nothing,
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
            ModifyHook::MaxEnergy { player, .. },
        ) if player == holder => Modify::Added(amount),
        (
            Power::WasteAwayPower,
            PowerAmount::Counted(amount),
            ModifyHook::MaxEnergy { player, .. },
        ) if player == holder => Modify::Added(-amount),
        (
            Power::DemesnePower | Power::PyrePower | Power::FriendshipPower | Power::WasteAwayPower,
            PowerAmount::Endless,
            ModifyHook::MaxEnergy { player, .. },
        ) if player == holder => unimplemented!("an endless {power:?}"),
        // `VoidFormPower.TryModifyEnergyCostInCombatLate` and
        // `TryModifyStarCost`: nothing for its owner's cards while fewer than
        // its stack have been played this turn. A fight here has one player,
        // whose cards are all its holder's, and every cost read is of a card
        // in hand or in play.
        (
            Power::VoidFormPower,
            PowerAmount::Counted(amount),
            ModifyHook::EnergyCostLate { .. } | ModifyHook::StarCost { .. },
        ) if owner == CombatSide::Player => {
            let State::VoidForm { played } = *state else {
                unreachable!("Void Form counts its plays")
            };
            match i64::from(played) < i64::from(amount) {
                true => Modify::Replaced(0),
                false => Modify::Nothing,
            }
        }
        (Power::VoidFormPower, PowerAmount::Endless, ModifyHook::EnergyCostLate { .. })
        | (Power::VoidFormPower, PowerAmount::Endless, ModifyHook::StarCost { .. }) => {
            unimplemented!("an endless Void Form")
        }
        (
            Power::VoidFormPower,
            _,
            ModifyHook::EnergyCostLate { .. } | ModifyHook::StarCost { .. },
        ) => Modify::Nothing,
        // `NoEnergyGainPower.ModifyEnergyGain`: nothing gained, by its owner.
        (Power::NoEnergyGainPower, _, ModifyHook::EnergyGain { player }) if player == holder => {
            Modify::Replaced(0)
        }
        (Power::NoEnergyGainPower, _, ModifyHook::EnergyGain { .. }) => Modify::Nothing,
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
                false => Modify::Scaled(Fixed::new(100 + amount, 2)),
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
        ) if target == Some(holder) => Modify::Scaled(Fixed::new(2, 0)),
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
        ) if target == holder => {
            // `(decimal)Math.Pow(2.0, Amount)`, which is exact for any stack
            // a fight reaches.
            let doubled = u32::try_from(amount)
                .ok()
                .and_then(|amount| 1_i32.checked_shl(amount))
                .unwrap_or_else(|| unimplemented!("Shadowmeld at {amount}"));
            Modify::Scaled(Fixed::new(doubled, 0))
        }
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
                card: Some(_),
                ..
            },
        ) if target == holder => Modify::Scaled(Fixed::new(0, 0)),
        (Power::NoBlockPower, _, ModifyHook::BlockMultiplicative { .. }) => Modify::Nothing,
        // `UnmovablePower.ModifyBlockMultiplicative`: double a card's or a
        // move's block on its owner, while fewer such gains than its stack
        // came this turn from other plays. A fight here has one player, who
        // alone plays Unmovable. The entry's `powered` stands for
        // `IsCardOrMonsterMove`: the two part only on a Move block that is
        // Unpowered, which no block here is.
        (
            Power::UnmovablePower,
            PowerAmount::Counted(amount),
            ModifyHook::BlockMultiplicative {
                target,
                powered: true,
                play,
                ..
            },
        ) if target == holder => {
            let gained = history
                .block_gained_this_turn()
                .filter(|entry| entry.receiver == target && entry.powered && entry.play != play)
                .count();
            match gained >= usize::try_from(amount).unwrap_or(0) {
                true => Modify::Nothing,
                false => Modify::Scaled(Fixed::new(2, 0)),
            }
        }
        (
            Power::UnmovablePower,
            PowerAmount::Endless,
            ModifyHook::BlockMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if target == holder => unimplemented!("an endless Unmovable"),
        (Power::UnmovablePower, _, ModifyHook::BlockMultiplicative { .. }) => Modify::Nothing,
        // `BufferPower.ModifyHpLostAfterOstyLate`: its holder loses nothing.
        (Power::BufferPower, _, ModifyHook::HpLostAfterOstyLate { target, .. })
            if target == holder =>
        {
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
        (Power::SlipperyPower, _, ModifyHook::HpLostAfterOsty { target, .. })
            if target == holder =>
        {
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
        // `FastenPower.ModifyBlockAdditive`: its stack on powered block its
        // owner gains, from a Defend or from no card at all.
        (
            Power::FastenPower,
            PowerAmount::Counted(amount),
            ModifyHook::BlockAdditive {
                target,
                powered: true,
                card,
            },
        ) if target == holder && card.is_none_or(|card| card.tagged(CardTag::Defend)) => {
            Modify::Added(amount)
        }
        (Power::FastenPower, _, ModifyHook::BlockAdditive { .. }) => Modify::Nothing,
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
                ..
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
        // `MindRotPower.ModifyHandDraw`: its stack fewer cards for its owner.
        // Its `Math.Max(0m, …)` floor bites only on a draw already below the
        // stack, which no ported listener takes it to.
        (
            Power::MindRotPower,
            PowerAmount::Counted(amount),
            ModifyHook::HandDraw { player, .. },
        ) if player == holder => Modify::Added(-amount),
        (Power::MindRotPower, PowerAmount::Endless, ModifyHook::HandDraw { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Mind Rot")
        }
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
            | Power::ClarityPower
            | Power::MindRotPower,
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
pub(crate) fn debuff_at(power: Power, amount: Number) -> bool {
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
        // `CorruptionPower`: a Skill its owner plays goes to the exhaust pile,
        // where it was bound.
        (Power::CorruptionPower, _) => {
            match hook.player == holder
                && <V as sts2_core::Rules>::card_kind(hook.card) == Some(sts2_core::CardKind::Skill)
            {
                true => HookResponse::Decision((Some(sts2_core::Pile::Exhaust), hook.position)),
                false => unchanged,
            }
        }
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
    amount: PowerAmount<Number>,
    held: Held<'_>,
    state: &State,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::{DecisionHook, HookResponse};

    match (power, hook) {
        // `RingingPower.ShouldPlay`: a card with Ringing waits while its owner
        // has started a play this turn. A fight here has one player, whose
        // cards are its holder's.
        // `SlothPower.ShouldPlay`: no more once its owner has played its
        // stack of cards this turn. A fight here has one player, whose cards
        // are its holder's.
        (Power::SlothPower, DecisionHook::ShouldPlay { .. }) => {
            let (PowerAmount::Counted(amount), State::Sloth { played }) = (amount, state) else {
                unimplemented!("an endless Sloth")
            };
            HookResponse::Decision(*played < amount)
        }
        // `ChainsOfBindingPower.ShouldPlay`: one Bound card a turn.
        (Power::ChainsOfBindingPower, DecisionHook::ShouldPlay { affliction, .. }) => {
            let State::ChainsOfBinding { bound_played } = state else {
                unreachable!("Chains of Binding holding {state:?}")
            };
            HookResponse::Decision(affliction != Some(Affliction::Bound) || !*bound_played)
        }
        (Power::RingingPower, DecisionHook::ShouldPlay { affliction, .. }) => {
            HookResponse::Decision(
                affliction != Some(Affliction::Ringing)
                    || held.history.started_this_turn().next().is_none(),
            )
        }
        // `ShouldCreatureBeRemovedFromCombatAfterDeath`, which keeps its own
        // holder and no other.
        (
            Power::IllusionPower
            | Power::ReattachPower
            | Power::DieForYouPower
            | Power::AdaptablePower
            | Power::PainfulStabsPower,
            DecisionHook::ShouldRemoveCreatureAfterDeath { creature },
        ) => HookResponse::Decision(creature != held.holder),
        // `OwnerIsSecondaryEnemy`.
        (Power::MinionPower, DecisionHook::ShouldCountForCombatEnd) => {
            HookResponse::Decision(false)
        }
        // `ShouldOwnerDeathTriggerFatal`: a Minion's death, and one Reattach
        // brings back, are not a kill that pays out.
        (Power::MinionPower | Power::ReattachPower, DecisionHook::ShouldOwnerDeathTriggerFatal) => {
            HookResponse::Decision(false)
        }
        // `ShouldPowerBeRemovedAfterOwnerDeath`. A power that keeps its owner
        // in the fight keeps itself, or the creature would come back without
        // whatever brought it back.
        (
            Power::ReattachPower
            | Power::MinionPower
            | Power::DieForYouPower
            | Power::AdaptablePower
            | Power::PainfulStabsPower,
            DecisionHook::ShouldRemoveSelfAfterOwnerDeath,
        ) => HookResponse::Decision(false),
        // `IllusionPower.ShouldPowerBeRemovedOnDeath`, which reads the power
        // and not whose it is: a debuff goes unless it is an `ITemporaryPower`,
        // and anything else stays. The temporary powers are the ones that
        // lend a stat.
        (Power::IllusionPower, DecisionHook::ShouldRemovePowerOnDeath { power, .. }) => {
            use sts2_core::PowerTable as _;

            if lends(power).is_some() {
                return HookResponse::Decision(false);
            }
            let debuff = power.debuff().unwrap_or_else(|| {
                unimplemented!("whether {power:?} is a debuff on a dying creature")
            });
            HookResponse::Decision(debuff)
        }
        // `ShouldAllowHitting`, which refuses its own holder while it revives.
        (
            Power::IllusionPower | Power::ReattachPower | Power::AdaptablePower,
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
        (
            Power::NoDrawPower,
            DecisionHook::ShouldDraw {
                player, hand_draw, ..
            },
        ) => HookResponse::Decision(hand_draw || player != held.holder),
        // `RetainHandPower.ShouldFlush`, which keeps its own holder's hand.
        (Power::RetainHandPower, DecisionHook::ShouldFlush { player }) => {
            HookResponse::Decision(player != held.holder)
        }
        // `ShouldClearBlock`, which keeps its own holder's. Blur's
        // `AfterPreventingBlockClear` only flashes.
        (
            Power::BurrowedPower | Power::BarricadePower | Power::BlurPower,
            DecisionHook::ShouldClearBlock { creature, .. },
        ) => HookResponse::Decision(creature != held.holder),
        // `InfestedPower.ShouldStopCombatFromEnding` and `StockPower`'s, for as
        // long as it is on a creature in the fight.
        (
            Power::InfestedPower | Power::StockPower | Power::AdaptablePower,
            DecisionHook::ShouldStopCombatFromEnding,
        ) => HookResponse::Decision(true),
        (power, hook) if hook.model_hook().is_some() => {
            unimplemented!("what {power:?} decides for {hook:?}")
        }
        _ => HookResponse::DoesNotImplement,
    }
}

/// `OrbModel._validOrbs`, which `GetRandomOrb` draws from.
const VALID_ORBS: [Orb; 5] = [
    Orb::LightningOrb,
    Orb::FrostOrb,
    Orb::DarkOrb,
    Orb::PlasmaOrb,
    Orb::GlassOrb,
];

/// `WitheringPresencePower`'s `CardsLeft` as it is applied.
const WITHERING_CARDS: u8 = 6;

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
        // Their `AfterModifyingHandDraw` only flashes.
        (Power::PaleBlueDotPower | Power::MindRotPower, _, EventHook::HandDrawModified) => return,
        // Its `AfterModifyingEnergyGain` only flashes.
        (Power::NoEnergyGainPower, _, EventHook::EnergyGainModified) => return,
        // `NoEnergyGainPower.AfterSideTurnEnd`: gone once its owner's turn
        // ends.
        (Power::NoEnergyGainPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::NoEnergyGainPower, _, EventHook::SideTurnEnd { .. }) => None,
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
            asked.push(stunned(sts2_core::StunResume::Interrupted));
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
                ..
            },
        ) => Some(HookEffect::TickDownDuration),
        // `DebilitatePower.AfterSideTurnEnd`: one off its stack as its owner's
        // turn ends. What it does to Vulnerable and Weak, they read.
        (Power::DebilitatePower, PowerAmount::Counted(_), EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::DebilitatePower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Debilitate")
        }
        (Power::DebilitatePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ConsumingShadowPower.AfterSideTurnEnd`: `OrbCmd.EvokeLast` once for
        // each of its stack as its owner's turn ends, while an orb is left.
        (
            Power::ConsumingShadowPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnEnd { side, .. },
        ) if side == owner => Some(HookEffect::EvokeLast(amount)),
        (Power::ConsumingShadowPower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Consuming Shadow")
        }
        (Power::ConsumingShadowPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `DevourLifePower.AfterCardPlayed`: its stack as a summon of Osty for
        // each Soul its owner plays. A fight here has one player, whose cards
        // are all its holder's.
        (
            Power::DevourLifePower,
            PowerAmount::Counted(amount),
            EventHook::CardPlayed {
                card: Cards::Token(TokenCard::Soul),
                ..
            },
        ) => Some(HookEffect::Summon(amount)),
        (Power::DevourLifePower, PowerAmount::Endless, EventHook::CardPlayed { .. }) => {
            unimplemented!("an endless Devour Life")
        }
        (Power::DevourLifePower, _, EventHook::CardPlayed { .. }) => None,
        // `SpinnerPower.AfterEnergyReset`: a Glass orb channelled for each of
        // its stack as its owner's energy resets.
        (Power::SpinnerPower, PowerAmount::Counted(amount), EventHook::EnergyReset { player, .. })
            if player == holder =>
        {
            for _ in 0..amount {
                asked.push(HookEffect::Channel(Orb::GlassOrb));
            }
            return;
        }
        (Power::SpinnerPower, PowerAmount::Endless, EventHook::EnergyReset { .. }) => {
            unimplemented!("an endless Spinner")
        }
        (Power::SpinnerPower, _, EventHook::EnergyReset { .. }) => None,
        // `StormPower.BeforeCardPlayed`: a Power its owner plays noted with
        // the stack as it begins. A fight here has one player, whose cards
        // are all its holder's.
        (Power::StormPower, PowerAmount::Counted(amount), EventHook::CardPlaying { card, id, .. }) => {
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power) {
                note(state, id, amount);
            }
            None
        }
        (Power::StormPower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Storm")
        }
        // `StormPower.AfterCardPlayed`: a Lightning orb channelled for each of
        // the stack it noted.
        (Power::StormPower, _, EventHook::CardPlayed { id, .. }) => {
            if let Some(lightning) = take_noted(state, id) {
                for _ in 0..lightning {
                    asked.push(HookEffect::Channel(Orb::LightningOrb));
                }
            }
            return;
        }
        // `CalamityPower.BeforeCardPlayed`: an Attack its owner plays noted.
        // A fight here has one player, whose cards are all its holder's.
        (Power::CalamityPower, PowerAmount::Counted(amount), EventHook::CardPlaying { card, id, .. }) => {
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack) {
                note(state, id, amount);
            }
            None
        }
        (Power::CalamityPower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Calamity")
        }
        // `CalamityPower.AfterCardPlayed`: for a play it noted, `GetForCombat`
        // of its stack as it stands now in Attacks of the character's pool.
        (Power::CalamityPower, amount, EventHook::CardPlayed { id, .. }) => {
            take_noted(state, id).map(|_| match amount {
                PowerAmount::Counted(count) => HookEffect::GenerateForCombat {
                    kind: sts2_core::CardKind::Attack,
                    count,
                },
                PowerAmount::Endless => unimplemented!("an endless Calamity"),
            })
        }
        // `SerpentFormPower.BeforeCardPlayed`: a play of its owner's noted
        // with the stack as it begins. A fight here has one player, whose
        // cards are all its holder's.
        (Power::SerpentFormPower, PowerAmount::Counted(amount), EventHook::CardPlaying { id, .. }) => {
            note(state, id, amount);
            None
        }
        (Power::SerpentFormPower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Serpent Form")
        }
        // `SerpentFormPower.AfterCardPlayed`: the stack it noted, unpowered
        // and from its owner, on an enemy drawn off `CombatTargets`.
        (Power::SerpentFormPower, _, EventHook::CardPlayed { id, .. }) => take_noted(state, id)
            .filter(|&damage| damage > 0)
            .map(|amount| HookEffect::DamageRandomOpponent {
                amount,
                unblockable: false,
                from_owner: true,
            }),
        // `RupturePower.BeforeCardPlayed`: a play of its owner's, in its
        // owner's own turn, noted as owing nothing yet.
        (Power::RupturePower, _, EventHook::CardPlaying { id, side, .. }) => {
            if side == owner {
                note(state, id, 0);
            }
            None
        }
        // `RupturePower.AfterDamageReceived`: its stack of Strength for health
        // its owner loses in its own turn. A play it noted owes it instead,
        // until the play is over.
        (
            Power::RupturePower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                unblocked,
                card,
                side,
                ..
            },
        ) if target == holder && unblocked > 0 && side == owner => {
            let owed = card.is_some_and(|card| add_noted(state, card, amount));
            (!owed).then_some(HookEffect::ApplyPower {
                target: holder,
                power: Power::StrengthPower,
                amount: PowerAmount::Counted(amount),
            })
        }
        (Power::RupturePower, PowerAmount::Endless, EventHook::DamageReceived { .. }) => {
            unimplemented!("an endless Rupture")
        }
        (Power::RupturePower, _, EventHook::DamageReceived { .. }) => None,
        // `RupturePower.AfterCardPlayed`: the Strength a play it noted owes.
        (Power::RupturePower, _, EventHook::CardPlayed { id, .. }) => take_noted(state, id)
            .map(|owed| HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(owed))),
        // `BlackHolePower.AfterCardPlayed`: its stack, unpowered, to each enemy
        // that can be hit, once a play that spent stars is over and it ends
        // its replays. The play's started entry keeps what it spent. A fight
        // here has one player, whose cards are all its holder's.
        (
            Power::BlackHolePower,
            PowerAmount::Counted(amount),
            EventHook::CardPlayed { id, last, .. },
        ) => {
            let spent = history
                .started_this_turn()
                .filter(|started| started.id == id)
                .last()
                .is_some_and(|started| started.stars > 0);
            (spent && last).then_some(HookEffect::DamageOpponents(amount))
        }
        // `BlackHolePower.AfterStarsGained`: the same, for stars its owner
        // gains.
        (Power::BlackHolePower, PowerAmount::Counted(amount), EventHook::StarsGained { amount: gained }) => {
            (gained > 0).then_some(HookEffect::DamageOpponents(amount))
        }
        (Power::BlackHolePower, PowerAmount::Endless, EventHook::CardPlayed { .. } | EventHook::StarsGained { .. }) => {
            unimplemented!("an endless Black Hole")
        }
        // `MasterPlannerPower.AfterCardPlayed`: Sly written on each Skill its
        // owner plays, for as long as the copy lasts. A fight here has one
        // player, whose cards are all its holder's.
        (Power::MasterPlannerPower, _, EventHook::CardPlayed { card, id, .. })
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill) =>
        {
            Some(HookEffect::AddKeyword {
                to: sts2_core::Writing::Named(id),
                keyword: CardKeyword::Sly,
            })
        }
        (Power::MasterPlannerPower, _, EventHook::CardPlayed { .. }) => None,
        // `ShadowStepPower.AfterSideTurnStart`: its stack as Double Damage on
        // its owner as its owner's side begins a turn, then it goes.
        (Power::ShadowStepPower, amount, EventHook::SideTurnStart { side, .. }) if side == owner => {
            asked.push(HookEffect::ApplyToOwner(Power::DoubleDamagePower, amount));
            asked.push(HookEffect::Remove);
            return;
        }
        (Power::ShadowStepPower, _, EventHook::SideTurnStart { .. }) => None,
        // `DoubleDamagePower.AfterSideTurnEnd`: one off its stack as its
        // owner's turn ends.
        (Power::DoubleDamagePower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Decrement)
        }
        (Power::DoubleDamagePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ChildOfTheStarsPower.AfterStarsSpent`: its stack of block,
        // unpowered, for each star its owner spent.
        (
            Power::ChildOfTheStarsPower,
            PowerAmount::Counted(stack),
            EventHook::StarsSpent { player, amount },
        ) if player == holder && amount > 0 => Some(HookEffect::GainBlock(stack * amount)),
        (Power::ChildOfTheStarsPower, PowerAmount::Endless, EventHook::StarsSpent { .. }) => {
            unimplemented!("an endless Child of the Stars")
        }
        (Power::ChildOfTheStarsPower, _, EventHook::StarsSpent { .. }) => None,
        // `RagePower.AfterCardPlayed`: its stack of block, unpowered, for
        // each Attack its owner plays.
        (Power::RagePower, PowerAmount::Counted(amount), EventHook::CardPlayed { card, .. })
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (Power::RagePower, PowerAmount::Endless, EventHook::CardPlayed { .. }) => {
            unimplemented!("an endless Rage")
        }
        (Power::RagePower, _, EventHook::CardPlayed { .. }) => None,
        // `RagePower.AfterSideTurnEnd`: `PowerCmd.Remove(this)` as its owner's
        // turn ends.
        (Power::RagePower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::RagePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ViciousPower.AfterPowerAmountChanged`: its stack drawn each time
        // its owner puts more Vulnerable on anything.
        (
            Power::ViciousPower,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                power: Power::VulnerablePower,
                delta,
                applier: Some(applier),
                ..
            },
        ) if delta > 0 && applier == holder => Some(HookEffect::Draw(amount)),
        (Power::ViciousPower, PowerAmount::Endless, EventHook::PowerAmountChanged { .. }) => {
            unimplemented!("an endless Vicious")
        }
        (Power::ViciousPower, _, EventHook::PowerAmountChanged { .. }) => None,
        // `OutbreakPower.AfterPowerAmountChanged`: each Poison its owner adds
        // counts, and every third deals its stack, unpowered, to each enemy
        // that can be hit.
        (
            Power::OutbreakPower,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                power: Power::PoisonPower,
                delta,
                applier: Some(applier),
                ..
            },
        ) if applier == holder && delta > 0 => {
            let State::Outbreak { poisoned } = state else {
                unreachable!("Outbreak counts its poisonings")
            };
            *poisoned += 1;
            if *poisoned >= OUTBREAK_EVERY {
                *poisoned %= OUTBREAK_EVERY;
                Some(HookEffect::DamageOpponents(amount))
            } else {
                None
            }
        }
        (Power::OutbreakPower, PowerAmount::Endless, EventHook::PowerAmountChanged { .. }) => {
            unimplemented!("an endless Outbreak")
        }
        (Power::OutbreakPower, _, EventHook::PowerAmountChanged { .. }) => None,
        // `ShroudPower.AfterPowerAmountChanged`: its stack as block,
        // unpowered, whenever its owner applies Doom.
        (
            Power::ShroudPower,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                power: Power::DoomPower,
                applier: Some(applier),
                ..
            },
        ) if applier == holder => Some(HookEffect::GainBlock(amount)),
        (Power::ShroudPower, PowerAmount::Endless, EventHook::PowerAmountChanged { .. }) => {
            unimplemented!("an endless Shroud")
        }
        (Power::ShroudPower, _, EventHook::PowerAmountChanged { .. }) => None,
        // `SleightOfFleshPower.AfterPowerAmountChanged`: its stack as damage,
        // unpowered, on an enemy its owner just debuffed, but not through a
        // temporary power. The player's side is the player and its pets.
        (
            Power::SleightOfFleshPower,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                creature,
                power: changed,
                delta,
                applier: Some(applier),
            },
        ) if applier == holder
            && creature != fight.player()
            && fight.pet_owner(creature).is_none()
            && lends(changed).is_none()
            && debuff_at(changed, delta) =>
        {
            Some(HookEffect::Damage {
                target: creature,
                amount,
            })
        }
        (Power::SleightOfFleshPower, PowerAmount::Endless, EventHook::PowerAmountChanged { .. }) => {
            unimplemented!("an endless Sleight of Flesh")
        }
        (Power::SleightOfFleshPower, _, EventHook::PowerAmountChanged { .. }) => None,
        // `ToricToughnessPower.AfterBlockCleared`: its block on its owner,
        // then one off its turns.
        (Power::ToricToughnessPower, _, EventHook::BlockCleared { creature, .. })
            if creature == holder =>
        {
            let State::ToricToughness { block } = *state else {
                unreachable!("Toric Toughness keeps its block")
            };
            asked.push(HookEffect::GainBlock(block));
            asked.push(HookEffect::Decrement);
            return;
        }
        (Power::ToricToughnessPower, _, EventHook::BlockCleared { .. }) => None,
        // `InfernoPower.AfterPlayerTurnStart`: its self damage on its owner,
        // unblockable and unpowered, from its owner.
        (Power::InfernoPower, _, EventHook::PlayerTurnStart { player, .. }) if player == holder => {
            let State::SelfDamage { self_damage, .. } = *state else {
                unreachable!("Inferno keeps its self damage")
            };
            Some(HookEffect::DamageOwnerInFight {
                amount: self_damage,
                unblockable: true,
                self_dealt: true,
            })
        }
        (Power::InfernoPower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `CrimsonMantlePower.AfterPlayerTurnStart`: its self damage on its
        // owner, unblockable and unpowered, from its owner, then its stack of
        // block, unpowered.
        (
            Power::CrimsonMantlePower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player, .. },
        ) if player == holder => {
            let State::SelfDamage { self_damage, .. } = *state else {
                unreachable!("Crimson Mantle keeps its self damage")
            };
            asked.push(HookEffect::DamageOwnerInFight {
                amount: self_damage,
                unblockable: true,
                self_dealt: true,
            });
            asked.push(HookEffect::GainBlock(amount));
            return;
        }
        (Power::CrimsonMantlePower, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Crimson Mantle")
        }
        (Power::CrimsonMantlePower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `NightmarePower.BeforeHandDraw`: its stack of clones of the card it
        // keeps into its owner's hand, then `PowerCmd.Remove(this)`.
        (Power::NightmarePower, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if player == holder =>
        {
            let State::Nightmare { card } = *state else {
                unreachable!("Nightmare keeps its card")
            };
            let kept = card.unwrap_or_else(|| unimplemented!("a Nightmare keeping no card"));
            asked.push(HookEffect::CloneKeptIntoHand {
                kept,
                count: amount,
            });
            asked.push(HookEffect::Remove);
            return;
        }
        (Power::NightmarePower, _, EventHook::BeforeHandDraw { .. }) => None,
        // `InfernoPower.AfterDamageReceived`: its stack on every enemy that
        // can be hit, unpowered, where its owner lost health in its own
        // side's turn.
        (
            Power::InfernoPower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                unblocked,
                side,
                ..
            },
        ) if target == holder && unblocked > 0 && side == owner => {
            Some(HookEffect::DamageOpponents(amount))
        }
        (Power::InfernoPower, PowerAmount::Endless, EventHook::DamageReceived { .. }) => {
            unimplemented!("an endless Inferno")
        }
        (Power::InfernoPower, _, EventHook::DamageReceived { .. }) => None,
        // `SicEmPower.AfterDamageGiven`: `OstyCmd.Summon` of its stack for the
        // player whose Osty hit its owner, where that player applied it.
        (
            Power::SicEmPower,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer: Some(dealer),
                pet_of: Some(pet_of),
                target,
                ..
            },
        ) if target == holder
            && applier == Some(pet_of)
            && fight.monster(dealer) == Some(Monster::Osty) =>
        {
            Some(HookEffect::Summon(amount))
        }
        (Power::SicEmPower, PowerAmount::Endless, EventHook::DamageGiven { .. }) => {
            unimplemented!("an endless Sic 'Em")
        }
        (Power::SicEmPower, _, EventHook::DamageGiven { .. }) => None,
        // `SicEmPower.AfterSideTurnEnd`: `PowerCmd.Remove(this)` as its
        // owner's turn ends.
        (Power::SicEmPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::SicEmPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ConquerorPower.AfterSideTurnEnd`: one off its stack as its owner's
        // side's turn ends.
        (Power::ConquerorPower, PowerAmount::Counted(_), EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::ConquerorPower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. })
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
        (Power::RingingPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::RingingPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `RingingPower.AfterRemoved`: Ringing off every card that has it.
        (Power::RingingPower, _, EventHook::Removed) => Some(HookEffect::ClearAffliction(
            sts2_core::Writing::EveryAfflictedWith(Affliction::Ringing),
        )),
        // `HexPower.AfterApplied`: Hexed on every card its owner holds that
        // has no affliction. `AfterCardEnteredCombat`: on a card that joins.
        (Power::HexPower, PowerAmount::Counted(_), EventHook::Applied) => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::Every,
                affliction: Affliction::Hexed,
            })
        }
        (Power::HexPower, _, EventHook::CardEnteredCombat { at, .. }) => Some(HookEffect::Afflict {
            to: sts2_core::Writing::One(at),
            affliction: Affliction::Hexed,
        }),
        // `HexPower.AfterDeath`: gone once the creature that applied it dies.
        (
            Power::HexPower,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if Some(creature) == applier => Some(HookEffect::Remove),
        (Power::HexPower, _, EventHook::AfterDeath { .. }) => None,
        // `HexPower.AfterRemoved`: Hexed off every card that has it.
        (Power::HexPower, _, EventHook::Removed) => Some(HookEffect::ClearAffliction(
            sts2_core::Writing::EveryAfflictedWith(Affliction::Hexed),
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
        (Power::TangledPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::TangledPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `TangledPower.AfterRemoved`: Entangled off every card that has it.
        (Power::TangledPower, _, EventHook::Removed) => Some(HookEffect::ClearAffliction(
            sts2_core::Writing::EveryAfflictedWith(Affliction::Entangled),
        )),
        // `DemisePower.AfterSideTurnEnd`: its stack off its owner as its
        // owner's side's turn ends.
        (Power::DemisePower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            Some(HookEffect::DamageOwnerInFight {
                amount,
                unblockable: true,
                self_dealt: false,
            })
        }
        (Power::DemisePower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Demise")
        }
        (Power::DemisePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `BiasedCognitionPower.AfterSideTurnStart`: its stack off its owner's
        // Focus as its owner's side begins a turn.
        (
            Power::BiasedCognitionPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnStart { side, .. },
        ) if side == owner => Some(HookEffect::ApplyToOwner(
            Power::FocusPower,
            PowerAmount::Counted(-amount),
        )),
        (Power::BiasedCognitionPower, PowerAmount::Endless, EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Biased Cognition")
        }
        (Power::BiasedCognitionPower, _, EventHook::SideTurnStart { .. }) => None,
        // `StranglePower.BeforeCardPlayed`: a play by its applier, where a
        // player applied it, noted with the stack as it begins. A fight here
        // has one player, whose cards are all its applier's.
        (Power::StranglePower, PowerAmount::Counted(amount), EventHook::CardPlaying { id, .. }) => {
            if applier == Some(fight.player()) {
                note(state, id, amount);
            }
            None
        }
        (Power::StranglePower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Strangle")
        }
        // `StranglePower.AfterCardPlayed`: the stack it noted off its owner,
        // unblockable, unpowered and from nobody.
        (Power::StranglePower, _, EventHook::CardPlayed { id, .. }) => {
            take_noted(state, id).map(|amount| HookEffect::DamageOwnerInFight {
                amount,
                unblockable: true,
                self_dealt: false,
            })
        }
        // `StranglePower.AfterSideTurnEnd`: gone as its owner's side ends a
        // turn.
        (Power::StranglePower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::StranglePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `OblivionPower.BeforeCardPlayed`: a play by its applier, where a
        // player applied it, noted with the stack as it begins. A fight here
        // has one player, whose cards are all its applier's.
        (Power::OblivionPower, PowerAmount::Counted(amount), EventHook::CardPlaying { id, .. }) => {
            if applier == Some(fight.player()) {
                note(state, id, amount);
            }
            None
        }
        (Power::OblivionPower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Oblivion")
        }
        // `OblivionPower.AfterCardPlayed`: the stack it noted as Doom on its
        // owner.
        (Power::OblivionPower, _, EventHook::CardPlayed { id, .. }) => {
            take_noted(state, id).map(|amount| HookEffect::ApplyPower {
                target: holder,
                power: Power::DoomPower,
                amount: PowerAmount::Counted(amount),
            })
        }
        // `OblivionPower.AfterSideTurnEnd`: gone as the player's turn ends.
        (Power::OblivionPower, _, EventHook::SideTurnEnd { side: CombatSide::Player, .. }) => {
            Some(HookEffect::Remove)
        }
        (Power::OblivionPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `AfterimagePower.BeforeCardPlayed`: a play of its owner's noted with
        // the stack as it begins. A fight here has one player, whose cards
        // are all the player's.
        (Power::AfterimagePower, PowerAmount::Counted(amount), EventHook::CardPlaying { id, .. })
            if owner == CombatSide::Player =>
        {
            note(state, id, amount);
            None
        }
        (Power::AfterimagePower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Afterimage")
        }
        // `AfterimagePower.AfterCardPlayed`: the stack it noted as unpowered
        // block, where it is more than nothing.
        (Power::AfterimagePower, _, EventHook::CardPlayed { id, .. }) => take_noted(state, id)
            .filter(|&amount| amount > 0)
            .map(HookEffect::GainBlock),
        // `SelfFormingClayPower.AfterBlockCleared`: its stack as unpowered
        // block once its owner's block clears, then gone.
        (
            Power::SelfFormingClayPower,
            PowerAmount::Counted(amount),
            EventHook::BlockCleared { creature, .. },
        ) if creature == holder => Some(HookEffect::GainBlockAndRemove(amount)),
        (Power::SelfFormingClayPower, PowerAmount::Endless, EventHook::BlockCleared { .. }) => {
            unimplemented!("an endless Self-Forming Clay")
        }
        (Power::SelfFormingClayPower, _, EventHook::BlockCleared { .. }) => None,
        // `MonologuePower.BeforeCardPlayed`: a play of its owner's noted with
        // its Strength, which the card that applied it set to its own
        // `Power`, one. A fight here has one player.
        (Power::MonologuePower, _, EventHook::CardPlaying { id, .. })
            if owner == CombatSide::Player =>
        {
            note(state, id, MONOLOGUE_STRENGTH);
            None
        }
        // `MonologuePower.AfterCardPlayed`: the Strength noted lent to its
        // owner, and counted as lent.
        (Power::MonologuePower, _, EventHook::CardPlayed { id, .. }) => {
            take_noted(state, id).map(|lent| {
                let State::Monologue { applied, .. } = state else {
                    unreachable!("Strength counted for {:?}", state.power())
                };
                *applied += MONOLOGUE_STRENGTH;
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(lent))
            })
        }
        // `MonologuePower.AfterSideTurnEnd` on its owner's side: gone, and
        // all it lent taken back.
        (Power::MonologuePower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            let State::Monologue { applied, .. } = state else {
                unreachable!("Strength counted for {:?}", state.power())
            };
            asked.extend([
                HookEffect::Remove,
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(-*applied)),
            ]);
            return;
        }
        (Power::MonologuePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `SubroutinePower.BeforeCardPlayed`: a Power its owner plays noted
        // with the stack as it begins. A fight here has one player.
        (Power::SubroutinePower, PowerAmount::Counted(amount), EventHook::CardPlaying { card, id, .. })
            if owner == CombatSide::Player
                && <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power) =>
        {
            note(state, id, amount);
            None
        }
        (Power::SubroutinePower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Subroutine")
        }
        (Power::SubroutinePower, _, EventHook::CardPlaying { .. }) => None,
        // `SubroutinePower.AfterCardPlayed`: an energy for each of the stack
        // it noted, one at a time.
        (Power::SubroutinePower, _, EventHook::CardPlayed { id, .. }) => {
            let noted = take_noted(state, id).unwrap_or(0);
            let count = usize::try_from(noted).expect("a noted stack is not negative");
            asked.extend(std::iter::repeat_n(HookEffect::GainEnergy(1), count));
            return;
        }
        // `HauntPower.AfterCardPlayed`: for a Soul its owner plays, its stack
        // on an enemy drawn off `CombatTargets`, unblockable, unpowered and
        // from nobody. A fight here has one player.
        (
            Power::HauntPower,
            PowerAmount::Counted(amount),
            EventHook::CardPlayed {
                card: Cards::Token(TokenCard::Soul),
                ..
            },
        ) if owner == CombatSide::Player => Some(HookEffect::DamageRandomOpponent {
            amount,
            unblockable: true,
            from_owner: false,
        }),
        (Power::HauntPower, PowerAmount::Endless, EventHook::CardPlayed { .. }) => {
            unimplemented!("an endless Haunt")
        }
        (Power::HauntPower, _, EventHook::CardPlayed { .. }) => None,
        // `TheSealedThronePower.BeforeCardPlayed`: its stack as stars for
        // each play of its owner's. A fight here has one player.
        (
            Power::TheSealedThronePower,
            PowerAmount::Counted(amount),
            EventHook::CardPlaying { .. },
        ) if owner == CombatSide::Player => Some(HookEffect::GainStars(amount)),
        (Power::TheSealedThronePower, PowerAmount::Endless, EventHook::CardPlaying { .. }) => {
            unimplemented!("an endless Sealed Throne")
        }
        // `WraithFormPower.AfterSideTurnStart`: its stack off its owner's
        // Dexterity as its owner's side starts a turn.
        (Power::WraithFormPower, PowerAmount::Counted(amount), EventHook::SideTurnStart { side, .. })
            if side == owner =>
        {
            Some(HookEffect::ApplyToOwner(
                Power::DexterityPower,
                PowerAmount::Counted(-amount),
            ))
        }
        (Power::WraithFormPower, PowerAmount::Endless, EventHook::SideTurnStart { .. }) => {
            unimplemented!("an endless Wraith Form")
        }
        (Power::WraithFormPower, _, EventHook::SideTurnStart { .. }) => None,
        // `DisintegrationPower.AfterSideTurnEndLate`: its stack off its owner,
        // dealt by the owner and unpowered, as its owner's side's turn ends.
        (
            Power::DisintegrationPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnEndLate { side },
        ) if side == owner => Some(HookEffect::DamageOwnerInFight {
            amount,
            unblockable: false,
            self_dealt: true,
        }),
        (Power::DisintegrationPower, PowerAmount::Endless, EventHook::SideTurnEndLate { side })
            if side == owner =>
        {
            unimplemented!("an endless Disintegration")
        }
        (Power::DisintegrationPower, _, EventHook::SideTurnEndLate { .. }) => None,
        // `SlothPower.BeforeCardPlayed`: one more play counted. A fight here
        // has one player, whose cards are its holder's.
        (Power::SlothPower, _, EventHook::CardPlaying { .. }) => {
            let State::Sloth { played } = state else {
                unreachable!("plays counted for {:?}", state.power())
            };
            *played += one;
            None
        }
        // `VoidFormPower.AfterCardPlayed`: one more play of its owner's, where
        // it is not auto-played and is the last of its replays.
        (Power::VoidFormPower, _, EventHook::CardPlayed { auto, last, .. }) => {
            if owner == CombatSide::Player && !auto && last {
                let State::VoidForm { played } = state else {
                    unreachable!("Void Form counts its plays")
                };
                *played = played.saturating_add(1);
            }
            None
        }
        // `VoidFormPower.BeforeSideTurnStart`: the count starts again as its
        // owner's side's turn begins.
        (Power::VoidFormPower, _, EventHook::BeforeSideTurnStart { side, .. }) => {
            if side == owner {
                let State::VoidForm { played } = state else {
                    unreachable!("Void Form counts its plays")
                };
                *played = 0;
            }
            None
        }
        // `VoidFormPower.BeforeApplied` and `BeforePowerAmountChanged` on
        // itself: `HideTemporaryZeroCostVisual`, which frees nothing more this
        // turn.
        (Power::VoidFormPower, _, EventHook::BeforeApplied { .. }) => {
            *state = State::VoidForm {
                played: VOID_FORM_SPENT,
            };
            None
        }
        (
            Power::VoidFormPower,
            _,
            EventHook::PowerAmountChanging {
                power: Power::VoidFormPower,
                target,
                ..
            },
        ) if target == holder => {
            *state = State::VoidForm {
                played: VOID_FORM_SPENT,
            };
            None
        }
        (Power::VoidFormPower, _, EventHook::PowerAmountChanging { .. }) => None,
        // `SlothPower.BeforeSideTurnStart`: the count starts again as its
        // owner's side's turn begins.
        (Power::SlothPower, _, EventHook::BeforeSideTurnStart { side, .. }) => {
            if side == owner {
                let State::Sloth { played } = state else {
                    unreachable!("plays counted for {:?}", state.power())
                };
                *played = 0;
            }
            None
        }
        // `ConstrictPower.AfterSideTurnEnd`: its stack off its owner, dealt by
        // the owner and unpowered, as its owner's side's turn ends.
        (Power::ConstrictPower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            Some(HookEffect::DamageOwnerInFight {
                amount,
                unblockable: false,
                self_dealt: true,
            })
        }
        (Power::ConstrictPower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. })
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
                ..
            },
        ) => Some(HookEffect::Decrement),
        (
            Power::IntangiblePower,
            PowerAmount::Endless,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) => unimplemented!("an endless Intangible"),
        (Power::IntangiblePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `BorrowedTimePower.AfterSideTurnEnd`: gone once its owner's turn
        // ends.
        (Power::BorrowedTimePower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
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
        // `PaperCutsPower.AfterDamageGiven`: a powered hit from its owner that
        // gets past the player's block takes its amount off their maximum.
        (
            Power::PaperCutsPower,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer,
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if dealer == Some(holder) && target == fight.player() && unblocked > 0 => {
            Some(HookEffect::LoseMaxHpOf { target, amount })
        }
        (Power::PaperCutsPower, PowerAmount::Endless, EventHook::DamageGiven { dealer, .. })
            if dealer == Some(holder) =>
        {
            unimplemented!("an endless Paper Cuts")
        }
        (Power::PaperCutsPower, _, EventHook::DamageGiven { .. }) => None,
        // `RampartPower.AfterSideTurnStart`, for the player's side: its
        // amount of block on each Turret Operator among the enemies.
        (
            Power::RampartPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnStart {
                side: CombatSide::Player,
                ..
            },
        ) => {
            let player = fight.player();
            asked.extend(
                fight
                    .living_opponents(player)
                    .into_iter()
                    .filter(|&enemy| fight.monster(enemy) == Some(Monster::TurretOperator))
                    .map(|target| HookEffect::GainBlockOf { target, amount }),
            );
            return;
        }
        (Power::RampartPower, _, EventHook::SideTurnStart { .. }) => None,
        // `PossessStrengthPower.AfterPowerAmountChanged` and its Dexterity
        // twin: a stack its holder took off the player is counted as stolen.
        (
            Power::PossessStrengthPower | Power::PossessSpeedPower,
            _,
            EventHook::PowerAmountChanged {
                creature,
                power: changed,
                delta,
                applier,
            },
        ) => {
            let possessed = match power {
                Power::PossessStrengthPower => Power::StrengthPower,
                _ => Power::DexterityPower,
            };
            if applier == Some(holder)
                && creature == fight.player()
                && changed == possessed
                && delta < 0
            {
                *state.stolen_mut() += delta;
            }
            None
        }
        // `AfterDeath` for its own holder: the stolen stack given back, from
        // nobody.
        (
            Power::PossessStrengthPower | Power::PossessSpeedPower,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if creature == holder => {
            let stolen = *state.stolen_mut();
            (stolen != 0).then(|| HookEffect::ApplyPower {
                target: fight.player(),
                power: match power {
                    Power::PossessStrengthPower => Power::StrengthPower,
                    _ => Power::DexterityPower,
                },
                amount: PowerAmount::Counted(-stolen),
            })
        }
        (Power::PossessStrengthPower | Power::PossessSpeedPower, _, EventHook::AfterDeath { .. }) => {
            None
        }
        // `HighVoltagePower.AfterSideTurnEnd`: its stack of Strength as its
        // owner's side ends a turn.
        (Power::HighVoltagePower, amount, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::ApplyToOwner(Power::StrengthPower, amount))
        }
        (Power::HighVoltagePower, _, EventHook::SideTurnEnd { .. }) => None,
        // `GalvanicPower.BeforeCombatStart`: Galvanized on every Power the
        // player holds. `AfterCardEnteredCombat`: on a Power that joins the
        // fight. The fight leaves a card afflicted already as it is.
        (Power::GalvanicPower, _, EventHook::CombatStarting { .. }) => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::EveryOfKind(sts2_core::CardKind::Power),
                affliction: Affliction::Galvanized,
            })
        }
        (Power::GalvanicPower, _, EventHook::CardEnteredCombat { at, card, .. })
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: Affliction::Galvanized,
            })
        }
        (Power::GalvanicPower, _, EventHook::CardEnteredCombat { .. }) => None,
        // `GalvanicPower.AfterCardPlayed`: its amount to the player who
        // played a Galvanized card, unpowered and from nobody.
        (
            Power::GalvanicPower,
            PowerAmount::Counted(amount),
            EventHook::CardPlayed {
                affliction: Some(Affliction::Galvanized),
                ..
            },
        ) => Some(HookEffect::DamageUnpowered {
            target: fight.player(),
            amount,
        }),
        (Power::GalvanicPower, _, EventHook::CardPlayed { .. }) => None,
        // `DiamondDiademPower.AfterSideTurnEnd`: gone once the enemies' turn
        // ends.
        (
            Power::DiamondDiademPower,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) => Some(HookEffect::Remove),
        (Power::DiamondDiademPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `NoDrawPower.AfterSideTurnEnd`: gone once its owner's turn ends.
        (Power::NoDrawPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
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
                ..
            },
        ) => Some(HookEffect::Decrement),
        (
            Power::ColossusPower,
            PowerAmount::Endless,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) => unimplemented!("an endless Colossus"),
        (Power::ColossusPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `ShrinkPower.AfterSideTurnEnd`, for the side its owner is among the
        // participants of. An infinite Shrink, the one the game gives a
        // negative amount, lasts the fight.
        (Power::ShrinkPower, PowerAmount::Counted(_), EventHook::SideTurnEnd { side, .. })
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
            EventHook::SideTurnEnd { side, .. },
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
            EventHook::SideTurnEnd { side, .. },
        ) if side == owner && amount > one => Some(HookEffect::Decrement),
        (Power::EscapeArtistPower, PowerAmount::Endless, EventHook::SideTurnEnd { .. }) => {
            unimplemented!("an endless Escape Artist")
        }
        (Power::EscapeArtistPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `HatchPower.AfterSideTurnEnd`: `PowerCmd.Decrement` as its owner's
        // side ends.
        (Power::HatchPower, PowerAmount::Counted(_), EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            Some(HookEffect::Decrement)
        }
        (Power::HatchPower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Hatch")
        }
        (Power::HatchPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `PersonalHivePower.AfterDamageReceived`: a Dazed for each of its
        // stack into the dealer's draw pile, at random places, after a
        // powered attack on its owner. A fight here has one player, so the
        // dealer's pile, or its Osty's owner's, is the one there is.
        (
            Power::PersonalHivePower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                dealer: Some(_),
                powered: true,
                ..
            },
        ) if target == holder => Some(HookEffect::AddToDrawAtRandom {
            card: Cards::Status(StatusCard::Dazed),
            count: amount,
        }),
        (
            Power::PersonalHivePower,
            PowerAmount::Endless,
            EventHook::DamageReceived { target, .. },
        ) if target == holder => unimplemented!("an endless Personal Hive"),
        (Power::PersonalHivePower, _, EventHook::DamageReceived { .. }) => None,
        // `SurroundedPower.BeforeCardPlayed` and `BeforePotionUsed`: its owner
        // turns toward a target behind it. A fight here has one player, whose
        // every card and potion is its owner's.
        (
            Power::SurroundedPower,
            _,
            EventHook::CardPlaying {
                target: Some(target),
                ..
            }
            | EventHook::PotionUsing {
                target: Some(target),
            },
        ) => {
            state.turn_toward(fight, target);
            None
        }
        (Power::SurroundedPower, _, EventHook::CardPlaying { .. } | EventHook::PotionUsing { .. }) => {
            None
        }
        // `SurroundedPower.AfterDeath`: once an enemy dies, its owner turns
        // toward the first enemy left where every one left stands on the
        // same side of it.
        (
            Power::SurroundedPower,
            _,
            EventHook::AfterDeath {
                side,
                removal_prevented: false,
                ..
            },
        ) if side != owner => {
            let hittable = fight.living_opponents(holder);
            let all = |power| hittable.iter().all(|&enemy| fight.power(enemy, power).is_some());
            if let Some(&first) = hittable.first()
                && (all(Power::BackAttackLeftPower) || all(Power::BackAttackRightPower))
            {
                state.turn_toward(fight, first);
            }
            None
        }
        (Power::SurroundedPower, _, EventHook::AfterDeath { .. }) => None,
        // `CrabRagePower.AfterDeath`: when another on its side dies, six
        // Strength, ninety-nine unpowered block, and then it goes.
        (
            Power::CrabRagePower,
            _,
            EventHook::AfterDeath {
                creature, side, ..
            },
        ) if creature != holder && side == owner => {
            asked.extend([
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(CRAB_RAGE)),
                HookEffect::GainBlock(CRAB_BLOCK),
                HookEffect::Remove,
            ]);
            return;
        }
        (Power::CrabRagePower, _, EventHook::AfterDeath { .. }) => None,
        // `SandpitPower.AfterSideTurnStartLate`: one off as the enemies' turn
        // starts.
        (Power::SandpitPower, _, EventHook::SideTurnStartLate { side: CombatSide::Enemy }) => {
            Some(HookEffect::Decrement)
        }
        (Power::SandpitPower, _, EventHook::SideTurnStartLate { .. }) => None,
        // `SandpitPower.AfterCreatureAddedToCombat` moves creatures on the
        // screen, and nothing else.
        (Power::SandpitPower, _, EventHook::CreatureAdded { .. }) => None,
        // `SandpitPower.AfterRemoved`: while its owner and its target both
        // live, `CreatureCmd.Kill` on the target, forced. A fight here has one
        // player, who is the target, and the player's death ends the fight
        // before any pet's matters.
        (Power::SandpitPower, _, EventHook::Removed) => {
            let player = fight.player();
            let alive = |creature| fight.current_hp(creature).is_some_and(|hp| hp > 0);
            (alive(holder) && alive(player)).then_some(HookEffect::KillCreature(player))
        }
        // Its other overrides move the player's figure and the music.
        (
            Power::SandpitPower,
            _,
            EventHook::Applied
            | EventHook::PowerAmountChanged { .. }
            | EventHook::BeforeSideTurnEnd { .. }
            | EventHook::OstyRevived,
        ) => None,
        // `VitalSparkPower.BeforeCombatStart`: Tainted by its stack on every
        // Skill the player holds.
        (Power::VitalSparkPower, _, EventHook::CombatStarting { .. }) => Some(HookEffect::Afflict {
            to: sts2_core::Writing::EveryOfKind(sts2_core::CardKind::Skill),
            affliction: Affliction::Tainted,
        }),
        // `VitalSparkPower.AfterCardEnteredCombat`: Tainted on a Skill that
        // arrives with no affliction.
        (Power::VitalSparkPower, _, EventHook::CardEnteredCombat { at, card, .. })
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: Affliction::Tainted,
            })
        }
        (Power::VitalSparkPower, _, EventHook::CardEnteredCombat { .. }) => None,
        // `VitalSparkPower.AfterCardPlayed`: Tainted Power by its stack on the
        // player, from nobody, once a Tainted card's play is over.
        (
            Power::VitalSparkPower,
            PowerAmount::Counted(amount),
            EventHook::CardPlayed {
                affliction: Some(Affliction::Tainted),
                ..
            },
        ) => Some(HookEffect::ApplyPower {
            target: fight.player(),
            power: Power::TaintedPower,
            amount: PowerAmount::Counted(amount),
        }),
        (
            Power::VitalSparkPower,
            PowerAmount::Endless,
            EventHook::CardPlayed {
                affliction: Some(Affliction::Tainted),
                ..
            },
        ) => unimplemented!("an endless Vital Spark"),
        (Power::VitalSparkPower, _, EventHook::CardPlayed { .. }) => None,
        // `VitalSparkPower.AfterRemoved`: Tainted off every card that has it.
        (Power::VitalSparkPower, _, EventHook::Removed) => Some(HookEffect::ClearAffliction(
            sts2_core::Writing::EveryAfflictedWith(Affliction::Tainted),
        )),
        // `AfterPowerAmountChanged` writes its stack onto each Tainted card's
        // own amount, which nothing here reads.
        (Power::VitalSparkPower, _, EventHook::PowerAmountChanged { .. }) => None,
        // `TaintedPower.AfterSideTurnEnd`: gone as the enemies' turn ends.
        (Power::TaintedPower, _, EventHook::SideTurnEnd { side: CombatSide::Enemy, .. }) => {
            Some(HookEffect::Remove)
        }
        (Power::TaintedPower, _, EventHook::SideTurnEnd { .. }) => None,
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
            EventHook::EnergyReset { player, .. },
        ) if player == holder => {
            Some(HookEffect::GainEnergyAndRemove(amount))
        }
        (Power::EnergyNextTurnPower, PowerAmount::Endless, EventHook::EnergyReset { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless EnergyNextTurnPower handing over energy")
        }
        (
            Power::BlockNextTurnPower,
            PowerAmount::Counted(amount),
            EventHook::BlockCleared { creature, .. },
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
        // `AdaptablePower.AfterDeath`: a Test Subject that dies is sent to
        // its respawn. `TriggerDeadState` also counts the kill for the run's
        // title, which nothing here reads.
        (
            Power::AdaptablePower,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if creature == holder && monster == Some(Monster::TestSubject) => {
            state.set_reviving(true);
            Some(HookEffect::EnterMove(MoveId::RespawnMove))
        }
        // `EnragePower.AfterCardPlayed`: its stack as Strength on its owner for
        // every Skill played.
        (Power::EnragePower, PowerAmount::Counted(amount), EventHook::CardPlayed { card, .. }) => {
            (<V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill)).then_some(
                HookEffect::ApplyToOwner(Power::StrengthPower, PowerAmount::Counted(amount)),
            )
        }
        (Power::EnragePower, PowerAmount::Endless, EventHook::CardPlayed { .. }) => {
            unimplemented!("an endless Enrage")
        }
        // `PainfulStabsPower.AfterAttack`: Wounds for each hit its owner's
        // powered attack lands on a player's health.
        (
            Power::PainfulStabsPower,
            PowerAmount::Counted(amount),
            EventHook::AfterAttack {
                attacker,
                powered: true,
                ..
            },
        ) if attacker == holder => Some(HookEffect::GiveCardsPerUnblockedHit {
            card: Cards::Status(StatusCard::Wound),
            per: amount,
        }),
        (Power::PainfulStabsPower, PowerAmount::Endless, EventHook::AfterAttack { attacker, .. })
            if attacker == holder =>
        {
            unimplemented!("endless Painful Stabs")
        }
        (Power::PainfulStabsPower, _, EventHook::AfterAttack { .. }) => None,
        // `NemesisPower.AfterSideTurnEnd`, as its owner's side's turn ends:
        // Intangible every other turn, and taken off on the turns between.
        (Power::NemesisPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            let State::Nemesis { intangible } = state else {
                unreachable!("Nemesis holding {state:?}")
            };
            *intangible = !*intangible;
            if *intangible {
                Some(HookEffect::ApplyToOwner(
                    Power::IntangiblePower,
                    PowerAmount::Counted(one),
                ))
            } else {
                fight
                    .power(holder, Power::IntangiblePower)
                    .map(|_| HookEffect::RemoveFromOwner(Power::IntangiblePower))
            }
        }
        (Power::NemesisPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `BattlewornDummyTimeLimitPower.AfterSideTurnEnd`, as its owner's
        // side's turn ends: one off, and at the last the dummy escapes. The
        // escape is what the encounter's `RanOutOfTime` records.
        (
            Power::BattlewornDummyTimeLimitPower,
            PowerAmount::Counted(amount),
            EventHook::SideTurnEnd { side, .. },
        ) if side == owner => Some(match amount > one {
            true => HookEffect::Decrement,
            false => HookEffect::Escape,
        }),
        (Power::BattlewornDummyTimeLimitPower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless time limit")
        }
        (Power::BattlewornDummyTimeLimitPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `AfterCardGeneratedForCombat`, for a card its holder made: every
        // enemy hit for a Status, orbs channelled for a Status, Strength or
        // block for any card.
        (
            Power::SmokestackPower
            | Power::TrashToTreasurePower
            | Power::ArsenalPower
            | Power::PillarOfCreationPower,
            PowerAmount::Counted(amount),
            EventHook::CardGeneratedForCombat {
                card,
                creator: Some(creator),
                ..
            },
        ) if creator == holder => {
            let status = <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Status);
            match power {
                Power::SmokestackPower => status.then_some(HookEffect::DamageOpponents(amount)),
                Power::TrashToTreasurePower => status.then_some(HookEffect::ChannelRandomOrbs {
                    orbs: &VALID_ORBS,
                    count: amount,
                }),
                Power::ArsenalPower => Some(HookEffect::ApplyToOwner(
                    Power::StrengthPower,
                    PowerAmount::Counted(amount),
                )),
                _ => Some(HookEffect::GainBlock(amount)),
            }
        }
        (
            Power::SmokestackPower
            | Power::TrashToTreasurePower
            | Power::ArsenalPower
            | Power::PillarOfCreationPower,
            PowerAmount::Endless,
            EventHook::CardGeneratedForCombat { .. },
        ) => unimplemented!("an endless {power:?}"),
        (
            Power::SmokestackPower
            | Power::TrashToTreasurePower
            | Power::ArsenalPower
            | Power::PillarOfCreationPower,
            _,
            EventHook::CardGeneratedForCombat { .. },
        ) => None,
        // `ChainsOfBindingPower.AfterCardDrawn`, in its owner's turn: Bound on
        // the card drawn, while fewer cards than its stack were Bound this
        // turn.
        (
            Power::ChainsOfBindingPower,
            PowerAmount::Counted(amount),
            EventHook::CardDrawn { id, side, .. },
        ) if side == owner => {
            let bound = history.afflicted_this_turn(Affliction::Bound);
            (Number::try_from(bound).is_ok_and(|bound| bound < amount)).then_some(
                HookEffect::Afflict {
                    to: sts2_core::Writing::Named(id),
                    affliction: Affliction::Bound,
                },
            )
        }
        (Power::ChainsOfBindingPower, PowerAmount::Endless, EventHook::CardDrawn { .. }) => {
            unimplemented!("endless Chains of Binding")
        }
        (Power::ChainsOfBindingPower, _, EventHook::CardDrawn { .. }) => None,
        // `BeforeCardPlayed`: a Bound card of its owner's is played.
        (
            Power::ChainsOfBindingPower,
            _,
            EventHook::CardPlaying {
                affliction: Some(Affliction::Bound),
                ..
            },
        ) => {
            *state = State::ChainsOfBinding { bound_played: true };
            None
        }
        (Power::ChainsOfBindingPower, _, EventHook::CardPlaying { .. }) => None,
        // `BeforeSideTurnEnd`, as its owner's side's turn ends: none played,
        // and Bound off every card that has it.
        (Power::ChainsOfBindingPower, _, EventHook::BeforeSideTurnEnd { side, .. })
            if side == owner =>
        {
            *state = State::ChainsOfBinding {
                bound_played: false,
            };
            Some(HookEffect::ClearAffliction(
                sts2_core::Writing::EveryAfflictedWith(Affliction::Bound),
            ))
        }
        (Power::ChainsOfBindingPower, _, EventHook::BeforeSideTurnEnd { .. }) => None,
        // `TheBombPower.BeforeSideTurnEnd`: one off its turns as its owner's
        // turn is about to end. At the last, its damage, unpowered, to each
        // enemy that can be hit, and then it goes.
        (Power::TheBombPower, PowerAmount::Counted(turns), EventHook::BeforeSideTurnEnd { side, .. })
            if side == owner =>
        {
            if turns > 1 {
                Some(HookEffect::Decrement)
            } else {
                let State::TheBomb { damage } = *state else {
                    unreachable!("The Bomb keeps its damage")
                };
                asked.push(HookEffect::DamageOpponents(damage));
                asked.push(HookEffect::Remove);
                return;
            }
        }
        (Power::TheBombPower, PowerAmount::Endless, EventHook::BeforeSideTurnEnd { side, .. })
            if side == owner =>
        {
            unimplemented!("an endless Bomb")
        }
        (Power::TheBombPower, _, EventHook::BeforeSideTurnEnd { .. }) => None,
        // `DampenPower.AfterApplied`: every upgraded card its owner holds in
        // the fight downgraded, and kept with the level it had.
        (Power::DampenPower, _, EventHook::Applied) => Some(HookEffect::DowngradeUpgraded),
        // `AfterDeath` of one of its casters, which the fight keeps beside it:
        // off the list, and once none is left, it comes off.
        (
            Power::DampenPower,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) => {
            let casters = fight
                .kept(holder, power)
                .map_or(&[][..], |kept| &kept.creatures[..]);
            if !casters.contains(&creature) {
                return;
            }
            asked.push(HookEffect::ForgetCreature(creature));
            (casters.len() == 1).then_some(HookEffect::Remove)
        }
        (Power::DampenPower, _, EventHook::AfterDeath { .. }) => None,
        // `AfterRemoved`: each copy it downgraded upgraded back, once for each
        // level it had.
        (Power::DampenPower, _, EventHook::Removed) => {
            let downgraded = fight
                .kept(holder, power)
                .map_or(&[][..], |kept| &kept.cards[..]);
            asked.extend(
                downgraded
                    .iter()
                    .map(|&(card, times)| HookEffect::UpgradeCopy { card, times }),
            );
            return;
        }
        // `WitheringPresencePower.AfterCardPlayed`, for each card its target
        // plays, which is the one player: every sixth, a Wither into the hand.
        (Power::WitheringPresencePower, _, EventHook::CardPlayed { .. }) => {
            let State::WitheringPresence { cards_left } = state else {
                unreachable!("Withering Presence holding {state:?}")
            };
            *cards_left -= 1;
            (*cards_left == 0).then(|| {
                *cards_left = WITHERING_CARDS;
                HookEffect::GenerateInHand {
                    card: Cards::Status(StatusCard::Wither),
                    count: one,
                }
            })
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
        // `StockPower.AfterDeath` for its own holder: a new Axebot in its
        // slot, with one stock fewer, which opens on Boot Up.
        (
            Power::StockPower,
            PowerAmount::Counted(amount),
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if creature == holder && amount > 0 => Some(HookEffect::SummonWritten {
            monster: Monster::Axebot,
            written: amount - 1,
            opens_on: Some(MoveId::BootUpMove),
        }),
        (Power::StockPower, _, EventHook::AfterDeath { .. }) => None,
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
        (Power::TenderPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
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
        (Power::SlumberPower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side, .. })
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
        (Power::DarkEmbracePower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            let counted = state.counted();
            state.clear();
            Some(HookEffect::Draw(amount * counted))
        }
        // `JugglingPower.AfterApplied`: the attacks whose plays started this
        // turn, counted. A fight here has one player, whose every card is.
        (Power::JugglingPower, _, EventHook::Applied) => {
            let attacks = history
                .started_this_turn()
                .filter(|started| <V as sts2_core::Rules>::card_kind(started.card) == Some(sts2_core::CardKind::Attack))
                .count();
            state.clear();
            state.count(Number::try_from(attacks).expect("a turn's plays fit a number"));
            None
        }
        // `AfterCardPlayed`: one more attack, and on the third, its stack of
        // clones of that attack into hand.
        (Power::JugglingPower, PowerAmount::Counted(amount), EventHook::CardPlayed { card, id, .. })
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack) =>
        {
            state.count(one);
            (state.counted() == JUGGLING_ATTACKS)
                .then_some(HookEffect::CloneIntoHand { card: id, count: amount })
        }
        (Power::JugglingPower, PowerAmount::Endless, EventHook::CardPlayed { .. }) => {
            unimplemented!("an endless Juggling")
        }
        (Power::JugglingPower, _, EventHook::CardPlayed { .. }) => None,
        // `AfterSideTurnEnd` on its owner's side starts the count again.
        (Power::JugglingPower, _, EventHook::SideTurnEnd { side, .. }) => {
            if side == owner {
                state.clear();
            }
            None
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
        // `CurlUpPower.AfterDamageReceived`: a powered attack on its owner
        // from a card names that card, unless another is named already.
        (
            Power::CurlUpPower,
            _,
            EventHook::DamageReceived {
                target,
                powered: true,
                card: Some(card),
                ..
            },
        ) if target == holder => {
            let played = state.curl_played();
            if played.is_none_or(|named| named == card) {
                *played = Some(card);
            }
            None
        }
        (Power::CurlUpPower, _, EventHook::DamageReceived { .. }) => None,
        // `CurlUpPower.AfterCardPlayed`: once the named card's play is over,
        // `CreatureCmd.GainBlock` of its stack, unpowered, then
        // `PowerCmd.Remove(this)`.
        (Power::CurlUpPower, PowerAmount::Counted(amount), EventHook::CardPlayed { id, .. })
            if *state.curl_played() == Some(id) =>
        {
            *state.curl_played() = None;
            Some(HookEffect::GainBlockAndRemove(amount))
        }
        (Power::CurlUpPower, PowerAmount::Endless, EventHook::CardPlayed { id, .. })
            if *state.curl_played() == Some(id) =>
        {
            unimplemented!("an endless Curl Up")
        }
        (Power::CurlUpPower, _, EventHook::CardPlayed { .. }) => None,
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
                    stunned(sts2_core::StunResume::Interrupted),
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
        // `FlutterPower.AfterDamageReceived`: a powered attack that reaches
        // its owner's hit points spends one. The last one stuns its owner,
        // which then goes on past the move the stun interrupted.
        (
            Power::FlutterPower,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if target == holder && unblocked != 0 => {
            if amount == one {
                asked.extend([
                    HookEffect::Decrement,
                    stunned(sts2_core::StunResume::AfterInterrupted),
                ]);
                return;
            }
            Some(HookEffect::Decrement)
        }
        (Power::FlutterPower, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if target == holder =>
        {
            unimplemented!("an endless Flutter")
        }
        (Power::FlutterPower, _, EventHook::DamageReceived { .. }) => None,
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
                ..
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
                    stunned(sts2_core::StunResume::At(cry))
                }
                _ => stunned(sts2_core::StunResume::Interrupted),
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
                stunned(sts2_core::StunResume::At(bite)),
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
        // `PanachePower.AfterCardPlayed`: each play after the one that
        // applied it counts down, and the fifth hits every enemy for its
        // stack, unpowered. A fight here has one player, whose cards are all
        // its holder's.
        (Power::PanachePower, PowerAmount::Counted(amount), EventHook::CardPlayed { .. }) => {
            let State::Panache {
                cards_left,
                applied,
            } = state
            else {
                unreachable!("cards left for {:?}", state.power())
            };
            let hit = *applied && {
                *cards_left -= 1;
                *cards_left == 0
            };
            *applied = true;
            if hit {
                *cards_left = PANACHE_CARDS;
                Some(HookEffect::DamageOpponents(amount))
            } else {
                None
            }
        }
        // `PanachePower.AfterSideTurnEnd`: the count starts again as its
        // owner's side ends a turn.
        (Power::PanachePower, _, EventHook::SideTurnEnd { side, .. }) => {
            if side == owner {
                let State::Panache { cards_left, .. } = state else {
                    unreachable!("cards left for {:?}", state.power())
                };
                *cards_left = PANACHE_CARDS;
            }
            None
        }
        (Power::PanachePower, PowerAmount::Endless, EventHook::CardPlayed { .. }) => {
            unimplemented!("an endless Panache")
        }
        // `StratagemPower.AfterShuffle`: `FromCombatPile` over the draw pile
        // for its stack of cards, each into hand. A fight here has one
        // player, whose shuffle is its holder's.
        (Power::StratagemPower, PowerAmount::Counted(amount), EventHook::Shuffled) => {
            Some(HookEffect::SelectFromPile {
                pile: sts2_core::Pile::Draw,
                selection: exactly(amount),
                act: sts2_core::PileAct::IntoHand,
            })
        }
        (Power::StratagemPower, PowerAmount::Endless, EventHook::Shuffled) => {
            unimplemented!("an endless Stratagem")
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
        (Power::FlameBarrierPower, _, EventHook::SideTurnEnd { side, .. }) if side != owner => {
            Some(HookEffect::Remove)
        }
        // `DuplicationPower.AfterModifyingCardPlayCount`, and its
        // `AfterSideTurnEnd` for its owner's own side, the same as One-Two
        // Punch's.
        (Power::DuplicationPower, _, EventHook::PlayCountModified { .. }) => {
            Some(HookEffect::Decrement)
        }
        (Power::DuplicationPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            Some(HookEffect::Remove)
        }
        (Power::DuplicationPower, _, EventHook::SideTurnEnd { .. }) => None,
        (Power::BurstPower | Power::SignalBoostPower, _, EventHook::PlayCountModified { .. }) => {
            Some(HookEffect::Decrement)
        }
        // `EchoFormPower.AfterModifyingCardPlayCount` does nothing.
        (Power::EchoFormPower, _, EventHook::PlayCountModified { .. }) => None,
        (Power::BurstPower | Power::ShadowmeldPower | Power::CorrosiveWavePower, _, EventHook::SideTurnEnd { side, .. })
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
                ..
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
        // `ConfusedPower.AfterCardDrawn`: a cost drawn below four for each of
        // its owner's cards drawn, unless the card's cost is negative. An X
        // cost counts as zero, so it draws and keeps spending its X.
        (Power::ConfusedPower, _, EventHook::CardDrawn { card, id, .. })
            if owner == CombatSide::Player && card.cost() != sts2_core::Cost::None =>
        {
            Some(HookEffect::RandomCost {
                card: id,
                below: CONFUSED_COSTS,
            })
        }
        (Power::ConfusedPower, _, EventHook::CardDrawn { .. }) => None,
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
        // `SpiritOfAshPower.BeforeCardPlayed`: its stack as block, unpowered,
        // before each of its owner's Ethereal cards is played.
        (
            Power::SpiritOfAshPower,
            PowerAmount::Counted(amount),
            EventHook::CardPlaying { ethereal: true, .. },
        ) if owner == CombatSide::Player => Some(HookEffect::GainBlock(amount)),
        (Power::SpiritOfAshPower, PowerAmount::Endless, EventHook::CardPlaying { ethereal: true, .. })
            if owner == CombatSide::Player =>
        {
            unimplemented!("an endless Spirit of Ash")
        }
        (Power::SpiritOfAshPower, _, EventHook::CardPlaying { .. }) => None,
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
        (Power::RegenPower, PowerAmount::Counted(amount), EventHook::SideTurnEnd { side, .. })
            if side == owner =>
        {
            asked.extend([HookEffect::Heal(amount), HookEffect::Decrement]);
            return;
        }
        (Power::RegenPower, PowerAmount::Endless, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            unimplemented!("an endless Regen")
        }
        (Power::RegenPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `RitualPower.AfterSideTurnEnd`: its stack of Strength as its
        // owner's side ends a turn. A monster's skips the turn it was applied
        // in, through `AfterApplied`.
        (Power::RitualPower, amount, EventHook::SideTurnEnd { side, .. }) if side == owner => {
            match state.swap_just_applied(false) {
                true => None,
                false => Some(HookEffect::ApplyToOwner(Power::StrengthPower, amount)),
            }
        }
        (Power::RitualPower, _, EventHook::SideTurnEnd { .. }) => None,
        // `RitualPower.AfterApplied`, which marks a monster's alone.
        (Power::RitualPower, _, EventHook::Applied) => {
            if owner != CombatSide::Player {
                state.swap_just_applied(true);
            }
            None
        }
        // `RadiancePower.AfterEnergyReset`: its `Energy`, then one off.
        (Power::RadiancePower, PowerAmount::Counted(_), EventHook::EnergyReset { player, .. })
            if player == holder =>
        {
            asked.extend([HookEffect::GainEnergy(RADIANCE_ENERGY), HookEffect::Decrement]);
            return;
        }
        (Power::RadiancePower, PowerAmount::Endless, EventHook::EnergyReset { player, .. })
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
        (Power::OneTwoPunchPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
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
        (Power::DoomPower, _, EventHook::BeforeSideTurnEnd { side, .. }) => match side == owner {
            true => Some(HookEffect::DoomKill),
            false => None,
        },
        // `HailstormPower.BeforeSideTurnEnd`: its stack of damage to every enemy
        // while its owner holds a Frost orb.
        (Power::HailstormPower, PowerAmount::Counted(amount), EventHook::BeforeSideTurnEnd { side, .. })
            if side == owner =>
        {
            Some(HookEffect::DamageOpponentsHolding {
                orb: Orb::FrostOrb,
                amount,
            })
        }
        (Power::HailstormPower, PowerAmount::Endless, EventHook::BeforeSideTurnEnd { side, .. })
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
        (Power::LoopPower, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if player == holder =>
        {
            Some(HookEffect::FirstOrbPassive(amount))
        }
        (Power::LoopPower, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
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
            writes: &[],
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
            writes: &[(
                sts2_core::Change::AddKeyword(CardKeyword::Ethereal),
                sts2_core::Lasting::ThisFight,
            )],
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
            EventHook::PlayerTurnStart { player, .. },
        ) if player == holder && held.on_turn_start != PowerAmount::Counted(0) => {
            asked.extend([HookEffect::Summon(amount), HookEffect::Remove]);
            return;
        }
        (Power::SummonNextTurnPower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `GenesisPower.AfterEnergyReset`: its stack of stars, every turn.
        (Power::GenesisPower, PowerAmount::Counted(amount), EventHook::EnergyReset { player, .. })
            if player == holder =>
        {
            Some(HookEffect::GainStars(amount))
        }
        // `LightningRodPower.AfterEnergyReset`: a Lightning orb for its owner,
        // then one off the stack.
        (
            Power::LightningRodPower,
            PowerAmount::Counted(_),
            EventHook::EnergyReset { player, .. },
        ) if player == holder => {
            asked.extend([HookEffect::Channel(Orb::LightningOrb), HookEffect::Decrement]);
            return;
        }
        (Power::LightningRodPower, PowerAmount::Endless, EventHook::EnergyReset { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless LightningRodPower")
        }
        (Power::LightningRodPower, _, EventHook::EnergyReset { .. }) => None,
        // `StarNextTurnPower.AfterEnergyReset`: its stars once, then it goes.
        (
            Power::StarNextTurnPower,
            PowerAmount::Counted(amount),
            EventHook::EnergyReset { player, .. },
        ) if player == holder => {
            asked.extend([HookEffect::GainStars(amount), HookEffect::Remove]);
            return;
        }
        (
            Power::GenesisPower | Power::StarNextTurnPower,
            PowerAmount::Endless,
            EventHook::EnergyReset { player, .. },
        ) if player == holder => unimplemented!("an endless {power:?}"),
        (Power::GenesisPower | Power::StarNextTurnPower, _, EventHook::EnergyReset { .. }) => None,
        // `TyrannyPower.AfterPlayerTurnStart`: its owner picks its stack of
        // cards from hand, and each is exhausted.
        (
            Power::TyrannyPower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player, .. },
        ) if player == holder => Some(HookEffect::SelectFromPile {
            pile: sts2_core::Pile::Hand,
            selection: exactly(amount),
            act: sts2_core::PileAct::Exhaust,
        }),
        // `NecroMasteryPower.AfterCurrentHpChanged`: the hit points its
        // owner's Osty lost, times its stack, to each enemy that can be hit,
        // unblockable and unpowered.
        (
            Power::NecroMasteryPower,
            PowerAmount::Counted(amount),
            EventHook::CurrentHpChanged {
                monster: Some(Monster::Osty),
                pet_of: Some(pet_of),
                delta,
                ..
            },
        ) if delta < 0 && pet_of == holder => {
            Some(HookEffect::DamageOpponentsUnblockable(-delta * amount))
        }
        (Power::NecroMasteryPower, PowerAmount::Endless, EventHook::CurrentHpChanged { .. }) => {
            unimplemented!("an endless Necro Mastery")
        }
        (Power::NecroMasteryPower, _, EventHook::CurrentHpChanged { .. }) => None,
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
            EventHook::PlayerTurnStart { player, .. },
        ) if player == holder => Some(HookEffect::SelectFromPile {
            pile: sts2_core::Pile::Hand,
            selection: exactly(amount),
            act: sts2_core::PileAct::TransformToRandom,
        }),
        (Power::EntropyPower, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
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
            EventHook::PlayerTurnStart { player, .. },
        ) if player == holder => {
            asked.extend([
                HookEffect::DamageOpponents(amount),
                HookEffect::SetAmount(amount + ROLLING_BOULDER_GROWTH),
            ]);
            return;
        }
        (Power::RollingBoulderPower, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
            if player == holder =>
        {
            unimplemented!("an endless Rolling Boulder")
        }
        (Power::RollingBoulderPower, _, EventHook::PlayerTurnStart { .. }) => None,
        // `ToolsOfTheTradePower.AfterPlayerTurnStart`: the same, discarded.
        (
            Power::ToolsOfTheTradePower,
            PowerAmount::Counted(amount),
            EventHook::PlayerTurnStart { player, .. },
        ) if player == holder => Some(HookEffect::SelectFromPile {
            pile: sts2_core::Pile::Hand,
            selection: exactly(amount),
            act: sts2_core::PileAct::Discard,
        }),
        (
            Power::TyrannyPower | Power::ToolsOfTheTradePower,
            PowerAmount::Endless,
            EventHook::PlayerTurnStart { player, .. },
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
                    selection: exactly(amount),
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
        // `ImprovementPower.AfterCombatEnd`: its stack of upgradable deck
        // cards upgraded, each `NextItem` off `CombatCardSelection`. Every
        // card upgrades once, so the list is the deck's as each draw is made.
        (Power::ImprovementPower, PowerAmount::Counted(amount), EventHook::CombatEnd { .. }) => {
            use sts2_core::RunStreamKinds as _;

            let count = usize::try_from(amount).expect("a stack is not negative");
            let stream = sts2_core::Stream::Run(
                <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_SELECTION,
            );
            asked.extend(std::iter::repeat_n(
                HookEffect::UpgradeRandomDeckCard { stream },
                count,
            ));
            return;
        }
        (Power::ImprovementPower, PowerAmount::Endless, EventHook::CombatEnd { .. }) => {
            unimplemented!("an endless Improvement")
        }
        // `ForbiddenGrimoirePower.AfterCombatEnd`: a card removal reward for
        // each of its stack.
        (
            Power::ForbiddenGrimoirePower,
            PowerAmount::Counted(amount),
            EventHook::CombatEnd { .. },
        ) => {
            let count = usize::try_from(amount).expect("a stack is not negative");
            asked.extend(std::iter::repeat_n(HookEffect::AddCardRemovalReward, count));
            return;
        }
        (Power::ForbiddenGrimoirePower, PowerAmount::Endless, EventHook::CombatEnd { .. }) => {
            unimplemented!("an endless Forbidden Grimoire")
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
        (Power::RetainHandPower, _, EventHook::SideTurnEnd { side, .. }) if side == owner => {
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
            | Power::AdaptablePower
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
        // has Minion already.
        (Power::IllusionPower, _, EventHook::Applied) => fight
            .power(holder, Power::MinionPower)
            .is_none()
            .then_some(HookEffect::ApplyToOwner(
                Power::MinionPower,
                PowerAmount::Counted(one),
            )),
        // `SwordSagePower.AfterRemoved`: every Sovereign Blade its owner holds
        // loses as many replays as the instance held.
        (Power::SwordSagePower, PowerAmount::Counted(amount), EventHook::Removed) => {
            Some(HookEffect::AddReplays {
                to: sts2_core::Writing::EveryCopyOf(Cards::Token(TokenCard::SovereignBlade)),
                amount: -amount,
            })
        }
        (Power::SwordSagePower, PowerAmount::Endless, EventHook::Removed) => {
            unimplemented!("an endless Sword Sage coming off")
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
