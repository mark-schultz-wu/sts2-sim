//! What a power does, ported.
//!
//! One function per hook, matching on the power the way `PowerModel`'s
//! subclasses override one. A power gets an arm when something in a fight can
//! apply it.
//!
//! A power nobody has ported panics naming itself. One is only in a fight
//! because ported behaviour put it there. So an unported power reaching here
//! is a gap in the port, and not a state a run can reach.

mod lends;

pub(crate) use lends::{debuff_at, lends};

use crate::vars::PowerVars as _;
use sts2_core::{
    CardTable as _, Fixed, Modify, ModifyHook, PowerAmount, PowerModel as _, PowerTable as _,
};

use crate::generated::cards::{CardKeyword, CardTag};
use crate::generated::monsters::IntentKind;
use crate::generated::runstate::CombatSide;
use crate::ids::{
    AfflictionIds as _, CardIds as _, MonsterIds as _, MoveIds as _, OrbIds as _, PowerIds as _,
    RelicIds as _,
};
type Number = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State<V: crate::Build> {
    Stateless(V::Power),
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
    /// `StolenDexterity`: what it took off each player, in the order it
    /// first took from them.
    Possess {
        power: V::Power,
        from: [Option<sts2_core::CombatId>; POSSESSED],
        stolen: [Number; POSSESSED],
    },
    /// `InterceptPower`'s `coveredCreatures`, in the order they were added.
    Intercept {
        covered: [Option<sts2_core::CombatId>; POSSESSED],
    },
    /// `IllusionPower`'s `isReviving`.
    Illusion {
        reviving: bool,
    },
    /// `ReattachPower`'s `isReviving`.
    Reattach {
        reviving: bool,
    },
    /// `BeaconOfHopePower`'s `HasAlreadyBeenGivenBlock`, set while the block
    /// it gives its teammates lands.
    BeaconOfHope {
        busy: bool,
    },
    /// `SoulboundPower`'s `IsAddingSoul`, set while the Souls it adds land.
    Soulbound {
        power: V::Power,
        busy: bool,
    },
    /// `CacophonyPower`'s `Cards`: the draws left until it hits.
    Cacophony {
        power: V::Power,
        cards_left: u8,
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
        power: V::Power,
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
    /// `SkittishPower`'s `hasGainedBlockThisTurn`.
    Skittish {
        gained: bool,
    },
    /// `HardenedShellPower`'s `damageReceivedThisTurn`.
    HardenedShell {
        received: Number,
    },
    /// `ThieveryPower`'s `GoldVar`, the gold it has stolen.
    Thievery {
        stolen: Number,
    },
    /// `SwipePower.StolenCard`'s deck version.
    Swipe {
        stolen: Option<sts2_core::DeckCardId>,
    },
    /// `OrbitPower`'s `energySpent` and `triggerCount`.
    Orbit {
        spent: Number,
        triggers: Number,
    },
    /// `VigorPower`'s `commandToModify`, which stays bound once the command
    /// is over.
    Vigor {
        bound: Option<Bound<V>>,
    },
    /// `OutbreakPower`'s `timesPoisoned`, and the power, which is the build's
    /// own.
    Outbreak {
        power: V::Power,
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
        power: V::Power,
        playing: [Option<(sts2_core::CardId<V>, Number)>; NOTED_AT_ONCE],
    },
    /// A power that [`lends`] a stat, with `_shouldIgnoreNextInstance`,
    /// which `IgnoreNextInstance` sets.
    Temporary {
        power: V::Power,
        ignore_next: bool,
    },
    /// v0.111.0's `PaleBlueDotPower`: `alreadyActivatedThisTurn`.
    PaleBlueDot {
        activated: bool,
    },
    /// v0.111.0's `ImitationLearningPower`: `PlayerTarget`, whose Power
    /// cards it copies, and its `cardsAndClones`. That list holds a pair
    /// while one of those plays is under way, from `BeforeCardPlayed` to
    /// `AfterCardPlayed`, and the clone stands in no pile meanwhile.
    ImitationLearning {
        power: V::Power,
        player: Option<sts2_core::CombatId>,
        original: Option<sts2_core::CardId<V>>,
        clone: Option<sts2_core::CardId<V>>,
    },
}

impl<V: crate::Build> State<V> {
    /// `ITemporaryPower.IgnoreNextInstance`, on a power that lends a stat.
    ///
    /// # Panics
    ///
    /// Any other power, which is no `ITemporaryPower`.
    pub(crate) fn ignore_next_instance(&mut self) {
        let Self::Temporary { ignore_next, .. } = self else {
            unimplemented!("{:?} ignoring its next instance", self.power());
        };
        *ignore_next = true;
    }
}

/// Whether a clone of `power`, as `ClonePreservingMutability` makes one,
/// starts as a new instance does, whatever this one holds.
///
/// A noted play is `amountsForPlayedCards`, the power's internal data, which
/// `DeepCloneFields` builds anew. Imbalanced's off-balance stands for the
/// Bowlbug Rock's `IsOffBalance` or a stun, which its model holds nothing of.
pub(crate) fn clone_starts_anew<V: crate::Build>(power: V::Power) -> bool {
    power == V::Power::IMBALANCED_POWER
        || power == V::Power::STRANGLE_POWER
        || power == V::Power::OBLIVION_POWER
        || power == V::Power::AFTERIMAGE_POWER
        || power == V::Power::SUBROUTINE_POWER
        || power == V::Power::STORM_POWER
        || power == V::Power::CALAMITY_POWER
        || power == V::Power::RUPTURE_POWER
        || power == V::Power::SERPENT_FORM_POWER
}

/// `GetScaledAmountForMultiplayer` for a power whose
/// `ShouldScaleInMultiplayer` holds, and `None` for any other. The default
/// is `amount * players * factor`. Plating, Artifact, Slippery and Skittish
/// have formulas of their own.
pub(crate) fn scaled_for_multiplayer<V: crate::Build>(
    power: V::Power,
    amount: sts2_core::Decimal,
    players: usize,
    factor: sts2_core::Decimal,
) -> Option<sts2_core::Decimal> {
    use crate::generated::powers::PowerFlag;
    use sts2_core::Decimal;

    let crate::Field::Constant(scales) = sts2_core::Table::data(power)
        .flags
        .get(PowerFlag::ScalesInMultiplayer)
    else {
        unimplemented!("whether {power:?} scales in multiplayer")
    };
    if !scales {
        return None;
    }
    let players = Decimal::from(players);
    let others = players - Decimal::ONE;
    Some(match power {
        __c0 if __c0 == V::Power::PLATING_POWER => (others * Decimal::TWO + Decimal::ONE) * amount,
        __c0 if __c0 == V::Power::ARTIFACT_POWER => amount + others,
        __c0 if __c0 == V::Power::SLIPPERY_POWER => amount * players,
        __c0 if __c0 == V::Power::SKITTISH_POWER => {
            amount * (Decimal::ONE + others * Decimal::new(5, 1))
        }
        _ => amount * players * factor,
    })
}

/// `VoidFormPower.HideTemporaryZeroCostVisual`'s count, which leaves no
/// card free for the rest of the turn.
const VOID_FORM_SPENT: u32 = 999_999_999;

/// `OutbreakPower.poisonThreshold`: every third Poison deals its damage.
pub(crate) const OUTBREAK_EVERY: u8 = 3;

/// How many plays `amountsForPlayedCards` keeps at once: one, and those
/// played inside it. A Havoc that plays a Havoc that plays a card holds
/// three, and a replay that auto-plays a card that auto-plays another holds
/// four. The game's dictionary has no bound, so a deeper nest stops here.
const NOTED_AT_ONCE: usize = 6;

/// `amountsForPlayedCards.Add`: `id`'s play noted with the stack as it began.
fn note<V: crate::Build>(state: &mut State<V>, id: sts2_core::CardId<V>, amount: Number) {
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
fn add_noted<V: crate::Build>(
    state: &mut State<V>,
    id: sts2_core::CardId<V>,
    amount: Number,
) -> bool {
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

fn take_noted<V: crate::Build>(state: &mut State<V>, id: sts2_core::CardId<V>) -> Option<Number> {
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
pub struct Bound<V: crate::Build> {
    command: sts2_core::AttackId,
    /// The command's `ModelSource`: the card, or nothing for a monster's.
    source: Option<sts2_core::CardId<V>>,
    /// `amountWhenAttackStarted`.
    amount: Number,
}

/// Whether `ReattachPower.AreAllOtherSegmentsDead` is false for `holder`:
/// one of its other teammates holds Reattach and is not `IsDead`, which is
/// `CurrentHp <= 0`. A segment the same command took to nothing is dead to
/// it, though its own death is still to come.
fn other_segment_standing<V: crate::Build>(
    fight: &dyn sts2_core::FightView<V>,
    holder: sts2_core::CombatId,
) -> bool {
    fight.teammates(holder).into_iter().any(|other| {
        other != holder
            && fight.power(other, V::Power::REATTACH_POWER).is_some()
            && fight.current_hp(other).is_some_and(|hp| hp > 0)
    })
}

/// The players a Possess power keeps a count for, and Intercept covers: a
/// run's most, which
/// `NMultiplayerHostSubmenu` hosts with `InitializeMultiplayerAsHost(netService, 4)`.
const POSSESSED: usize = 4;

/// `JugglingPower`'s third attack in a turn.
const JUGGLING_ATTACKS: Number = 3;

/// Where a power keeps the clone of a card the fight holds for it.
///
/// # Panics
///
/// A power that keeps no card.
pub(crate) fn kept_card<V: crate::Build>(state: &mut State<V>, kept: sts2_core::CardId<V>) {
    match state {
        // `NightmarePower.SetSelectedCard`.
        State::Nightmare { card } => *card = Some(kept),
        // `CreateCloneForPlayer`, paired with the card its play started.
        State::ImitationLearning {
            original: Some(_),
            clone: clone @ None,
            ..
        } => *clone = Some(kept),
        state => unimplemented!("{:?} keeping a card", state.power()),
    }
}

/// What a card's note does to the power it just applied.
///
/// # Panics
///
/// A power no card writes on.
pub(crate) fn noted<V: crate::Build>(state: &mut State<V>, note: Number) {
    match state {
        // `ToricToughnessPower.SetBlock`.
        State::ToricToughness { block } => *block = note,
        // `IncrementSelfDamage`, once for each note of one.
        State::SelfDamage { self_damage, .. } => *self_damage += note,
        state => unimplemented!("a card writing {note} on {:?}", state.power()),
    }
}

impl<V: crate::Build> sts2_core::PowerModel<V> for State<V> {
    fn new(power: V::Power) -> Self {
        // A power whose rule is the build's own.
        if let Some(state) = V::own_power_state(power) {
            return state;
        }
        match power {
            __c0 if __c0 == V::Power::SLOW_POWER => Self::Slow { counted: 0 },
            __c0 if __c0 == V::Power::TENDER_POWER => Self::Tender { counted: 0 },
            __c0 if __c0 == V::Power::DARK_EMBRACE_POWER => Self::DarkEmbrace { counted: 0 },
            __c0 if __c0 == V::Power::JUGGLING_POWER => Self::Juggling { counted: 0 },
            __c0 if __c0 == V::Power::IMBALANCED_POWER => Self::Imbalanced { off_balance: false },
            __c0 if __c0 == V::Power::RITUAL_POWER => Self::Ritual {
                just_applied: false,
            },
            __c0 if __c0 == V::Power::POSSESS_STRENGTH_POWER
                || __c0 == V::Power::POSSESS_SPEED_POWER =>
            {
                Self::Possess {
                    power,
                    from: [None; POSSESSED],
                    stolen: [0; POSSESSED],
                }
            }
            __c0 if __c0 == V::Power::INTERCEPT_POWER => Self::Intercept {
                covered: [None; POSSESSED],
            },
            __c0 if __c0 == V::Power::ILLUSION_POWER => Self::Illusion { reviving: false },
            __c0 if __c0 == V::Power::REATTACH_POWER => Self::Reattach { reviving: false },
            __c0 if __c0 == V::Power::BEACON_OF_HOPE_POWER => Self::BeaconOfHope { busy: false },
            __c0 if __c0 == V::Power::ADAPTABLE_POWER => Self::Adaptable { reviving: false },
            __c0 if __c0 == V::Power::NEMESIS_POWER => Self::Nemesis { intangible: false },
            __c0 if __c0 == V::Power::TORIC_TOUGHNESS_POWER => Self::ToricToughness { block: 0 },
            __c0 if __c0 == V::Power::INFERNO_POWER || __c0 == V::Power::CRIMSON_MANTLE_POWER => {
                Self::SelfDamage {
                    power,
                    self_damage: 0,
                }
            }
            __c0 if __c0 == V::Power::NIGHTMARE_POWER => Self::Nightmare { card: None },
            __c0 if __c0 == V::Power::WITHERING_PRESENCE_POWER => Self::WitheringPresence {
                cards_left: WITHERING_CARDS,
            },
            __c0 if __c0 == V::Power::CHAINS_OF_BINDING_POWER => Self::ChainsOfBinding {
                bound_played: false,
            },
            __c0 if __c0 == V::Power::SWIPE_POWER => Self::Swipe { stolen: None },
            __c0 if __c0 == V::Power::THIEVERY_POWER => Self::Thievery { stolen: 0 },
            __c0 if __c0 == V::Power::HARDENED_SHELL_POWER => Self::HardenedShell { received: 0 },
            __c0 if __c0 == V::Power::SKITTISH_POWER => Self::Skittish { gained: false },
            __c0 if __c0 == V::Power::ORBIT_POWER => Self::Orbit {
                spent: 0,
                triggers: 0,
            },
            __c0 if __c0 == V::Power::VIGOR_POWER => Self::Vigor { bound: None },
            __c0 if __c0 == V::Power::FERAL_POWER => Self::Feral { played: 0 },
            __c0 if __c0 == V::Power::VOID_FORM_POWER => Self::VoidForm { played: 0 },
            __c0 if __c0 == V::Power::THE_BOMB_POWER => Self::TheBomb {
                // Its `DamageVar`, before `SetDamage` writes its own.
                damage: V::Power::THE_BOMB_POWER_DAMAGE,
            },
            __c0 if __c0 == V::Power::GIGANTIFICATION_POWER => {
                Self::Gigantification { bound: None }
            }
            __c0 if __c0 == V::Power::PANACHE_POWER => Self::Panache {
                cards_left: PANACHE_CARDS,
                applied: false,
            },
            __c0 if __c0 == V::Power::AUTOMATION_POWER => Self::Automation {
                cards_left: AUTOMATION_CARDS,
            },
            __c0 if __c0 == V::Power::CURL_UP_POWER => Self::CurlUp { played: None },
            __c0 if __c0 == V::Power::SURROUNDED_POWER => Self::Surrounded { facing_left: false },
            __c0 if __c0 == V::Power::SLOTH_POWER => Self::Sloth { played: 0 },
            __c0 if __c0 == V::Power::MONOLOGUE_POWER => Self::Monologue {
                playing: [None; NOTED_AT_ONCE],
                applied: 0,
            },
            __c0 if __c0 == V::Power::STRANGLE_POWER
                || __c0 == V::Power::OBLIVION_POWER
                || __c0 == V::Power::AFTERIMAGE_POWER
                || __c0 == V::Power::SUBROUTINE_POWER
                || __c0 == V::Power::STORM_POWER
                || __c0 == V::Power::CALAMITY_POWER
                || __c0 == V::Power::RUPTURE_POWER
                || __c0 == V::Power::SERPENT_FORM_POWER =>
            {
                Self::Noted {
                    power,
                    playing: [None; NOTED_AT_ONCE],
                }
            }
            power if lends::<V>(power).is_some() => Self::Temporary {
                power,
                ignore_next: false,
            },
            power => Self::Stateless(power),
        }
    }

    fn power(&self) -> V::Power {
        match self {
            Self::Stateless(power) => *power,
            Self::Slow { .. } => V::Power::SLOW_POWER,
            Self::Tender { .. } => V::Power::TENDER_POWER,
            Self::DarkEmbrace { .. } => V::Power::DARK_EMBRACE_POWER,
            Self::Juggling { .. } => V::Power::JUGGLING_POWER,
            Self::Imbalanced { .. } => V::Power::IMBALANCED_POWER,
            Self::Ritual { .. } => V::Power::RITUAL_POWER,
            Self::Possess { power, .. } => *power,
            Self::Intercept { .. } => V::Power::INTERCEPT_POWER,
            Self::Illusion { .. } => V::Power::ILLUSION_POWER,
            Self::Reattach { .. } => V::Power::REATTACH_POWER,
            Self::BeaconOfHope { .. } => V::Power::BEACON_OF_HOPE_POWER,
            Self::Soulbound { power, .. } => *power,
            Self::Cacophony { power, .. } => *power,
            Self::Adaptable { .. } => V::Power::ADAPTABLE_POWER,
            Self::Nemesis { .. } => V::Power::NEMESIS_POWER,
            Self::ToricToughness { .. } => V::Power::TORIC_TOUGHNESS_POWER,
            Self::SelfDamage { power, .. } => *power,
            Self::Nightmare { .. } => V::Power::NIGHTMARE_POWER,
            Self::WitheringPresence { .. } => V::Power::WITHERING_PRESENCE_POWER,
            Self::ChainsOfBinding { .. } => V::Power::CHAINS_OF_BINDING_POWER,
            Self::Swipe { .. } => V::Power::SWIPE_POWER,
            Self::Thievery { .. } => V::Power::THIEVERY_POWER,
            Self::HardenedShell { .. } => V::Power::HARDENED_SHELL_POWER,
            Self::Skittish { .. } => V::Power::SKITTISH_POWER,
            Self::Orbit { .. } => V::Power::ORBIT_POWER,
            Self::Outbreak { power, .. } => *power,
            Self::VoidForm { .. } => V::Power::VOID_FORM_POWER,
            Self::TheBomb { .. } => V::Power::THE_BOMB_POWER,
            Self::Vigor { .. } => V::Power::VIGOR_POWER,
            Self::Feral { .. } => V::Power::FERAL_POWER,
            Self::Gigantification { .. } => V::Power::GIGANTIFICATION_POWER,
            Self::Automation { .. } => V::Power::AUTOMATION_POWER,
            Self::Panache { .. } => V::Power::PANACHE_POWER,
            Self::CurlUp { .. } => V::Power::CURL_UP_POWER,
            Self::Surrounded { .. } => V::Power::SURROUNDED_POWER,
            Self::Sloth { .. } => V::Power::SLOTH_POWER,
            Self::Noted { power, .. } => *power,
            Self::Monologue { .. } => V::Power::MONOLOGUE_POWER,
            Self::Temporary { power, .. } => *power,
            Self::PaleBlueDot { .. } => V::Power::PALE_BLUE_DOT_POWER,
            Self::ImitationLearning { power, .. } => *power,
        }
    }

    fn mark_busy(&mut self, busy: bool) {
        if let Self::BeaconOfHope { busy: held } | Self::Soulbound { busy: held, .. } = self {
            *held = busy;
        }
    }

    fn add_covered(&mut self, creature: sts2_core::CombatId) {
        let Self::Intercept { covered } = self else {
            unimplemented!("{:?} covering a creature", self.power())
        };
        if covered.contains(&Some(creature)) {
            return;
        }
        let free = (covered.iter_mut().find(|at| at.is_none()))
            .unwrap_or_else(|| unimplemented!("Intercept covering {POSSESSED}+ creatures"));
        *free = Some(creature);
    }

    fn debuff(&self) -> bool {
        use sts2_core::PowerTable as _;

        let power = self.power();
        match lends::<V>(power) {
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

impl<V: crate::Build> State<V> {
    pub(crate) fn counted(&self) -> Number {
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
            false => V::Power::BACK_ATTACK_LEFT_POWER,
            true => V::Power::BACK_ATTACK_RIGHT_POWER,
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

    pub(crate) fn count(&mut self, amount: Number) {
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

    /// The stolen stack a Possess power keeps for `player`, to write on.
    /// A player it has not taken from yet joins the end.
    ///
    /// # Panics
    ///
    /// More players than it keeps room for.
    fn stolen_from(&mut self, player: sts2_core::CombatId) -> &mut Number {
        let Self::Possess { from, stolen, .. } = self else {
            unreachable!("stolen state for {:?}", self.power())
        };
        let at = match from.iter().position(|&held| held == Some(player)) {
            Some(at) => at,
            None => {
                let at = (from.iter().position(Option::is_none)).unwrap_or_else(|| {
                    unimplemented!("Possess in a fight of {POSSESSED}+ players")
                });
                from[at] = Some(player);
                at
            }
        };
        &mut stolen[at]
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

    pub(crate) fn set_off_balance(&mut self, off_balance: bool) {
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
    fn bound(&self) -> Option<Bound<V>> {
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

    /// The gold `ThieveryPower.Steal` has taken, which its `GoldVar` adds up.
    pub(crate) fn stolen_gold(&mut self) -> &mut Number {
        let Self::Thievery { stolen } = self else {
            unreachable!("stolen gold for {:?}", self.power())
        };
        stolen
    }

    /// `SwipePower.Steal`, which remembers the deck card taken.
    pub(crate) fn steal(&mut self, card: sts2_core::DeckCardId) {
        let Self::Swipe { stolen } = self else {
            unreachable!("stolen state for {:?}", self.power())
        };
        *stolen = Some(card);
    }
}

/// `TriggerAboutToBlowState`'s `SetMaxAndCurrentHp`.
const ABOUT_TO_BLOW_HP: Number = 999_999_999;

/// `CreatureCmd.Stun`'s own state, which advertises a stunned turn.
fn stunned<V: crate::Build>(then: sts2_core::StunResume) -> sts2_core::HookEffect<V> {
    sts2_core::HookEffect::StunOwner {
        stun: V::powers_stunned(),
        then,
    }
}

/// A state a power builds: one move, drawing one intent with no number.
pub(crate) fn built<V: crate::build::BuildContent>(
    id: V::MoveId,
    kind: IntentKind,
) -> sts2_core::NextMove<V> {
    sts2_core::NextMove {
        id,
        intents: [sts2_core::Intent { kind, attack: None }]
            .into_iter()
            .collect(),
    }
}

/// `PaperPhrog.ModifyVulnerableMultiplier`'s `0.25m`, in hundredths.
const PHROG: i32 = 25;

/// A multiplier the game holds as a decimal, such as Vulnerable's `1.5m`, in
/// hundredths. Vulnerable's and Weak's multipliers are sums of the game's
/// decimals, which are summed here in hundredths.
fn hundredths(multiplier: Fixed) -> i32 {
    let scale = multiplier.scale();
    assert!(scale <= 2, "{multiplier:?} is finer than hundredths");
    multiplier.coefficient() * 10_i32.pow(u32::from(2 - scale))
}

/// `PaperKrane.ModifyWeakMultiplier`'s `0.15m`, in hundredths.
const KRANE: i32 = 15;

/// `SurroundedPower`'s `1.5m` on a hit from behind.
const SURROUNDED: Fixed = Fixed::new(15, 1);

/// `FrailPower.ModifyBlockMultiplicative`'s `0.75m`.
const FRAIL: Fixed = Fixed::new(75, 2);

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

/// `OrbitPower`'s `EnergyVar(4)`: the energy spent for each payout.
const ORBIT_ENERGY: Number = 4;

/// `TankPower.ModifyDamageMultiplicative`'s `2m`.
const TANKED: Fixed = Fixed::new(2, 0);

/// `GuardedPower`'s `DamageDecrease`, 0.5.
const GUARDED: Fixed = Fixed::new(5, 1);

/// `GigantificationPower.ModifyDamageMultiplicative`'s `3m`.
const GIGANTIFIED: Fixed = Fixed::new(3, 0);

/// What one card played adds to what `SlowPower`'s owner takes, in tenths.
const SLOW_TENTHS: i32 = 1;

/// Whether a power that makes a card free takes `card`. The three `Free`
/// powers take an Attack, a Skill or a Power, and Veilpiercer an Ethereal card.
fn frees<V: crate::Build>(power: V::Power, card: V::Card, ethereal: bool) -> bool {
    use sts2_core::CardKind;

    let kind = V::card_kind(card);
    match power {
        __c0 if __c0 == V::Power::FREE_ATTACK_POWER => kind == Some(CardKind::Attack),
        __c0 if __c0 == V::Power::FREE_SKILL_POWER => kind == Some(CardKind::Skill),
        __c0 if __c0 == V::Power::FREE_POWER_POWER => kind == Some(CardKind::Power),
        __c0 if __c0 == V::Power::VEILPIERCER_POWER => ethereal,
        power => unreachable!("{power:?} makes no card free"),
    }
}

/// What Vulnerable on `holder` multiplies a powered hit from `dealer` by.
/// `pet_owners` says whether a dealer without Cruelty reads its pet
/// owner's.
///
/// # Panics
///
/// An endless Cruelty.
pub(crate) fn vulnerable<V: crate::Build>(
    fight: &dyn sts2_core::FightView<V>,
    holder: sts2_core::CombatId,
    dealer: Option<sts2_core::CombatId>,
    pet_owners: bool,
) -> Fixed {
    let mut by = hundredths(V::Power::VULNERABLE_POWER_DAMAGE_INCREASE);
    // `if (dealer != null)`, and both answer the dealer hitting itself with
    // nothing.
    if let Some(dealer) = dealer
        && dealer != holder
    {
        if fight.relic(dealer, V::Relic::PAPER_PHROG) {
            by += PHROG;
        }
        let cruelty = fight.power(dealer, V::Power::CRUELTY_POWER).or_else(|| {
            let owner = fight.pet_owner(dealer).filter(|_| pet_owners)?;
            fight.power(owner, V::Power::CRUELTY_POWER)
        });
        match cruelty {
            // `Amount / 100m`, which is its stack in hundredths.
            Some(PowerAmount::Counted(amount)) => by += amount,
            Some(PowerAmount::Endless) => unimplemented!("an endless Cruelty"),
            None => {}
        }
    }
    // `amount + (amount - 1m)`.
    if fight.power(holder, V::Power::DEBILITATE_POWER).is_some() {
        by += by - 100;
    }
    Fixed::new(by, 2)
}

/// What Weak on `holder` multiplies a powered hit it deals `target` by.
fn weak<V: crate::Build>(
    fight: &dyn sts2_core::FightView<V>,
    holder: sts2_core::CombatId,
    target: Option<sts2_core::CombatId>,
) -> Fixed {
    let mut by = hundredths(V::Power::WEAK_POWER_DAMAGE_DECREASE);
    if target.is_some_and(|target| fight.relic(target, V::Relic::PAPER_KRANE)) {
        by -= KRANE;
    }
    // `amount - (1m - amount)`.
    if fight.power(holder, V::Power::DEBILITATE_POWER).is_some() {
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
fn temporary<V: crate::Build>(
    lent: V::Power,
    positive: bool,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    state: &mut State<V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use sts2_core::{EventHook, HookEffect};

    let State::Temporary { power, ignore_next } = state else {
        unreachable!("a power that lends is built as `State::Temporary`")
    };
    let power = *power;
    // `Sign`, which every amount this power moves is multiplied by.
    let signed = |amount: Number| match positive {
        true => amount,
        false => -amount,
    };
    match (amount, hook) {
        // Each skips once where Misery's copy asked it to.
        (_, EventHook::BeforeApplied { .. }) if *ignore_next => *ignore_next = false,
        (_, EventHook::BeforeApplied { landing }) => {
            let PowerAmount::Counted(landing) = landing else {
                unimplemented!("an endless temporary {lent:?} landing")
            };
            asked.push(HookEffect::ApplyToOwner(
                lent,
                PowerAmount::Counted(signed(landing)),
            ))
        }
        (PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if participants.contains(held.holder) =>
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
        ) if creature == held.holder && changed == power && delta != total => match *ignore_next {
            true => *ignore_next = false,
            false => asked.push(HookEffect::ApplyToOwner(
                lent,
                PowerAmount::Counted(signed(delta)),
            )),
        },
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
pub(crate) fn modify_hook<V: crate::Build>(
    power: V::Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    state: &State<V>,
    hook: sts2_core::ModifyHook<V>,
) -> Modify<Number> {
    let Held { history, fight, .. } = held;
    let one: Number = num_traits::one();
    debug_assert_eq!(state.power(), power);
    let counted = state.counted();
    let holder = held.holder;
    let owner = held.owner;
    // A power whose rule is the build's own, for a hook it answers itself.
    if V::own_power(power)
        && let Some(modify) = V::own_power_modify_hook(power, amount, held, state, hook)
    {
        return modify;
    }
    match (power, amount, hook) {
        // `card.Owner.Creature != base.Owner`, or `.Player`: each of these
        // leaves another player's card as it costs.
        (__c0, _, ModifyHook::EnergyCost { player, .. })
            if player != holder
                && (__c0 == V::Power::BORROWED_TIME_POWER
                    || __c0 == V::Power::CURIOUS_POWER
                    || __c0 == V::Power::TANGLED_POWER) =>
        {
            Modify::Nothing
        }
        (__c0, _, ModifyHook::EnergyCostLate { player, .. })
            if player != holder
                && (__c0 == V::Power::CORRUPTION_POWER || __c0 == V::Power::VOID_FORM_POWER) =>
        {
            Modify::Nothing
        }
        (__c0, _, ModifyHook::StarCost { player, .. })
            if player != holder && __c0 == V::Power::VOID_FORM_POWER =>
        {
            Modify::Nothing
        }
        // `SlowPower.ModifyDamageMultiplicative`, which reads the cards played
        // since its owner's turn began and takes a tenth more for each.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::SLOW_POWER && (target == Some(holder)) => {
            Modify::Scaled(Fixed::new(10 + SLOW_TENTHS * counted, 1))
        }
        // `VulnerablePower.ModifyDamageMultiplicative`: its `DamageIncrease`,
        // raised by the dealer's Paper Phrog and Cruelty, then by the
        // holder's own Debilitate. A pet's hit reads the pet's Cruelty alone.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::VULNERABLE_POWER && (target == Some(holder)) => {
            Modify::Scaled(vulnerable::<V>(fight, holder, dealer, false))
        }
        // `WeakPower.ModifyDamageMultiplicative`: its `DamageDecrease`,
        // lowered by the target's Paper Krane, then by the holder's own
        // Debilitate.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::WEAK_POWER && (dealer == Some(holder)) => {
            Modify::Scaled(weak::<V>(fight, holder, target))
        }
        // `KnockdownPower.ModifyDamageMultiplicative`: Flanking's rule. Its
        // stack times a powered attack's damage to its owner, from anyone but
        // whoever applied it.
        (
            __c0,
            PowerAmount::Counted(stack),
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::KNOCKDOWN_POWER
            && (target == Some(holder) && dealer != held.applier) =>
        {
            Modify::Scaled(Fixed::new(stack, 0))
        }
        (__c0, PowerAmount::Endless, ModifyHook::DamageMultiplicative { .. })
            if __c0 == V::Power::KNOCKDOWN_POWER =>
        {
            unimplemented!("an endless Knockdown")
        }
        // `CoveredPower.ModifyDamageMultiplicative`: no damage from a powered
        // attack to its owner.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::COVERED_POWER && (target == Some(holder)) => {
            Modify::Scaled(Fixed::new(0, 0))
        }
        // `InterceptPower.ModifyDamageMultiplicative`: a powered attack's
        // damage to its owner, times one more than those it covers.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::INTERCEPT_POWER && (target == Some(holder)) => {
            let State::Intercept { covered } = state else {
                unreachable!("Intercept's state is its own")
            };
            let covering = covered.iter().flatten().count();
            Modify::Scaled(Fixed::new(
                Number::try_from(covering + 1).expect("a lobby's players fit a game number"),
                0,
            ))
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::INTERCEPT_POWER => {
            Modify::Nothing
        }
        // `TagTeamPower.ModifyCardPlayCount`: its stack more plays of an
        // Attack another player than its applier plays, aimed at its owner
        // or at every enemy.
        (
            __c0,
            PowerAmount::Counted(stack),
            ModifyHook::CardPlayCount {
                player,
                card,
                target,
            },
        ) if __c0 == V::Power::TAG_TEAM_POWER
            && (<V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack)
                && Some(player) != held.applier) =>
        {
            use crate::generated::cards::TargetType;
            use sts2_core::CardTable as _;

            let aims = card.target().unwrap_or_else(|| {
                crate::behaviour::cards::computed_target_by::<V>(card, |power| {
                    fight.power(player, power).is_some()
                })
            });
            match aims {
                TargetType::AnyEnemy if target == Some(holder) => Modify::Added(stack),
                TargetType::AllEnemies => Modify::Added(stack),
                _ => Modify::Nothing,
            }
        }
        (__c0, _, ModifyHook::CardPlayCount { .. }) if __c0 == V::Power::TAG_TEAM_POWER => {
            Modify::Nothing
        }
        // `FlankingPower.ModifyDamageMultiplicative`: its stack times a
        // powered attack's damage to its owner, from anyone but whoever
        // applied it. With one player that is a pet.
        (
            __c0,
            PowerAmount::Counted(stack),
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::FLANKING_POWER
            && (target == Some(holder) && dealer != held.applier) =>
        {
            Modify::Scaled(Fixed::new(stack, 0))
        }
        (__c0, PowerAmount::Endless, ModifyHook::DamageMultiplicative { .. })
            if __c0 == V::Power::FLANKING_POWER =>
        {
            unimplemented!("an endless Flanking")
        }
        // `TankPower.ModifyDamageMultiplicative`: double a powered attack's
        // damage to its owner.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::TANK_POWER && (target == Some(holder)) => Modify::Scaled(TANKED),
        // `GuardedPower.ModifyDamageMultiplicative`: half a powered attack's
        // damage to its owner.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::GUARDED_POWER && (target == Some(holder)) => Modify::Scaled(GUARDED),
        // `ShrinkPower`'s `DamageDecrease` is the percentage taken off, so a hit
        // keeps the share left.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::SHRINK_POWER && (dealer == Some(holder)) => {
            Modify::Scaled(Fixed::new(100 - V::Power::SHRINK_POWER_DAMAGE_DECREASE, 2))
        }
        // `TryModifyEnergyCostInCombatLate`: nothing for a card of its kind
        // in its owner's hand or play pile. A card in another pile keeps its
        // cost, and so does another player's card.
        (
            __c0,
            PowerAmount::Counted(_),
            ModifyHook::EnergyCostLate {
                card: (_, card),
                player,
                ethereal,
                pile: Some(sts2_core::Pile::Hand) | None,
                ..
            },
        ) if (__c0 == V::Power::FREE_ATTACK_POWER
            || __c0 == V::Power::FREE_SKILL_POWER
            || __c0 == V::Power::FREE_POWER_POWER
            || __c0 == V::Power::VEILPIERCER_POWER)
            && (player == holder && frees::<V>(power, card, ethereal)) =>
        {
            Modify::AtMost(num_traits::zero())
        }
        (__c0, PowerAmount::Endless, ModifyHook::EnergyCostLate { .. })
            if (__c0 == V::Power::FREE_ATTACK_POWER
                || __c0 == V::Power::FREE_SKILL_POWER
                || __c0 == V::Power::FREE_POWER_POWER
                || __c0 == V::Power::VEILPIERCER_POWER) =>
        {
            unimplemented!("an endless {power:?}")
        }
        (__c0, _, ModifyHook::EnergyCostLate { .. })
            if (__c0 == V::Power::FREE_ATTACK_POWER
                || __c0 == V::Power::FREE_SKILL_POWER
                || __c0 == V::Power::FREE_POWER_POWER
                || __c0 == V::Power::VEILPIERCER_POWER) =>
        {
            Modify::Nothing
        }
        // `HangPower.ModifyDamageMultiplicative`: its stack as a multiplier on
        // a hit on its owner from a Hang.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageMultiplicative {
                target: Some(target),
                card: Some(sts2_core::CardSource { card: __c1, .. }),
                ..
            },
        ) if (__c0 == V::Power::HANG_POWER && __c1 == V::Card::HANG) && (target == holder) => {
            Modify::Scaled(Fixed::new(amount, 0))
        }
        (__c0, PowerAmount::Endless, ModifyHook::DamageMultiplicative { .. })
            if __c0 == V::Power::HANG_POWER =>
        {
            unimplemented!("an endless Hang")
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::HANG_POWER => {
            Modify::Nothing
        }
        // `DoubleDamagePower.ModifyDamageMultiplicative`: double a powered
        // hit from a card, by its owner or one of its pets.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                dealer: Some(dealer),
                card: Some(_),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::DOUBLE_DAMAGE_POWER
            && (dealer == holder || fight.pet_owner(dealer) == Some(holder)) =>
        {
            Modify::Scaled(Fixed::new(2, 0))
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. })
            if __c0 == V::Power::DOUBLE_DAMAGE_POWER =>
        {
            Modify::Nothing
        }
        // `TrackingPower.ModifyDamageMultiplicative`: its stack as a
        // multiplier on a powered hit from a card, by its owner or one of its
        // pets, on a target carrying Weak.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageMultiplicative {
                target: Some(target),
                dealer: Some(dealer),
                card: Some(_),
                powered: true,
            },
        ) if __c0 == V::Power::TRACKING_POWER
            && ((dealer == holder || fight.pet_owner(dealer) == Some(holder))
                && fight.power(target, V::Power::WEAK_POWER).is_some()) =>
        {
            Modify::Scaled(Fixed::new(amount, 0))
        }
        (__c0, PowerAmount::Endless, ModifyHook::DamageMultiplicative { .. })
            if __c0 == V::Power::TRACKING_POWER =>
        {
            unimplemented!("an endless Tracking")
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::TRACKING_POWER => {
            Modify::Nothing
        }
        // `SoarPower.ModifyDamageMultiplicative`: its `DamageDecrease`, half, of
        // a powered hit on its holder.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::SOAR_POWER && (target == Some(holder)) => {
            Modify::Scaled(Fixed::new(5, 1))
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::SOAR_POWER => {
            Modify::Nothing
        }
        // `CorruptionPower.TryModifyEnergyCostInCombatLate`: nothing for a
        // Skill its owner holds or is playing.
        (
            __c0,
            _,
            ModifyHook::EnergyCostLate {
                card: (_, card), ..
            },
        ) if __c0 == V::Power::CORRUPTION_POWER
            && (owner == CombatSide::Player
                && <V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Skill)) =>
        {
            Modify::AtMost(num_traits::zero())
        }
        (__c0, _, ModifyHook::EnergyCostLate { .. }) if __c0 == V::Power::CORRUPTION_POWER => {
            Modify::Nothing
        }
        // `BorrowedTimePower.TryModifyEnergyCostInCombat`: its stack onto
        // every card its owner holds.
        (__c0, PowerAmount::Counted(amount), ModifyHook::EnergyCost { .. })
            if __c0 == V::Power::BORROWED_TIME_POWER && (owner == CombatSide::Player) =>
        {
            Modify::Added(amount)
        }
        (__c0, PowerAmount::Endless, ModifyHook::EnergyCost { .. })
            if __c0 == V::Power::BORROWED_TIME_POWER =>
        {
            unimplemented!("an endless Borrowed Time")
        }
        (__c0, _, ModifyHook::EnergyCost { .. }) if __c0 == V::Power::BORROWED_TIME_POWER => {
            Modify::Nothing
        }
        // `CuriousPower.TryModifyEnergyCostInCombat`: its stack off a Power
        // its owner holds, and no lower than nothing in its own step. A
        // listener after it, such as Spiked Gauntlets, adds to the floored
        // cost. It leaves a cost of nothing alone, which the floor does too:
        // no listener before it lowers a cost.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::EnergyCost {
                card: (_, card), ..
            },
        ) if __c0 == V::Power::CURIOUS_POWER
            && (owner == CombatSide::Player
                && <V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Power)) =>
        {
            Modify::Less(amount)
        }
        (__c0, PowerAmount::Endless, ModifyHook::EnergyCost { .. })
            if __c0 == V::Power::CURIOUS_POWER =>
        {
            unimplemented!("an endless Curious")
        }
        (__c0, _, ModifyHook::EnergyCost { .. }) if __c0 == V::Power::CURIOUS_POWER => {
            Modify::Nothing
        }
        // `TangledPower.TryModifyEnergyCostInCombat`: its stack onto a card
        // of its owner's with Entangled.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::EnergyCost {
                affliction: Some(__c1),
                ..
            },
        ) if (__c0 == V::Power::TANGLED_POWER && __c1 == V::Affliction::ENTANGLED) => {
            Modify::Added(amount)
        }
        (
            __c0,
            PowerAmount::Endless,
            ModifyHook::EnergyCost {
                affliction: Some(__c1),
                ..
            },
        ) if (__c0 == V::Power::TANGLED_POWER && __c1 == V::Affliction::ENTANGLED) => {
            unimplemented!("an endless Tangled")
        }
        (__c0, _, ModifyHook::EnergyCost { .. }) if __c0 == V::Power::TANGLED_POWER => {
            Modify::Nothing
        }
        // `IntangiblePower`: a hit on its owner capped at one, and the hit
        // points it loses capped the same, whatever the hit's props.
        (__c0, _, ModifyHook::DamageCap { target, .. })
            if __c0 == V::Power::INTANGIBLE_POWER && (target == Some(holder)) =>
        {
            Modify::AtMost(one)
        }
        (__c0, _, ModifyHook::HpLostAfterOsty { target, .. })
            if __c0 == V::Power::INTANGIBLE_POWER && (target == holder) =>
        {
            Modify::AtMost(one)
        }
        (__c0, _, ModifyHook::DamageCap { .. } | ModifyHook::HpLostAfterOsty { .. })
            if __c0 == V::Power::INTANGIBLE_POWER =>
        {
            Modify::Nothing
        }
        // `FlutterPower.ModifyDamageMultiplicative`: its `DamageDecrease` off
        // a powered attack on its owner.
        // `FlutterPower`'s `DamageDecrease` is the percentage taken off, so a hit
        // keeps the share left.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::FLUTTER_POWER && (target == Some(holder)) => {
            Modify::Scaled(Fixed::new(100 - V::Power::FLUTTER_POWER_DAMAGE_DECREASE, 2))
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::FLUTTER_POWER => {
            Modify::Nothing
        }
        // `SurroundedPower.ModifyDamageMultiplicative`: half again on a hit
        // on its owner from behind, which is from a dealer with Back Attack
        // Left while it faces right, and the other way round.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer: Some(dealer),
                ..
            },
        ) if __c0 == V::Power::SURROUNDED_POWER && (target == Some(holder)) => {
            let behind = match state.facing_left() {
                false => V::Power::BACK_ATTACK_LEFT_POWER,
                true => V::Power::BACK_ATTACK_RIGHT_POWER,
            };
            match fight.power(dealer, behind) {
                Some(_) => Modify::Scaled(SURROUNDED),
                None => Modify::Nothing,
            }
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. })
            if __c0 == V::Power::SURROUNDED_POWER =>
        {
            Modify::Nothing
        }
        // `TaintedPower.ModifyDamageAdditive`: its stack onto a powered attack
        // on its owner.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::TAINTED_POWER && (target == Some(holder)) => Modify::Added(amount),
        (
            __c0,
            PowerAmount::Endless,
            ModifyHook::DamageAdditive {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::TAINTED_POWER && (target == Some(holder)) => {
            unimplemented!("an endless Tainted")
        }
        (__c0, _, ModifyHook::DamageAdditive { .. }) if __c0 == V::Power::TAINTED_POWER => {
            Modify::Nothing
        }
        // `HardToKillPower.ModifyDamageCap`: a hit on its owner capped at its
        // stack, whatever the hit's props.
        (__c0, PowerAmount::Counted(amount), ModifyHook::DamageCap { target, .. })
            if __c0 == V::Power::HARD_TO_KILL_POWER && (target == Some(holder)) =>
        {
            Modify::AtMost(amount)
        }
        (__c0, PowerAmount::Endless, ModifyHook::DamageCap { target, .. })
            if __c0 == V::Power::HARD_TO_KILL_POWER && (target == Some(holder)) =>
        {
            unimplemented!("an endless Hard To Kill")
        }
        (__c0, _, ModifyHook::DamageCap { .. }) if __c0 == V::Power::HARD_TO_KILL_POWER => {
            Modify::Nothing
        }
        // `CalcifyPower.ModifyDamageAdditive`: its stack onto a powered hit
        // from Osty, where its holder is the player Osty stands beside.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::CALCIFY_POWER
            && (dealer.and_then(|dealer| fight.monster(dealer)) == Some(V::Monster::OSTY)
                && dealer.and_then(|dealer| fight.pet_owner(dealer)) == Some(holder)) =>
        {
            Modify::Added(amount)
        }
        (__c0, PowerAmount::Endless, ModifyHook::DamageAdditive { dealer, .. })
            if __c0 == V::Power::CALCIFY_POWER
                && (dealer.and_then(|dealer| fight.pet_owner(dealer)) == Some(holder)) =>
        {
            unimplemented!("an endless Calcify")
        }
        (__c0, _, ModifyHook::DamageAdditive { .. }) if __c0 == V::Power::CALCIFY_POWER => {
            Modify::Nothing
        }
        // `ModifyMaxEnergy`: its stack onto its owner's energy, or off it for
        // Waste Away.
        (__c0, PowerAmount::Counted(amount), ModifyHook::MaxEnergy { player, .. })
            if (__c0 == V::Power::DEMESNE_POWER
                || __c0 == V::Power::PYRE_POWER
                || __c0 == V::Power::FRIENDSHIP_POWER)
                && (player == holder) =>
        {
            Modify::Added(amount)
        }
        (__c0, PowerAmount::Counted(amount), ModifyHook::MaxEnergy { player, .. })
            if __c0 == V::Power::WASTE_AWAY_POWER && (player == holder) =>
        {
            Modify::Added(-amount)
        }
        (__c0, PowerAmount::Endless, ModifyHook::MaxEnergy { player, .. })
            if (__c0 == V::Power::DEMESNE_POWER
                || __c0 == V::Power::PYRE_POWER
                || __c0 == V::Power::FRIENDSHIP_POWER
                || __c0 == V::Power::WASTE_AWAY_POWER)
                && (player == holder) =>
        {
            unimplemented!("an endless {power:?}")
        }
        // `VoidFormPower.TryModifyEnergyCostInCombatLate` and
        // `TryModifyStarCost`: nothing for its owner's cards in hand or in
        // play while fewer than its stack have been played this turn. A Sly
        // card auto-played from the discard pile pays its cost in full.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::EnergyCostLate { pile, .. } | ModifyHook::StarCost { pile, .. },
        ) if __c0 == V::Power::VOID_FORM_POWER
            && (owner == CombatSide::Player)
            && matches!(
                pile,
                None | Some(sts2_core::Pile::Hand | sts2_core::Pile::Play)
            ) =>
        {
            let State::VoidForm { played } = *state else {
                unreachable!("Void Form counts its plays")
            };
            match i64::from(played) < i64::from(amount) {
                true => Modify::Replaced(0),
                false => Modify::Nothing,
            }
        }
        (__c0, PowerAmount::Endless, ModifyHook::EnergyCostLate { .. })
            if __c0 == V::Power::VOID_FORM_POWER =>
        {
            unimplemented!("an endless Void Form")
        }
        (__c0, PowerAmount::Endless, ModifyHook::StarCost { .. })
            if __c0 == V::Power::VOID_FORM_POWER =>
        {
            unimplemented!("an endless Void Form")
        }
        (__c0, _, ModifyHook::EnergyCostLate { .. } | ModifyHook::StarCost { .. })
            if __c0 == V::Power::VOID_FORM_POWER =>
        {
            Modify::Nothing
        }
        // `NoEnergyGainPower.ModifyEnergyGain`: nothing gained, by its owner.
        (__c0, _, ModifyHook::EnergyGain { player })
            if __c0 == V::Power::NO_ENERGY_GAIN_POWER && (player == holder) =>
        {
            Modify::Replaced(0)
        }
        (__c0, _, ModifyHook::EnergyGain { .. }) if __c0 == V::Power::NO_ENERGY_GAIN_POWER => {
            Modify::Nothing
        }
        (__c0, _, ModifyHook::MaxEnergy { .. })
            if (__c0 == V::Power::DEMESNE_POWER
                || __c0 == V::Power::PYRE_POWER
                || __c0 == V::Power::FRIENDSHIP_POWER
                || __c0 == V::Power::WASTE_AWAY_POWER) =>
        {
            Modify::Nothing
        }
        // `LethalityPower.ModifyDamageMultiplicative`: its stack in hundredths
        // onto a powered hit from its owner's first attack of the turn. A
        // card being played has started its own play, and a replay starts
        // another, so the first attack's first play is the one that counts
        // one.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageMultiplicative {
                card: Some(source),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::LETHALITY_POWER && source.owner == holder => {
            use sts2_core::CardKind;

            let attacks = history
                .started_this_turn()
                .filter(|started| V::card_kind(started.card) == Some(CardKind::Attack))
                .count();
            match attacks > usize::from(source.playing) {
                true => Modify::Nothing,
                false => Modify::Scaled(Fixed::new(100 + amount, 2)),
            }
        }
        (__c0, PowerAmount::Endless, ModifyHook::DamageMultiplicative { card: Some(_), .. })
            if __c0 == V::Power::LETHALITY_POWER =>
        {
            unimplemented!("an endless Lethality")
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::LETHALITY_POWER => {
            Modify::Nothing
        }
        // `ColossusPower.ModifyDamageMultiplicative`: half of a powered hit on
        // its owner, from a dealer that is vulnerable.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::COLOSSUS_POWER
            && (target == Some(holder)
                && dealer.is_some_and(|dealer| {
                    fight.power(dealer, V::Power::VULNERABLE_POWER).is_some()
                })) =>
        {
            Modify::Scaled(V::Power::COLOSSUS_POWER_DAMAGE_DECREASE)
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::COLOSSUS_POWER => {
            Modify::Nothing
        }
        // `ConquerorPower.ModifyDamageMultiplicative`: double a powered hit on
        // its owner from a Sovereign Blade.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                target,
                card: Some(sts2_core::CardSource { card: __c1, .. }),
                powered: true,
                ..
            },
        ) if (__c0 == V::Power::CONQUEROR_POWER && __c1 == V::Card::SOVEREIGN_BLADE)
            && (target == Some(holder)) =>
        {
            Modify::Scaled(Fixed::new(2, 0))
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. }) if __c0 == V::Power::CONQUEROR_POWER => {
            Modify::Nothing
        }
        // `GigantificationPower.ModifyDamageMultiplicative`: a powered hit
        // from a card its owner played, before it binds or from the card it
        // bound to. It reads the card's owner and not the dealer, so Osty's
        // hit from a card counts.
        (
            __c0,
            _,
            ModifyHook::DamageMultiplicative {
                card: Some(sts2_core::CardSource { id, owner, .. }),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::GIGANTIFICATION_POWER
            && owner == holder
            && (state.giant().is_none_or(|(_, source)| source == id)) =>
        {
            Modify::Scaled(GIGANTIFIED)
        }
        (__c0, _, ModifyHook::DamageMultiplicative { .. })
            if __c0 == V::Power::GIGANTIFICATION_POWER =>
        {
            Modify::Nothing
        }
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::STRENGTH_POWER && (dealer == Some(holder)) => Modify::Added(amount),
        (
            __c0,
            PowerAmount::Endless,
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::STRENGTH_POWER && (dealer == Some(holder)) => {
            unimplemented!("an endless Strength")
        }
        // `VigorPower.ModifyDamageAdditive`. Once bound, it pays the card
        // that started the command and no other card. A hit from no card
        // is paid either way. The command it bound was its own owner's, so
        // the attacker it names is this dealer.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                card,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::VIGOR_POWER && (dealer == Some(holder)) => {
            let other =
                |bound: Bound<V>| card.is_some_and(|source| Some(source.id) != bound.source);
            match state.bound().is_some_and(other) {
                true => Modify::Nothing,
                false => Modify::Added(amount),
            }
        }
        // `AccuracyPower.ModifyDamageAdditive`, for a Shiv its owner plays.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                card: Some(sts2_core::CardSource { card, .. }),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::ACCURACY_POWER
            && (dealer == Some(holder) && card.tagged(CardTag::Shiv)) =>
        {
            Modify::Added(amount)
        }
        // `PhantomBladesPower.ModifyDamageAdditive`: the first Shiv its owner
        // plays in a turn, and no other. A play counts once it has finished,
        // so the Shiv being played is not one of them. A fight here has one
        // player, so every play is its owner's.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::DamageAdditive {
                dealer,
                card: Some(sts2_core::CardSource { card, .. }),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::PHANTOM_BLADES_POWER
            && (dealer == Some(holder)
                && card.tagged(CardTag::Shiv)
                && !history
                    .finished_this_turn()
                    .any(|played| played.tagged(CardTag::Shiv))) =>
        {
            Modify::Added(amount)
        }
        (
            __c0,
            PowerAmount::Endless,
            ModifyHook::DamageAdditive {
                dealer,
                powered: true,
                ..
            },
        ) if (__c0 == V::Power::VIGOR_POWER
            || __c0 == V::Power::ACCURACY_POWER
            || __c0 == V::Power::PHANTOM_BLADES_POWER)
            && (dealer == Some(holder)) =>
        {
            unimplemented!("an endless {power:?}")
        }
        // `OneTwoPunchPower.ModifyCardPlayCount`: one more play of an attack
        // its owner plays.
        (__c0, _, ModifyHook::CardPlayCount { player, card, .. })
            if __c0 == V::Power::ONE_TWO_PUNCH_POWER
                && (player == holder
                    && <V as sts2_core::Rules>::card_kind(card)
                        == Some(sts2_core::CardKind::Attack)) =>
        {
            Modify::Added(one)
        }
        (__c0, _, ModifyHook::CardPlayCount { .. }) if __c0 == V::Power::ONE_TWO_PUNCH_POWER => {
            Modify::Nothing
        }
        // `DuplicationPower.ModifyCardPlayCount`: one more play of any card
        // its owner plays.
        (__c0, _, ModifyHook::CardPlayCount { player, .. })
            if __c0 == V::Power::DUPLICATION_POWER && (player == holder) =>
        {
            Modify::Added(one)
        }
        (__c0, _, ModifyHook::CardPlayCount { .. }) if __c0 == V::Power::DUPLICATION_POWER => {
            Modify::Nothing
        }
        // `BurstPower.ModifyCardPlayCount`: one more play of a Skill its
        // owner plays.
        (__c0, _, ModifyHook::CardPlayCount { player, card, .. })
            if __c0 == V::Power::BURST_POWER
                && (player == holder
                    && <V as sts2_core::Rules>::card_kind(card)
                        == Some(sts2_core::CardKind::Skill)) =>
        {
            Modify::Added(one)
        }
        (__c0, _, ModifyHook::CardPlayCount { .. }) if __c0 == V::Power::BURST_POWER => {
            Modify::Nothing
        }
        // `SignalBoostPower.ModifyCardPlayCount`: one more play of a Power
        // card its owner plays.
        (__c0, _, ModifyHook::CardPlayCount { player, card, .. })
            if __c0 == V::Power::SIGNAL_BOOST_POWER
                && (player == holder
                    && <V as sts2_core::Rules>::card_kind(card)
                        == Some(sts2_core::CardKind::Power)) =>
        {
            Modify::Added(one)
        }
        (__c0, _, ModifyHook::CardPlayCount { .. }) if __c0 == V::Power::SIGNAL_BOOST_POWER => {
            Modify::Nothing
        }
        // `EchoFormPower.ModifyCardPlayCount`: one more play of a card its
        // owner plays, while the turn has started fewer first plays of a
        // series than its stack. This play is not started yet. A fight here
        // has one player, whose plays are all its holder's.
        (__c0, PowerAmount::Counted(amount), ModifyHook::CardPlayCount { player, .. })
            if __c0 == V::Power::ECHO_FORM_POWER && (player == holder) =>
        {
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
        (__c0, PowerAmount::Endless, ModifyHook::CardPlayCount { player, .. })
            if __c0 == V::Power::ECHO_FORM_POWER && (player == holder) =>
        {
            unimplemented!("an endless Echo Form")
        }
        (__c0, _, ModifyHook::CardPlayCount { .. }) if __c0 == V::Power::ECHO_FORM_POWER => {
            Modify::Nothing
        }
        // `ShadowmeldPower.ModifyBlockMultiplicative`: two to the power of
        // its stack, on its owner's block.
        (__c0, PowerAmount::Counted(amount), ModifyHook::BlockMultiplicative { target, .. })
            if __c0 == V::Power::SHADOWMELD_POWER && (target == holder) =>
        {
            // `(decimal)Math.Pow(2.0, Amount)`, which is exact for any stack
            // a fight reaches.
            let doubled = u32::try_from(amount)
                .ok()
                .and_then(|amount| 1_i32.checked_shl(amount))
                .unwrap_or_else(|| unimplemented!("Shadowmeld at {amount}"));
            Modify::Scaled(Fixed::new(doubled, 0))
        }
        (__c0, PowerAmount::Endless, ModifyHook::BlockMultiplicative { target, .. })
            if __c0 == V::Power::SHADOWMELD_POWER && (target == holder) =>
        {
            unimplemented!("an endless Shadowmeld")
        }
        (__c0, _, ModifyHook::BlockMultiplicative { .. }) if __c0 == V::Power::SHADOWMELD_POWER => {
            Modify::Nothing
        }
        // `NoBlockPower.ModifyBlockMultiplicative`: none of its owner's
        // powered block from a card.
        (
            __c0,
            _,
            ModifyHook::BlockMultiplicative {
                target,
                powered: true,
                card: Some(_),
                ..
            },
        ) if __c0 == V::Power::NO_BLOCK_POWER && (target == holder) => {
            Modify::Scaled(Fixed::new(0, 0))
        }
        (__c0, _, ModifyHook::BlockMultiplicative { .. }) if __c0 == V::Power::NO_BLOCK_POWER => {
            Modify::Nothing
        }
        // `UnmovablePower.ModifyBlockMultiplicative`: double a card's or a
        // move's block on its owner, while fewer such gains than its stack
        // came this turn from other plays. It reads `IsCardOrMonsterMove`, on
        // the gain and on the turn's entries. A card's block counts only where
        // its owner holds the power, on whichever player gains it.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::BlockMultiplicative {
                target,
                moved: true,
                player,
                play,
                ..
            },
        ) if __c0 == V::Power::UNMOVABLE_POWER
            && fight.monster(target).is_none()
            && player.is_none_or(|player| player == holder) =>
        {
            let gained = history
                .block_gained_this_turn()
                .filter(|entry| entry.moved && entry.play != play)
                .count();
            match gained >= usize::try_from(amount).unwrap_or(0) {
                true => Modify::Nothing,
                false => Modify::Scaled(Fixed::new(2, 0)),
            }
        }
        (
            __c0,
            PowerAmount::Endless,
            ModifyHook::BlockMultiplicative {
                target,
                moved: true,
                player,
                ..
            },
        ) if __c0 == V::Power::UNMOVABLE_POWER
            && fight.monster(target).is_none()
            && player.is_none_or(|player| player == holder) =>
        {
            unimplemented!("an endless Unmovable")
        }
        (__c0, _, ModifyHook::BlockMultiplicative { .. }) if __c0 == V::Power::UNMOVABLE_POWER => {
            Modify::Nothing
        }
        // `BufferPower.ModifyHpLostAfterOstyLate`: its holder loses nothing.
        (__c0, _, ModifyHook::HpLostAfterOstyLate { target, .. })
            if __c0 == V::Power::BUFFER_POWER && (target == holder) =>
        {
            Modify::AtMost(num_traits::zero())
        }
        (__c0, _, ModifyHook::HpLostAfterOstyLate { .. }) if __c0 == V::Power::BUFFER_POWER => {
            Modify::Nothing
        }
        // `HardenedShellPower.ModifyHpLostBeforeOstyLate`: its owner loses no
        // more in a turn than its amount, less what it has lost this turn. A
        // hit that takes nothing stays at nothing.
        (__c0, PowerAmount::Counted(amount), ModifyHook::HpLostBeforeOstyLate { target })
            if __c0 == V::Power::HARDENED_SHELL_POWER && (target == holder) =>
        {
            let State::HardenedShell { received } = state else {
                unreachable!("Hardened Shell holding {state:?}")
            };
            Modify::AtMostUnlessNothing(amount - received)
        }
        (__c0, PowerAmount::Endless, ModifyHook::HpLostBeforeOstyLate { target })
            if __c0 == V::Power::HARDENED_SHELL_POWER && (target == holder) =>
        {
            unimplemented!("an endless Hardened Shell")
        }
        (__c0, _, ModifyHook::HpLostBeforeOstyLate { .. })
            if __c0 == V::Power::HARDENED_SHELL_POWER =>
        {
            Modify::Nothing
        }
        // `FocusPower.ModifyOrbValue`: its stack onto each number its owner's
        // orbs show, and no lower than nothing in its own step. A listener
        // after it, such as Infused Core, adds to the floored number.
        (__c0, PowerAmount::Counted(amount), ModifyHook::OrbValue { player, .. })
            if __c0 == V::Power::FOCUS_POWER && (player == holder) =>
        {
            match amount < 0 {
                true => Modify::Less(-amount),
                false => Modify::Added(amount),
            }
        }
        (__c0, PowerAmount::Endless, ModifyHook::OrbValue { player, .. })
            if __c0 == V::Power::FOCUS_POWER && (player == holder) =>
        {
            unimplemented!("an endless Focus")
        }
        (__c0, _, ModifyHook::OrbValue { .. }) if __c0 == V::Power::FOCUS_POWER => Modify::Nothing,
        // `SlipperyPower.ModifyHpLostAfterOsty`, which hands back one however
        // much its owner was about to lose. A hit that the block took whole
        // loses nothing, and this leaves that where it is.
        (__c0, _, ModifyHook::HpLostAfterOsty { target, .. })
            if __c0 == V::Power::SLIPPERY_POWER && (target == holder) =>
        {
            Modify::AtMost(one)
        }
        // `DexterityPower.ModifyBlockAdditive` pays powered block from a card
        // its owner plays, whoever gains it, or powered block its owner gains
        // from no card.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::BlockAdditive {
                target,
                powered: true,
                player,
                ..
            },
        ) if __c0 == V::Power::DEXTERITY_POWER && player.unwrap_or(target) == holder => {
            Modify::Added(amount)
        }
        (
            __c0,
            PowerAmount::Endless,
            ModifyHook::BlockAdditive {
                target,
                powered: true,
                player,
                ..
            },
        ) if __c0 == V::Power::DEXTERITY_POWER && player.unwrap_or(target) == holder => {
            unimplemented!("an endless Dexterity")
        }
        // `FastenPower.ModifyBlockAdditive`: its stack on powered block its
        // owner gains, from a Defend or from no card at all.
        (
            __c0,
            PowerAmount::Counted(amount),
            ModifyHook::BlockAdditive {
                target,
                powered: true,
                card,
                ..
            },
        ) if __c0 == V::Power::FASTEN_POWER
            && (target == holder && card.is_none_or(|card| card.tagged(CardTag::Defend))) =>
        {
            Modify::Added(amount)
        }
        (__c0, _, ModifyHook::BlockAdditive { .. }) if __c0 == V::Power::FASTEN_POWER => {
            Modify::Nothing
        }
        // `FrailPower.ModifyBlockMultiplicative`: a quarter off the block a
        // card or a move gives its owner.
        (
            __c0,
            _,
            ModifyHook::BlockMultiplicative {
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::FRAIL_POWER && (target == holder) => Modify::Scaled(FRAIL),
        (__c0, _, ModifyHook::BlockMultiplicative { .. }) if __c0 == V::Power::FRAIL_POWER => {
            Modify::Nothing
        }
        // `ArtifactPower.TryModifyPowerAmountReceived`: nothing of a power
        // that is a debuff at the amount landing on its owner.
        (
            __c0,
            _,
            ModifyHook::PowerAmountReceived {
                power: landing,
                target,
                amount,
                ..
            },
        ) if __c0 == V::Power::ARTIFACT_POWER
            && (target == holder && debuff_at::<V>(landing, amount)) =>
        {
            Modify::Replaced(0)
        }
        (__c0, _, ModifyHook::PowerAmountReceived { .. }) if __c0 == V::Power::ARTIFACT_POWER => {
            Modify::Nothing
        }
        // `ModifyHandDraw`: its stack more cards for its owner.
        (__c0, PowerAmount::Counted(amount), ModifyHook::HandDraw { player, .. })
            if (__c0 == V::Power::MACHINE_LEARNING_POWER
                || __c0 == V::Power::TYRANNY_POWER
                || __c0 == V::Power::TOOLS_OF_THE_TRADE_POWER
                || __c0 == V::Power::DEMESNE_POWER)
                && (player == holder) =>
        {
            Modify::Added(amount)
        }
        // `MindRotPower.ModifyHandDraw`: its stack fewer cards for its owner.
        // Its `Math.Max(0m, …)` floor bites only on a draw already below the
        // stack, which no ported listener takes it to.
        (__c0, PowerAmount::Counted(amount), ModifyHook::HandDraw { player, .. })
            if __c0 == V::Power::MIND_ROT_POWER && (player == holder) =>
        {
            Modify::Added(-amount)
        }
        (__c0, PowerAmount::Endless, ModifyHook::HandDraw { player, .. })
            if __c0 == V::Power::MIND_ROT_POWER && (player == holder) =>
        {
            unimplemented!("an endless Mind Rot")
        }
        // `ClarityPower.ModifyHandDraw`: one more card for its owner, whatever
        // its stack.
        (__c0, _, ModifyHook::HandDraw { player, .. })
            if __c0 == V::Power::CLARITY_POWER && (player == holder) =>
        {
            Modify::Added(one)
        }
        // `PaleBlueDotPower.ModifyHandDraw`: its stack more cards for its
        // owner, once it finished enough plays in its last turn. A fight here
        // has one player, whose plays are all its holder's.
        (__c0, PowerAmount::Counted(amount), ModifyHook::HandDraw { player, turn, .. })
            if __c0 == V::Power::PALE_BLUE_DOT_POWER && (player == holder) =>
        {
            match history.finished_last_player_turn(turn).count() >= PALE_BLUE_DOT_PLAYS {
                true => Modify::Added(amount),
                false => Modify::Nothing,
            }
        }
        (__c0, PowerAmount::Endless, ModifyHook::HandDraw { player, .. })
            if __c0 == V::Power::PALE_BLUE_DOT_POWER && (player == holder) =>
        {
            unimplemented!("an endless Pale Blue Dot")
        }
        // `DrawCardsNextTurnPower.ModifyHandDraw`, which waits for the stack
        // to have been there as its owner's turn began.
        (__c0, PowerAmount::Counted(amount), ModifyHook::HandDraw { player, .. })
            if __c0 == V::Power::DRAW_CARDS_NEXT_TURN_POWER
                && (player == holder && held.on_turn_start != PowerAmount::Counted(0)) =>
        {
            Modify::Added(amount)
        }
        (__c0, PowerAmount::Endless, ModifyHook::HandDraw { .. })
            if (__c0 == V::Power::MACHINE_LEARNING_POWER
                || __c0 == V::Power::DRAW_CARDS_NEXT_TURN_POWER
                || __c0 == V::Power::TYRANNY_POWER
                || __c0 == V::Power::TOOLS_OF_THE_TRADE_POWER
                || __c0 == V::Power::DEMESNE_POWER) =>
        {
            unimplemented!("an endless {power:?}")
        }
        // The overrides above, for a number some other creature is party to.
        (__c0, _, ModifyHook::DamageMultiplicative { .. })
            if (__c0 == V::Power::SLOW_POWER
                || __c0 == V::Power::VULNERABLE_POWER
                || __c0 == V::Power::WEAK_POWER
                || __c0 == V::Power::SHRINK_POWER
                || __c0 == V::Power::TANK_POWER
                || __c0 == V::Power::GUARDED_POWER
                || __c0 == V::Power::FLANKING_POWER
                || __c0 == V::Power::KNOCKDOWN_POWER
                || __c0 == V::Power::COVERED_POWER) =>
        {
            Modify::Nothing
        }
        (__c0, _, ModifyHook::DamageAdditive { .. })
            if (__c0 == V::Power::STRENGTH_POWER
                || __c0 == V::Power::VIGOR_POWER
                || __c0 == V::Power::ACCURACY_POWER
                || __c0 == V::Power::PHANTOM_BLADES_POWER) =>
        {
            Modify::Nothing
        }
        (__c0, _, ModifyHook::HpLostAfterOsty { .. }) if __c0 == V::Power::SLIPPERY_POWER => {
            Modify::Nothing
        }
        (__c0, _, ModifyHook::BlockAdditive { .. }) if __c0 == V::Power::DEXTERITY_POWER => {
            Modify::Nothing
        }
        (__c0, _, ModifyHook::HandDraw { .. })
            if (__c0 == V::Power::MACHINE_LEARNING_POWER
                || __c0 == V::Power::DRAW_CARDS_NEXT_TURN_POWER
                || __c0 == V::Power::TYRANNY_POWER
                || __c0 == V::Power::TOOLS_OF_THE_TRADE_POWER
                || __c0 == V::Power::DEMESNE_POWER
                || __c0 == V::Power::PALE_BLUE_DOT_POWER
                || __c0 == V::Power::CLARITY_POWER
                || __c0 == V::Power::MIND_ROT_POWER) =>
        {
            Modify::Nothing
        }
        (power, _, hook) => unimplemented!("what {power:?} does for {hook:?}"),
    }
}

/// Whether a card's type is `Status`.
fn status<V: crate::Build>(card: V::Card) -> bool {
    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Status)
}

/// Whether a card's type is `Attack`.
fn attack<V: crate::Build>(card: V::Card) -> bool {
    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack)
}

/// Answer `ModifyCardPlayResultPileTypeAndPosition` for one power.
pub(crate) fn result_pile<V: crate::Build>(
    power: V::Power,
    amount: PowerAmount<Number>,
    holder: sts2_core::CombatId,
    state: &State<V>,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::ResultPile<V>,
) -> sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)> {
    use sts2_core::HookResponse;

    // A power whose rule is the build's own, for a hook it answers itself.
    if V::own_power(power)
        && let Some(result) = V::own_power_result_pile(power, amount, holder, state, hook)
    {
        return result;
    }
    let unchanged = HookResponse::Decision((hook.pile, hook.position));
    match (power, amount) {
        // `FeralPower`: a zero-cost attack its owner plays goes back to the
        // top of the hand, up to its stack of them each turn.
        (__c0, PowerAmount::Counted(amount)) if __c0 == V::Power::FERAL_POWER => {
            match hook.player == holder
                && attack::<V>(hook.card)
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
        (__c0, PowerAmount::Endless) if __c0 == V::Power::FERAL_POWER => {
            unimplemented!("an endless Feral")
        }
        // `ReboundPower`: a card of its owner's bound for the discard pile
        // goes to the top of the draw pile instead.
        (__c0, _) if __c0 == V::Power::REBOUND_POWER => match hook.player == holder
            && hook.pile == Some(sts2_core::Pile::Discard)
        {
            true => {
                HookResponse::Decision((Some(sts2_core::Pile::Draw), sts2_core::PilePosition::Top))
            }
            false => unchanged,
        },
        // `CorruptionPower`: a Skill its owner plays goes to the exhaust pile,
        // where it was bound.
        (__c0, _) if __c0 == V::Power::CORRUPTION_POWER => {
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
        (__c0, PowerAmount::Counted(amount)) if __c0 == V::Power::NOSTALGIA_POWER => {
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
        (__c0, PowerAmount::Endless) if __c0 == V::Power::NOSTALGIA_POWER => {
            unimplemented!("an endless Nostalgia")
        }
        (power, _) => unimplemented!("where {power:?} sends a played card"),
    }
}

/// Answer a yes-or-no hook for one power.
///
/// A hook the fight raises reaches only a power that overrides it. A question
/// the game asks every power about itself reaches all of them, and one with
/// no override does not implement it.
pub(crate) fn decide_hook<V: crate::Build>(
    power: V::Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    state: &State<V>,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::{DecisionHook, HookResponse};

    // A power whose rule is the build's own, for a hook it answers itself.
    if V::own_power(power)
        && let Some(response) = V::own_power_decide_hook(power, amount, held, state, hook)
    {
        return response;
    }
    match (power, hook) {
        // `card.Owner != base.Owner`, or `.Player`: another player's card
        // plays as it would.
        (__c0, DecisionHook::ShouldPlay { player, .. })
            if player != held.holder
                && (__c0 == V::Power::SLOTH_POWER
                    || __c0 == V::Power::CHAINS_OF_BINDING_POWER
                    || __c0 == V::Power::SMOGGY_POWER
                    || __c0 == V::Power::RINGING_POWER) =>
        {
            HookResponse::Decision(true)
        }
        // v0.111.0's `WellLaidPlansPower.ShouldFlush`: no for its owner, who
        // keeps the hand. v0.107.1's picks cards to retain in
        // `BeforeFlushLate` instead, and overrides no `ShouldFlush`.
        (__c0, DecisionHook::ShouldFlush { player, .. })
            if __c0 == V::Power::WELL_LAID_PLANS_POWER =>
        {
            HookResponse::Decision(player != held.holder)
        }
        // `RingingPower.ShouldPlay`: a card with Ringing waits while its owner
        // has started a play this turn. A fight here has one player, whose
        // cards are its holder's.
        // `SlothPower.ShouldPlay`: no more once its owner has played its
        // stack of cards this turn.
        (__c0, DecisionHook::ShouldPlay { .. }) if __c0 == V::Power::SLOTH_POWER => {
            let (PowerAmount::Counted(amount), State::Sloth { played }) = (amount, state) else {
                unimplemented!("an endless Sloth")
            };
            HookResponse::Decision(*played < amount)
        }
        // `ChainsOfBindingPower.ShouldPlay`: one Bound card a turn.
        (__c0, DecisionHook::ShouldPlay { affliction, .. })
            if __c0 == V::Power::CHAINS_OF_BINDING_POWER =>
        {
            let State::ChainsOfBinding { bound_played } = state else {
                unreachable!("Chains of Binding holding {state:?}")
            };
            HookResponse::Decision(affliction != Some(V::Affliction::BOUND) || !*bound_played)
        }
        // `SmoggyPower.ShouldPlay`: no card with Smog.
        (__c0, DecisionHook::ShouldPlay { affliction, .. }) if __c0 == V::Power::SMOGGY_POWER => {
            HookResponse::Decision(affliction != Some(V::Affliction::SMOG))
        }
        (__c0, DecisionHook::ShouldPlay { affliction, .. }) if __c0 == V::Power::RINGING_POWER => {
            HookResponse::Decision(
                affliction != Some(V::Affliction::RINGING)
                    || held.history.started_this_turn().next().is_none(),
            )
        }
        // `ShouldCreatureBeRemovedFromCombatAfterDeath`, which keeps its own
        // holder and no other.
        (__c0, DecisionHook::ShouldRemoveCreatureAfterDeath { creature })
            if (__c0 == V::Power::ILLUSION_POWER
                || __c0 == V::Power::REATTACH_POWER
                || __c0 == V::Power::DIE_FOR_YOU_POWER
                || __c0 == V::Power::ADAPTABLE_POWER
                || __c0 == V::Power::PAINFUL_STABS_POWER
                || __c0 == V::Power::STEAM_ERUPTION_POWER) =>
        {
            HookResponse::Decision(creature != held.holder)
        }
        // `OwnerIsSecondaryEnemy`.
        (__c0, DecisionHook::ShouldCountForCombatEnd) if __c0 == V::Power::MINION_POWER => {
            HookResponse::Decision(false)
        }
        // `MinionPower.ShouldOwnerDeathTriggerFatal`: a Minion's death is not
        // a kill that pays out.
        (__c0, DecisionHook::ShouldOwnerDeathTriggerFatal) if __c0 == V::Power::MINION_POWER => {
            HookResponse::Decision(false)
        }
        // `ReattachPower.ShouldOwnerDeathTriggerFatal`: `AreAllOtherSegmentsDead`,
        // over its owner's other teammates that hold Reattach.
        (__c0, DecisionHook::ShouldOwnerDeathTriggerFatal) if __c0 == V::Power::REATTACH_POWER => {
            HookResponse::Decision(!other_segment_standing::<V>(held.fight, held.holder))
        }
        // `ShouldPowerBeRemovedAfterOwnerDeath`. A power that keeps its owner
        // in the fight keeps itself, or the creature would come back without
        // whatever brought it back.
        (__c0, DecisionHook::ShouldRemoveSelfAfterOwnerDeath)
            if (__c0 == V::Power::REATTACH_POWER
                || __c0 == V::Power::MINION_POWER
                || __c0 == V::Power::DIE_FOR_YOU_POWER
                || __c0 == V::Power::ADAPTABLE_POWER
                || __c0 == V::Power::PAINFUL_STABS_POWER
                || __c0 == V::Power::STEAM_ERUPTION_POWER) =>
        {
            HookResponse::Decision(false)
        }
        // `IllusionPower.ShouldPowerBeRemovedOnDeath`, which reads the power
        // and not whose it is: a debuff goes unless it is an `ITemporaryPower`,
        // and anything else stays. The temporary powers are the ones that
        // lend a stat.
        (__c0, DecisionHook::ShouldRemovePowerOnDeath { power, .. })
            if __c0 == V::Power::ILLUSION_POWER =>
        {
            use sts2_core::PowerTable as _;

            if lends::<V>(power).is_some() {
                return HookResponse::Decision(false);
            }
            let debuff = power.debuff().unwrap_or_else(|| {
                unimplemented!("whether {power:?} is a debuff on a dying creature")
            });
            HookResponse::Decision(debuff)
        }
        // `ShouldAllowHitting`, which refuses its own holder while it revives.
        (__c0, DecisionHook::ShouldAllowHitting { creature, .. })
            if (__c0 == V::Power::ILLUSION_POWER
                || __c0 == V::Power::REATTACH_POWER
                || __c0 == V::Power::ADAPTABLE_POWER) =>
        {
            HookResponse::Decision(creature != held.holder || !state.reviving())
        }
        // `DieForYouPower.ShouldAllowHitting`, which refuses any creature that
        // is dead, whoever holds it.
        (__c0, DecisionHook::ShouldAllowHitting { alive, .. })
            if __c0 == V::Power::DIE_FOR_YOU_POWER =>
        {
            HookResponse::Decision(alive)
        }
        // `ModifyUnblockedDamageTarget`, which takes a powered attack on the
        // pet's owner onto the pet while it lives.
        (
            __c0,
            DecisionHook::TakesUnblockedDamage {
                target,
                powered,
                pet_of,
                alive,
            },
        ) if __c0 == V::Power::DIE_FOR_YOU_POWER => {
            HookResponse::Decision(pet_of == Some(target) && alive && powered)
        }
        // `NoDrawPower.ShouldDraw`: its owner draws nothing but the turn's own
        // hand.
        (
            __c0,
            DecisionHook::ShouldDraw {
                player, hand_draw, ..
            },
        ) if __c0 == V::Power::NO_DRAW_POWER => {
            HookResponse::Decision(hand_draw || player != held.holder)
        }
        // `RetainHandPower.ShouldFlush`, which keeps its own holder's hand.
        (__c0, DecisionHook::ShouldFlush { player, .. }) if __c0 == V::Power::RETAIN_HAND_POWER => {
            HookResponse::Decision(player != held.holder)
        }
        // `ShouldClearBlock`, which keeps its own holder's. Blur's
        // `AfterPreventingBlockClear` only flashes.
        (__c0, DecisionHook::ShouldClearBlock { creature, .. })
            if (__c0 == V::Power::BURROWED_POWER
                || __c0 == V::Power::BARRICADE_POWER
                || __c0 == V::Power::BLUR_POWER) =>
        {
            HookResponse::Decision(creature != held.holder)
        }
        // `InfestedPower.ShouldStopCombatFromEnding` and `StockPower`'s, for as
        // long as it is on a creature in the fight.
        (__c0, DecisionHook::ShouldStopCombatFromEnding)
            if (__c0 == V::Power::INFESTED_POWER
                || __c0 == V::Power::STOCK_POWER
                || __c0 == V::Power::ADAPTABLE_POWER
                || __c0 == V::Power::STEAM_ERUPTION_POWER
                || __c0 == V::Power::SURPRISE_POWER) =>
        {
            HookResponse::Decision(true)
        }
        (power, hook) if hook.model_hook().is_some() => {
            unimplemented!("what {power:?} decides for {hook:?}")
        }
        _ => HookResponse::DoesNotImplement,
    }
}

/// `SurprisePower.AfterDeath`'s Heists: for each Thievery `holder` carries,
/// one on `fat` of the gold it stole, from `holder`, aimed at the
/// Thievery's target. The gold stolen in all.
pub(crate) fn heists<V: crate::Build, F: sts2_core::Fight<V>>(
    fight: &mut F,
    holder: sts2_core::CombatId,
    fat: sts2_core::CombatId,
) -> Number {
    let mut total = 0;
    for (nth, (target, _)) in fight
        .power_instances(holder, V::Power::THIEVERY_POWER)
        .into_iter()
        .enumerate()
    {
        let mut stolen = 0;
        fight.write_power_at(holder, V::Power::THIEVERY_POWER, nth, |state| {
            stolen = *state.stolen_gold();
        });
        let target = target.unwrap_or_else(|| unreachable!("a Thievery is aimed at a player"));
        fight.apply_aimed(
            fat,
            V::Power::HEIST_POWER,
            PowerAmount::Counted(stolen),
            Some(holder),
            target,
        );
        total += stolen;
    }
    total
}

/// `WitheringPresencePower`'s `CardsLeft` as it is applied.
const WITHERING_CARDS: u8 = 6;

/// Whose a power is, and what of the fight it can read, as a hook reaching
/// it reads that.
#[derive(Clone, Copy)]
pub struct Held<'a, V: crate::Build> {
    /// The side of the creature carrying it.
    pub(crate) owner: CombatSide,
    /// The creature carrying it, which is `PowerModel.Owner`.
    pub(crate) holder: sts2_core::CombatId,
    /// `PowerModel.Applier`.
    pub(crate) applier: Option<sts2_core::CombatId>,
    /// `PowerModel.Target`.
    pub(crate) target: Option<sts2_core::CombatId>,
    /// `Owner.Monster`, which is nothing for a power on the player.
    pub(crate) monster: Option<V::Monster>,
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
pub(crate) fn event_hook<V: crate::Build>(
    power: V::Power,
    amount: PowerAmount<Number>,
    held: Held<'_, V>,
    state: &mut State<V>,
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
        target,
        monster,
        ..
    } = held;

    // A temporary power asks for two things when a turn ends, and the match
    // below spells one. Its subclasses answer there instead.
    if let Some((lent, positive)) = lends::<V>(power) {
        return temporary::<V>(lent, positive, amount, held, state, hook, asked);
    }

    let one: Number = num_traits::one();

    // A power whose rule is the build's own, for a hook it answers itself.
    if V::own_power(power)
        && let Some(effect) = V::own_power_event_hook(power, amount, held, state, hook)
    {
        if let Some(effect) = effect {
            asked.push(effect);
        }
        return;
    }
    let effect = match (power, amount, hook) {
        // `cardPlay.Card.Owner != base.Owner`, or `.Player`: each of these
        // returns at once on another player's card.
        (__c0, _, EventHook::CardPlaying { player, .. })
            if player != holder
                && (__c0 == V::Power::STORM_POWER
                    || __c0 == V::Power::CALAMITY_POWER
                    || __c0 == V::Power::SERPENT_FORM_POWER
                    || __c0 == V::Power::RUPTURE_POWER
                    || __c0 == V::Power::AFTERIMAGE_POWER
                    || __c0 == V::Power::MONOLOGUE_POWER
                    || __c0 == V::Power::SUBROUTINE_POWER
                    || __c0 == V::Power::THE_SEALED_THRONE_POWER
                    || __c0 == V::Power::SPIRIT_OF_ASH_POWER
                    || __c0 == V::Power::DANSE_MACABRE_POWER
                    || __c0 == V::Power::SLOTH_POWER
                    || __c0 == V::Power::CHAINS_OF_BINDING_POWER) =>
        {
            None
        }
        (__c0, _, EventHook::CardPlayed { player, .. })
            if player != holder
                && (__c0 == V::Power::DEVOUR_LIFE_POWER
                    || __c0 == V::Power::MASTER_PLANNER_POWER
                    || __c0 == V::Power::RAGE_POWER
                    || __c0 == V::Power::HAUNT_POWER
                    || __c0 == V::Power::VOID_FORM_POWER
                    || __c0 == V::Power::PANACHE_POWER
                    || __c0 == V::Power::TENDER_POWER) =>
        {
            None
        }
        (
            __c0,
            _,
            EventHook::CardDrawn { player, .. } | EventHook::CardDrawnEarly { player, .. },
        ) if player != holder
            && (__c0 == V::Power::AUTOMATION_POWER
                || __c0 == V::Power::CORROSIVE_WAVE_POWER
                || __c0 == V::Power::CONFUSED_POWER
                || __c0 == V::Power::SPEEDSTER_POWER
                || __c0 == V::Power::ITERATION_POWER
                || __c0 == V::Power::PAGESTORM_POWER
                || __c0 == V::Power::CHAINS_OF_BINDING_POWER
                || __c0 == V::Power::HELLRAISER_POWER) =>
        {
            None
        }
        (__c0, _, EventHook::CardExhausted { player, .. })
            if player != holder
                && (__c0 == V::Power::DARK_EMBRACE_POWER
                    || __c0 == V::Power::FEEL_NO_PAIN_POWER) =>
        {
            None
        }
        (__c0, _, EventHook::CardEnteredCombat { player, .. })
            if player != holder
                && (__c0 == V::Power::RINGING_POWER
                    || __c0 == V::Power::HEX_POWER
                    || __c0 == V::Power::TANGLED_POWER
                    || __c0 == V::Power::SWORD_SAGE_POWER
                    || __c0 == V::Power::PHANTOM_BLADES_POWER) =>
        {
            None
        }
        // Their `AfterModifyingHandDraw` only flashes.
        (__c0, _, EventHook::HandDrawModified)
            if (__c0 == V::Power::PALE_BLUE_DOT_POWER || __c0 == V::Power::MIND_ROT_POWER) =>
        {
            return;
        }
        // `MayhemPower.AfterAutoPrePlayPhaseEntered`: its stack, off the top
        // of the draw pile, auto-played on its owner's turn.
        (__c0, PowerAmount::Counted(amount), EventHook::AutoPrePlayPhase { player, .. })
            if __c0 == V::Power::MAYHEM_POWER && player == holder =>
        {
            Some(HookEffect::AutoPlayFromDraw(amount))
        }
        (__c0, PowerAmount::Counted(_), EventHook::AutoPrePlayPhase { .. })
            if __c0 == V::Power::MAYHEM_POWER =>
        {
            return;
        }
        // Its `AfterModifyingEnergyGain` only flashes.
        (__c0, _, EventHook::EnergyGainModified) if __c0 == V::Power::NO_ENERGY_GAIN_POWER => {
            return;
        }
        // `OrbitPower.AfterEnergySpent`: the energy its owner spends, counted,
        // and its stack as energy for each four of it not yet paid out.
        (__c0, PowerAmount::Counted(stack), EventHook::EnergySpent { player, amount })
            if __c0 == V::Power::ORBIT_POWER && player == holder && (amount > 0) =>
        {
            let State::Orbit { spent, triggers } = state else {
                unreachable!("Orbit's state for {:?}", state.power())
            };
            *spent += amount;
            let due = *spent / ORBIT_ENERGY - *triggers;
            (due > 0).then(|| {
                *triggers += due;
                HookEffect::GainEnergy(stack * due)
            })
        }
        (__c0, PowerAmount::Endless, EventHook::EnergySpent { .. })
            if __c0 == V::Power::ORBIT_POWER =>
        {
            unimplemented!("an endless Orbit")
        }
        (__c0, _, EventHook::EnergySpent { .. }) if __c0 == V::Power::ORBIT_POWER => None,
        // `HammerTimePower.AfterForge`: when its owner forges, from anything
        // but a Hammer Time, as much for each other player still alive.
        (__c0, _, EventHook::Forged { player, source, .. })
            if __c0 == V::Power::HAMMER_TIME_POWER
                && (source == Some(V::Power::HAMMER_TIME_POWER) || player != holder) =>
        {
            None
        }
        (__c0, _, EventHook::Forged { amount, .. }) if __c0 == V::Power::HAMMER_TIME_POWER => {
            Some(HookEffect::ForgeOthers {
                card: V::Card::SOVEREIGN_BLADE,
                amount,
            })
        }
        // `WellLaidPlansPower.BeforeFlushLate`: its owner picks up to its
        // stack of cards to retain this turn.
        (__c0, _, EventHook::FlushingLate { player })
            if __c0 == V::Power::WELL_LAID_PLANS_POWER && player != holder =>
        {
            None
        }
        (__c0, PowerAmount::Counted(count), EventHook::FlushingLate { .. })
            if __c0 == V::Power::WELL_LAID_PLANS_POWER =>
        {
            Some(HookEffect::PickToRetain(count))
        }
        (__c0, PowerAmount::Endless, EventHook::FlushingLate { .. })
            if __c0 == V::Power::WELL_LAID_PLANS_POWER =>
        {
            unimplemented!("an endless Well-Laid Plans")
        }
        // `KnockdownPower.AfterSideTurnEnd`: gone once its owner's side ends a
        // turn. Its `AfterApplied` only writes its applier's name.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::KNOCKDOWN_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. } | EventHook::Applied)
            if __c0 == V::Power::KNOCKDOWN_POWER =>
        {
            None
        }
        // `CoveredPower.AfterSideTurnEnd`: gone once the enemies end a turn.
        (
            __c0,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::COVERED_POWER => Some(HookEffect::Remove),
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::COVERED_POWER => None,
        // `CoveredPower.AfterDeath`: gone with whoever applied it.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::COVERED_POWER && (Some(creature) == applier) => {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::COVERED_POWER => None,
        // `CoveredPower.AfterApplied` gives its applier Intercept, from its
        // owner, unless the applier carries one, and adds its owner to those
        // the Intercept covers.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::COVERED_POWER => {
            let interceptor =
                applier.unwrap_or_else(|| unimplemented!("Covered applied by nobody"));
            Some(HookEffect::Cover {
                interceptor,
                covered: holder,
                power: V::Power::INTERCEPT_POWER,
            })
        }
        // `InterceptPower.AfterSideTurnEnd`: gone once the enemies end a turn.
        (
            __c0,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::INTERCEPT_POWER => Some(HookEffect::Remove),
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::INTERCEPT_POWER => None,
        // `TagTeamPower.AfterModifyingCardPlayCount`: gone once it added plays.
        (__c0, _, EventHook::PlayCountModified { .. }) if __c0 == V::Power::TAG_TEAM_POWER => {
            Some(HookEffect::Remove)
        }
        // `FlankingPower.AfterSideTurnEnd`: gone once its owner's side ends
        // a turn.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::FLANKING_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::FLANKING_POWER => None,
        // `FlankingPower.AfterApplied` writes its applier's name for the text.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::FLANKING_POWER => None,
        // `SneakyPower.AfterCardPlayed`: its stack of block, unpowered, for
        // each Attack another player plays.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { card, player, .. })
            if __c0 == V::Power::SNEAKY_POWER
                && player != holder
                && <V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Attack) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { player, card, .. })
            if __c0 == V::Power::SNEAKY_POWER
                && player != holder
                && <V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Attack) =>
        {
            unimplemented!("an endless Sneaky")
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::SNEAKY_POWER => None,
        // `NoEnergyGainPower.AfterSideTurnEnd`: gone once its owner's turn
        // ends.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::NO_ENERGY_GAIN_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::NO_ENERGY_GAIN_POWER => None,
        (
            __c0,
            _,
            EventHook::DamageGiven {
                dealer,
                fully_blocked: true,
                ..
            },
        ) if __c0 == V::Power::IMBALANCED_POWER && (dealer == Some(holder)) => {
            state.set_off_balance(true);
            asked.push(stunned::<V>(sts2_core::StunResume::Interrupted));
            return;
        }
        (__c0, _, EventHook::MonsterMovePerformed { move_id: __c1 })
            if (__c0 == V::Power::IMBALANCED_POWER && __c1 == V::MoveId::STUNNED) =>
        {
            state.set_off_balance(false);
            None
        }
        (
            __c0,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if (__c0 == V::Power::VULNERABLE_POWER
            || __c0 == V::Power::WEAK_POWER
            || __c0 == V::Power::FRAIL_POWER) =>
        {
            Some(HookEffect::TickDownDuration)
        }
        // `DebilitatePower.AfterSideTurnEnd`: one off its stack as its owner's
        // turn ends. What it does to Vulnerable and Weak, they read.
        (__c0, PowerAmount::Counted(_), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::DEBILITATE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::DEBILITATE_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Debilitate")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::DEBILITATE_POWER => None,
        // `ConsumingShadowPower.AfterSideTurnEnd`: `OrbCmd.EvokeLast` once for
        // each of its stack as its owner's turn ends, while an orb is left.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::CONSUMING_SHADOW_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::EvokeLast(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::CONSUMING_SHADOW_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Consuming Shadow")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::CONSUMING_SHADOW_POWER => {
            None
        }
        // `DevourLifePower.AfterCardPlayed`: its stack as a summon of Osty for
        // each Soul its owner plays.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { card: __c1, .. })
            if (__c0 == V::Power::DEVOUR_LIFE_POWER && __c1 == V::Card::SOUL) =>
        {
            Some(HookEffect::Summon(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { .. })
            if __c0 == V::Power::DEVOUR_LIFE_POWER =>
        {
            unimplemented!("an endless Devour Life")
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::DEVOUR_LIFE_POWER => None,
        // `SpinnerPower.AfterEnergyReset`: a Glass orb channelled for each of
        // its stack as its owner's energy resets.
        (__c0, PowerAmount::Counted(amount), EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::SPINNER_POWER && (player == holder) =>
        {
            for _ in 0..amount {
                asked.push(HookEffect::Channel(V::Orb::GLASS_ORB));
            }
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::EnergyReset { .. })
            if __c0 == V::Power::SPINNER_POWER =>
        {
            unimplemented!("an endless Spinner")
        }
        (__c0, _, EventHook::EnergyReset { .. }) if __c0 == V::Power::SPINNER_POWER => None,
        // `StormPower.BeforeCardPlayed`: a Power its owner plays noted with
        // the stack as it begins.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { card, id, .. })
            if __c0 == V::Power::STORM_POWER =>
        {
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power) {
                note::<V>(state, id, amount);
            }
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::STORM_POWER =>
        {
            unimplemented!("an endless Storm")
        }
        // `StormPower.AfterCardPlayed`: a Lightning orb channelled for each of
        // the stack it noted.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::STORM_POWER => {
            if let Some(lightning) = take_noted::<V>(state, id) {
                for _ in 0..lightning {
                    asked.push(HookEffect::Channel(V::Orb::LIGHTNING_ORB));
                }
            }
            return;
        }
        // `CalamityPower.BeforeCardPlayed`: an Attack its owner plays noted.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { card, id, .. })
            if __c0 == V::Power::CALAMITY_POWER =>
        {
            if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack) {
                note::<V>(state, id, amount);
            }
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::CALAMITY_POWER =>
        {
            unimplemented!("an endless Calamity")
        }
        // `CalamityPower.AfterCardPlayed`: for a play it noted, `GetForCombat`
        // of its stack as it stands now in Attacks of the character's pool.
        (__c0, amount, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::CALAMITY_POWER => {
            take_noted::<V>(state, id).map(|_| match amount {
                PowerAmount::Counted(count) => HookEffect::GenerateForCombat {
                    kind: sts2_core::CardKind::Attack,
                    count,
                },
                PowerAmount::Endless => unimplemented!("an endless Calamity"),
            })
        }
        // `SerpentFormPower.BeforeCardPlayed`: a play of its owner's noted
        // with the stack as it begins.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { id, .. })
            if __c0 == V::Power::SERPENT_FORM_POWER =>
        {
            note::<V>(state, id, amount);
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::SERPENT_FORM_POWER =>
        {
            unimplemented!("an endless Serpent Form")
        }
        // `SerpentFormPower.AfterCardPlayed`: the stack it noted, unpowered
        // and from its owner, on an enemy drawn off `CombatTargets`.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::SERPENT_FORM_POWER => {
            take_noted::<V>(state, id)
                .filter(|&damage| damage > 0)
                .map(|amount| HookEffect::DamageRandomOpponent {
                    amount,
                    unblockable: false,
                    from_owner: true,
                })
        }
        // `RupturePower.BeforeCardPlayed`: a play of its owner's, in its
        // owner's own turn, noted as owing nothing yet.
        (__c0, _, EventHook::CardPlaying { id, side, .. }) if __c0 == V::Power::RUPTURE_POWER => {
            if side == owner {
                note::<V>(state, id, 0);
            }
            None
        }
        // `RupturePower.AfterDamageReceived`: its stack of Strength for health
        // its owner loses in its own turn. A play it noted owes it instead,
        // until the play is over.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                unblocked,
                card,
                side,
                ..
            },
        ) if __c0 == V::Power::RUPTURE_POWER
            && (target == holder && unblocked > 0 && side == owner) =>
        {
            let owed = card.is_some_and(|card| add_noted::<V>(state, card, amount));
            (!owed).then_some(HookEffect::ApplyPower {
                target: holder,
                power: V::Power::STRENGTH_POWER,
                amount: PowerAmount::Counted(amount),
                applier: Some(holder),
            })
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { .. })
            if __c0 == V::Power::RUPTURE_POWER =>
        {
            unimplemented!("an endless Rupture")
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::RUPTURE_POWER => None,
        // `RupturePower.AfterCardPlayed`: the Strength a play it noted owes.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::RUPTURE_POWER => {
            take_noted::<V>(state, id).map(|owed| {
                HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, PowerAmount::Counted(owed))
            })
        }
        // `BlackHolePower.AfterCardPlayed`: its stack, unpowered, to each enemy
        // that can be hit, once a play that spent stars is over and it ends
        // its replays. The play's started entry keeps what it spent, and an
        // auto-play spends none. A fight here has one player, whose cards are
        // all its holder's.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { id, last, .. })
            if __c0 == V::Power::BLACK_HOLE_POWER =>
        {
            let spent = history
                .started_this_turn()
                .filter(|started| started.id == id)
                .last()
                .is_some_and(|started| !started.auto && started.stars > 0);
            (spent && last).then_some(HookEffect::DamageOpponents(amount))
        }
        // `BlackHolePower.AfterStarsGained`: the same, for stars its owner
        // gains.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::StarsGained {
                player,
                amount: gained,
            },
        ) if __c0 == V::Power::BLACK_HOLE_POWER => {
            (gained > 0 && player == holder).then_some(HookEffect::DamageOpponents(amount))
        }
        (
            __c0,
            PowerAmount::Endless,
            EventHook::CardPlayed { .. } | EventHook::StarsGained { .. },
        ) if __c0 == V::Power::BLACK_HOLE_POWER => {
            unimplemented!("an endless Black Hole")
        }
        // `StampedePower.AfterAutoPostPlayPhaseEntered`: its stack of Attacks
        // in hand, each drawn off `Shuffle`, auto-played on its owner's turn.
        (__c0, PowerAmount::Counted(count), EventHook::AutoPostPlayPhase { player, .. })
            if __c0 == V::Power::STAMPEDE_POWER && player == holder =>
        {
            Some(HookEffect::AutoPlayRandomFromHand {
                kind: sts2_core::CardKind::Attack,
                count,
            })
        }
        (__c0, PowerAmount::Counted(_), EventHook::AutoPostPlayPhase { .. })
            if __c0 == V::Power::STAMPEDE_POWER =>
        {
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::AutoPostPlayPhase { .. })
            if __c0 == V::Power::STAMPEDE_POWER =>
        {
            unimplemented!("an endless Stampede")
        }
        // `JuggernautPower.AfterBlockGained`: its stack, unpowered, from its
        // owner to one enemy that can be hit, drawn off `CombatTargets`, for
        // each gain of its owner's that left any block.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::BlockGained {
                creature, positive, ..
            },
        ) if __c0 == V::Power::JUGGERNAUT_POWER && (creature == holder && positive) => {
            Some(HookEffect::DamageRandomOpponent {
                amount,
                unblockable: false,
                from_owner: true,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::BlockGained { .. })
            if __c0 == V::Power::JUGGERNAUT_POWER =>
        {
            unimplemented!("an endless Juggernaut")
        }
        (__c0, _, EventHook::BlockGained { .. }) if __c0 == V::Power::JUGGERNAUT_POWER => None,
        // `BeaconOfHopePower.AfterBlockGained`: half of a gain of one or more
        // its owner made on their own side's turn, unpowered, on each living
        // player among its owner's teammates.
        (
            __c0,
            _,
            EventHook::BlockGained {
                creature,
                amount: gained,
                side,
                ..
            },
        ) if __c0 == V::Power::BEACON_OF_HOPE_POWER => {
            let half = gained / 2;
            let busy = matches!(state, State::BeaconOfHope { busy: true });
            if creature != holder
                || gained < one
                || fight.side_of(holder) != Some(side)
                || busy
                || half < one
            {
                return;
            }
            for teammate in fight.teammates(holder) {
                if teammate == holder
                    || !fight.is_player(teammate)
                    || !fight.current_hp(teammate).is_some_and(|hp| hp > 0)
                {
                    continue;
                }
                asked.push(HookEffect::GainBlockOf {
                    target: teammate,
                    amount: half,
                });
            }
            return;
        }
        // `TankPower.AfterApplied`: Guarded of its amount on each living
        // player among its owner's teammates, from its owner.
        (__c0, amount, EventHook::Applied) if __c0 == V::Power::TANK_POWER => {
            for teammate in fight.teammates(holder) {
                if teammate == holder
                    || !fight.is_player(teammate)
                    || !fight.current_hp(teammate).is_some_and(|hp| hp > 0)
                {
                    continue;
                }
                asked.push(HookEffect::ApplyPower {
                    target: teammate,
                    power: V::Power::GUARDED_POWER,
                    amount,
                    applier: Some(holder),
                });
            }
            return;
        }
        // `GuardedPower.AfterDeath`: gone with whoever applied it.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::GUARDED_POWER && (Some(creature) == applier) => {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::AfterDeath { .. } | EventHook::Applied)
            if __c0 == V::Power::GUARDED_POWER =>
        {
            None
        }
        // `MasterPlannerPower.AfterCardPlayed`: Sly written on each Skill its
        // owner plays, for as long as the copy lasts.
        (__c0, _, EventHook::CardPlayed { card, id, .. })
            if __c0 == V::Power::MASTER_PLANNER_POWER
                && (<V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Skill)) =>
        {
            Some(HookEffect::AddKeyword {
                to: sts2_core::Writing::Named(id),
                keyword: CardKeyword::Sly,
            })
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::MASTER_PLANNER_POWER => None,
        // `ShadowStepPower.AfterSideTurnStart`: its stack as Double Damage on
        // its owner as its owner's side begins a turn, then it goes.
        (__c0, amount, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::SHADOW_STEP_POWER && (participants.contains(holder)) =>
        {
            asked.push(HookEffect::ApplyToOwner(
                V::Power::DOUBLE_DAMAGE_POWER,
                amount,
            ));
            asked.push(HookEffect::Remove);
            return;
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::SHADOW_STEP_POWER => None,
        // `DoubleDamagePower.AfterSideTurnEnd`: one off its stack as its
        // owner's turn ends.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::DOUBLE_DAMAGE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::DOUBLE_DAMAGE_POWER => None,
        // `ChildOfTheStarsPower.AfterStarsSpent`: its stack of block,
        // unpowered, for each star its owner spent.
        (__c0, PowerAmount::Counted(stack), EventHook::StarsSpent { player, amount })
            if __c0 == V::Power::CHILD_OF_THE_STARS_POWER && (player == holder && amount > 0) =>
        {
            Some(HookEffect::GainBlock(stack * amount))
        }
        (__c0, PowerAmount::Endless, EventHook::StarsSpent { .. })
            if __c0 == V::Power::CHILD_OF_THE_STARS_POWER =>
        {
            unimplemented!("an endless Child of the Stars")
        }
        (__c0, _, EventHook::StarsSpent { .. }) if __c0 == V::Power::CHILD_OF_THE_STARS_POWER => {
            None
        }
        // `RagePower.AfterCardPlayed`: its stack of block, unpowered, for
        // each Attack its owner plays.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { card, .. })
            if __c0 == V::Power::RAGE_POWER
                && (<V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Attack)) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { .. })
            if __c0 == V::Power::RAGE_POWER =>
        {
            unimplemented!("an endless Rage")
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::RAGE_POWER => None,
        // `RagePower.AfterSideTurnEnd`: `PowerCmd.Remove(this)` as its owner's
        // turn ends.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::RAGE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::RAGE_POWER => None,
        // `ViciousPower.AfterPowerAmountChanged`: its stack drawn each time
        // its owner puts more Vulnerable on anything.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                power: __c1,
                delta,
                applier: Some(applier),
                ..
            },
        ) if (__c0 == V::Power::VICIOUS_POWER && __c1 == V::Power::VULNERABLE_POWER)
            && (delta > 0 && applier == holder) =>
        {
            Some(HookEffect::Draw(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::PowerAmountChanged { .. })
            if __c0 == V::Power::VICIOUS_POWER =>
        {
            unimplemented!("an endless Vicious")
        }
        (__c0, _, EventHook::PowerAmountChanged { .. }) if __c0 == V::Power::VICIOUS_POWER => None,
        // `ShroudPower.AfterPowerAmountChanged`: its stack as block,
        // unpowered, whenever its owner applies Doom.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                power: __c1,
                applier: Some(applier),
                ..
            },
        ) if (__c0 == V::Power::SHROUD_POWER && __c1 == V::Power::DOOM_POWER)
            && (applier == holder) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::PowerAmountChanged { .. })
            if __c0 == V::Power::SHROUD_POWER =>
        {
            unimplemented!("an endless Shroud")
        }
        (__c0, _, EventHook::PowerAmountChanged { .. }) if __c0 == V::Power::SHROUD_POWER => None,
        // `SleightOfFleshPower.AfterPowerAmountChanged`: its stack as damage,
        // unpowered, on an enemy its owner just debuffed, but not through a
        // temporary power.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::PowerAmountChanged {
                creature,
                power: changed,
                delta,
                applier: Some(applier),
            },
        ) if __c0 == V::Power::SLEIGHT_OF_FLESH_POWER
            && (applier == holder
                && fight.side_of(creature) == Some(CombatSide::Enemy)
                && lends::<V>(changed).is_none()
                && debuff_at::<V>(changed, delta)) =>
        {
            Some(HookEffect::Damage {
                target: creature,
                amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::PowerAmountChanged { .. })
            if __c0 == V::Power::SLEIGHT_OF_FLESH_POWER =>
        {
            unimplemented!("an endless Sleight of Flesh")
        }
        (__c0, _, EventHook::PowerAmountChanged { .. })
            if __c0 == V::Power::SLEIGHT_OF_FLESH_POWER =>
        {
            None
        }
        // `ToricToughnessPower.AfterBlockCleared`: its block on its owner,
        // then one off its turns.
        (__c0, _, EventHook::BlockCleared { creature, .. })
            if __c0 == V::Power::TORIC_TOUGHNESS_POWER && (creature == holder) =>
        {
            let State::ToricToughness { block } = *state else {
                unreachable!("Toric Toughness keeps its block")
            };
            asked.push(HookEffect::GainBlock(block));
            asked.push(HookEffect::Decrement);
            return;
        }
        (__c0, _, EventHook::BlockCleared { .. }) if __c0 == V::Power::TORIC_TOUGHNESS_POWER => {
            None
        }
        // `InfernoPower.AfterPlayerTurnStart`: its self damage on its owner,
        // unblockable and unpowered, from its owner.
        (__c0, _, EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::INFERNO_POWER && (player == holder) =>
        {
            let State::SelfDamage { self_damage, .. } = *state else {
                unreachable!("Inferno keeps its self damage")
            };
            Some(HookEffect::DamageOwnerInFight {
                amount: self_damage,
                unblockable: true,
                self_dealt: true,
            })
        }
        (__c0, _, EventHook::PlayerTurnStart { .. }) if __c0 == V::Power::INFERNO_POWER => None,
        // `CrimsonMantlePower.AfterPlayerTurnStart`: its self damage on its
        // owner, unblockable and unpowered, from its owner, then its stack of
        // block, unpowered.
        (__c0, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::CRIMSON_MANTLE_POWER && (player == holder) =>
        {
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
        (__c0, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::CRIMSON_MANTLE_POWER && (player == holder) =>
        {
            unimplemented!("an endless Crimson Mantle")
        }
        (__c0, _, EventHook::PlayerTurnStart { .. }) if __c0 == V::Power::CRIMSON_MANTLE_POWER => {
            None
        }
        // `NightmarePower.BeforeHandDraw`: its stack of clones of the card it
        // keeps into its owner's hand, then `PowerCmd.Remove(this)`.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::NIGHTMARE_POWER && (player == holder) =>
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
        (__c0, _, EventHook::BeforeHandDraw { .. }) if __c0 == V::Power::NIGHTMARE_POWER => None,
        // `InfernoPower.AfterDamageReceived`: its stack on every enemy that
        // can be hit, unpowered, where its owner lost health in its own
        // side's turn.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                unblocked,
                side,
                ..
            },
        ) if __c0 == V::Power::INFERNO_POWER
            && (target == holder && unblocked > 0 && side == owner) =>
        {
            Some(HookEffect::DamageOpponents(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { .. })
            if __c0 == V::Power::INFERNO_POWER =>
        {
            unimplemented!("an endless Inferno")
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::INFERNO_POWER => None,
        // `SicEmPower.AfterDamageGiven`: `OstyCmd.Summon` of its stack for the
        // player whose Osty hit its owner, where that player applied it.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer: Some(dealer),
                pet_of: Some(pet_of),
                target,
                ..
            },
        ) if __c0 == V::Power::SIC_EM_POWER
            && (target == holder
                && applier == Some(pet_of)
                && fight.monster(dealer) == Some(V::Monster::OSTY)) =>
        {
            Some(HookEffect::Summon(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::DamageGiven { .. })
            if __c0 == V::Power::SIC_EM_POWER =>
        {
            unimplemented!("an endless Sic 'Em")
        }
        (__c0, _, EventHook::DamageGiven { .. }) if __c0 == V::Power::SIC_EM_POWER => None,
        // `SicEmPower.AfterSideTurnEnd`: `PowerCmd.Remove(this)` as its
        // owner's turn ends.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::SIC_EM_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::SIC_EM_POWER => None,
        // `ConquerorPower.AfterSideTurnEnd`: one off its stack as its owner's
        // side's turn ends.
        (__c0, PowerAmount::Counted(_), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::CONQUEROR_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::CONQUEROR_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Conqueror")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::CONQUEROR_POWER => None,
        // `ArtifactPower.AfterModifyingPowerAmountReceived`: one used.
        (__c0, PowerAmount::Counted(_), EventHook::PowerAmountReceivedModified { .. })
            if __c0 == V::Power::ARTIFACT_POWER =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::PowerAmountReceivedModified { .. })
            if __c0 == V::Power::ARTIFACT_POWER =>
        {
            unimplemented!("an endless Artifact")
        }
        // `TangledPower.AfterApplied`: Entangled on every Attack its owner has.
        // `RingingPower.AfterApplied`: Ringing on every card its owner has.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::RINGING_POWER => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::Every,
                affliction: V::Affliction::RINGING,
            })
        }
        // `RingingPower.AfterCardEnteredCombat`: Ringing on a card that
        // arrives with no affliction.
        (__c0, _, EventHook::CardEnteredCombat { at, .. }) if __c0 == V::Power::RINGING_POWER => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: V::Affliction::RINGING,
            })
        }
        // `RingingPower.AfterSideTurnEnd`: gone as its owner's side ends a
        // turn.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::RINGING_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::RINGING_POWER => None,
        // `SmoggyPower.AfterCardPlayed`: a Skill its owner plays puts Smog on
        // every Skill its owner has with no affliction.
        (__c0, _, EventHook::CardPlayed { card, player, .. })
            if __c0 == V::Power::SMOGGY_POWER
                && (player == holder
                    && <V as sts2_core::Rules>::card_kind(card)
                        == Some(sts2_core::CardKind::Skill)) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::EveryOfKind(sts2_core::CardKind::Skill),
                affliction: V::Affliction::SMOG,
            })
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::SMOGGY_POWER => None,
        // `SmoggyPower.AfterCardEnteredCombat`: Smog on a Skill that arrives
        // after its owner started a Skill's play this turn.
        (
            __c0,
            _,
            EventHook::CardEnteredCombat {
                at, card, player, ..
            },
        ) if __c0 == V::Power::SMOGGY_POWER
            && (player == holder
                && <V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Skill)
                && history.started_this_turn().any(|started| {
                    <V as sts2_core::Rules>::card_kind(started.card)
                        == Some(sts2_core::CardKind::Skill)
                })) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: V::Affliction::SMOG,
            })
        }
        (__c0, _, EventHook::CardEnteredCombat { .. }) if __c0 == V::Power::SMOGGY_POWER => None,
        // `SmoggyPower.AfterSideTurnEnd`, as its owner's side ends a turn: Smog
        // off every card that has it. The power stays.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::SMOGGY_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ClearAffliction(
                sts2_core::Writing::EveryAfflictedWith(V::Affliction::SMOG),
            ))
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::SMOGGY_POWER => None,
        // `RingingPower.AfterRemoved`: Ringing off every card that has it.
        (__c0, _, EventHook::Removed) if __c0 == V::Power::RINGING_POWER => {
            Some(HookEffect::ClearAffliction(
                sts2_core::Writing::EveryAfflictedWith(V::Affliction::RINGING),
            ))
        }
        // `HexPower.AfterApplied`: Hexed on every card its owner holds that
        // has no affliction. `AfterCardEnteredCombat`: on a card that joins.
        (__c0, PowerAmount::Counted(_), EventHook::Applied) if __c0 == V::Power::HEX_POWER => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::Every,
                affliction: V::Affliction::HEXED,
            })
        }
        (__c0, _, EventHook::CardEnteredCombat { at, .. }) if __c0 == V::Power::HEX_POWER => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: V::Affliction::HEXED,
            })
        }
        // `HexPower.AfterDeath`: gone once the creature that applied it dies.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::HEX_POWER && (Some(creature) == applier) => Some(HookEffect::Remove),
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::HEX_POWER => None,
        // `HexPower.AfterRemoved`: Hexed off every card that has it.
        (__c0, _, EventHook::Removed) if __c0 == V::Power::HEX_POWER => {
            Some(HookEffect::ClearAffliction(
                sts2_core::Writing::EveryAfflictedWith(V::Affliction::HEXED),
            ))
        }
        (__c0, _, EventHook::Applied) if __c0 == V::Power::TANGLED_POWER => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::EveryOfKind(sts2_core::CardKind::Attack),
                affliction: V::Affliction::ENTANGLED,
            })
        }
        // `TangledPower.AfterCardEnteredCombat`: Entangled on an Attack that
        // arrives with no affliction.
        (__c0, _, EventHook::CardEnteredCombat { at, card, .. })
            if __c0 == V::Power::TANGLED_POWER
                && (<V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Attack)) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: V::Affliction::ENTANGLED,
            })
        }
        (__c0, _, EventHook::CardEnteredCombat { .. }) if __c0 == V::Power::TANGLED_POWER => None,
        // `TangledPower.AfterSideTurnEnd`: gone as its owner's side ends a
        // turn.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::TANGLED_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::TANGLED_POWER => None,
        // `TangledPower.AfterRemoved`: Entangled off every card that has it.
        (__c0, _, EventHook::Removed) if __c0 == V::Power::TANGLED_POWER => {
            Some(HookEffect::ClearAffliction(
                sts2_core::Writing::EveryAfflictedWith(V::Affliction::ENTANGLED),
            ))
        }
        // `DemisePower.AfterSideTurnEnd`: its stack off its owner as its
        // owner's side's turn ends.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::DEMISE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::DamageOwnerInFight {
                amount,
                unblockable: true,
                self_dealt: false,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::DEMISE_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Demise")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::DEMISE_POWER => None,
        // `BiasedCognitionPower.AfterSideTurnStart`: its stack off its owner's
        // Focus as its owner's side begins a turn.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::BIASED_COGNITION_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOwner(
                V::Power::FOCUS_POWER,
                PowerAmount::Counted(-amount),
            ))
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::BIASED_COGNITION_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Biased Cognition")
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::BIASED_COGNITION_POWER => {
            None
        }
        // `StranglePower.BeforeCardPlayed`: a play by its applier, where a
        // player applied it, noted with the stack as it begins.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { id, player, .. })
            if __c0 == V::Power::STRANGLE_POWER =>
        {
            if applier == Some(player) {
                note::<V>(state, id, amount);
            }
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::STRANGLE_POWER =>
        {
            unimplemented!("an endless Strangle")
        }
        // `StranglePower.AfterCardPlayed`: the stack it noted off its owner,
        // unblockable, unpowered and from nobody.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::STRANGLE_POWER => {
            take_noted::<V>(state, id).map(|amount| HookEffect::DamageOwnerInFight {
                amount,
                unblockable: true,
                self_dealt: false,
            })
        }
        // `StranglePower.AfterSideTurnEnd`: gone as its owner's side ends a
        // turn.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::STRANGLE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::STRANGLE_POWER => None,
        // `OblivionPower.BeforeCardPlayed`: a play by its applier, where a
        // player applied it, noted with the stack as it begins.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { id, player, .. })
            if __c0 == V::Power::OBLIVION_POWER =>
        {
            if applier == Some(player) {
                note::<V>(state, id, amount);
            }
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::OBLIVION_POWER =>
        {
            unimplemented!("an endless Oblivion")
        }
        // `OblivionPower.AfterCardPlayed`: the stack it noted as Doom on its
        // owner.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::OBLIVION_POWER => {
            take_noted::<V>(state, id).map(|amount| HookEffect::ApplyPower {
                target: holder,
                power: V::Power::DOOM_POWER,
                amount: PowerAmount::Counted(amount),
                applier,
            })
        }
        // `OblivionPower.AfterSideTurnEnd`: gone as the player's turn ends.
        (
            __c0,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Player,
                ..
            },
        ) if __c0 == V::Power::OBLIVION_POWER => Some(HookEffect::Remove),
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::OBLIVION_POWER => None,
        // `AfterimagePower.BeforeCardPlayed`: a play of its owner's noted with
        // the stack as it begins.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { id, .. })
            if __c0 == V::Power::AFTERIMAGE_POWER && (owner == CombatSide::Player) =>
        {
            note::<V>(state, id, amount);
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::AFTERIMAGE_POWER =>
        {
            unimplemented!("an endless Afterimage")
        }
        // `AfterimagePower.AfterCardPlayed`: the stack it noted as unpowered
        // block, where it is more than nothing.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::AFTERIMAGE_POWER => {
            take_noted::<V>(state, id)
                .filter(|&amount| amount > 0)
                .map(HookEffect::GainBlock)
        }
        // `SelfFormingClayPower.AfterBlockCleared`: its stack as unpowered
        // block once its owner's block clears, then gone.
        (__c0, PowerAmount::Counted(amount), EventHook::BlockCleared { creature, .. })
            if __c0 == V::Power::SELF_FORMING_CLAY_POWER && (creature == holder) =>
        {
            Some(HookEffect::GainBlockAndRemove(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::BlockCleared { .. })
            if __c0 == V::Power::SELF_FORMING_CLAY_POWER =>
        {
            unimplemented!("an endless Self-Forming Clay")
        }
        (__c0, _, EventHook::BlockCleared { .. }) if __c0 == V::Power::SELF_FORMING_CLAY_POWER => {
            None
        }
        // `MonologuePower.BeforeCardPlayed`: a play of its owner's noted with
        // its Strength, which the card that applied it set to its own `Power`,
        // one.
        (__c0, _, EventHook::CardPlaying { id, .. })
            if __c0 == V::Power::MONOLOGUE_POWER && (owner == CombatSide::Player) =>
        {
            note::<V>(state, id, MONOLOGUE_STRENGTH);
            None
        }
        // `MonologuePower.AfterCardPlayed`: the Strength noted lent to its
        // owner, and counted as lent.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::MONOLOGUE_POWER => {
            take_noted::<V>(state, id).map(|lent| {
                let State::Monologue { applied, .. } = state else {
                    unreachable!("Strength counted for {:?}", state.power())
                };
                *applied += MONOLOGUE_STRENGTH;
                HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, PowerAmount::Counted(lent))
            })
        }
        // `MonologuePower.AfterSideTurnEnd` on its owner's side: gone, and
        // all it lent taken back.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::MONOLOGUE_POWER && (participants.contains(holder)) =>
        {
            let State::Monologue { applied, .. } = state else {
                unreachable!("Strength counted for {:?}", state.power())
            };
            asked.extend([
                HookEffect::Remove,
                HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, PowerAmount::Counted(-*applied)),
            ]);
            return;
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::MONOLOGUE_POWER => None,
        // `SubroutinePower.BeforeCardPlayed`: a Power its owner plays noted
        // with the stack as it begins.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { card, id, .. })
            if __c0 == V::Power::SUBROUTINE_POWER
                && (owner == CombatSide::Player
                    && <V as sts2_core::Rules>::card_kind(card)
                        == Some(sts2_core::CardKind::Power)) =>
        {
            note::<V>(state, id, amount);
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::SUBROUTINE_POWER =>
        {
            unimplemented!("an endless Subroutine")
        }
        (__c0, _, EventHook::CardPlaying { .. }) if __c0 == V::Power::SUBROUTINE_POWER => None,
        // `SubroutinePower.AfterCardPlayed`: an energy for each of the stack
        // it noted, one at a time.
        (__c0, _, EventHook::CardPlayed { id, .. }) if __c0 == V::Power::SUBROUTINE_POWER => {
            let noted = take_noted::<V>(state, id).unwrap_or(0);
            let count = usize::try_from(noted).expect("a noted stack is not negative");
            asked.extend(std::iter::repeat_n(HookEffect::GainEnergy(1), count));
            return;
        }
        // `HauntPower.AfterCardPlayed`: for a Soul its owner plays, its stack
        // on an enemy drawn off `CombatTargets`, unblockable, unpowered and
        // from nobody.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { card: __c1, .. })
            if (__c0 == V::Power::HAUNT_POWER && __c1 == V::Card::SOUL)
                && (owner == CombatSide::Player) =>
        {
            Some(HookEffect::DamageRandomOpponent {
                amount,
                unblockable: true,
                from_owner: false,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { .. })
            if __c0 == V::Power::HAUNT_POWER =>
        {
            unimplemented!("an endless Haunt")
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::HAUNT_POWER => None,
        // `TheSealedThronePower.BeforeCardPlayed`: its stack as stars for each
        // play of its owner's.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { .. })
            if __c0 == V::Power::THE_SEALED_THRONE_POWER && (owner == CombatSide::Player) =>
        {
            Some(HookEffect::GainStars(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { .. })
            if __c0 == V::Power::THE_SEALED_THRONE_POWER =>
        {
            unimplemented!("an endless Sealed Throne")
        }
        // `WraithFormPower.AfterSideTurnStart`: its stack off its owner's
        // Dexterity as its owner's side starts a turn.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::WRAITH_FORM_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOwner(
                V::Power::DEXTERITY_POWER,
                PowerAmount::Counted(-amount),
            ))
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { .. })
            if __c0 == V::Power::WRAITH_FORM_POWER =>
        {
            unimplemented!("an endless Wraith Form")
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::WRAITH_FORM_POWER => None,
        // `DisintegrationPower.AfterSideTurnEndLate`: its stack off its owner,
        // dealt by the owner and unpowered, as its owner's side's turn ends.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEndLate { participants, .. })
            if __c0 == V::Power::DISINTEGRATION_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::DamageOwnerInFight {
                amount,
                unblockable: false,
                self_dealt: true,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEndLate { participants, .. })
            if __c0 == V::Power::DISINTEGRATION_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Disintegration")
        }
        (__c0, _, EventHook::SideTurnEndLate { .. }) if __c0 == V::Power::DISINTEGRATION_POWER => {
            None
        }
        // `SlothPower.BeforeCardPlayed`: one more play counted. A fight here
        // has one player, whose cards are its holder's.
        (__c0, _, EventHook::CardPlaying { .. }) if __c0 == V::Power::SLOTH_POWER => {
            let State::Sloth { played } = state else {
                unreachable!("plays counted for {:?}", state.power())
            };
            *played += one;
            None
        }
        // `VoidFormPower.AfterCardPlayed`: one more play of its owner's, where
        // it is not auto-played and is the last of its replays.
        (__c0, _, EventHook::CardPlayed { auto, last, .. })
            if __c0 == V::Power::VOID_FORM_POWER =>
        {
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
        (__c0, _, EventHook::BeforeSideTurnStart { participants, .. })
            if __c0 == V::Power::VOID_FORM_POWER =>
        {
            if participants.contains(holder) {
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
        (__c0, _, EventHook::BeforeApplied { .. }) if __c0 == V::Power::VOID_FORM_POWER => {
            *state = State::VoidForm {
                played: VOID_FORM_SPENT,
            };
            None
        }
        (
            __c0,
            _,
            EventHook::PowerAmountChanging {
                power: __c1,
                target,
                ..
            },
        ) if (__c0 == V::Power::VOID_FORM_POWER && __c1 == V::Power::VOID_FORM_POWER)
            && (target == holder) =>
        {
            *state = State::VoidForm {
                played: VOID_FORM_SPENT,
            };
            None
        }
        (__c0, _, EventHook::PowerAmountChanging { .. }) if __c0 == V::Power::VOID_FORM_POWER => {
            None
        }
        // `SlothPower.BeforeSideTurnStart`: the count starts again as its
        // owner's side's turn begins.
        (__c0, _, EventHook::BeforeSideTurnStart { participants, .. })
            if __c0 == V::Power::SLOTH_POWER =>
        {
            if participants.contains(holder) {
                let State::Sloth { played } = state else {
                    unreachable!("plays counted for {:?}", state.power())
                };
                *played = 0;
            }
            None
        }
        // `ConstrictPower.AfterSideTurnEnd`: its stack off its owner, dealt by
        // the owner and unpowered, as its owner's side's turn ends.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::CONSTRICT_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::DamageOwnerInFight {
                amount,
                unblockable: false,
                self_dealt: true,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::CONSTRICT_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Constrict")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::CONSTRICT_POWER => None,
        // `ConstrictPower.AfterDeath`: gone with the creature that applied it.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::CONSTRICT_POWER && (applier == Some(creature)) => {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::CONSTRICT_POWER => None,
        // `BeforeCardPlayed`: one off the stack for each card of its kind its
        // owner plays, which stands in the play pile.
        (
            __c0,
            _,
            EventHook::CardPlaying {
                card,
                player,
                ethereal,
                ..
            },
        ) if (__c0 == V::Power::FREE_ATTACK_POWER
            || __c0 == V::Power::FREE_SKILL_POWER
            || __c0 == V::Power::FREE_POWER_POWER
            || __c0 == V::Power::VEILPIERCER_POWER) =>
        {
            (player == holder && frees::<V>(power, card, ethereal)).then_some(HookEffect::Decrement)
        }
        // `IntangiblePower.AfterSideTurnEnd`: one off its stack as the
        // enemies' turn ends.
        (
            __c0,
            PowerAmount::Counted(_),
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::INTANGIBLE_POWER => Some(HookEffect::Decrement),
        (
            __c0,
            PowerAmount::Endless,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::INTANGIBLE_POWER => unimplemented!("an endless Intangible"),
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::INTANGIBLE_POWER => None,
        // `BorrowedTimePower.AfterSideTurnEnd`: gone once its owner's turn
        // ends.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::BORROWED_TIME_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::BORROWED_TIME_POWER => None,
        // `PoisonPower.AfterSideTurnStart`, for its owner's side: `TriggerCount`
        // rounds, which is its stack or one more than the Accelerant its
        // owner's living opponents carry, whichever is fewer.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::POISON_POWER && (participants.contains(holder)) =>
        {
            let accelerant: Number = fight
                .living_opponents(holder)
                .into_iter()
                .filter_map(
                    |opponent| match fight.power(opponent, V::Power::ACCELERANT_POWER) {
                        Some(PowerAmount::Counted(amount)) => Some(amount),
                        Some(PowerAmount::Endless) => unimplemented!("an endless Accelerant"),
                        None => None,
                    },
                )
                .sum();
            Some(HookEffect::DamageSelfByStack {
                times: amount.min(1 + accelerant),
            })
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::POISON_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Poison")
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::POISON_POWER => None,
        // `NoxiousFumesPower.AfterSideTurnStart`: its stack of Poison onto
        // every enemy that can be hit, from its owner.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::NOXIOUS_FUMES_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOpponents(
                V::Power::POISON_POWER,
                PowerAmount::Counted(amount),
            ))
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::NOXIOUS_FUMES_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Noxious Fumes")
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::NOXIOUS_FUMES_POWER => None,
        // `EnvenomPower.AfterDamageGiven`: its stack of Poison onto whoever a
        // powered hit from its owner took hit points from.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer,
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::ENVENOM_POWER && (dealer == Some(holder) && unblocked > 0) => {
            Some(HookEffect::ApplyPower {
                target,
                power: V::Power::POISON_POWER,
                amount: PowerAmount::Counted(amount),
                applier: Some(holder),
            })
        }
        (__c0, PowerAmount::Endless, EventHook::DamageGiven { dealer, .. })
            if __c0 == V::Power::ENVENOM_POWER && (dealer == Some(holder)) =>
        {
            unimplemented!("an endless Envenom")
        }
        (__c0, _, EventHook::DamageGiven { .. }) if __c0 == V::Power::ENVENOM_POWER => None,
        // `PaperCutsPower.AfterDamageGiven`: a powered hit from its owner that
        // gets past a player's block takes its amount off their maximum.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer,
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::PAPER_CUTS_POWER
            && (dealer == Some(holder) && fight.is_player(target) && unblocked > 0) =>
        {
            Some(HookEffect::LoseMaxHpOf { target, amount })
        }
        (__c0, PowerAmount::Endless, EventHook::DamageGiven { dealer, .. })
            if __c0 == V::Power::PAPER_CUTS_POWER && (dealer == Some(holder)) =>
        {
            unimplemented!("an endless Paper Cuts")
        }
        (__c0, _, EventHook::DamageGiven { .. }) if __c0 == V::Power::PAPER_CUTS_POWER => None,
        // `RampartPower.AfterSideTurnStart`, for the player's side when no
        // player takes an extra turn: its amount of block on each Turret
        // Operator among the enemies.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::SideTurnStart {
                side: CombatSide::Player,
                extra: false,
                ..
            },
        ) if __c0 == V::Power::RAMPART_POWER => {
            // `CombatState.Enemies`: a Living Shield holds it, so they are its
            // teammates. `CreatureCmd.GainBlock` passes over one `IsDead`.
            asked.extend(
                fight
                    .teammates(holder)
                    .into_iter()
                    .filter(|&enemy| fight.current_hp(enemy).is_some_and(|hp| hp > 0))
                    .filter(|&enemy| fight.monster(enemy) == Some(V::Monster::TURRET_OPERATOR))
                    .map(|target| HookEffect::GainBlockOf { target, amount }),
            );
            return;
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::RAMPART_POWER => None,
        // `PossessStrengthPower.AfterPowerAmountChanged` and its Dexterity
        // twin: a stack its holder took off the player is counted as stolen.
        (
            __c0,
            _,
            EventHook::PowerAmountChanged {
                creature,
                power: changed,
                delta,
                applier,
            },
        ) if (__c0 == V::Power::POSSESS_STRENGTH_POWER
            || __c0 == V::Power::POSSESS_SPEED_POWER) =>
        {
            let possessed = match power {
                __c0 if __c0 == V::Power::POSSESS_STRENGTH_POWER => V::Power::STRENGTH_POWER,
                _ => V::Power::DEXTERITY_POWER,
            };
            if applier == Some(holder)
                && fight.is_player(creature)
                && changed == possessed
                && delta < 0
            {
                *state.stolen_from(creature) += delta;
            }
            None
        }
        // `AfterDeath` for its own holder: the stolen stack given back, from
        // nobody.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if (__c0 == V::Power::POSSESS_STRENGTH_POWER
            || __c0 == V::Power::POSSESS_SPEED_POWER)
            && (creature == holder) =>
        {
            let State::Possess { from, stolen, .. } = *state else {
                unreachable!("a Possess keeps what it stole")
            };
            let gives = match power {
                __c0 if __c0 == V::Power::POSSESS_STRENGTH_POWER => V::Power::STRENGTH_POWER,
                _ => V::Power::DEXTERITY_POWER,
            };
            // Each player it took from, in the order it first took.
            asked.extend(from.into_iter().zip(stolen).filter_map(|(player, stolen)| {
                Some(HookEffect::ApplyPower {
                    target: player?,
                    power: gives,
                    amount: PowerAmount::Counted(-stolen),
                    applier: None,
                })
            }));
            None
        }
        (__c0, _, EventHook::AfterDeath { .. })
            if (__c0 == V::Power::POSSESS_STRENGTH_POWER
                || __c0 == V::Power::POSSESS_SPEED_POWER) =>
        {
            None
        }
        // `RavenousPower.AfterDeath`: another creature of its owner's side
        // dies, and the owner, still standing, is stunned and gains its stack
        // as Strength. The stun's own move only sets the animation back.
        (
            __c0,
            amount,
            EventHook::AfterDeath {
                creature,
                side,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::RAVENOUS_POWER
            && (creature != holder
                && side == owner
                && fight.current_hp(holder).is_some_and(|hp| hp > 0)) =>
        {
            let PowerAmount::Counted(amount) = amount else {
                unimplemented!("an endless Ravenous")
            };
            asked.extend([
                stunned::<V>(sts2_core::StunResume::Interrupted),
                HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, PowerAmount::Counted(amount)),
            ]);
            return;
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::RAVENOUS_POWER => None,
        // `HighVoltagePower.AfterSideTurnEnd`: its stack of Strength as its
        // owner's side ends a turn.
        (__c0, amount, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::HIGH_VOLTAGE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, amount))
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::HIGH_VOLTAGE_POWER => None,
        // `GalvanicPower.BeforeCombatStart`: Galvanized on every Power the
        // player holds. `AfterCardEnteredCombat`: on a Power with no
        // affliction that joins the fight, which a clone of an afflicted
        // card is not.
        (__c0, _, EventHook::CombatStarting { .. }) if __c0 == V::Power::GALVANIC_POWER => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::EveryOfKind(sts2_core::CardKind::Power),
                affliction: V::Affliction::GALVANIZED,
            })
        }
        (
            __c0,
            _,
            EventHook::CardEnteredCombat {
                at,
                card,
                afflicted: false,
                ..
            },
        ) if __c0 == V::Power::GALVANIC_POWER
            && (<V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power)) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: V::Affliction::GALVANIZED,
            })
        }
        (__c0, _, EventHook::CardEnteredCombat { .. }) if __c0 == V::Power::GALVANIC_POWER => None,
        // `GalvanicPower.AfterCardPlayed`: its amount to the player who
        // played a Galvanized card, unpowered and from nobody.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::CardPlayed {
                affliction: Some(__c1),
                player,
                ..
            },
        ) if (__c0 == V::Power::GALVANIC_POWER && __c1 == V::Affliction::GALVANIZED) => {
            Some(HookEffect::DamageUnpowered {
                target: player,
                amount,
            })
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::GALVANIC_POWER => None,
        // `NoDrawPower.AfterSideTurnEnd`: gone once its owner's turn ends.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::NO_DRAW_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::NO_DRAW_POWER => None,
        // `HellraiserPower.AfterCardDrawnEarly`: a Strike its owner draws is
        // auto-played. The cap it keeps on auto-plays counts only against
        // enemies whose hit points are infinite.
        (__c0, _, EventHook::CardDrawnEarly { card, id, .. })
            if __c0 == V::Power::HELLRAISER_POWER =>
        {
            card.tagged(CardTag::Strike)
                .then_some(HookEffect::AutoPlay(id))
        }
        // `HellraiserPower.AfterSideTurnEnd` resets that cap, and its
        // `BeforeAttack` sets how an auto-played Strike's hit looks.
        (__c0, _, EventHook::SideTurnEnd { .. } | EventHook::BeforeAttack { .. })
            if __c0 == V::Power::HELLRAISER_POWER =>
        {
            None
        }
        // `ColossusPower.AfterSideTurnEnd`: one off its stack as the enemies'
        // turn ends, whichever side its owner is on.
        (
            __c0,
            PowerAmount::Counted(_),
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::COLOSSUS_POWER => Some(HookEffect::Decrement),
        (
            __c0,
            PowerAmount::Endless,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::COLOSSUS_POWER => unimplemented!("an endless Colossus"),
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::COLOSSUS_POWER => None,
        // `ShrinkPower.AfterSideTurnEnd`, for the side its owner is among the
        // participants of. An infinite Shrink, the one the game gives a
        // negative amount, lasts the fight.
        (__c0, PowerAmount::Counted(_), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::SHRINK_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::SHRINK_POWER => None,
        // `ShrinkPower.AfterDeath`, which goes with the creature that applied
        // it.
        (__c0, _, EventHook::AfterDeath { creature, .. })
            if __c0 == V::Power::SHRINK_POWER && (applier == Some(creature)) =>
        {
            Some(HookEffect::Remove)
        }
        // `AfterSideTurnEnd` for the side its owner is among the participants
        // of, which is its owner's own side.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::TERRITORIAL_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOwner(
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(amount),
            ))
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { .. })
            if __c0 == V::Power::TERRITORIAL_POWER =>
        {
            unimplemented!("an endless Territorial")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::TERRITORIAL_POWER => None,
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::ESCAPE_ARTIST_POWER
                && (participants.contains(holder) && amount > one) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { .. })
            if __c0 == V::Power::ESCAPE_ARTIST_POWER =>
        {
            unimplemented!("an endless Escape Artist")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::ESCAPE_ARTIST_POWER => None,
        // `HatchPower.AfterSideTurnEnd`: `PowerCmd.Decrement` as its owner's
        // side ends.
        (__c0, PowerAmount::Counted(_), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::HATCH_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::HATCH_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Hatch")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::HATCH_POWER => None,
        // `PersonalHivePower.AfterDamageReceived`: a Dazed for each of its
        // stack into the dealer's draw pile, at random places, after a
        // powered attack on its owner. A fight here has one player, so the
        // dealer's pile, or its Osty's owner's, is the one there is.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                dealer: Some(_),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::PERSONAL_HIVE_POWER && (target == holder) => {
            Some(HookEffect::AddToDrawAtRandom {
                card: V::Card::DAZED,
                count: amount,
                by_owner: false,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if __c0 == V::Power::PERSONAL_HIVE_POWER && (target == holder) =>
        {
            unimplemented!("an endless Personal Hive")
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::PERSONAL_HIVE_POWER => {
            None
        }
        // `SurroundedPower.BeforeCardPlayed` and `BeforePotionUsed`: its owner
        // turns toward a target behind it, for its owner's card or potion.
        (
            __c0,
            _,
            EventHook::CardPlaying {
                target: Some(target),
                player,
                ..
            }
            | EventHook::PotionUsing {
                target: Some(target),
                player,
            },
        ) if __c0 == V::Power::SURROUNDED_POWER && player == holder => {
            state.turn_toward(fight, target);
            None
        }
        (__c0, _, EventHook::CardPlaying { .. } | EventHook::PotionUsing { .. })
            if __c0 == V::Power::SURROUNDED_POWER =>
        {
            None
        }
        // `SurroundedPower.AfterDeath`: once an enemy dies, its owner turns
        // toward the first enemy left where every one left stands on the
        // same side of it.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                side,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::SURROUNDED_POWER && (side != owner) => {
            let hittable = fight.living_opponents(holder);
            let all = |power| {
                hittable
                    .iter()
                    .all(|&enemy| fight.power(enemy, power).is_some())
            };
            if let Some(&first) = hittable.first()
                && (all(V::Power::BACK_ATTACK_LEFT_POWER) || all(V::Power::BACK_ATTACK_RIGHT_POWER))
            {
                state.turn_toward(fight, first);
            }
            None
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::SURROUNDED_POWER => None,
        // `CrabRagePower.AfterDeath`: when another on its side dies, six
        // Strength, ninety-nine unpowered block, and then it goes.
        (__c0, _, EventHook::AfterDeath { creature, side, .. })
            if __c0 == V::Power::CRAB_RAGE_POWER && (creature != holder && side == owner) =>
        {
            asked.extend([
                HookEffect::ApplyToOwner(
                    V::Power::STRENGTH_POWER,
                    PowerAmount::Counted(V::Power::CRAB_RAGE_POWER_STRENGTH_POWER),
                ),
                HookEffect::GainBlock(V::Power::CRAB_RAGE_POWER_BLOCK),
                HookEffect::Remove,
            ]);
            return;
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::CRAB_RAGE_POWER => None,
        // `SandpitPower.AfterSideTurnStartLate`: one off as the enemies' turn
        // starts.
        (
            __c0,
            _,
            EventHook::SideTurnStartLate {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::SANDPIT_POWER => Some(HookEffect::Decrement),
        (__c0, _, EventHook::SideTurnStartLate { .. }) if __c0 == V::Power::SANDPIT_POWER => None,
        // `SandpitPower.AfterCreatureAddedToCombat` moves creatures on the
        // screen, and nothing else.
        (__c0, _, EventHook::CreatureAdded { .. }) if __c0 == V::Power::SANDPIT_POWER => None,
        // `SandpitPower.AfterRemoved`: while its owner and its target both
        // live, `CreatureCmd.Kill` on the target, forced.
        (__c0, _, EventHook::Removed) if __c0 == V::Power::SANDPIT_POWER => {
            let target = target.unwrap_or_else(|| unreachable!("a Sandpit is aimed at a player"));
            let alive = |creature| fight.current_hp(creature).is_some_and(|hp| hp > 0);
            (alive(holder) && alive(target)).then_some(HookEffect::KillCreature(target))
        }
        // Its other overrides move the player's figure and the music.
        (
            __c0,
            _,
            EventHook::Applied
            | EventHook::PowerAmountChanged { .. }
            | EventHook::BeforeSideTurnEnd { .. }
            | EventHook::OstyRevived,
        ) if __c0 == V::Power::SANDPIT_POWER => None,
        // `VitalSparkPower.BeforeCombatStart`: Tainted by its stack on every
        // Skill the player holds.
        (__c0, _, EventHook::CombatStarting { .. }) if __c0 == V::Power::VITAL_SPARK_POWER => {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::EveryOfKind(sts2_core::CardKind::Skill),
                affliction: V::Affliction::TAINTED,
            })
        }
        // `VitalSparkPower.AfterCardEnteredCombat`: Tainted on a Skill that
        // arrives with no affliction.
        (
            __c0,
            _,
            EventHook::CardEnteredCombat {
                at,
                card,
                afflicted: false,
                ..
            },
        ) if __c0 == V::Power::VITAL_SPARK_POWER
            && (<V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill)) =>
        {
            Some(HookEffect::Afflict {
                to: sts2_core::Writing::One(at),
                affliction: V::Affliction::TAINTED,
            })
        }
        (__c0, _, EventHook::CardEnteredCombat { .. }) if __c0 == V::Power::VITAL_SPARK_POWER => {
            None
        }
        // `VitalSparkPower.AfterCardPlayed`: Tainted Power by its stack on the
        // card's owner, from nobody, once a Tainted card's play is over.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::CardPlayed {
                affliction: Some(__c1),
                player,
                ..
            },
        ) if (__c0 == V::Power::VITAL_SPARK_POWER && __c1 == V::Affliction::TAINTED) => {
            Some(HookEffect::ApplyPower {
                target: player,
                power: V::Power::TAINTED_POWER,
                amount: PowerAmount::Counted(amount),
                applier: None,
            })
        }
        (
            __c0,
            PowerAmount::Endless,
            EventHook::CardPlayed {
                affliction: Some(__c1),
                ..
            },
        ) if (__c0 == V::Power::VITAL_SPARK_POWER && __c1 == V::Affliction::TAINTED) => {
            unimplemented!("an endless Vital Spark")
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::VITAL_SPARK_POWER => None,
        // `VitalSparkPower.AfterRemoved`: Tainted off every card that has it.
        (__c0, _, EventHook::Removed) if __c0 == V::Power::VITAL_SPARK_POWER => {
            Some(HookEffect::ClearAffliction(
                sts2_core::Writing::EveryAfflictedWith(V::Affliction::TAINTED),
            ))
        }
        // `AfterPowerAmountChanged` writes its stack onto each Tainted card's
        // own amount, which nothing here reads.
        (__c0, _, EventHook::PowerAmountChanged { .. }) if __c0 == V::Power::VITAL_SPARK_POWER => {
            None
        }
        // `TaintedPower.AfterSideTurnEnd`: gone as the enemies' turn ends.
        (
            __c0,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::TAINTED_POWER => Some(HookEffect::Remove),
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::TAINTED_POWER => None,
        (__c0, _, EventHook::SideTurnEnd { .. })
            if (__c0 == V::Power::VULNERABLE_POWER
                || __c0 == V::Power::WEAK_POWER
                || __c0 == V::Power::FRAIL_POWER
                || __c0 == V::Power::ENERGY_NEXT_TURN_POWER
                || __c0 == V::Power::BLOCK_NEXT_TURN_POWER
                || __c0 == V::Power::STRENGTH_POWER
                || __c0 == V::Power::INFESTED_POWER
                || __c0 == V::Power::ILLUSION_POWER
                || __c0 == V::Power::MINION_POWER
                || __c0 == V::Power::DEXTERITY_POWER
                || __c0 == V::Power::SLOW_POWER
                || __c0 == V::Power::SLIPPERY_POWER
                || __c0 == V::Power::BURROWED_POWER
                || __c0 == V::Power::SWIPE_POWER
                || __c0 == V::Power::SWORD_SAGE_POWER
                || __c0 == V::Power::IMBALANCED_POWER
                || __c0 == V::Power::PLATING_POWER
                || __c0 == V::Power::FURNACE_POWER
                || __c0 == V::Power::REATTACH_POWER) =>
        {
            None
        }
        (__c0, PowerAmount::Counted(amount), EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::ENERGY_NEXT_TURN_POWER && (player == holder) =>
        {
            Some(HookEffect::GainEnergyAndRemove(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::ENERGY_NEXT_TURN_POWER && (player == holder) =>
        {
            unimplemented!("an endless EnergyNextTurnPower handing over energy")
        }
        (__c0, PowerAmount::Counted(amount), EventHook::BlockCleared { creature, .. })
            if __c0 == V::Power::BLOCK_NEXT_TURN_POWER && (creature == holder) =>
        {
            Some(HookEffect::GainBlockAndRemove(amount))
        }
        // `ReattachPower.AfterDeath`, which sends the segment to the dead move
        // its own graph holds, and sets `isReviving`, while another segment
        // stands. The reattach that follows it is the graph's, so nothing
        // here installs one.
        //
        // The last segment to die keeps the move it showed. The game spends
        // that branch on the fade every segment plays, and the fight ends.
        (__c0, _, EventHook::AfterDeath { creature, .. })
            if __c0 == V::Power::REATTACH_POWER && (creature == holder) =>
        {
            let standing = other_segment_standing::<V>(fight, holder);
            standing.then(|| {
                state.set_reviving(true);
                HookEffect::EnterMove {
                    to: V::MoveId::DEAD_MOVE,
                    force: false,
                }
            })
        }
        (__c0, _, EventHook::AfterDeath { creature, .. })
            if __c0 == V::Power::ILLUSION_POWER && (creature == holder) =>
        {
            state.set_reviving(true);
            Some(HookEffect::InstallMove(V::powers_revive()))
        }
        // `SteamEruptionPower.AfterDeath`: the Waterfall Giant's
        // `TriggerAboutToBlowState`. It comes back at 999999999 health, and is
        // sent to About to Blow, forced over whatever it advertised.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::STEAM_ERUPTION_POWER && (creature == holder) => {
            asked.extend([
                HookEffect::SetMaxAndCurrentHp(ABOUT_TO_BLOW_HP),
                HookEffect::EnterMove {
                    to: V::MoveId::ABOUT_TO_BLOW_MOVE,
                    force: true,
                },
            ]);
            return;
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::STEAM_ERUPTION_POWER => None,
        // `AdaptablePower.AfterDeath`: a Test Subject that dies is sent to
        // its respawn, forced over a stun. `TriggerDeadState` also counts the
        // kill for the run's title, which nothing here reads.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::ADAPTABLE_POWER
            && (creature == holder && monster == Some(V::Monster::TEST_SUBJECT)) =>
        {
            state.set_reviving(true);
            Some(HookEffect::EnterMove {
                to: V::MoveId::RESPAWN_MOVE,
                force: true,
            })
        }
        // `EnragePower.AfterCardPlayed`: its stack as Strength on its owner for
        // every Skill played.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { card, .. })
            if __c0 == V::Power::ENRAGE_POWER =>
        {
            (<V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill))
                .then_some(HookEffect::ApplyToOwner(
                    V::Power::STRENGTH_POWER,
                    PowerAmount::Counted(amount),
                ))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { .. })
            if __c0 == V::Power::ENRAGE_POWER =>
        {
            unimplemented!("an endless Enrage")
        }
        // `PainfulStabsPower.AfterAttack`: Wounds for each hit its owner's
        // powered attack lands on a player's health.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::AfterAttack {
                attacker,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::PAINFUL_STABS_POWER && (attacker == holder) => {
            Some(HookEffect::GiveCardsPerUnblockedHit {
                card: V::Card::WOUND,
                per: amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::AfterAttack { attacker, .. })
            if __c0 == V::Power::PAINFUL_STABS_POWER && (attacker == holder) =>
        {
            unimplemented!("endless Painful Stabs")
        }
        (__c0, _, EventHook::AfterAttack { .. }) if __c0 == V::Power::PAINFUL_STABS_POWER => None,
        // `SuckPower.AfterAttack`: Strength for each hit its owner's powered
        // attack lands on an opponent's health.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::AfterAttack {
                attacker,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::SUCK_POWER && (attacker == holder) => {
            Some(HookEffect::ApplyToOwnerPerUnblockedHit {
                power: V::Power::STRENGTH_POWER,
                per: amount,
            })
        }
        (__c0, _, EventHook::AfterAttack { .. }) if __c0 == V::Power::SUCK_POWER => None,
        // `SkittishPower.AfterAttack`: once a turn, a card's attack that took
        // health off its owner gives the owner its amount in block.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::AfterAttack {
                powered: true,
                source: Some(_),
                ..
            },
        ) if __c0 == V::Power::SKITTISH_POWER => {
            let State::Skittish { gained } = state else {
                unreachable!("Skittish holding {state:?}")
            };
            let hit = fight
                .attack_result(holder)
                .is_some_and(|result| result.unblocked != 0);
            (!*gained && hit).then(|| {
                *gained = true;
                HookEffect::GainBlock(amount)
            })
        }
        (__c0, PowerAmount::Endless, EventHook::AfterAttack { .. })
            if __c0 == V::Power::SKITTISH_POWER =>
        {
            unimplemented!("an endless Skittish")
        }
        (__c0, _, EventHook::AfterAttack { .. }) if __c0 == V::Power::SKITTISH_POWER => None,
        // `SkittishPower.AfterSideTurnEnd`, as the other side ends its turn:
        // the block can come again.
        (__c0, _, EventHook::SideTurnEnd { side, .. }) if __c0 == V::Power::SKITTISH_POWER => {
            let State::Skittish { gained } = state else {
                unreachable!("Skittish holding {state:?}")
            };
            if side != owner {
                *gained = false;
            }
            None
        }
        // `NemesisPower.AfterSideTurnEnd`, as its owner's side's turn ends:
        // Intangible every other turn, and taken off on the turns between.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::NEMESIS_POWER && (participants.contains(holder)) =>
        {
            let State::Nemesis { intangible } = state else {
                unreachable!("Nemesis holding {state:?}")
            };
            *intangible = !*intangible;
            if *intangible {
                Some(HookEffect::ApplyToOwner(
                    V::Power::INTANGIBLE_POWER,
                    PowerAmount::Counted(one),
                ))
            } else {
                fight
                    .power(holder, V::Power::INTANGIBLE_POWER)
                    .map(|_| HookEffect::RemoveFromOwner(V::Power::INTANGIBLE_POWER))
            }
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::NEMESIS_POWER => None,
        // `BattlewornDummyTimeLimitPower.AfterSideTurnEnd`, as its owner's
        // side's turn ends: one off, and at the last the dummy escapes. The
        // escape is what the encounter's `RanOutOfTime` records.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::BATTLEWORN_DUMMY_TIME_LIMIT_POWER
                && (participants.contains(holder)) =>
        {
            Some(match amount > one {
                true => HookEffect::Decrement,
                false => HookEffect::Escape,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::BATTLEWORN_DUMMY_TIME_LIMIT_POWER
                && (participants.contains(holder)) =>
        {
            unimplemented!("an endless time limit")
        }
        (__c0, _, EventHook::SideTurnEnd { .. })
            if __c0 == V::Power::BATTLEWORN_DUMMY_TIME_LIMIT_POWER =>
        {
            None
        }
        // `AfterCardGeneratedForCombat`, for a card its holder made: every
        // enemy hit for a Status, orbs channelled for a Status, Strength or
        // block for any card.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::CardGeneratedForCombat {
                card,
                creator: Some(creator),
                ..
            },
        ) if (__c0 == V::Power::SMOKESTACK_POWER
            || __c0 == V::Power::TRASH_TO_TREASURE_POWER
            || __c0 == V::Power::ARSENAL_POWER
            || __c0 == V::Power::PILLAR_OF_CREATION_POWER)
            && (creator == holder) =>
        {
            let status =
                <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Status);
            match power {
                __c0 if __c0 == V::Power::SMOKESTACK_POWER => {
                    status.then_some(HookEffect::DamageOpponents(amount))
                }
                __c0 if __c0 == V::Power::TRASH_TO_TREASURE_POWER => {
                    status.then_some(HookEffect::ChannelRandomOrbs {
                        orbs: V::POWERS_VALID_ORBS,
                        count: amount,
                    })
                }
                __c0 if __c0 == V::Power::ARSENAL_POWER => Some(HookEffect::ApplyToOwner(
                    V::Power::STRENGTH_POWER,
                    PowerAmount::Counted(amount),
                )),
                _ => Some(HookEffect::GainBlock(amount)),
            }
        }
        (__c0, PowerAmount::Endless, EventHook::CardGeneratedForCombat { .. })
            if (__c0 == V::Power::SMOKESTACK_POWER
                || __c0 == V::Power::TRASH_TO_TREASURE_POWER
                || __c0 == V::Power::ARSENAL_POWER
                || __c0 == V::Power::PILLAR_OF_CREATION_POWER) =>
        {
            unimplemented!("an endless {power:?}")
        }
        (__c0, _, EventHook::CardGeneratedForCombat { .. })
            if (__c0 == V::Power::SMOKESTACK_POWER
                || __c0 == V::Power::TRASH_TO_TREASURE_POWER
                || __c0 == V::Power::ARSENAL_POWER
                || __c0 == V::Power::PILLAR_OF_CREATION_POWER) =>
        {
            None
        }
        // `ChainsOfBindingPower.AfterCardDrawn`, in its owner's turn: Bound on
        // the card drawn, while fewer cards than its stack were Bound this
        // turn.
        (__c0, PowerAmount::Counted(amount), EventHook::CardDrawn { id, side, .. })
            if __c0 == V::Power::CHAINS_OF_BINDING_POWER && (side == owner) =>
        {
            let bound = history.afflicted_this_turn(V::Affliction::BOUND);
            (Number::try_from(bound).is_ok_and(|bound| bound < amount)).then_some(
                HookEffect::Afflict {
                    to: sts2_core::Writing::Named(id),
                    affliction: V::Affliction::BOUND,
                },
            )
        }
        (__c0, PowerAmount::Endless, EventHook::CardDrawn { .. })
            if __c0 == V::Power::CHAINS_OF_BINDING_POWER =>
        {
            unimplemented!("endless Chains of Binding")
        }
        (__c0, _, EventHook::CardDrawn { .. }) if __c0 == V::Power::CHAINS_OF_BINDING_POWER => None,
        // `BeforeCardPlayed`: a Bound card of its owner's is played, which a
        // dupe never counts as.
        (
            __c0,
            _,
            EventHook::CardPlaying {
                affliction: Some(__c1),
                dupe: false,
                ..
            },
        ) if (__c0 == V::Power::CHAINS_OF_BINDING_POWER && __c1 == V::Affliction::BOUND) => {
            *state = State::ChainsOfBinding { bound_played: true };
            None
        }
        (__c0, _, EventHook::CardPlaying { .. }) if __c0 == V::Power::CHAINS_OF_BINDING_POWER => {
            None
        }
        // `BeforeSideTurnEnd`, as its owner's side's turn ends: none played,
        // and Bound off every card that has it.
        (__c0, _, EventHook::BeforeSideTurnEnd { participants, .. })
            if __c0 == V::Power::CHAINS_OF_BINDING_POWER && (participants.contains(holder)) =>
        {
            *state = State::ChainsOfBinding {
                bound_played: false,
            };
            Some(HookEffect::ClearAffliction(
                sts2_core::Writing::EveryAfflictedWith(V::Affliction::BOUND),
            ))
        }
        (__c0, _, EventHook::BeforeSideTurnEnd { .. })
            if __c0 == V::Power::CHAINS_OF_BINDING_POWER =>
        {
            None
        }
        // `TheBombPower.BeforeSideTurnEnd`: one off its turns as its owner's
        // turn is about to end. At the last, its damage, unpowered, to each
        // enemy that can be hit, and then it goes.
        (__c0, PowerAmount::Counted(turns), EventHook::BeforeSideTurnEnd { participants, .. })
            if __c0 == V::Power::THE_BOMB_POWER && (participants.contains(holder)) =>
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
        (__c0, PowerAmount::Endless, EventHook::BeforeSideTurnEnd { participants, .. })
            if __c0 == V::Power::THE_BOMB_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Bomb")
        }
        (__c0, _, EventHook::BeforeSideTurnEnd { .. }) if __c0 == V::Power::THE_BOMB_POWER => None,
        // `DampenPower.AfterApplied`: every upgraded card its owner holds in
        // the fight downgraded, and kept with the level it had.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::DAMPEN_POWER => {
            Some(HookEffect::DowngradeUpgraded)
        }
        // `AfterDeath` of one of its casters, which the fight keeps beside it:
        // off the list, and once none is left, it comes off.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::DAMPEN_POWER => {
            let casters = fight
                .kept(holder, power)
                .map_or(&[][..], |kept| &kept.creatures[..]);
            if !casters.contains(&creature) {
                return;
            }
            asked.push(HookEffect::ForgetCreature(creature));
            (casters.len() == 1).then_some(HookEffect::Remove)
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::DAMPEN_POWER => None,
        // `AfterRemoved`: each copy it downgraded upgraded back, once for each
        // level it had.
        (__c0, _, EventHook::Removed) if __c0 == V::Power::DAMPEN_POWER => {
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
        // plays: every sixth, a Wither into the player's hand. One aimed at
        // a pet hears none, since a pet's `Player` is null.
        (__c0, _, EventHook::CardPlayed { player, .. })
            if __c0 == V::Power::WITHERING_PRESENCE_POWER && Some(player) != target =>
        {
            None
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::WITHERING_PRESENCE_POWER => {
            let State::WitheringPresence { cards_left } = state else {
                unreachable!("Withering Presence holding {state:?}")
            };
            *cards_left -= 1;
            (*cards_left == 0).then(|| {
                *cards_left = WITHERING_CARDS;
                HookEffect::GenerateInHand {
                    card: V::Card::WITHER,
                    count: one,
                }
            })
        }
        (__c0, PowerAmount::Counted(amount), EventHook::AfterDeath { creature, .. })
            if __c0 == V::Power::INFESTED_POWER && (creature == holder) =>
        {
            Some(HookEffect::SummonAfterOwnerDeath {
                monster: V::Monster::WRIGGLER,
                count: amount,
                opening: V::powers_spawned(),
            })
        }
        (__c0, PowerAmount::Endless, EventHook::AfterDeath { creature, .. })
            if __c0 == V::Power::INFESTED_POWER && (creature == holder) =>
        {
            unimplemented!("an endless Infested power")
        }
        // `StockPower.AfterDeath` for its own holder: a new Axebot in its
        // slot, with one stock fewer, which opens on Boot Up.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::STOCK_POWER && (creature == holder && amount > 0) => {
            Some(HookEffect::SummonWritten {
                monster: V::Monster::AXEBOT,
                written: amount - 1,
                opens_on: Some(V::MoveId::BOOT_UP_MOVE),
            })
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::STOCK_POWER => None,
        // `TenderPower.AfterCardPlayed`, which counts only its owner's own
        // cards. Each one takes a point of Strength and of Dexterity, and the
        // count is what gives them back.
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::TENDER_POWER => {
            state.count(one);
            asked.extend([
                HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, PowerAmount::Counted(-one)),
                HookEffect::ApplyToOwner(V::Power::DEXTERITY_POWER, PowerAmount::Counted(-one)),
            ]);
            return;
        }
        // `TenderPower.AfterSideTurnEnd` for the side its owner is among the
        // participants of. Everything taken this turn comes back, and the
        // count starts again.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::TENDER_POWER && (participants.contains(holder)) =>
        {
            state.clear();
            asked.extend([
                HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, PowerAmount::Counted(counted)),
                HookEffect::ApplyToOwner(V::Power::DEXTERITY_POWER, PowerAmount::Counted(counted)),
            ]);
            return;
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::TENDER_POWER => None,
        // `SlumberPower.AfterSideTurnEnd` for the side its owner is among. The
        // beetle sleeps through its own turns and wakes when the count is
        // spent, which `PowerCmd.Decrement` taking the last instance off is.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::SLUMBER_POWER && (participants.contains(holder)) =>
        {
            if amount == one {
                asked.extend([
                    HookEffect::Decrement,
                    HookEffect::RemoveFromOwner(V::Power::PLATING_POWER),
                ]);
                return;
            }
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::SLUMBER_POWER => None,
        // `AsleepPower.AfterSideTurnEnd` for its owner's side: one off. The
        // `WakeUpMove` it then calls at nothing left only plays the waking.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::ASLEEP_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::ASLEEP_POWER => None,
        // `AsleepPower.BeforeSideTurnEndVeryEarly` for its owner's side: on
        // its last turn, the Plating goes.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnEndVeryEarly { participants, .. },
        ) if __c0 == V::Power::ASLEEP_POWER && (participants.contains(holder) && amount <= one) => {
            Some(HookEffect::RemoveFromOwner(V::Power::PLATING_POWER))
        }
        (__c0, _, EventHook::BeforeSideTurnEndVeryEarly { .. })
            if __c0 == V::Power::ASLEEP_POWER =>
        {
            None
        }
        // `AsleepPower.AfterDamageReceived`: a hit that takes health wakes the
        // Matriarch. The Plating goes, a stunned turn comes back on Slash, and
        // the power goes.
        (
            __c0,
            _,
            EventHook::DamageReceived {
                target, unblocked, ..
            },
        ) if __c0 == V::Power::ASLEEP_POWER && (target == holder && unblocked != 0) => {
            let slash =
                V::monster_state_of(V::Monster::LAGAVULIN_MATRIARCH, V::MoveId::SLASH_MOVE, 0)
                    .unwrap_or_else(|_| unreachable!("the Matriarch's graph holds its Slash"));
            asked.extend([
                HookEffect::RemoveFromOwner(V::Power::PLATING_POWER),
                stunned::<V>(sts2_core::StunResume::At(slash)),
                HookEffect::Remove,
            ]);
            return;
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::ASLEEP_POWER => None,
        // `BlurPower.AfterSideTurnStart`: one off, as its owner's side starts
        // a turn.
        (__c0, _, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::BLUR_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::BLUR_POWER => None,
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::FURNACE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Forge {
                card: V::Card::SOVEREIGN_BLADE,
                amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { .. })
            if __c0 == V::Power::FURNACE_POWER =>
        {
            unimplemented!("an endless Furnace")
        }
        // `DarkEmbracePower.AfterCardExhausted`: its stack of cards for each
        // of its owner's cards exhausted, and a count of those Ethereal
        // exhausted, which its `AfterSideTurnEnd` draws for once the hand is
        // flushed.
        (__c0, _, EventHook::CardExhausted { ethereal: true, .. })
            if __c0 == V::Power::DARK_EMBRACE_POWER && (owner == CombatSide::Player) =>
        {
            state.count(one);
            None
        }
        (__c0, PowerAmount::Counted(amount), EventHook::CardExhausted { .. })
            if __c0 == V::Power::DARK_EMBRACE_POWER && (owner == CombatSide::Player) =>
        {
            Some(HookEffect::Draw(amount))
        }
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::DARK_EMBRACE_POWER && (participants.contains(holder)) =>
        {
            let counted = state.counted();
            state.clear();
            Some(HookEffect::Draw(amount * counted))
        }
        // `JugglingPower.AfterApplied`: the attacks whose plays started this
        // turn, counted. A fight here has one player, whose every card is.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::JUGGLING_POWER => {
            let attacks = history
                .started_this_turn()
                .filter(|started| {
                    <V as sts2_core::Rules>::card_kind(started.card)
                        == Some(sts2_core::CardKind::Attack)
                })
                .count();
            state.clear();
            state.count(Number::try_from(attacks).expect("a turn's plays fit a number"));
            None
        }
        // `AfterCardPlayed`: one more attack, and on the third, its stack of
        // clones of that attack into hand.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { card, id, .. })
            if __c0 == V::Power::JUGGLING_POWER
                && (<V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Attack)) =>
        {
            state.count(one);
            (state.counted() == JUGGLING_ATTACKS).then_some(HookEffect::CloneIntoHand {
                card: id,
                count: amount,
                writes: &[],
            })
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { .. })
            if __c0 == V::Power::JUGGLING_POWER =>
        {
            unimplemented!("an endless Juggling")
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::JUGGLING_POWER => None,
        // `AfterSideTurnEnd` on its owner's side starts the count again.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::JUGGLING_POWER =>
        {
            if participants.contains(holder) {
                state.clear();
            }
            None
        }
        (__c0, PowerAmount::Endless, _) if __c0 == V::Power::DARK_EMBRACE_POWER => {
            unimplemented!("an endless Dark Embrace")
        }
        (__c0, _, EventHook::CardExhausted { .. } | EventHook::SideTurnEnd { .. })
            if __c0 == V::Power::DARK_EMBRACE_POWER =>
        {
            None
        }
        // `FeelNoPainPower.AfterCardExhausted`: its stack as block, unpowered,
        // for each of its owner's cards exhausted.
        (__c0, PowerAmount::Counted(amount), EventHook::CardExhausted { .. })
            if __c0 == V::Power::FEEL_NO_PAIN_POWER && (owner == CombatSide::Player) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardExhausted { .. })
            if __c0 == V::Power::FEEL_NO_PAIN_POWER =>
        {
            unimplemented!("an endless Feel No Pain")
        }
        (__c0, _, EventHook::CardExhausted { .. }) if __c0 == V::Power::FEEL_NO_PAIN_POWER => None,
        // `SlowPower.AfterCardPlayed` counts every card anyone plays, and its
        // `AfterSideTurnStart` forgets the count when the side its owner is
        // among takes its turn.
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::SLOW_POWER => {
            state.count(one);
            None
        }
        // `CurlUpPower.AfterDamageReceived`: a powered attack on its owner
        // from a card names that card, unless another is named already.
        (
            __c0,
            _,
            EventHook::DamageReceived {
                target,
                powered: true,
                card: Some(card),
                ..
            },
        ) if __c0 == V::Power::CURL_UP_POWER && (target == holder) => {
            let played = state.curl_played();
            if played.is_none_or(|named| named == card) {
                *played = Some(card);
            }
            None
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::CURL_UP_POWER => None,
        // `CurlUpPower.AfterCardPlayed`: once the named card's play is over,
        // `CreatureCmd.GainBlock` of its stack, unpowered, then
        // `PowerCmd.Remove(this)`.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { id, .. })
            if __c0 == V::Power::CURL_UP_POWER && (*state.curl_played() == Some(id)) =>
        {
            *state.curl_played() = None;
            Some(HookEffect::GainBlockAndRemove(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { id, .. })
            if __c0 == V::Power::CURL_UP_POWER && (*state.curl_played() == Some(id)) =>
        {
            unimplemented!("an endless Curl Up")
        }
        (__c0, _, EventHook::CardPlayed { .. }) if __c0 == V::Power::CURL_UP_POWER => None,
        (__c0, _, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::SLOW_POWER && (participants.contains(holder)) =>
        {
            state.clear();
            None
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::SLOW_POWER => None,
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target, unblocked, ..
            },
        ) if __c0 == V::Power::SLUMBER_POWER && (target == holder && unblocked >= one) => {
            // The last one: `CreatureCmd.Stun(owner, WakeUpMove,
            // "ROLL_OUT_MOVE")`. The stunned turn wakes it, which takes its
            // Plating off, and it goes on to Roll Out.
            if amount == one {
                let roll_out =
                    V::monster_state_of(V::Monster::SLUMBERING_BEETLE, V::MoveId::ROLL_OUT_MOVE, 0)
                        .unwrap_or_else(|_| unreachable!("the beetle's graph holds its Roll Out"));
                asked.extend([
                    HookEffect::Decrement,
                    stunned::<V>(sts2_core::StunResume::At(roll_out)),
                ]);
                return;
            }
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if __c0 == V::Power::SLUMBER_POWER && (target == holder) =>
        {
            unimplemented!("an endless Slumber power")
        }
        // `FlutterPower.AfterDamageReceived`: a powered attack that reaches
        // its owner's hit points spends one. The last one stuns its owner,
        // which then goes on past the move the stun interrupted.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::FLUTTER_POWER && (target == holder && unblocked != 0) => {
            if amount == one {
                asked.extend([
                    HookEffect::Decrement,
                    stunned::<V>(sts2_core::StunResume::AfterInterrupted),
                ]);
                return;
            }
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if __c0 == V::Power::FLUTTER_POWER && (target == holder) =>
        {
            unimplemented!("an endless Flutter")
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::FLUTTER_POWER => None,
        // `SlipperyPower.AfterDamageReceived`, which spends an instance on
        // every hit that reaches its owner's hit points.
        (
            __c0,
            _,
            EventHook::DamageReceived {
                target, unblocked, ..
            },
        ) if __c0 == V::Power::SLIPPERY_POWER && (target == holder && unblocked >= one) => {
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
            __c0,
            _,
            EventHook::PowerAmountChanged {
                creature,
                power: __c1,
                delta,
                ..
            },
        ) if (__c0 == V::Power::SWORD_SAGE_POWER && __c1 == V::Power::SWORD_SAGE_POWER)
            && (creature == holder) =>
        {
            Some(HookEffect::AddReplays {
                to: sts2_core::Writing::EveryCopyOf(V::Card::SOVEREIGN_BLADE),
                amount: delta,
            })
        }
        // `SwordSagePower.AfterCardEnteredCombat`, which gives a blade that
        // joins later the replays the ones already here were given. A clone
        // copied its original's, and is given none.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::CardEnteredCombat {
                at,
                card: __c1,
                clone: false,
                ..
            },
        ) if (__c0 == V::Power::SWORD_SAGE_POWER && __c1 == V::Card::SOVEREIGN_BLADE) => {
            Some(HookEffect::AddReplays {
                to: sts2_core::Writing::One(at),
                amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::CardEnteredCombat { .. })
            if __c0 == V::Power::SWORD_SAGE_POWER =>
        {
            unimplemented!("an endless Sword Sage")
        }
        // `PlatingPower.AfterSideTurnStart` on a monster: its `Decrement` off
        // each of its side's turns once the first round is over, by
        // `PowerCmd.ModifyAmount`. `AfterApplied` set that to the run's
        // player count as it landed on the enemy.
        (
            __c0,
            PowerAmount::Counted(_),
            EventHook::SideTurnStart {
                participants,
                round,
                players,
                ..
            },
        ) if __c0 == V::Power::PLATING_POWER
            && (owner == CombatSide::Enemy
                && participants.contains(holder)
                && round > <V as sts2_core::ContentVersion>::Count::from(1u8)) =>
        {
            Some(HookEffect::ModifyAmount(-Number::from(players)))
        }
        // `PlatingPower.BeforeSideTurnStart`: a monster that opens the fight
        // with Plating opens it behind that much block. It answers each
        // player turn of the first round, an extra one among them, and only
        // on a monster.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnStart { side, round, .. },
        ) if __c0 == V::Power::PLATING_POWER
            && (side == CombatSide::Player
                && owner == CombatSide::Enemy
                && round <= <V as sts2_core::ContentVersion>::Count::from(1u8)) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnEndEarly { participants, .. },
        ) if __c0 == V::Power::PLATING_POWER && (participants.contains(holder)) => {
            Some(HookEffect::GainBlock(amount))
        }
        // `PlatingPower.AfterSideTurnStart` on a player: one comes off each
        // of its turns but the first, by `PowerCmd.Decrement`.
        (
            __c0,
            PowerAmount::Counted(_),
            EventHook::SideTurnStart {
                participants, turn, ..
            },
        ) if __c0 == V::Power::PLATING_POWER
            && (owner != CombatSide::Enemy
                && participants.contains(holder)
                && turn > <V as sts2_core::ContentVersion>::Count::from(1u8)) =>
        {
            Some(HookEffect::Decrement)
        }
        // `HeistPower.BeforeDeath`: a gold reward of its stack, the gold the
        // Gremlin Merc stole, as its owner dies.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeDeath { creature })
            if __c0 == V::Power::HEIST_POWER && (creature == holder) =>
        {
            Some(HookEffect::AddGoldReward(amount))
        }
        (__c0, _, EventHook::BeforeDeath { .. }) if __c0 == V::Power::HEIST_POWER => None,
        // `SurprisePower.AfterDeath`: the gremlins its owner leaves behind,
        // which the fight carries out.
        (
            __c0,
            _,
            EventHook::AfterDeath {
                creature,
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Power::SURPRISE_POWER && (creature == holder) => {
            Some(HookEffect::PerformDeath(V::Power::SURPRISE_POWER))
        }
        (__c0, _, EventHook::AfterDeath { .. }) if __c0 == V::Power::SURPRISE_POWER => None,
        // `SwipePower.BeforeDeath`, which hands the stolen card back as its
        // owner dies.
        (__c0, _, EventHook::BeforeDeath { creature })
            if __c0 == V::Power::SWIPE_POWER && (creature == holder) =>
        {
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
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target, unblocked, ..
            },
        ) if __c0 == V::Power::PLOW_POWER
            && (target == holder
                && unblocked > 0
                && fight
                    .current_hp(holder)
                    .is_some_and(|hp| Number::from(hp) <= amount)) =>
        {
            asked.extend(
                <<V as sts2_core::ContentVersion>::Power as sts2_core::Table>::ALL
                    .iter()
                    .copied()
                    .filter(|&lent| {
                        lends::<V>(lent).is_some_and(|(to, _)| to == V::Power::STRENGTH_POWER)
                            && fight.power(holder, lent).is_some()
                    })
                    .map(HookEffect::RemoveFromOwner),
            );
            asked.push(HookEffect::RemoveFromOwner(V::Power::STRENGTH_POWER));
            asked.push(match monster {
                Some(__c0) if __c0 == V::Monster::CEREMONIAL_BEAST => {
                    let cry = V::monster_state_of(
                        V::Monster::CEREMONIAL_BEAST,
                        V::MoveId::BEAST_CRY_MOVE,
                        0,
                    )
                    .unwrap_or_else(|_| unreachable!("the Ceremonial Beast's graph holds its cry"));
                    stunned::<V>(sts2_core::StunResume::At(cry))
                }
                _ => stunned::<V>(sts2_core::StunResume::Interrupted),
            });
            asked.push(HookEffect::RemoveFromOwner(V::Power::PLOW_POWER));
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if __c0 == V::Power::PLOW_POWER && (target == holder) =>
        {
            unimplemented!("an endless Plow")
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::PLOW_POWER => None,
        // `ShriekPower.AfterDamageReceived`: a hit that takes health and
        // leaves the Terror Eel at its amount or below stuns it. It comes back
        // on its Terror, and the power goes.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target, unblocked, ..
            },
        ) if __c0 == V::Power::SHRIEK_POWER
            && (target == holder
                && unblocked > 0
                && fight
                    .current_hp(holder)
                    .is_some_and(|hp| Number::from(hp) <= amount)) =>
        {
            let terror = V::monster_state_of(V::Monster::TERROR_EEL, V::MoveId::TERROR_MOVE, 0)
                .unwrap_or_else(|_| unreachable!("the Terror Eel's graph holds its Terror"));
            asked.extend([
                stunned::<V>(sts2_core::StunResume::At(terror)),
                HookEffect::Remove,
            ]);
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if __c0 == V::Power::SHRIEK_POWER && (target == holder) =>
        {
            unimplemented!("an endless Shriek")
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::SHRIEK_POWER => None,
        // `HardenedShellPower.AfterDamageReceived`: what a hit took off its
        // owner counts toward the turn's cap.
        (
            __c0,
            _,
            EventHook::DamageReceived {
                target, unblocked, ..
            },
        ) if __c0 == V::Power::HARDENED_SHELL_POWER && (target == holder) => {
            let State::HardenedShell { received } = state else {
                unreachable!("Hardened Shell holding {state:?}")
            };
            *received += unblocked;
            None
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::HARDENED_SHELL_POWER => {
            None
        }
        // `HardenedShellPower.BeforeSideTurnStart`, for either side: the cap
        // starts again.
        (__c0, _, EventHook::BeforeSideTurnStart { .. })
            if __c0 == V::Power::HARDENED_SHELL_POWER =>
        {
            let State::HardenedShell { received } = state else {
                unreachable!("Hardened Shell holding {state:?}")
            };
            *received = 0;
            None
        }
        // `BurrowedPower.AfterBlockBroken`: a Tunneler whose block breaks is
        // stunned, comes back with a bite, and stops burrowing.
        (__c0, _, EventHook::BlockBroken { creature, .. })
            if __c0 == V::Power::BURROWED_POWER
                && (creature == holder && monster == Some(V::Monster::TUNNELER)) =>
        {
            let bite = V::monster_state_of(V::Monster::TUNNELER, V::MoveId::BITE_MOVE, 0)
                .unwrap_or_else(|_| unreachable!("the Tunneler's graph holds its bite"));
            asked.extend([
                stunned::<V>(sts2_core::StunResume::At(bite)),
                HookEffect::RemoveFromOwner(V::Power::BURROWED_POWER),
            ]);
            return;
        }
        // `BurrowedPower.AfterRemoved`, which takes all of its owner's block.
        (__c0, _, EventHook::Removed) if __c0 == V::Power::BURROWED_POWER => {
            Some(HookEffect::LoseBlock)
        }
        // `IllusionPower.AfterCombatEnd` plays the dead owner's animation,
        // which is all it does.
        (__c0, _, EventHook::CombatEnd { .. }) if __c0 == V::Power::ILLUSION_POWER => None,
        // `VigorPower.BeforeAttack`, which binds to the first powered attack
        // its owner makes and keeps that one.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::BeforeAttack {
                command,
                attacker,
                source,
                powered: true,
            },
        ) if __c0 == V::Power::VIGOR_POWER && (attacker == holder && state.bound().is_none()) => {
            *state = State::Vigor {
                bound: Some(Bound {
                    command,
                    source: source.map(|(id, ..)| id),
                    amount,
                }),
            };
            None
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeAttack { attacker, .. })
            if __c0 == V::Power::VIGOR_POWER && (attacker == holder) =>
        {
            unimplemented!("an endless Vigor")
        }
        (__c0, _, EventHook::BeforeAttack { .. }) if __c0 == V::Power::VIGOR_POWER => None,
        // `GigantificationPower.BeforeAttack`: binds to the first powered
        // attack an Attack card of its owner's makes, and keeps that one.
        (
            __c0,
            PowerAmount::Counted(_),
            EventHook::BeforeAttack {
                command,
                source: Some((id, card, owner)),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::GIGANTIFICATION_POWER
            && owner == holder
            && (state.giant().is_none()
                && <V as sts2_core::Rules>::card_kind(card)
                    == Some(sts2_core::CardKind::Attack)) =>
        {
            *state = State::Gigantification {
                bound: Some((command, id)),
            };
            None
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeAttack { .. })
            if __c0 == V::Power::GIGANTIFICATION_POWER =>
        {
            unimplemented!("an endless Gigantification")
        }
        (__c0, _, EventHook::BeforeAttack { .. }) if __c0 == V::Power::GIGANTIFICATION_POWER => {
            None
        }
        // `GigantificationPower.AfterAttack`: once the command it bound is
        // over, it lets go and takes one off its stack.
        (__c0, _, EventHook::AfterAttack { command, .. })
            if __c0 == V::Power::GIGANTIFICATION_POWER =>
        {
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
        (__c0, _, EventHook::AfterAttack { command, .. }) if __c0 == V::Power::VIGOR_POWER => state
            .bound()
            .filter(|bound| bound.command == command)
            .map(|bound| HookEffect::ModifyAmount(-bound.amount)),
        // `FlameBarrierPower.AfterDamageReceived`: a powered attack on its
        // owner takes its stack back off the attacker.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceived {
                target,
                dealer: Some(dealer),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::FLAME_BARRIER_POWER && (target == holder) => {
            Some(HookEffect::Damage {
                target: dealer,
                amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceived { target, .. })
            if __c0 == V::Power::FLAME_BARRIER_POWER && (target == holder) =>
        {
            unimplemented!("an endless Flame Barrier")
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::FLAME_BARRIER_POWER => {
            None
        }
        // `ThornsPower.BeforeDamageReceived`: its stack at whoever hits its
        // owner with a powered attack or an Omnislice, unpowered, before the
        // hit meets block.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageReceiving {
                target,
                dealer: Some(dealer),
                powered,
                card,
                ..
            },
        ) if __c0 == V::Power::THORNS_POWER
            && (target == holder && (powered || card == Some(V::Card::OMNISLICE))) =>
        {
            Some(HookEffect::Damage {
                target: dealer,
                amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::DamageReceiving { target, .. })
            if __c0 == V::Power::THORNS_POWER && (target == holder) =>
        {
            unimplemented!("an endless Thorns")
        }
        (__c0, _, EventHook::DamageReceiving { .. }) if __c0 == V::Power::THORNS_POWER => None,
        // `AutomationPower.AfterCardDrawn`: its stack of energy each tenth
        // card its owner draws.
        (__c0, PowerAmount::Counted(amount), EventHook::CardDrawn { .. })
            if __c0 == V::Power::AUTOMATION_POWER =>
        {
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
        (__c0, PowerAmount::Endless, EventHook::CardDrawn { .. })
            if __c0 == V::Power::AUTOMATION_POWER =>
        {
            unimplemented!("an endless Automation")
        }
        // `PanachePower.AfterCardPlayed`: each play after the one that applied
        // it counts down, and the fifth hits every enemy for its stack,
        // unpowered.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlayed { .. })
            if __c0 == V::Power::PANACHE_POWER =>
        {
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
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::PANACHE_POWER =>
        {
            if participants.contains(holder) {
                let State::Panache { cards_left, .. } = state else {
                    unreachable!("cards left for {:?}", state.power())
                };
                *cards_left = PANACHE_CARDS;
            }
            None
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlayed { .. })
            if __c0 == V::Power::PANACHE_POWER =>
        {
            unimplemented!("an endless Panache")
        }
        // `StratagemPower.AfterShuffle`: `FromCombatPile` over the draw pile
        // for its stack of cards, each into hand, after its owner's shuffle.
        (__c0, _, EventHook::Shuffled { player })
            if __c0 == V::Power::STRATAGEM_POWER && player != holder =>
        {
            None
        }
        (__c0, PowerAmount::Counted(amount), EventHook::Shuffled { .. })
            if __c0 == V::Power::STRATAGEM_POWER =>
        {
            Some(HookEffect::SelectFromPile {
                pile: sts2_core::Pile::Draw,
                selection: exactly(amount),
                act: sts2_core::PileAct::IntoHand,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::Shuffled { .. })
            if __c0 == V::Power::STRATAGEM_POWER =>
        {
            unimplemented!("an endless Stratagem")
        }
        // `ReflectPower.AfterDamageReceived`: what the block took off a
        // powered attack on its owner goes back at the attacker, unpowered.
        (
            __c0,
            _,
            EventHook::DamageReceived {
                target,
                blocked,
                dealer: Some(dealer),
                powered: true,
                ..
            },
        ) if __c0 == V::Power::REFLECT_POWER
            && (target == holder && blocked > num_traits::zero()) =>
        {
            Some(HookEffect::Damage {
                target: dealer,
                amount: blocked,
            })
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::REFLECT_POWER => None,
        // `ReflectPower.AfterSideTurnStart`: one off its stack as its owner's
        // side's turn starts.
        (__c0, PowerAmount::Counted(_), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::REFLECT_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::REFLECT_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Reflect")
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::REFLECT_POWER => None,
        // `ClarityPower.AfterSideTurnStart`: one off its stack as its owner's
        // side's turn starts.
        (__c0, PowerAmount::Counted(_), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::CLARITY_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::CLARITY_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Clarity")
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::CLARITY_POWER => None,
        // `FlameBarrierPower.AfterSideTurnEnd`, which goes once the other side
        // has had its turn.
        (__c0, _, EventHook::SideTurnEnd { side, .. })
            if __c0 == V::Power::FLAME_BARRIER_POWER && (side != owner) =>
        {
            Some(HookEffect::Remove)
        }
        // `DuplicationPower.AfterModifyingCardPlayCount`, and its
        // `AfterSideTurnEnd` for its owner's own side, the same as One-Two
        // Punch's.
        (__c0, _, EventHook::PlayCountModified { .. }) if __c0 == V::Power::DUPLICATION_POWER => {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::DUPLICATION_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::DUPLICATION_POWER => None,
        (__c0, _, EventHook::PlayCountModified { .. })
            if (__c0 == V::Power::BURST_POWER || __c0 == V::Power::SIGNAL_BOOST_POWER) =>
        {
            Some(HookEffect::Decrement)
        }
        // `EchoFormPower.AfterModifyingCardPlayCount` does nothing.
        (__c0, _, EventHook::PlayCountModified { .. }) if __c0 == V::Power::ECHO_FORM_POWER => None,
        // v0.111.0's `EchoFormPower.AfterCardPlayed` and
        // `BeforeSideTurnStart`, which turn its look off and on alone.
        (__c0, _, EventHook::CardPlayed { .. } | EventHook::BeforeSideTurnStart { .. })
            if __c0 == V::Power::ECHO_FORM_POWER =>
        {
            None
        }
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if (__c0 == V::Power::BURST_POWER
                || __c0 == V::Power::SHADOWMELD_POWER
                || __c0 == V::Power::CORROSIVE_WAVE_POWER)
                && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. })
            if (__c0 == V::Power::BURST_POWER
                || __c0 == V::Power::SHADOWMELD_POWER
                || __c0 == V::Power::CORROSIVE_WAVE_POWER) =>
        {
            None
        }
        // `NoBlockPower.AfterSideTurnEnd`: one off as the enemies' turn
        // ends, whoever holds it.
        (
            __c0,
            _,
            EventHook::SideTurnEnd {
                side: CombatSide::Enemy,
                ..
            },
        ) if __c0 == V::Power::NO_BLOCK_POWER => Some(HookEffect::Decrement),
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::NO_BLOCK_POWER => None,
        // `CorrosiveWavePower.AfterCardDrawn`: its stack of Poison onto every
        // enemy that can be hit, for each of its owner's cards drawn.
        (__c0, amount, EventHook::CardDrawn { .. })
            if __c0 == V::Power::CORROSIVE_WAVE_POWER && (owner == CombatSide::Player) =>
        {
            Some(HookEffect::ApplyToOpponents(V::Power::POISON_POWER, amount))
        }
        (__c0, _, EventHook::CardDrawn { .. }) if __c0 == V::Power::CORROSIVE_WAVE_POWER => None,
        // `ConfusedPower.AfterCardDrawn`: a cost drawn below four for each of
        // its owner's cards drawn, unless the card's cost is negative. An X
        // cost counts as zero, so it draws and keeps spending its X.
        (__c0, _, EventHook::CardDrawn { card, id, .. })
            if __c0 == V::Power::CONFUSED_POWER
                && (owner == CombatSide::Player && card.cost() != sts2_core::Cost::None) =>
        {
            Some(HookEffect::RandomCost {
                card: id,
                below: CONFUSED_COSTS,
            })
        }
        (__c0, _, EventHook::CardDrawn { .. }) if __c0 == V::Power::CONFUSED_POWER => None,
        // `SpeedsterPower.AfterCardDrawn`: its stack to every enemy that can
        // be hit, unpowered, for a card of its owner's drawn outside the hand
        // draw and in its owner's own turn.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::CardDrawn {
                hand_draw: false,
                side,
                ..
            },
        ) if __c0 == V::Power::SPEEDSTER_POWER
            && (owner == CombatSide::Player && side == owner) =>
        {
            Some(HookEffect::DamageOpponents(amount))
        }
        (
            __c0,
            PowerAmount::Endless,
            EventHook::CardDrawn {
                hand_draw: false,
                side,
                ..
            },
        ) if __c0 == V::Power::SPEEDSTER_POWER
            && (owner == CombatSide::Player && side == owner) =>
        {
            unimplemented!("an endless Speedster")
        }
        (__c0, _, EventHook::CardDrawn { .. }) if __c0 == V::Power::SPEEDSTER_POWER => None,
        // `PrepTimePower.AfterSideTurnStart`: its stack of Vigor onto its
        // owner, as its owner's side starts a turn.
        (__c0, amount, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::PREP_TIME_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOwner(V::Power::VIGOR_POWER, amount))
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::PREP_TIME_POWER => None,
        // `DemonFormPower.AfterSideTurnStart`: its stack of Strength onto its
        // owner, as its owner's side starts a turn.
        (__c0, amount, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::DEMON_FORM_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, amount))
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::DEMON_FORM_POWER => None,
        // v0.111.0's forms' `AfterApplied` and `AfterRemoved`, which make and
        // put away their look alone.
        (__c0, _, EventHook::Applied | EventHook::Removed)
            if [
                V::Power::DEMON_FORM_POWER,
                V::Power::ECHO_FORM_POWER,
                V::Power::REAPER_FORM_POWER,
                V::Power::SERPENT_FORM_POWER,
                V::Power::VOID_FORM_POWER,
            ]
            .contains(&__c0) =>
        {
            None
        }
        // `SpiritOfAshPower.BeforeCardPlayed`: its stack as block, unpowered,
        // before each of its owner's Ethereal cards is played.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { ethereal: true, .. })
            if __c0 == V::Power::SPIRIT_OF_ASH_POWER && (owner == CombatSide::Player) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { ethereal: true, .. })
            if __c0 == V::Power::SPIRIT_OF_ASH_POWER && (owner == CombatSide::Player) =>
        {
            unimplemented!("an endless Spirit of Ash")
        }
        (__c0, _, EventHook::CardPlaying { .. }) if __c0 == V::Power::SPIRIT_OF_ASH_POWER => None,
        // `DanseMacabrePower.BeforeCardPlayed`: its stack as block, unpowered,
        // before each of its owner's cards resolved at two or more.
        (__c0, PowerAmount::Counted(amount), EventHook::CardPlaying { resolved, .. })
            if __c0 == V::Power::DANSE_MACABRE_POWER
                && (owner == CombatSide::Player
                    && resolved >= V::Power::DANSE_MACABRE_POWER_ENERGY) =>
        {
            Some(HookEffect::GainBlock(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardPlaying { resolved, .. })
            if __c0 == V::Power::DANSE_MACABRE_POWER
                && (owner == CombatSide::Player
                    && resolved >= V::Power::DANSE_MACABRE_POWER_ENERGY) =>
        {
            unimplemented!("an endless Danse Macabre")
        }
        (__c0, _, EventHook::CardPlaying { .. }) if __c0 == V::Power::DANSE_MACABRE_POWER => None,
        // `TheGambitPower.AfterDamageReceived`: a powered hit that takes hit
        // points from its owner removes it, then kills the owner.
        (
            __c0,
            _,
            EventHook::DamageReceived {
                target,
                unblocked,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::THE_GAMBIT_POWER && (target == holder && unblocked > 0) => {
            asked.extend([HookEffect::Remove, HookEffect::KillOwner]);
            return;
        }
        (__c0, _, EventHook::DamageReceived { .. }) if __c0 == V::Power::THE_GAMBIT_POWER => None,
        // `MonarchsGazePower.AfterDamageGiven`: its stack of Monarch's Gaze
        // Strength Down onto whoever a powered hit from its owner lands on.
        (
            __c0,
            amount,
            EventHook::DamageGiven {
                dealer: Some(dealer),
                target,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::MONARCHS_GAZE_POWER && (dealer == holder) => {
            Some(HookEffect::ApplyPower {
                target,
                power: V::Power::MONARCHS_GAZE_STRENGTH_DOWN_POWER,
                amount,
                applier: Some(holder),
            })
        }
        (__c0, _, EventHook::DamageGiven { .. }) if __c0 == V::Power::MONARCHS_GAZE_POWER => None,
        // `RegenPower.AfterSideTurnEnd`: its stack healed, then one off, as
        // its owner's side ends a turn.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::REGEN_POWER && (participants.contains(holder)) =>
        {
            asked.extend([HookEffect::Heal(amount), HookEffect::Decrement]);
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::REGEN_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Regen")
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::REGEN_POWER => None,
        // v0.111.0's `RegenPower.BeforeSideTurnEndEarly`, which does the same
        // earlier in the turn's end. Each build's class overrides one of the
        // two, so it hears only that one.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnEndEarly { participants, .. },
        ) if __c0 == V::Power::REGEN_POWER && (participants.contains(holder)) => {
            asked.extend([HookEffect::Heal(amount), HookEffect::Decrement]);
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeSideTurnEndEarly { participants, .. })
            if __c0 == V::Power::REGEN_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Regen")
        }
        (__c0, _, EventHook::BeforeSideTurnEndEarly { .. }) if __c0 == V::Power::REGEN_POWER => {
            None
        }
        // `RitualPower.AfterSideTurnEnd`: its stack of Strength as its
        // owner's side ends a turn. A monster's skips the turn it was applied
        // in, through `AfterApplied`.
        (__c0, amount, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::RITUAL_POWER && (participants.contains(holder)) =>
        {
            match state.swap_just_applied(false) {
                true => None,
                false => Some(HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, amount)),
            }
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::RITUAL_POWER => None,
        // `RitualPower.AfterApplied`, which marks a monster's alone.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::RITUAL_POWER => {
            if owner != CombatSide::Player {
                state.swap_just_applied(true);
            }
            None
        }
        // `RadiancePower.AfterEnergyReset`: its `Energy`, then one off.
        (__c0, PowerAmount::Counted(_), EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::RADIANCE_POWER && (player == holder) =>
        {
            asked.extend([
                HookEffect::GainEnergy(V::Power::RADIANCE_POWER_ENERGY),
                HookEffect::Decrement,
            ]);
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::RADIANCE_POWER && (player == holder) =>
        {
            unimplemented!("an endless Radiance")
        }
        (__c0, _, EventHook::EnergyReset { .. }) if __c0 == V::Power::RADIANCE_POWER => None,
        // `OneTwoPunchPower.AfterModifyingCardPlayCount`, and its
        // `AfterSideTurnEnd` for its owner's own side.
        (__c0, _, EventHook::PlayCountModified { .. }) if __c0 == V::Power::ONE_TWO_PUNCH_POWER => {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::ONE_TWO_PUNCH_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. })
            if (__c0 == V::Power::FLAME_BARRIER_POWER || __c0 == V::Power::ONE_TWO_PUNCH_POWER) =>
        {
            None
        }
        // `AggressionPower.BeforeSideTurnStart`, for its owner's own side.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::BeforeSideTurnStart { participants, .. },
        ) if __c0 == V::Power::AGGRESSION_POWER && (participants.contains(holder)) => {
            Some(HookEffect::AttacksFromDiscard(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeSideTurnStart { participants, .. })
            if __c0 == V::Power::AGGRESSION_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Aggression")
        }
        (__c0, _, EventHook::BeforeSideTurnStart { .. }) if __c0 == V::Power::AGGRESSION_POWER => {
            None
        }
        // `DoomPower.BeforeSideTurnEnd`, for its owner's own side. A class
        // that also overrides `AfterSideTurnEnd`, as v0.111.0's does, leaves
        // the player's side to that.
        (
            __c0,
            _,
            EventHook::BeforeSideTurnEnd {
                side, participants, ..
            },
        ) if __c0 == V::Power::DOOM_POWER => {
            use sts2_core::PowerTable as _;
            let after = power
                .overrides()
                .contains(sts2_core::ModelHook::AfterSideTurnEnd);
            match participants.contains(holder) && !(after && side == CombatSide::Player) {
                true => Some(HookEffect::DoomKill),
                false => None,
            }
        }
        // v0.111.0's `DoomPower.AfterSideTurnEnd`, for its owner's own side
        // when that is not the enemies'.
        (
            __c0,
            _,
            EventHook::SideTurnEnd {
                side, participants, ..
            },
        ) if __c0 == V::Power::DOOM_POWER => {
            match participants.contains(holder) && side != CombatSide::Enemy {
                true => Some(HookEffect::DoomKill),
                false => None,
            }
        }
        // `HailstormPower.BeforeSideTurnEnd`: its stack of damage to every enemy
        // while its owner holds a Frost orb.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeSideTurnEnd { participants, .. })
            if __c0 == V::Power::HAILSTORM_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::DamageOpponentsHolding {
                orb: V::Orb::FROST_ORB,
                amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeSideTurnEnd { participants, .. })
            if __c0 == V::Power::HAILSTORM_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Hailstorm")
        }
        (__c0, _, EventHook::BeforeSideTurnEnd { .. }) if __c0 == V::Power::HAILSTORM_POWER => None,
        // `NeurosurgePower.AfterSideTurnStart`: its stack of Doom on its own
        // owner, each of its owner's turns.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::NEUROSURGE_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToOwner(
                V::Power::DOOM_POWER,
                PowerAmount::Counted(amount),
            ))
        }
        // `CountdownPower.AfterSideTurnStart`: its stack of Doom on one enemy
        // that can be hit.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::COUNTDOWN_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::ApplyToRandomOpponent {
                power: V::Power::DOOM_POWER,
                amount: PowerAmount::Counted(amount),
            })
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { participants, .. })
            if (__c0 == V::Power::NEUROSURGE_POWER || __c0 == V::Power::COUNTDOWN_POWER)
                && (participants.contains(holder)) =>
        {
            unimplemented!("an endless {power:?}")
        }
        (__c0, _, EventHook::SideTurnStart { .. })
            if (__c0 == V::Power::NEUROSURGE_POWER || __c0 == V::Power::COUNTDOWN_POWER) =>
        {
            None
        }
        // `ReaperFormPower.AfterDamageGiven`: a powered hit from its owner, or
        // from its owner's pet, puts Doom on what it hit, its stack for each
        // point the hit dealt.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::DamageGiven {
                dealer,
                pet_of,
                target,
                total,
                powered: true,
                ..
            },
        ) if __c0 == V::Power::REAPER_FORM_POWER
            && ((dealer == Some(holder) || pet_of == Some(holder)) && total > 0) =>
        {
            Some(HookEffect::ApplyPower {
                target,
                power: V::Power::DOOM_POWER,
                amount: PowerAmount::Counted(total * amount),
                applier: Some(holder),
            })
        }
        (__c0, PowerAmount::Endless, EventHook::DamageGiven { .. })
            if __c0 == V::Power::REAPER_FORM_POWER =>
        {
            unimplemented!("an endless Reaper Form")
        }
        (__c0, _, EventHook::DamageGiven { .. }) if __c0 == V::Power::REAPER_FORM_POWER => None,
        // `BufferPower.AfterModifyingHpLostAfterOsty`: one spent on each hit it
        // took to nothing.
        (__c0, _, EventHook::HpLostModified) if __c0 == V::Power::BUFFER_POWER => {
            Some(HookEffect::Decrement)
        }
        // `IntangiblePower.AfterModifyingHpLostAfterOsty` flashes, which is
        // all it does.
        (__c0, _, EventHook::HpLostModified) if __c0 == V::Power::INTANGIBLE_POWER => None,
        // `IterationPower.AfterCardDrawn`: the first Status card its owner
        // draws in a turn draws its stack more. The draw is in the history
        // before the hook, so the first is a count of one. A fight here has
        // one player, so every draw is its owner's.
        (__c0, PowerAmount::Counted(amount), EventHook::CardDrawn { card, .. })
            if __c0 == V::Power::ITERATION_POWER && (status::<V>(card)) =>
        {
            match history
                .drawn_this_turn()
                .filter(|drawn| status::<V>(*drawn))
                .count()
                <= 1
            {
                true => Some(HookEffect::Draw(amount)),
                false => None,
            }
        }
        (__c0, PowerAmount::Endless, EventHook::CardDrawn { card, .. })
            if __c0 == V::Power::ITERATION_POWER && (status::<V>(card)) =>
        {
            unimplemented!("an endless Iteration")
        }
        (__c0, _, EventHook::CardDrawn { .. }) if __c0 == V::Power::ITERATION_POWER => None,
        // `FeralPower.AfterApplied`: the zero-cost attacks its owner started
        // this turn count against it from the start.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::FERAL_POWER => {
            let played = history
                .started_this_turn()
                .filter(|started| started.energy == 0 && attack::<V>(started.card))
                .count();
            state.set_feral_played(num_traits::cast(played).expect("a turn's plays fit a number"));
            None
        }
        // `FeralPower.AfterModifyingCardPlayResultPileOrPosition` counts the
        // one it sent back, and `AfterSideTurnStart` for its owner's side
        // forgets the count.
        // `NostalgiaPower.AfterModifyingCardPlayResultPileOrPosition` only
        // flashes.
        (__c0, _, EventHook::ResultPileModified { .. }) if __c0 == V::Power::NOSTALGIA_POWER => {
            None
        }
        (__c0, _, EventHook::ResultPileModified { .. }) if __c0 == V::Power::FERAL_POWER => {
            state.set_feral_played(state.feral_played() + one);
            None
        }
        (__c0, _, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::FERAL_POWER =>
        {
            if participants.contains(holder) {
                state.set_feral_played(0);
            }
            None
        }
        // `CoolantPower.AfterSideTurnStart`: its stack of block for each kind
        // of orb its owner holds.
        (__c0, PowerAmount::Counted(amount), EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::COOLANT_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::GainBlockPerOrbKind(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::COOLANT_POWER && (participants.contains(holder)) =>
        {
            unimplemented!("an endless Coolant")
        }
        (__c0, _, EventHook::SideTurnStart { .. }) if __c0 == V::Power::COOLANT_POWER => None,
        // `LoopPower.AfterPlayerTurnStart`: the first orb's passive, its stack
        // of times.
        (__c0, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::LOOP_POWER && (player == holder) =>
        {
            Some(HookEffect::FirstOrbPassive(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::LOOP_POWER && (player == holder) =>
        {
            unimplemented!("an endless Loop")
        }
        (__c0, _, EventHook::PlayerTurnStart { .. }) if __c0 == V::Power::LOOP_POWER => None,
        // `HelloWorldPower.BeforeHandDraw`: as many distinct Common cards of
        // the character's pool as its stack stood at as the turn began, into
        // hand.
        (__c0, _, EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::HELLO_WORLD_POWER && (player == holder) =>
        {
            match held.on_turn_start {
                PowerAmount::Counted(count) if count >= 1 => {
                    Some(HookEffect::GenerateDistinctFromPool {
                        filter: sts2_core::PoolFilter::Common,
                        count,
                    })
                }
                PowerAmount::Counted(_) => None,
                PowerAmount::Endless => unimplemented!("an endless Hello World"),
            }
        }
        (__c0, _, EventHook::BeforeHandDraw { .. }) if __c0 == V::Power::HELLO_WORLD_POWER => None,
        // `ReboundPower.AfterModifyingCardPlayResultPileOrPosition`: one off
        // its stack for each card it moved. It goes once its owner's side
        // ends a turn.
        (__c0, _, EventHook::ResultPileModified { .. }) if __c0 == V::Power::REBOUND_POWER => {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::REBOUND_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::REBOUND_POWER => None,
        // `CreativeAiPower.BeforeHandDraw`: its stack of Power cards from the
        // character's pool, one at a time.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::CREATIVE_AI_POWER && (player == holder) =>
        {
            Some(HookEffect::GenerateFromPool {
                filter: sts2_core::PoolFilter::OfKind(sts2_core::CardKind::Power),
                count: amount,
                writes: &[],
            })
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::CREATIVE_AI_POWER && (player == holder) =>
        {
            unimplemented!("an endless Creative AI")
        }
        (__c0, _, EventHook::BeforeHandDraw { .. }) if __c0 == V::Power::CREATIVE_AI_POWER => None,
        // `SpectrumShiftPower.BeforeHandDraw`: its stack of distinct
        // colorless cards.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::SPECTRUM_SHIFT_POWER && (player == holder) =>
        {
            Some(HookEffect::GenerateDistinctColorless { count: amount })
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::SPECTRUM_SHIFT_POWER && (player == holder) =>
        {
            unimplemented!("an endless Spectrum Shift")
        }
        (__c0, _, EventHook::BeforeHandDraw { .. }) if __c0 == V::Power::SPECTRUM_SHIFT_POWER => {
            None
        }
        // `CallOfTheVoidPower.BeforeHandDraw`: its stack of cards from the
        // character's pool, none of them Basic or Ancient, each made Ethereal.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::CALL_OF_THE_VOID_POWER && (player == holder) =>
        {
            Some(HookEffect::GenerateFromPool {
                filter: sts2_core::PoolFilter::NotBasicOrAncient,
                count: amount,
                writes: &[(
                    sts2_core::Change::AddKeyword(CardKeyword::Ethereal),
                    sts2_core::Lasting::ThisFight,
                )],
            })
        }
        // `SentryModePower.BeforeHandDraw`: its stack of Sweeping Gazes.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::SENTRY_MODE_POWER && (player == holder) =>
        {
            Some(HookEffect::GenerateInHand {
                card: V::Card::SWEEPING_GAZE,
                count: amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
            if (__c0 == V::Power::CALL_OF_THE_VOID_POWER
                || __c0 == V::Power::SENTRY_MODE_POWER)
                && (player == holder) =>
        {
            unimplemented!("an endless {power:?}")
        }
        (__c0, _, EventHook::BeforeHandDraw { .. })
            if (__c0 == V::Power::CALL_OF_THE_VOID_POWER
                || __c0 == V::Power::SENTRY_MODE_POWER) =>
        {
            None
        }
        // `PagestormPower.AfterCardDrawn`: an Ethereal card its owner draws
        // draws its stack more.
        (__c0, PowerAmount::Counted(amount), EventHook::CardDrawn { ethereal: true, .. })
            if __c0 == V::Power::PAGESTORM_POWER =>
        {
            Some(HookEffect::Draw(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CardDrawn { ethereal: true, .. })
            if __c0 == V::Power::PAGESTORM_POWER =>
        {
            unimplemented!("an endless Pagestorm")
        }
        (__c0, _, EventHook::CardDrawn { .. }) if __c0 == V::Power::PAGESTORM_POWER => None,
        // `SummonNextTurnPower.AfterPlayerTurnStart`: once the stack was there
        // as the turn began, Osty is summoned for it, and the power goes.
        (__c0, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::SUMMON_NEXT_TURN_POWER
                && (player == holder && held.on_turn_start != PowerAmount::Counted(0)) =>
        {
            asked.extend([HookEffect::Summon(amount), HookEffect::Remove]);
            return;
        }
        (__c0, _, EventHook::PlayerTurnStart { .. })
            if __c0 == V::Power::SUMMON_NEXT_TURN_POWER =>
        {
            None
        }
        // `GenesisPower.AfterEnergyReset`: its stack of stars, every turn.
        (__c0, PowerAmount::Counted(amount), EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::GENESIS_POWER && (player == holder) =>
        {
            Some(HookEffect::GainStars(amount))
        }
        // `LightningRodPower.AfterEnergyReset`: a Lightning orb for its owner,
        // then one off the stack.
        (__c0, PowerAmount::Counted(_), EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::LIGHTNING_ROD_POWER && (player == holder) =>
        {
            asked.extend([
                HookEffect::Channel(V::Orb::LIGHTNING_ORB),
                HookEffect::Decrement,
            ]);
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::LIGHTNING_ROD_POWER && (player == holder) =>
        {
            unimplemented!("an endless LightningRodPower")
        }
        (__c0, _, EventHook::EnergyReset { .. }) if __c0 == V::Power::LIGHTNING_ROD_POWER => None,
        // `StarNextTurnPower.AfterEnergyReset`: its stars once, then it goes.
        (__c0, PowerAmount::Counted(amount), EventHook::EnergyReset { player, .. })
            if __c0 == V::Power::STAR_NEXT_TURN_POWER && (player == holder) =>
        {
            asked.extend([HookEffect::GainStars(amount), HookEffect::Remove]);
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::EnergyReset { player, .. })
            if (__c0 == V::Power::GENESIS_POWER || __c0 == V::Power::STAR_NEXT_TURN_POWER)
                && (player == holder) =>
        {
            unimplemented!("an endless {power:?}")
        }
        (__c0, _, EventHook::EnergyReset { .. })
            if (__c0 == V::Power::GENESIS_POWER || __c0 == V::Power::STAR_NEXT_TURN_POWER) =>
        {
            None
        }
        // `TyrannyPower.AfterPlayerTurnStart`: its owner picks its stack of
        // cards from hand, and each is exhausted.
        (__c0, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::TYRANNY_POWER && (player == holder) =>
        {
            Some(HookEffect::SelectFromPile {
                pile: sts2_core::Pile::Hand,
                selection: exactly(amount),
                act: sts2_core::PileAct::Exhaust,
            })
        }
        // `NecroMasteryPower.AfterCurrentHpChanged`: the hit points its
        // owner's Osty lost, times its stack, to each enemy that can be hit,
        // unblockable and unpowered.
        (
            __c0,
            PowerAmount::Counted(amount),
            EventHook::CurrentHpChanged {
                monster: Some(__c1),
                pet_of: Some(pet_of),
                delta,
                ..
            },
        ) if (__c0 == V::Power::NECRO_MASTERY_POWER && __c1 == V::Monster::OSTY)
            && (delta < 0 && pet_of == holder) =>
        {
            Some(HookEffect::DamageOpponentsUnblockable(-delta * amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CurrentHpChanged { .. })
            if __c0 == V::Power::NECRO_MASTERY_POWER =>
        {
            unimplemented!("an endless Necro Mastery")
        }
        (__c0, _, EventHook::CurrentHpChanged { .. }) if __c0 == V::Power::NECRO_MASTERY_POWER => {
            None
        }
        // `ThunderPower.AfterOrbEvoked`: its stack to each creature a
        // Lightning of its owner's evoke hit that is still alive, unpowered.
        (__c0, PowerAmount::Counted(amount), EventHook::OrbEvoked { player, orb: __c1 })
            if (__c0 == V::Power::THUNDER_POWER && __c1 == V::Orb::LIGHTNING_ORB)
                && player == holder =>
        {
            Some(HookEffect::DamageTargets(amount))
        }
        (__c0, _, EventHook::OrbEvoked { .. }) if __c0 == V::Power::THUNDER_POWER => None,
        // `EntropyPower.AfterPlayerTurnStart`: its owner picks its stack of
        // cards from hand, and each is transformed at random.
        (__c0, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::ENTROPY_POWER && (player == holder) =>
        {
            Some(HookEffect::SelectFromPile {
                pile: sts2_core::Pile::Hand,
                selection: exactly(amount),
                act: sts2_core::PileAct::TransformToRandom,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::ENTROPY_POWER && (player == holder) =>
        {
            unimplemented!("an endless Entropy")
        }
        (__c0, _, EventHook::PlayerTurnStart { .. }) if __c0 == V::Power::ENTROPY_POWER => None,
        // `RollingBoulderPower.AfterPlayerTurnStart`: its stack to each enemy
        // that can be hit, unpowered, then its `DamageVar(5)` more on itself.
        // A headless game runs in `TestMode`, where the damage lands at once.
        (__c0, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::ROLLING_BOULDER_POWER && (player == holder) =>
        {
            asked.extend([
                HookEffect::DamageOpponents(amount),
                HookEffect::SetAmount(amount + V::Power::ROLLING_BOULDER_POWER_DAMAGE),
            ]);
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::ROLLING_BOULDER_POWER && (player == holder) =>
        {
            unimplemented!("an endless Rolling Boulder")
        }
        (__c0, _, EventHook::PlayerTurnStart { .. }) if __c0 == V::Power::ROLLING_BOULDER_POWER => {
            None
        }
        // `ToolsOfTheTradePower.AfterPlayerTurnStart`: the same, discarded.
        (__c0, PowerAmount::Counted(amount), EventHook::PlayerTurnStart { player, .. })
            if __c0 == V::Power::TOOLS_OF_THE_TRADE_POWER && (player == holder) =>
        {
            Some(HookEffect::SelectFromPile {
                pile: sts2_core::Pile::Hand,
                selection: exactly(amount),
                act: sts2_core::PileAct::Discard,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::PlayerTurnStart { player, .. })
            if (__c0 == V::Power::TYRANNY_POWER || __c0 == V::Power::TOOLS_OF_THE_TRADE_POWER)
                && (player == holder) =>
        {
            unimplemented!("an endless {power:?}")
        }
        (__c0, _, EventHook::PlayerTurnStart { .. })
            if (__c0 == V::Power::TYRANNY_POWER || __c0 == V::Power::TOOLS_OF_THE_TRADE_POWER) =>
        {
            None
        }
        // `ForegoneConclusionPower.BeforeHandDraw`: its owner picks its stack
        // of cards from the draw pile into hand, and the power goes.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::FOREGONE_CONCLUSION_POWER && (player == holder) =>
        {
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
        (__c0, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::FOREGONE_CONCLUSION_POWER && (player == holder) =>
        {
            unimplemented!("an endless Foregone Conclusion")
        }
        (__c0, _, EventHook::BeforeHandDraw { .. })
            if __c0 == V::Power::FOREGONE_CONCLUSION_POWER =>
        {
            None
        }
        // `InfiniteBladesPower.BeforeHandDraw`: its stack of Shivs into its
        // owner's hand, before the hand is drawn.
        (__c0, PowerAmount::Counted(amount), EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::INFINITE_BLADES_POWER && (player == holder) =>
        {
            Some(HookEffect::GenerateInHand {
                card: V::Card::SHIV,
                count: amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::BeforeHandDraw { player, .. })
            if __c0 == V::Power::INFINITE_BLADES_POWER && (player == holder) =>
        {
            unimplemented!("an endless Infinite Blades")
        }
        (__c0, _, EventHook::BeforeHandDraw { .. }) if __c0 == V::Power::INFINITE_BLADES_POWER => {
            None
        }
        // `PhantomBladesPower.AfterApplied`: every Shiv its owner holds, in
        // every pile, retains.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::PHANTOM_BLADES_POWER => {
            Some(HookEffect::AddKeyword {
                to: sts2_core::Writing::EveryTagged(CardTag::Shiv),
                keyword: CardKeyword::Retain,
            })
        }
        // `PhantomBladesPower.AfterCardEnteredCombat`, which gives a Shiv that
        // joins later the same.
        (__c0, _, EventHook::CardEnteredCombat { at, card, .. })
            if __c0 == V::Power::PHANTOM_BLADES_POWER && (card.tagged(CardTag::Shiv)) =>
        {
            Some(HookEffect::AddKeyword {
                to: sts2_core::Writing::One(at),
                keyword: CardKeyword::Retain,
            })
        }
        (__c0, _, EventHook::CardEnteredCombat { .. })
            if __c0 == V::Power::PHANTOM_BLADES_POWER =>
        {
            None
        }
        // `RoyaltiesPower.AfterCombatEnd`: a gold reward of its stack.
        (__c0, PowerAmount::Counted(amount), EventHook::CombatEnd { .. })
            if __c0 == V::Power::ROYALTIES_POWER =>
        {
            Some(HookEffect::AddGoldReward(amount))
        }
        (__c0, PowerAmount::Endless, EventHook::CombatEnd { .. })
            if __c0 == V::Power::ROYALTIES_POWER =>
        {
            unimplemented!("an endless Royalties")
        }
        // `ImprovementPower.AfterCombatEnd`: its stack of upgradable deck
        // cards upgraded, each `NextItem` off `CombatCardSelection`. Every
        // card upgrades once, so the list is the deck's as each draw is made.
        (__c0, PowerAmount::Counted(amount), EventHook::CombatEnd { .. })
            if __c0 == V::Power::IMPROVEMENT_POWER =>
        {
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
        (__c0, PowerAmount::Endless, EventHook::CombatEnd { .. })
            if __c0 == V::Power::IMPROVEMENT_POWER =>
        {
            unimplemented!("an endless Improvement")
        }
        // `ForbiddenGrimoirePower.AfterCombatEnd`: a card removal reward for
        // each of its stack.
        (__c0, PowerAmount::Counted(amount), EventHook::CombatEnd { .. })
            if __c0 == V::Power::FORBIDDEN_GRIMOIRE_POWER =>
        {
            let count = usize::try_from(amount).expect("a stack is not negative");
            asked.extend(std::iter::repeat_n(HookEffect::AddCardRemovalReward, count));
            return;
        }
        (__c0, PowerAmount::Endless, EventHook::CombatEnd { .. })
            if __c0 == V::Power::FORBIDDEN_GRIMOIRE_POWER =>
        {
            unimplemented!("an endless Forbidden Grimoire")
        }
        // `DrawCardsNextTurnPower.AfterSideTurnStart` goes once the turn it
        // drew for has begun.
        (__c0, _, EventHook::SideTurnStart { participants, .. })
            if __c0 == V::Power::DRAW_CARDS_NEXT_TURN_POWER
                && (participants.contains(holder)
                    && held.on_turn_start != PowerAmount::Counted(0)) =>
        {
            Some(HookEffect::Remove)
        }
        (__c0, _, EventHook::SideTurnStart { .. })
            if __c0 == V::Power::DRAW_CARDS_NEXT_TURN_POWER =>
        {
            None
        }
        // `RetainHandPower.AfterSideTurnEnd`, for the side its owner is among.
        (__c0, _, EventHook::SideTurnEnd { participants, .. })
            if __c0 == V::Power::RETAIN_HAND_POWER && (participants.contains(holder)) =>
        {
            Some(HookEffect::Decrement)
        }
        (__c0, _, EventHook::SideTurnEnd { .. }) if __c0 == V::Power::RETAIN_HAND_POWER => None,
        // The overrides above that answer for their own holder, or for their
        // own side's turn, and leave every other event alone.
        (__c0, PowerAmount::Counted(_), EventHook::BlockCleared { .. })
            if __c0 == V::Power::BLOCK_NEXT_TURN_POWER =>
        {
            None
        }
        (__c0, _, EventHook::EnergyReset { .. }) if __c0 == V::Power::ENERGY_NEXT_TURN_POWER => {
            None
        }
        (__c0, PowerAmount::Counted(_), EventHook::SideTurnStart { .. })
            if __c0 == V::Power::FURNACE_POWER =>
        {
            None
        }
        (__c0, _, EventHook::AfterDeath { .. })
            if (__c0 == V::Power::ILLUSION_POWER
                || __c0 == V::Power::INFESTED_POWER
                || __c0 == V::Power::REATTACH_POWER
                || __c0 == V::Power::ADAPTABLE_POWER
                || __c0 == V::Power::SHRINK_POWER) =>
        {
            None
        }
        (__c0, _, EventHook::DamageGiven { .. }) if __c0 == V::Power::IMBALANCED_POWER => None,
        (__c0, _, EventHook::BeforeDeath { .. }) if __c0 == V::Power::SWIPE_POWER => None,
        (__c0, _, EventHook::BlockBroken { .. }) if __c0 == V::Power::BURROWED_POWER => None,
        (__c0, _, EventHook::DamageReceived { .. })
            if (__c0 == V::Power::SLIPPERY_POWER || __c0 == V::Power::SLUMBER_POWER) =>
        {
            None
        }
        (
            __c0,
            PowerAmount::Counted(_),
            EventHook::SideTurnStart { .. }
            | EventHook::BeforeSideTurnStart { .. }
            | EventHook::BeforeSideTurnEndEarly { .. },
        ) if __c0 == V::Power::PLATING_POWER => None,
        (__c0, _, EventHook::PowerAmountChanged { .. } | EventHook::CardEnteredCombat { .. })
            if __c0 == V::Power::SWORD_SAGE_POWER =>
        {
            None
        }
        // `AfterApplied` and `AfterRemoved` where the class overrides them for
        // what it shows: a name in its text, a size, a sound.
        (__c0, _, EventHook::Applied)
            if (__c0 == V::Power::BARRICADE_POWER
                || __c0 == V::Power::SHRINK_POWER
                || __c0 == V::Power::PLATING_POWER) =>
        {
            None
        }
        (__c0, _, EventHook::Removed)
            if (__c0 == V::Power::SHRINK_POWER || __c0 == V::Power::SLUMBER_POWER) =>
        {
            None
        }
        // `IllusionPower.AfterApplied` applies Minion to its owner, unless it
        // has Minion already.
        (__c0, _, EventHook::Applied) if __c0 == V::Power::ILLUSION_POWER => fight
            .power(holder, V::Power::MINION_POWER)
            .is_none()
            .then_some(HookEffect::ApplyToOwner(
                V::Power::MINION_POWER,
                PowerAmount::Counted(one),
            )),
        // `SwordSagePower.AfterRemoved`: every Sovereign Blade its owner holds
        // loses as many replays as the instance held.
        (__c0, PowerAmount::Counted(amount), EventHook::Removed)
            if __c0 == V::Power::SWORD_SAGE_POWER =>
        {
            Some(HookEffect::AddReplays {
                to: sts2_core::Writing::EveryCopyOf(V::Card::SOVEREIGN_BLADE),
                amount: -amount,
            })
        }
        (__c0, PowerAmount::Endless, EventHook::Removed) if __c0 == V::Power::SWORD_SAGE_POWER => {
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

/// A power's `AfterDeath`, run against the fight.
///
/// # Panics
///
/// A power whose death commands nobody has ported.
pub(crate) fn perform_death<V: crate::Build, F: sts2_core::Fight<V>>(
    power: V::Power,
    holder: sts2_core::CombatId,
    fight: &mut F,
) {
    // A power whose death the build ports for itself.
    if V::own_power_death(power, holder, fight).is_some() {
        return;
    }
    match power {
        // `SurprisePower.AfterDeath`: a Sneaky Gremlin, then a Fat Gremlin,
        // which takes a Heist of the gold each Thievery stole, aimed at the
        // Thievery's target. Any gold at all marks the encounter's gold
        // stolen.
        __c0 if __c0 == V::Power::SURPRISE_POWER => {
            fight.summon(V::Monster::SNEAKY_GREMLIN, usize::MAX);
            let fat = fight.summon(V::Monster::FAT_GREMLIN, usize::MAX);
            if heists::<V, _>(fight, holder, fat) > 0 {
                fight.write_encounter(crate::behaviour::encounters::State::mark_gold_stolen);
            }
        }
        power => unimplemented!("what {power:?} does as its owner dies"),
    }
}

/// `STUNNED`, `REVIVE`, `SPAWNED`, `VALID_ORBS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `Creature.Stun`'s state.
        fn powers_stunned() -> &'static sts2_core::NextMove<$build> {
            static LIST: std::sync::LazyLock<sts2_core::NextMove<$build>> =
                std::sync::LazyLock::new(|| {
                    crate::behaviour::powers::built::<$build>(
                        <$build as sts2_core::ContentVersion>::MoveId::STUNNED,
                        crate::generated::monsters::IntentKind::Stun,
                    )
                });
            &LIST
        }
        /// `IllusionPower`'s revive, a heal.
        fn powers_revive() -> &'static sts2_core::NextMove<$build> {
            static LIST: std::sync::LazyLock<sts2_core::NextMove<$build>> =
                std::sync::LazyLock::new(|| {
                    crate::behaviour::powers::built::<$build>(
                        <$build as sts2_core::ContentVersion>::MoveId::REVIVE_MOVE,
                        crate::generated::monsters::IntentKind::Heal,
                    )
                });
            &LIST
        }
        /// The stunned turn a Wriggler that `InfestedPower` spawns opens on.
        fn powers_spawned() -> &'static sts2_core::NextMove<$build> {
            static LIST: std::sync::LazyLock<sts2_core::NextMove<$build>> =
                std::sync::LazyLock::new(|| {
                    crate::behaviour::powers::built::<$build>(
                        <$build as sts2_core::ContentVersion>::MoveId::SPAWNED_MOVE,
                        crate::generated::monsters::IntentKind::Stun,
                    )
                });
            &LIST
        }
        /// `OrbModel._validOrbs`, which `GetRandomOrb` draws from.
        const POWERS_VALID_ORBS: &'static [<$build as sts2_core::ContentVersion>::Orb] = &[
            <$build as sts2_core::ContentVersion>::Orb::LIGHTNING_ORB,
            <$build as sts2_core::ContentVersion>::Orb::FROST_ORB,
            <$build as sts2_core::ContentVersion>::Orb::DARK_ORB,
            <$build as sts2_core::ContentVersion>::Orb::PLASMA_ORB,
            <$build as sts2_core::ContentVersion>::Orb::GLASS_ORB,
        ];
    };
}

pub(crate) use lists;
#[cfg(test)]
mod tests {
    type V = crate::v0_107_1::V0_107_1;
    use crate::behaviour::powers::*;

    /// A power whose `Type` the table leaves computed has to lend, or asking
    /// whether it is a debuff panics in the middle of a fight. Hotfix did, on
    /// its first play.
    #[test]
    fn every_computed_type_is_a_lend() {
        use sts2_core::PowerTable as _;

        let unanswered: Vec<<V as sts2_core::ContentVersion>::Power> =
            <<V as sts2_core::ContentVersion>::Power as sts2_core::Table>::ALL
                .iter()
                .copied()
                .filter(|&power| power.debuff().is_none() && lends::<V>(power).is_none())
                .collect();
        assert!(
            unanswered.is_empty(),
            "computed types nothing answers: {unanswered:?}"
        );
    }
}
