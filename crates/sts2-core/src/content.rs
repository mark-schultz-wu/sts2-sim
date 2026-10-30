//! What a build's content looks like, abstractly.
//!
//! The shape every content version fills in. That is the [`ContentVersion`]
//! trait naming one build's types, and the per-kind [`Table`] traits saying
//! what each of them already knows. Then the data shapes those tables are
//! written in.
//!
//! Nothing here names a build. `sts2-content` is where builds live, and
//! implements these.

/// A kind of content with a table behind it.
///
/// One build's measurement of a kind, as an interface: every value, and a row
/// of static facts for each. A caller holding only a [`ContentVersion`] can
/// sweep the content without naming a build.
///
/// [`Table::Data`] is opaque here, so a generic caller reaches for the kind
/// traits below instead. The crate README says why.
pub trait Table: Sized + 'static {
    /// The static facts about one value.
    type Data: 'static;

    /// Every value of this kind, once each, in the game's own order.
    const ALL: &'static [Self];

    /// This value's row.
    ///
    /// Borrowed, because a row is one of the build's own constants and some
    /// of them are forty bytes. Handing back a copy put those bytes on the
    /// stack of every accessor that reads one field of one.
    fn data(self) -> &'static Self::Data;
}

/// What a table says about an act, beyond its row.
///
/// The two numbers room generation deals by. An act's map is one longer than
/// its room count, and the first few fights of it come from a smaller pool.
pub trait ActTable: Table {
    /// How many rooms an act's map holds, the boss and the first node aside.
    fn rooms(self) -> u8;

    /// How many of those open the act, dealt from the weak fights.
    fn weak_encounters(self) -> u8;

    /// `ActModel.Index`: the place in a run this act can fill. Two acts
    /// share the first place. The deprecated act has none.
    fn index(self) -> Option<u8>;
}

/// How one act rolls the rest sites on its map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rests {
    /// `NextGaussianInt(mean, 1, min, max)`, which three of the four acts use.
    Around { mean: usize, min: usize, max: usize },
    /// `NextInt(min, max)`, which the last act uses.
    Between { min: usize, max: usize },
}

/// What one act's map is made of, before any of it is rolled.
///
/// The acts differ in three numbers and in nothing else, so the generator
/// takes them rather than knowing about acts. Which numbers an act uses is
/// `BaseNumberOfRooms` and `GetMapPointTypes`, both of them per build, which
/// is why [`Rules::act_layout`] is what answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// `BaseNumberOfRooms`, which is one less than the grid's height.
    pub rooms: usize,
    pub rests: Rests,
    /// Taken off the standard question mark roll. Zero in the first act.
    pub fewer_unknowns: usize,
}

/// A content rule that cannot produce a value from static state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContentError {
    #[error("monster state index {index} is outside its graph")]
    InvalidMonsterState { index: u8 },
    #[error("a random monster state has invalid weights")]
    InvalidMonsterWeights,
}

/// A `SetMoveImmediate` the graph refused.
///
/// `NextMove.CanTransitionAway` is false. The move the monster advertises has
/// to be performed before the graph can leave it. The command moves nothing,
/// and the monster goes on advertising that move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the advertised move must be performed before the graph can leave it")]
pub struct MovePinned;

/// How an encounter chooses the creatures that open its fight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncounterRoster<T> {
    Fixed(T),
    Random,
    Configured,
}

/// One creature a fixed roster names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RosterEntry<M, Id> {
    pub monster: M,
    /// The move it opens on, where the encounter writes one.
    pub opens: Option<Id>,
    /// Its place in `Encounter.Slots`, where the encounter names slots.
    pub slot: Option<u8>,
}

/// What a table says about an encounter, beyond its row.
///
/// The two keys a coverage measurement cuts on. A flat count over 96 encounters
/// measures the wrong thing, for the reason the crate README gives.
pub trait EncounterTable: Table {
    /// Where an encounter can appear.
    type Act: Copy + serde::Serialize + 'static;
    /// Monster, elite or boss.
    type Room: RoomKinds + serde::Serialize;
    /// A creature the encounter can place in combat.
    type Monster: Copy + serde::Serialize + 'static;
    /// A move one of those creatures can open on.
    type MoveId: Copy + 'static;

    /// The acts whose map can roll this encounter.
    ///
    /// Empty is meaningful and is the reachability answer: an encounter no act
    /// lists is one only an event or a debug command starts.
    fn acts(self) -> &'static [Self::Act];

    /// The kind of room the encounter is.
    fn room(self) -> Self::Room;

    /// One of the first fights of an act.
    ///
    /// Room generation deals these before the rest, so an act opens on
    /// something a starting deck can beat.
    fn weak(self) -> bool;

    /// `ShouldGiveRewards`: whether winning it offers the room's rewards. A
    /// fight that offers none goes straight on, as
    /// `NCombatUi.ProceedWithoutRewards` does.
    fn gives_rewards(self) -> bool;

    /// Whether the two field a creature family in common.
    ///
    /// Room generation places no encounter next to one this holds for. So a
    /// run does not meet the weak slimes and then the full ones.
    fn shares_family_with(self, other: Self) -> bool;

    /// The monsters this encounter can field, in table order.
    fn possible_monsters(self) -> impl Iterator<Item = Self::Monster>;

    /// How the encounter chooses its opening roster. A fixed one names each
    /// creature and the move it opens on where the encounter writes one. It
    /// names the slot each stands in where the encounter has slots.
    fn roster(
        self,
    ) -> EncounterRoster<impl Iterator<Item = RosterEntry<Self::Monster, Self::MoveId>>>;

    /// How many slots `Encounter.Slots` names, which a creature added
    /// mid-fight is placed among.
    fn slots(self) -> u8;

    /// The encounter entry’s hash, already cast for wrapping seed arithmetic.
    fn seed_name(self) -> &'static str;
}

/// What a table says about an event, beyond its row.
///
/// The same reachability question as [`EncounterTable::acts`], and the same
/// answer for the same reason: an event is offered by the act that lists it.
pub trait EventTable: Table {
    /// Where an event can appear.
    type Act: Copy + serde::Serialize + 'static;

    /// The acts that offer it. Empty for one in the any-act pool.
    fn acts(self) -> &'static [Self::Act];

    /// Offered by every act rather than by the acts named.
    fn any_act(self) -> bool;

    /// One of the nine ancients, which an act draws separately.
    ///
    /// An act offers these and its ordinary events from two different lists,
    /// so room generation has to tell them apart.
    fn ancient(self) -> bool;

    /// The event id's deterministic hash, already cast for seed arithmetic.
    fn seed_name(self) -> &'static str;

    /// `EventModel.IsShared`: every player goes through one copy of it and
    /// votes on each option, rather than each through their own.
    fn shared(self) -> bool;

    /// The fights a build names.
    type Encounter: Copy + 'static;

    /// The encounter a `Combat`-layout event builds as it opens, which is
    /// `GenerateInternalCombatState`. Nothing for any other layout.
    fn combat_layout(self) -> Option<Self::Encounter>;
}

/// What a table says about a monster, beyond its row.
pub trait MonsterTable: Table {
    /// Where a monster can appear.
    type Act: Copy + serde::Serialize + 'static;

    /// The acts that field it. Empty for one only another monster summons.
    fn acts(self) -> &'static [Self::Act];

    /// The hooks this monster's class overrides, as the hook dump measured
    /// them. See [`PowerTable::overrides`].
    fn overrides(self) -> Overrides;

    /// Whether this monster's class overrides `method`, as the monster dump
    /// measured it.
    fn overrides_virtual(self, method: MonsterVirtual) -> bool;
}

/// A virtual `MonsterModel` declares for itself, which the fight runs on a
/// monster. A monster whose class overrides one answers for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterVirtual {
    /// As the monster enters the fight.
    AfterAddedToRoom,
}

/// A pool a reward draws from, already split the way a rarity roll asks.
///
/// A draw rolls a rarity and then indexes into the pool members at it. The
/// split never changes, so the tables make it once.
///
/// Each slice keeps the pool's own order, so an index means what it meant
/// before the split. Anything at no drafted rarity is in none of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drafted<'a, T> {
    pub common: &'a [T],
    pub uncommon: &'a [T],
    pub rare: &'a [T],
}

impl<'a, T> Drafted<'a, T> {
    /// The members at one rarity, or nothing at all for [`RewardRarity::Other`].
    pub fn at(&self, rarity: RewardRarity) -> &'a [T] {
        match rarity {
            RewardRarity::Common => self.common,
            RewardRarity::Uncommon => self.uncommon,
            RewardRarity::Rare => self.rare,
            RewardRarity::Other => &[],
        }
    }
}

/// `CardType`, which a merchant's shelves and a relic's filter both name.
///
/// The game's own six. `_coloredCardTypes` names the first three, and a shelf
/// is stocked by filtering the character's pool down to one of them.
/// `CardMultiplayerConstraint`: the runs a card is dealt in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultiplayerConstraint {
    /// Any run deals it.
    None,
    /// Only a run of several players deals it.
    MultiplayerOnly,
    /// Only a run of one player deals it.
    SingleplayerOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardKind {
    Attack,
    Skill,
    Power,
    Status,
    Curse,
    Quest,
}

/// The three rarities a reward rolls, plus everything else.
///
/// One rule and two tables. A card reward and a potion reward each roll a
/// rarity and then draw from what the pool has at it. The game spells the two
/// rarities as separate enums with the same three names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RewardRarity {
    Common,
    Uncommon,
    Rare,
    Other,
}

/// What a table says about a card, beyond its row.
///
/// Both keys matter to a measurement and they cut differently. Rarity is how
/// often a card turns up at all, and the pool is which runs can ever see it.
/// The crate README says why a measurement needs both.
pub trait CardTable: Table {
    /// The character pool a card is drawn from.
    type Pool: Copy + serde::Serialize + 'static;
    /// How often it turns up.
    type Rarity: CardRarityKinds + serde::Serialize + 'static;
    /// What it can be pointed at.
    type Target: TargetKinds + serde::Serialize + 'static;
    /// Which of the build's printed numbers one of its rows is.
    type Var: CardVars;
    /// What a relic or card reads a card as, such as a Strike.
    type Tag: CardTagKinds;
    /// The exact numeric representation used by this build's card variables.
    ///
    /// Not a live game number. This is what the card's face prints, which one
    /// build spells as an exact decimal. [`num_traits::ToPrimitive`] is how a
    /// printed number reaches the run: it yields nothing for one no integer
    /// can hold.
    type Number: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + num_traits::ToPrimitive
        + Into<crate::Decimal>
        + 'static;

    /// The character whose pool holds it, or `None` for one held by no
    /// character: colourless, curses, statuses, tokens, quest and event cards.
    fn pool(self) -> Option<Self::Pool>;

    /// Its rarity, which for the poolless cards is also what they are.
    fn rarity(self) -> Self::Rarity;

    /// What it costs in energy.
    fn cost(self) -> Cost;

    /// What it costs in stars, the second pool the Regent's cards are priced
    /// in. [`Cost::None`] for every card outside it.
    fn star_cost(self) -> Cost;

    /// What it can be pointed at, or `None` for a card that works that out
    /// from the fight it is in. Four cards do.
    fn target(self) -> Option<Self::Target>;

    /// Whether it carries the keyword that forbids playing it.
    fn unplayable(self) -> bool;

    /// Whether it carries the keyword that exhausts a played copy.
    fn exhausts(self) -> bool;

    /// `GainsBlock`, which a card that gives block overrides.
    fn gains_block(self) -> bool;

    /// `VisualCardPool.IsColorless`: whether the card shows as colorless.
    /// Most cards show their own pool, and a few event cards a character's.
    fn visually_colorless(self) -> bool;

    /// `CardModel.MultiplayerConstraint`: which runs deal the card, by how
    /// many players they have.
    fn multiplayer_constraint(self) -> MultiplayerConstraint;

    /// `HasTurnEndInHandEffect`: whether a turn that ends with it in hand
    /// runs `OnTurnEndInHandWrapper` on it.
    fn turn_end_in_hand(self) -> bool;

    /// Whether it carries the keyword that exhausts it from hand at the end
    /// of a turn.
    fn ethereal(self) -> bool;

    /// Whether an unplayed copy stays in hand when the turn ends.
    fn retains(self) -> bool;

    /// Whether the table prints `Sly`, which plays a discarded copy.
    fn sly(self) -> bool;

    /// Whether it carries `Eternal`, which keeps it in the deck: no removal
    /// takes it, and no transform takes it from the deck.
    fn eternal(self) -> bool;

    /// Whether the table prints the keyword that opens a fight in hand.
    ///
    /// What the table prints, which an upgrade can add to. A copy is what
    /// carries the answer, because `AddKeyword` writes on one.
    fn innate(self) -> bool;

    /// Whether `Tags` holds this tag.
    fn tagged(self, tag: Self::Tag) -> bool;

    /// How many times it can be upgraded, which is `MaxUpgradeLevel`.
    ///
    /// Zero for a curse or a status, one for everything else on this build. A
    /// card is upgradable while its level is below this.
    fn max_upgrade(self) -> u8;

    /// Whether its type is Power, which decides where a played copy goes.
    ///
    /// `None` for the one card that works its type out from the fight, which
    /// this cannot answer with no fight in front of it.
    fn power(self) -> Option<bool>;

    /// Where this card sorts against every other.
    ///
    /// `ModelId.CompareTo` is ordinal over the id the wire spells, and a card
    /// then breaks a tie on its upgrade level. Shuffling a pile back into the
    /// draw pile sorts it first, so this decides every card drawn after one.
    ///
    /// A number rather than the id itself, because that sort is on the path
    /// from a discard to a hand.
    fn order(self) -> u16;

    /// The numbers printed on its face, in name order.
    ///
    /// What the card says with nothing modifying it, which is
    /// [`crate::Preview::base`]. `Preview::vars` is these after the fight has
    /// had its say, and that is not static data.
    fn vars(self) -> &'static [CardNumber<Self::Var, Self::Number>];

    /// The hooks this card's class overrides, as the hook dump measured them.
    /// See [`PowerTable::overrides`].
    fn overrides(self) -> Overrides;

    /// Every hook some card's class in the build overrides.
    fn heard_by_any() -> Overrides;
}

/// The game's `decimal`: a whole number of up to 96 bits over a power of ten
/// no higher than 28.
///
/// What a hit works out to before its floor, and what a card like Thrash
/// writes on itself. A product of a few hundredths stays exact in one.
pub use rust_decimal::Decimal;

/// An exact finite decimal as the game writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fixed {
    coefficient: i32,
    scale: u8,
}

impl Fixed {
    pub const fn new(coefficient: i32, scale: u8) -> Self {
        Self { coefficient, scale }
    }

    pub const fn coefficient(self) -> i32 {
        self.coefficient
    }
    pub const fn scale(self) -> u8 {
        self.scale
    }

    /// `number` held exactly, or `None` for one too wide to be.
    pub fn from_exact(number: Decimal) -> Option<Self> {
        let number = number.normalize();
        let coefficient = i32::try_from(number.mantissa()).ok()?;
        let scale = u8::try_from(number.scale()).ok()?;
        Some(Self { coefficient, scale })
    }

    /// The same number as the game's `decimal`.
    pub const fn exact(self) -> Decimal {
        Decimal::from_parts(
            self.coefficient.unsigned_abs(),
            0,
            0,
            self.coefficient < 0,
            self.scale as u32,
        )
    }
}

/// The game's `decimal`, exactly.
impl From<Fixed> for crate::Decimal {
    fn from(number: Fixed) -> Self {
        number.exact()
    }
}

/// A whole number, and nothing at all for one with a fraction.
///
/// The live game holds integers. This build prints `1.5` on a card, and no
/// build's game number holds that. It reads as absent here, rather than as a
/// rounded lie. `num_traits::NumCast` then carries the whole ones into
/// whichever width a build chose.
impl num_traits::ToPrimitive for Fixed {
    fn to_i64(&self) -> Option<i64> {
        let divisor = 10_i32.checked_pow(u32::from(self.scale))?;
        (self.coefficient % divisor == 0).then_some(i64::from(self.coefficient / divisor))
    }

    fn to_u64(&self) -> Option<u64> {
        self.to_i64().and_then(|whole| u64::try_from(whole).ok())
    }
}

#[cfg(test)]
mod overrides_tests {
    use super::{ModelHook, Overrides};

    /// A hook's place in `ALL` is its discriminant, and the value hooks are
    /// the ones from `FIRST_VALUE` on.
    #[test]
    fn value_hooks_follow_the_event_hooks() {
        for (at, &hook) in ModelHook::ALL.iter().enumerate() {
            assert_eq!(hook as usize, at, "{hook:?} out of place");
            let value = ["Modify", "TryModify", "Should"]
                .iter()
                .any(|prefix| hook.name().starts_with(prefix));
            assert_eq!(
                value,
                hook as u8 >= ModelHook::FIRST_VALUE,
                "{hook:?} on the wrong side"
            );
        }
    }

    #[test]
    fn a_set_names_its_hooks_by_place() {
        let set = Overrides::of(ModelHook::AfterCardPlayed).with(ModelHook::ModifyXValue);
        let places: Vec<usize> = set.places().collect();
        assert_eq!(
            places,
            [
                ModelHook::AfterCardPlayed as usize,
                ModelHook::ModifyXValue as usize
            ]
        );
        assert!(set.contains(ModelHook::ModifyXValue));
        assert!(
            !set.without(ModelHook::ModifyXValue)
                .contains(ModelHook::ModifyXValue)
        );
        assert!(
            !Overrides::of(ModelHook::AfterCardPlayed)
                .intersects(Overrides::of(ModelHook::ModifyXValue))
        );
    }
}

#[cfg(test)]
mod card_number_tests {
    use super::Fixed;

    #[test]
    fn exact_decimals_are_whole_only_when_their_fraction_is_zero() {
        use num_traits::NumCast;

        assert_eq!(<i32 as NumCast>::from(Fixed::new(120, 1)), Some(12));
        assert_eq!(<i32 as NumCast>::from(Fixed::new(125, 1)), None);

        // A whole number a narrower build could not hold is absent too, rather
        // than the wrap an `as` would have made of it.
        assert_eq!(<i8 as NumCast>::from(Fixed::new(1280, 1)), None);
        assert_eq!(<u8 as NumCast>::from(Fixed::new(-10, 1)), None);
    }
}

/// One number printed on a card's face.
///
/// Keyed rather than named. Which number this is belongs to the build, and a
/// build generates the set as an enum. So a card asking for one it does not
/// print is a compile error, not a lookup that finds nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardNumber<K, N> {
    pub id: K,
    /// The value with nothing modifying the card, upgrades and enchantment
    /// included.
    pub base: N,
    /// Whether the game works the printed number out from a live fight.
    ///
    /// The game's `CalculatedVar`: a base plus a per-something count, such as
    /// Perfected Strike's two more damage for every Strike held. `base` is the
    /// base alone, so an implementation that reads it and stops reports a
    /// number the game does not print. It has to run the card's own rule.
    pub calculated: bool,
    /// Which of the fight's own hooks the number runs through.
    ///
    /// `DynamicVar`'s subclass decides it, and the key the number is filed
    /// under says nothing. Sovereign Blade's block is a `CalculatedBlockVar`
    /// named `CalculatedBlock`, and it runs the block hooks exactly as a
    /// `BlockVar` named `Block` does.
    pub modifier: Option<CardNumberModifier>,
}

/// What one printed number runs through before a card shows it.
///
/// `DynamicVar.UpdateCardPreview`, which each subclass overrides. The same
/// hooks run when the card is played, so this is not a display rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardNumberModifier {
    /// `Hook.ModifyDamage`, against the creature the card is pointed at.
    Damage,
    /// The same, for a `DamageVar` whose props carry `Unpowered`, which a
    /// hook reading a powered attack passes over.
    UnpoweredDamage,
    /// The same, for the owner's Osty rather than the owner. An
    /// `OstyDamageVar`, or a `CalculatedDamageVar` built `FromOsty`.
    OstyDamage,
    /// `Hook.ModifyBlock`, for the creature about to gain it.
    Block,
    /// The same, for a `BlockVar` whose props carry `Unpowered`, which a hook
    /// reading powered block passes over.
    UnpoweredBlock,
    /// `Hook.ModifyPowerAmountGiven`, for the power a `PowerVar` applies,
    /// from the card's owner to the creature the card is pointed at.
    PowerGiven,
    /// `ExtraDamageVar`: the number times the card's enchantment's
    /// `EnchantDamageMultiplicative`, and no hook. Only the preview reads it
    /// so. A play reads the number the card prints.
    ExtraDamage,
}

/// The numbers a build's cards can print, as one set.
///
/// The game keys them by string: `Damage`, `Block`, `Stars`, and one per power
/// a card applies, spelled as the power's type is (`WeakPower`). Two builds do
/// not print the same set, so the set belongs to a build and the strings stay
/// at the edges that spell them.
///
/// `Ord`, because a preview keys its numbers by one of these.
pub trait CardVars: std::fmt::Debug + Copy + Ord + 'static {
    /// The build's powers, which a `PowerVar` names.
    type Power;

    /// `CalculatedVar.GetBaseVar`, the base a calculated number adds to.
    const CALCULATION_BASE: Self;

    /// Every key, once each.
    const ALL: &'static [Self];

    /// The game's own key for it, which is what an observation carries.
    fn name(self) -> &'static str;

    /// The power a `PowerVar` filed under this key applies, which is the
    /// class's `T`. The key need not name it: Mad Science's Choking is
    /// `ChokingDamage`. Nothing for a key no `PowerVar` is filed under.
    fn power(self) -> Option<Self::Power>;
}

/// What a table says about a power, beyond its row.
///
/// One question, because one caller has one. A front end drawing a power on a
/// creature has to know which way it points, and the game's own tables say. Not
/// derived here from a name or a suffix, for the reason the crate README gives.
pub trait PowerTable: Table {
    /// Whether a negative amount means one endless instance rather than a
    /// count below zero.
    ///
    /// `PowerStackType` is a constant for every power in this build but one.
    /// `ShrinkPower` works it out at the time: below zero it is a single
    /// instance rather than a counter, which is `IsInfinite`. So the number
    /// the game sends is not always a count, and this says which powers it is
    /// not a count for.
    fn endless_below_zero(self) -> bool;

    /// Whether the game counts this as a debuff.
    ///
    /// `None` for a power that decides at the time. The game's own
    /// `TypeForCurrentAmount` is this together with the stacking rule and
    /// whether negatives are allowed. A debuff that forbids negatives reads as
    /// a buff while its amount is below zero. So this is the static half, and a
    /// caller wanting the exact answer needs the creature too.
    fn debuff(self) -> Option<bool>;

    /// The hooks this power's class overrides, as the hook dump measured them.
    ///
    /// A power whose class does not override a hook inherits
    /// `AbstractModel`'s, which does nothing. So an event raised as that hook
    /// has nothing to ask it.
    fn overrides(self) -> Overrides;

    /// Whether `PowerInstanceType` is set: a creature can carry more than one
    /// instance of it, each with its own amount.
    fn instanced(self) -> bool;

    /// Whether it is `InstancedPerApplier`: an instance for each applier, which
    /// `FindExistingInstanceForStacking` finds by that applier.
    fn per_applier(self) -> bool;

    /// Whether this power's class overrides `method`, as the power dump
    /// measured it.
    fn overrides_virtual(self, method: PowerVirtual) -> bool;
}

/// A hook `AbstractModel` declares, which a fight raises on its listeners.
///
/// Only the hooks some [`EventHook`], [`ModifyHook`] or [`DecisionHook`] is
/// raised as. Each is spelled the way the hook dump spells it, which is how a
/// model's overrides are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModelHook {
    AfterRoomEntered,
    AfterMapGenerated,
    AfterActEntered,
    BeforeCombatStart,
    AfterCombatEnd,
    AfterCardPlayed,
    AfterDamageReceived,
    AfterDamageGiven,
    AfterCardDrawn,
    BeforeSideTurnStart,
    AfterSideTurnStart,
    BeforeSideTurnEndEarly,
    AfterSideTurnEnd,
    AfterSideTurnEndLate,
    AfterEnergyReset,
    AfterTakingExtraTurn,
    AfterAutoPrePlayPhaseEnteredEarly,
    AfterAutoPrePlayPhaseEntered,
    AfterAutoPrePlayPhaseEnteredLate,
    AfterAutoPostPlayPhaseEntered,
    AfterStarsSpent,
    AfterHandEmptied,
    AfterBlockCleared,
    AfterDeath,
    AfterPowerAmountChanged,
    AfterCardEnteredCombat,
    AfterShuffle,
    AfterModifyingHandDraw,
    BeforePotionUsed,
    AfterPotionUsed,
    AfterPotionProcured,
    AfterPotionDiscarded,
    AfterCardPlayedLate,
    AfterSideTurnStartLate,
    AfterPreventingDraw,
    AfterCardDrawnEarly,
    BeforeCardAutoPlayed,
    BeforeCardPlayed,
    BeforeDeath,
    AfterBlockBroken,
    AfterCombatVictory,
    AfterPlayerTurnStart,
    AfterPlayerTurnStartLate,
    AfterOrbChanneled,
    AfterOrbEvoked,
    AfterModifyingOrbPassiveTriggerCount,
    AfterEnergyResetLate,
    AfterOstyRevived,
    AfterCreatureAddedToCombat,
    AfterSummon,
    BeforeDamageReceived,
    AfterCurrentHpChanged,
    AfterRestSiteHeal,
    AfterModifyingRewards,
    AfterModifyingGoldGained,
    AfterGoldGained,
    BeforeAttack,
    AfterAttack,
    BeforeHandDraw,
    AfterCardExhausted,
    AfterCardDiscarded,
    AfterModifyingCardPlayCount,
    AfterModifyingHpLostAfterOsty,
    AfterModifyingCardPlayResultPileOrPosition,
    BeforeSideTurnEndVeryEarly,
    BeforeSideTurnEnd,
    AfterDiedToDoom,
    BeforePowerAmountChanged,
    AfterModifyingPowerAmountGiven,
    AfterModifyingPowerAmountReceived,
    AfterCardChangedPiles,
    AfterCardGeneratedForCombat,
    AfterCombatVictoryEarly,
    BeforeCombatStartLate,
    AfterFlush,
    AfterModifyingBlockAmount,
    AfterModifyingCardRewardOptions,
    AfterModifyingEnergyGain,
    AfterStarsGained,
    AfterBlockGained,
    BeforeFlushLate,
    AfterEnergySpent,
    AfterForge,
    AfterItemPurchased,
    BeforeRoomEntered,
    BeforeCombatRewardOffered,
    ModifyDamageAdditive,
    ModifyDamageMultiplicative,
    ModifyDamageCap,
    ModifyBlockAdditive,
    ModifyBlockMultiplicative,
    ModifyHpLostBeforeOsty,
    ModifyHpLostBeforeOstyLate,
    ModifyHpLostAfterOsty,
    ModifyHpLostAfterOstyLate,
    ModifyHandDraw,
    ModifyHandDrawLate,
    ModifyShuffleOrder,
    ShouldProcurePotion,
    ModifyNextEvent,
    TryModifyCardBeingAddedToDeck,
    TryModifyCardBeingAddedToDeckLate,
    ModifyMerchantCardCreationResults,
    ShouldDraw,
    ShouldPlay,
    ModifyMaxEnergy,
    TryModifyEnergyCostInCombat,
    TryModifyEnergyCostInCombatLate,
    ShouldCreatureBeRemovedFromCombatAfterDeath,
    ShouldPowerBeRemovedOnDeath,
    ShouldClearBlock,
    ShouldStopCombatFromEnding,
    ShouldAllowHitting,
    ModifyOrbValue,
    ModifyOrbPassiveTriggerCounts,
    ModifyUnblockedDamageTarget,
    ModifySummonAmount,
    ModifyRestSiteHealAmount,
    TryModifyRestSiteHealRewards,
    ShouldAllowFreeTravel,
    ShouldAllowMerchantCardRemoval,
    TryModifyRewards,
    TryModifyRewardsLate,
    TryModifyCardRewardAlternatives,
    ModifyUnknownMapPointRoomTypes,
    ModifyGeneratedMap,
    ModifyGeneratedMapLate,
    ModifyCardRewardCreationOptions,
    ModifyMerchantCardPool,
    ModifyOddsIncreaseForUnrolledRoomType,
    ModifyCardRewardUpgradeOdds,
    TryModifyCardRewardOptions,
    TryModifyCardRewardOptionsLate,
    ModifyGoldGained,
    ShouldFlush,
    ModifyCardPlayCount,
    ModifyCardPlayResultPileTypeAndPosition,
    ModifyPowerAmountGivenAdditive,
    ModifyPowerAmountGivenMultiplicative,
    TryModifyPowerAmountReceived,
    ModifyXValue,
    ShouldDie,
    ShouldDieLate,
    ShouldTakeExtraTurn,
    ShouldPlayerResetEnergy,
    TryModifyRestSiteOptions,
    ShouldForcePotionReward,
    ModifyMerchantPrice,
    ShouldRefillMerchantEntry,
    ShouldGenerateTreasure,
    ModifyEnergyGain,
    TryModifyStarCost,
    ShouldDisableRemainingRestSiteOptions,
}

impl ModelHook {
    pub const ALL: [ModelHook; 153] = [
        ModelHook::AfterRoomEntered,
        ModelHook::AfterMapGenerated,
        ModelHook::AfterActEntered,
        ModelHook::BeforeCombatStart,
        ModelHook::AfterCombatEnd,
        ModelHook::AfterCardPlayed,
        ModelHook::AfterDamageReceived,
        ModelHook::AfterDamageGiven,
        ModelHook::AfterCardDrawn,
        ModelHook::BeforeSideTurnStart,
        ModelHook::AfterSideTurnStart,
        ModelHook::BeforeSideTurnEndEarly,
        ModelHook::AfterSideTurnEnd,
        ModelHook::AfterSideTurnEndLate,
        ModelHook::AfterEnergyReset,
        ModelHook::AfterTakingExtraTurn,
        ModelHook::AfterAutoPrePlayPhaseEnteredEarly,
        ModelHook::AfterAutoPrePlayPhaseEntered,
        ModelHook::AfterAutoPrePlayPhaseEnteredLate,
        ModelHook::AfterAutoPostPlayPhaseEntered,
        ModelHook::AfterStarsSpent,
        ModelHook::AfterHandEmptied,
        ModelHook::AfterBlockCleared,
        ModelHook::AfterDeath,
        ModelHook::AfterPowerAmountChanged,
        ModelHook::AfterCardEnteredCombat,
        ModelHook::AfterShuffle,
        ModelHook::AfterModifyingHandDraw,
        ModelHook::BeforePotionUsed,
        ModelHook::AfterPotionUsed,
        ModelHook::AfterPotionProcured,
        ModelHook::AfterPotionDiscarded,
        ModelHook::AfterCardPlayedLate,
        ModelHook::AfterSideTurnStartLate,
        ModelHook::AfterPreventingDraw,
        ModelHook::AfterCardDrawnEarly,
        ModelHook::BeforeCardAutoPlayed,
        ModelHook::BeforeCardPlayed,
        ModelHook::BeforeDeath,
        ModelHook::AfterBlockBroken,
        ModelHook::AfterCombatVictory,
        ModelHook::AfterPlayerTurnStart,
        ModelHook::AfterPlayerTurnStartLate,
        ModelHook::AfterOrbChanneled,
        ModelHook::AfterOrbEvoked,
        ModelHook::AfterModifyingOrbPassiveTriggerCount,
        ModelHook::AfterEnergyResetLate,
        ModelHook::AfterOstyRevived,
        ModelHook::AfterCreatureAddedToCombat,
        ModelHook::AfterSummon,
        ModelHook::BeforeDamageReceived,
        ModelHook::AfterCurrentHpChanged,
        ModelHook::AfterRestSiteHeal,
        ModelHook::AfterModifyingRewards,
        ModelHook::AfterModifyingGoldGained,
        ModelHook::AfterGoldGained,
        ModelHook::BeforeAttack,
        ModelHook::AfterAttack,
        ModelHook::BeforeHandDraw,
        ModelHook::AfterCardExhausted,
        ModelHook::AfterCardDiscarded,
        ModelHook::AfterModifyingCardPlayCount,
        ModelHook::AfterModifyingHpLostAfterOsty,
        ModelHook::AfterModifyingCardPlayResultPileOrPosition,
        ModelHook::BeforeSideTurnEndVeryEarly,
        ModelHook::BeforeSideTurnEnd,
        ModelHook::AfterDiedToDoom,
        ModelHook::BeforePowerAmountChanged,
        ModelHook::AfterModifyingPowerAmountGiven,
        ModelHook::AfterModifyingPowerAmountReceived,
        ModelHook::AfterCardChangedPiles,
        ModelHook::AfterCardGeneratedForCombat,
        ModelHook::AfterCombatVictoryEarly,
        ModelHook::BeforeCombatStartLate,
        ModelHook::AfterFlush,
        ModelHook::AfterModifyingBlockAmount,
        ModelHook::AfterModifyingCardRewardOptions,
        ModelHook::AfterModifyingEnergyGain,
        ModelHook::AfterStarsGained,
        ModelHook::AfterBlockGained,
        ModelHook::BeforeFlushLate,
        ModelHook::AfterEnergySpent,
        ModelHook::AfterForge,
        ModelHook::AfterItemPurchased,
        ModelHook::BeforeRoomEntered,
        ModelHook::BeforeCombatRewardOffered,
        ModelHook::ModifyDamageAdditive,
        ModelHook::ModifyDamageMultiplicative,
        ModelHook::ModifyDamageCap,
        ModelHook::ModifyBlockAdditive,
        ModelHook::ModifyBlockMultiplicative,
        ModelHook::ModifyHpLostBeforeOsty,
        ModelHook::ModifyHpLostBeforeOstyLate,
        ModelHook::ModifyHpLostAfterOsty,
        ModelHook::ModifyHpLostAfterOstyLate,
        ModelHook::ModifyHandDraw,
        ModelHook::ModifyHandDrawLate,
        ModelHook::ModifyShuffleOrder,
        ModelHook::ShouldProcurePotion,
        ModelHook::ModifyNextEvent,
        ModelHook::TryModifyCardBeingAddedToDeck,
        ModelHook::TryModifyCardBeingAddedToDeckLate,
        ModelHook::ModifyMerchantCardCreationResults,
        ModelHook::ShouldDraw,
        ModelHook::ShouldPlay,
        ModelHook::ModifyMaxEnergy,
        ModelHook::TryModifyEnergyCostInCombat,
        ModelHook::TryModifyEnergyCostInCombatLate,
        ModelHook::ShouldCreatureBeRemovedFromCombatAfterDeath,
        ModelHook::ShouldPowerBeRemovedOnDeath,
        ModelHook::ShouldClearBlock,
        ModelHook::ShouldStopCombatFromEnding,
        ModelHook::ShouldAllowHitting,
        ModelHook::ModifyOrbValue,
        ModelHook::ModifyOrbPassiveTriggerCounts,
        ModelHook::ModifyUnblockedDamageTarget,
        ModelHook::ModifySummonAmount,
        ModelHook::ModifyRestSiteHealAmount,
        ModelHook::TryModifyRestSiteHealRewards,
        ModelHook::ShouldAllowFreeTravel,
        ModelHook::ShouldAllowMerchantCardRemoval,
        ModelHook::TryModifyRewards,
        ModelHook::TryModifyRewardsLate,
        ModelHook::TryModifyCardRewardAlternatives,
        ModelHook::ModifyUnknownMapPointRoomTypes,
        ModelHook::ModifyGeneratedMap,
        ModelHook::ModifyGeneratedMapLate,
        ModelHook::ModifyCardRewardCreationOptions,
        ModelHook::ModifyMerchantCardPool,
        ModelHook::ModifyOddsIncreaseForUnrolledRoomType,
        ModelHook::ModifyCardRewardUpgradeOdds,
        ModelHook::TryModifyCardRewardOptions,
        ModelHook::TryModifyCardRewardOptionsLate,
        ModelHook::ModifyGoldGained,
        ModelHook::ShouldFlush,
        ModelHook::ModifyCardPlayCount,
        ModelHook::ModifyCardPlayResultPileTypeAndPosition,
        ModelHook::ModifyPowerAmountGivenAdditive,
        ModelHook::ModifyPowerAmountGivenMultiplicative,
        ModelHook::TryModifyPowerAmountReceived,
        ModelHook::ModifyXValue,
        ModelHook::ShouldDie,
        ModelHook::ShouldDieLate,
        ModelHook::ShouldTakeExtraTurn,
        ModelHook::ShouldPlayerResetEnergy,
        ModelHook::TryModifyRestSiteOptions,
        ModelHook::ShouldForcePotionReward,
        ModelHook::ModifyMerchantPrice,
        ModelHook::ShouldRefillMerchantEntry,
        ModelHook::ShouldGenerateTreasure,
        ModelHook::ModifyEnergyGain,
        ModelHook::TryModifyStarCost,
        ModelHook::ShouldDisableRemainingRestSiteOptions,
    ];

    /// The hook's name on `AbstractModel`, which is the dump's.
    pub const fn name(self) -> &'static str {
        match self {
            ModelHook::AfterRoomEntered => "AfterRoomEntered",
            ModelHook::AfterMapGenerated => "AfterMapGenerated",
            ModelHook::AfterActEntered => "AfterActEntered",
            ModelHook::ModifyGeneratedMap => "ModifyGeneratedMap",
            ModelHook::ModifyGeneratedMapLate => "ModifyGeneratedMapLate",
            ModelHook::ModifyCardRewardCreationOptions => "ModifyCardRewardCreationOptions",
            ModelHook::ModifyMerchantCardPool => "ModifyMerchantCardPool",
            ModelHook::BeforeCombatStart => "BeforeCombatStart",
            ModelHook::AfterCombatEnd => "AfterCombatEnd",
            ModelHook::AfterCardPlayed => "AfterCardPlayed",
            ModelHook::AfterDamageReceived => "AfterDamageReceived",
            ModelHook::AfterDamageGiven => "AfterDamageGiven",
            ModelHook::AfterCardDrawn => "AfterCardDrawn",
            ModelHook::BeforeSideTurnStart => "BeforeSideTurnStart",
            ModelHook::AfterSideTurnStart => "AfterSideTurnStart",
            ModelHook::BeforeSideTurnEndEarly => "BeforeSideTurnEndEarly",
            ModelHook::AfterSideTurnEnd => "AfterSideTurnEnd",
            ModelHook::AfterSideTurnEndLate => "AfterSideTurnEndLate",
            ModelHook::AfterEnergyReset => "AfterEnergyReset",
            ModelHook::AfterAutoPrePlayPhaseEnteredEarly => "AfterAutoPrePlayPhaseEnteredEarly",
            ModelHook::AfterAutoPrePlayPhaseEntered => "AfterAutoPrePlayPhaseEntered",
            ModelHook::AfterAutoPrePlayPhaseEnteredLate => "AfterAutoPrePlayPhaseEnteredLate",
            ModelHook::AfterAutoPostPlayPhaseEntered => "AfterAutoPostPlayPhaseEntered",
            ModelHook::AfterStarsSpent => "AfterStarsSpent",
            ModelHook::AfterHandEmptied => "AfterHandEmptied",
            ModelHook::AfterBlockCleared => "AfterBlockCleared",
            ModelHook::AfterDeath => "AfterDeath",
            ModelHook::AfterPowerAmountChanged => "AfterPowerAmountChanged",
            ModelHook::AfterCardEnteredCombat => "AfterCardEnteredCombat",
            ModelHook::BeforeCombatRewardOffered => "BeforeCombatRewardOffered",
            ModelHook::ModifyDamageAdditive => "ModifyDamageAdditive",
            ModelHook::ModifyDamageMultiplicative => "ModifyDamageMultiplicative",
            ModelHook::ModifyDamageCap => "ModifyDamageCap",
            ModelHook::ModifyBlockAdditive => "ModifyBlockAdditive",
            ModelHook::ModifyBlockMultiplicative => "ModifyBlockMultiplicative",
            ModelHook::ModifyHpLostBeforeOsty => "ModifyHpLostBeforeOsty",
            ModelHook::ModifyHpLostBeforeOstyLate => "ModifyHpLostBeforeOstyLate",
            ModelHook::ModifyHpLostAfterOsty => "ModifyHpLostAfterOsty",
            ModelHook::ModifyHpLostAfterOstyLate => "ModifyHpLostAfterOstyLate",
            ModelHook::ModifyHandDraw => "ModifyHandDraw",
            ModelHook::ModifyHandDrawLate => "ModifyHandDrawLate",
            ModelHook::ModifyShuffleOrder => "ModifyShuffleOrder",
            ModelHook::AfterShuffle => "AfterShuffle",
            ModelHook::AfterModifyingHandDraw => "AfterModifyingHandDraw",
            ModelHook::ShouldProcurePotion => "ShouldProcurePotion",
            ModelHook::ModifyNextEvent => "ModifyNextEvent",
            ModelHook::BeforePotionUsed => "BeforePotionUsed",
            ModelHook::AfterPotionUsed => "AfterPotionUsed",
            ModelHook::AfterPotionProcured => "AfterPotionProcured",
            ModelHook::AfterPotionDiscarded => "AfterPotionDiscarded",
            ModelHook::TryModifyCardBeingAddedToDeck => "TryModifyCardBeingAddedToDeck",
            ModelHook::TryModifyCardBeingAddedToDeckLate => "TryModifyCardBeingAddedToDeckLate",
            ModelHook::ModifyMerchantCardCreationResults => "ModifyMerchantCardCreationResults",
            ModelHook::AfterCardPlayedLate => "AfterCardPlayedLate",
            ModelHook::AfterSideTurnStartLate => "AfterSideTurnStartLate",
            ModelHook::ShouldDraw => "ShouldDraw",
            ModelHook::AfterPreventingDraw => "AfterPreventingDraw",
            ModelHook::AfterCardDrawnEarly => "AfterCardDrawnEarly",
            ModelHook::ShouldPlay => "ShouldPlay",
            ModelHook::BeforeCardAutoPlayed => "BeforeCardAutoPlayed",
            ModelHook::ModifyMaxEnergy => "ModifyMaxEnergy",
            ModelHook::TryModifyEnergyCostInCombat => "TryModifyEnergyCostInCombat",
            ModelHook::TryModifyEnergyCostInCombatLate => "TryModifyEnergyCostInCombatLate",
            ModelHook::BeforeCardPlayed => "BeforeCardPlayed",
            ModelHook::ShouldCreatureBeRemovedFromCombatAfterDeath => {
                "ShouldCreatureBeRemovedFromCombatAfterDeath"
            }
            ModelHook::ShouldPowerBeRemovedOnDeath => "ShouldPowerBeRemovedOnDeath",
            ModelHook::ShouldClearBlock => "ShouldClearBlock",
            ModelHook::ShouldStopCombatFromEnding => "ShouldStopCombatFromEnding",
            ModelHook::ShouldAllowHitting => "ShouldAllowHitting",
            ModelHook::BeforeDeath => "BeforeDeath",
            ModelHook::AfterBlockBroken => "AfterBlockBroken",
            ModelHook::AfterCombatVictory => "AfterCombatVictory",
            ModelHook::AfterPlayerTurnStart => "AfterPlayerTurnStart",
            ModelHook::AfterPlayerTurnStartLate => "AfterPlayerTurnStartLate",
            ModelHook::AfterOrbChanneled => "AfterOrbChanneled",
            ModelHook::AfterOrbEvoked => "AfterOrbEvoked",
            ModelHook::ModifyOrbValue => "ModifyOrbValue",
            ModelHook::AfterModifyingOrbPassiveTriggerCount => {
                "AfterModifyingOrbPassiveTriggerCount"
            }
            ModelHook::ModifyOrbPassiveTriggerCounts => "ModifyOrbPassiveTriggerCounts",
            ModelHook::AfterEnergyResetLate => "AfterEnergyResetLate",
            ModelHook::ModifyUnblockedDamageTarget => "ModifyUnblockedDamageTarget",
            ModelHook::AfterOstyRevived => "AfterOstyRevived",
            ModelHook::AfterCreatureAddedToCombat => "AfterCreatureAddedToCombat",
            ModelHook::ModifySummonAmount => "ModifySummonAmount",
            ModelHook::AfterSummon => "AfterSummon",
            ModelHook::BeforeDamageReceived => "BeforeDamageReceived",
            ModelHook::AfterCurrentHpChanged => "AfterCurrentHpChanged",
            ModelHook::AfterRestSiteHeal => "AfterRestSiteHeal",
            ModelHook::ModifyRestSiteHealAmount => "ModifyRestSiteHealAmount",
            ModelHook::TryModifyRestSiteHealRewards => "TryModifyRestSiteHealRewards",
            ModelHook::ShouldAllowFreeTravel => "ShouldAllowFreeTravel",
            ModelHook::ShouldAllowMerchantCardRemoval => "ShouldAllowMerchantCardRemoval",
            ModelHook::TryModifyRewards => "TryModifyRewards",
            ModelHook::TryModifyRewardsLate => "TryModifyRewardsLate",
            ModelHook::TryModifyCardRewardAlternatives => "TryModifyCardRewardAlternatives",
            ModelHook::AfterModifyingRewards => "AfterModifyingRewards",
            ModelHook::ModifyUnknownMapPointRoomTypes => "ModifyUnknownMapPointRoomTypes",
            ModelHook::ModifyOddsIncreaseForUnrolledRoomType => {
                "ModifyOddsIncreaseForUnrolledRoomType"
            }
            ModelHook::ModifyCardRewardUpgradeOdds => "ModifyCardRewardUpgradeOdds",
            ModelHook::TryModifyCardRewardOptions => "TryModifyCardRewardOptions",
            ModelHook::TryModifyCardRewardOptionsLate => "TryModifyCardRewardOptionsLate",
            ModelHook::ModifyGoldGained => "ModifyGoldGained",
            ModelHook::AfterModifyingGoldGained => "AfterModifyingGoldGained",
            ModelHook::AfterGoldGained => "AfterGoldGained",
            ModelHook::ShouldFlush => "ShouldFlush",
            ModelHook::BeforeAttack => "BeforeAttack",
            ModelHook::AfterAttack => "AfterAttack",
            ModelHook::BeforeHandDraw => "BeforeHandDraw",
            ModelHook::AfterCardExhausted => "AfterCardExhausted",
            ModelHook::AfterCardDiscarded => "AfterCardDiscarded",
            ModelHook::ModifyCardPlayCount => "ModifyCardPlayCount",
            ModelHook::AfterModifyingCardPlayCount => "AfterModifyingCardPlayCount",
            ModelHook::AfterModifyingHpLostAfterOsty => "AfterModifyingHpLostAfterOsty",
            ModelHook::ModifyCardPlayResultPileTypeAndPosition => {
                "ModifyCardPlayResultPileTypeAndPosition"
            }
            ModelHook::AfterModifyingCardPlayResultPileOrPosition => {
                "AfterModifyingCardPlayResultPileOrPosition"
            }
            ModelHook::BeforeSideTurnEndVeryEarly => "BeforeSideTurnEndVeryEarly",
            ModelHook::BeforeSideTurnEnd => "BeforeSideTurnEnd",
            ModelHook::AfterDiedToDoom => "AfterDiedToDoom",
            ModelHook::BeforePowerAmountChanged => "BeforePowerAmountChanged",
            ModelHook::ModifyPowerAmountGivenAdditive => "ModifyPowerAmountGivenAdditive",
            ModelHook::ModifyPowerAmountGivenMultiplicative => {
                "ModifyPowerAmountGivenMultiplicative"
            }
            ModelHook::TryModifyPowerAmountReceived => "TryModifyPowerAmountReceived",
            ModelHook::AfterModifyingPowerAmountGiven => "AfterModifyingPowerAmountGiven",
            ModelHook::AfterModifyingPowerAmountReceived => "AfterModifyingPowerAmountReceived",
            ModelHook::ModifyXValue => "ModifyXValue",
            ModelHook::AfterCardChangedPiles => "AfterCardChangedPiles",
            ModelHook::AfterCardGeneratedForCombat => "AfterCardGeneratedForCombat",
            ModelHook::ShouldDie => "ShouldDie",
            ModelHook::ShouldDieLate => "ShouldDieLate",
            ModelHook::ShouldTakeExtraTurn => "ShouldTakeExtraTurn",
            ModelHook::AfterTakingExtraTurn => "AfterTakingExtraTurn",
            ModelHook::AfterCombatVictoryEarly => "AfterCombatVictoryEarly",
            ModelHook::ShouldPlayerResetEnergy => "ShouldPlayerResetEnergy",
            ModelHook::BeforeCombatStartLate => "BeforeCombatStartLate",
            ModelHook::TryModifyRestSiteOptions => "TryModifyRestSiteOptions",
            ModelHook::ShouldForcePotionReward => "ShouldForcePotionReward",
            ModelHook::AfterFlush => "AfterFlush",
            ModelHook::AfterModifyingBlockAmount => "AfterModifyingBlockAmount",
            ModelHook::ModifyMerchantPrice => "ModifyMerchantPrice",
            ModelHook::ShouldRefillMerchantEntry => "ShouldRefillMerchantEntry",
            ModelHook::AfterModifyingCardRewardOptions => "AfterModifyingCardRewardOptions",
            ModelHook::ShouldGenerateTreasure => "ShouldGenerateTreasure",
            ModelHook::ModifyEnergyGain => "ModifyEnergyGain",
            ModelHook::AfterModifyingEnergyGain => "AfterModifyingEnergyGain",
            ModelHook::TryModifyStarCost => "TryModifyStarCost",
            ModelHook::ShouldDisableRemainingRestSiteOptions => {
                "ShouldDisableRemainingRestSiteOptions"
            }
            ModelHook::AfterStarsGained => "AfterStarsGained",
            ModelHook::AfterBlockGained => "AfterBlockGained",
            ModelHook::BeforeFlushLate => "BeforeFlushLate",
            ModelHook::AfterEnergySpent => "AfterEnergySpent",
            ModelHook::AfterForge => "AfterForge",
            ModelHook::AfterItemPurchased => "AfterItemPurchased",
            ModelHook::BeforeRoomEntered => "BeforeRoomEntered",
        }
    }
}

/// The [`ModelHook`]s one model's class overrides, one bit each.
///
/// Two words: the event hooks in one, and the value hooks in the other.
/// Value hooks are the `Modify`, `TryModify` and `Should` ones. A set built
/// from one known hook leaves the other word empty, so testing it reads one
/// word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Overrides {
    events: u128,
    values: u128,
}

impl ModelHook {
    /// The first value hook. Every hook declared before it is an event hook.
    const FIRST_VALUE: u8 = ModelHook::ModifyDamageAdditive as u8;
}

impl Overrides {
    /// The set holding no hook.
    pub const NONE: Self = Overrides {
        events: 0,
        values: 0,
    };

    pub const fn with(self, hook: ModelHook) -> Self {
        self.union(Self::of(hook))
    }

    /// The set without `hook`.
    pub const fn without(self, hook: ModelHook) -> Self {
        let one = Self::of(hook);
        Overrides {
            events: self.events & !one.events,
            values: self.values & !one.values,
        }
    }

    /// The set holding `hook` alone.
    pub const fn of(hook: ModelHook) -> Self {
        let at = hook as u8;
        match at < ModelHook::FIRST_VALUE {
            true => Overrides {
                events: 1 << at,
                values: 0,
            },
            false => Overrides {
                events: 0,
                values: 1 << (at - ModelHook::FIRST_VALUE),
            },
        }
    }

    pub const fn contains(self, hook: ModelHook) -> bool {
        let at = hook as u8;
        match at < ModelHook::FIRST_VALUE {
            true => self.events >> at & 1 != 0,
            false => self.values >> (at - ModelHook::FIRST_VALUE) & 1 != 0,
        }
    }

    /// Whether the two sets share a hook.
    pub const fn intersects(self, other: Self) -> bool {
        self.events & other.events != 0 || self.values & other.values != 0
    }

    pub const fn union(self, other: Self) -> Self {
        Overrides {
            events: self.events | other.events,
            values: self.values | other.values,
        }
    }

    /// Each hook in the set, by its place in [`ModelHook::ALL`].
    pub fn places(self) -> impl Iterator<Item = usize> {
        let first = u32::from(ModelHook::FIRST_VALUE);
        let (mut events, mut values) = (self.events, self.values);
        std::iter::from_fn(move || {
            if events != 0 {
                let at = events.trailing_zeros();
                events &= events - 1;
                Some(at as usize)
            } else if values != 0 {
                let at = values.trailing_zeros();
                values &= values - 1;
                Some((first + at) as usize)
            } else {
                None
            }
        })
    }
}

/// What `AfflictionModel.CanAfflict` reads off an affliction.
pub trait AfflictionTable: Copy {
    /// `CanAfflictCardType`: whether it goes on a card of this type.
    fn afflicts(self, kind: CardKind) -> bool;
    /// `CanAfflictUnplayableCards`.
    fn afflicts_unplayable(self) -> bool;
    /// `IsStackable`: whether a second goes onto a card that has this one.
    fn stackable(self) -> bool;
    /// The hooks this affliction's class overrides, as the hook dump
    /// measured them. A card carrying it hears them too. See
    /// [`PowerTable::overrides`].
    fn overrides(self) -> Overrides;
}

/// What a table says about an enchantment, beyond its row.
pub trait EnchantmentTable: Copy {
    /// The hooks this enchantment's class overrides, as the hook dump
    /// measured them. A card carrying it hears them too. See
    /// [`PowerTable::overrides`].
    fn overrides(self) -> Overrides;

    /// Every hook some enchantment's class in the build overrides.
    fn heard_by_any() -> Overrides;
}

/// What a table says about a run modifier, beyond its row.
pub trait ModifierTable: Copy {
    /// The hooks this modifier's class overrides, as the hook dump measured
    /// them. See [`PowerTable::overrides`].
    fn overrides(self) -> Overrides;

    /// `ClearsPlayerDeck`: `OnRunCreated` empties the starting deck.
    fn clears_deck(self) -> bool;

    /// `GenerateNeowOption` gives Neow an option for it.
    fn offers_neow_option(self) -> bool;
}

/// `EnchantmentStatus`: whether an enchantment still acts, which its own
/// `OnPlay` reads and sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum EnchantmentStatus {
    #[default]
    Normal,
    Disabled,
}

/// One enchantment on one card copy, as a build keeps it: its
/// `EnchantmentModel.Status`, and the fields its own class adds.
pub trait EnchantmentModel<V: ContentVersion>:
    std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static
{
    /// A fresh instance of `enchantment`, as `CardCmd.Enchant` makes one.
    /// Nothing is the state a copy holds with no enchantment.
    fn new(enchantment: Option<V::Enchantment>) -> Self;

    fn status(&self) -> EnchantmentStatus;

    fn set_status(&mut self, status: EnchantmentStatus);
}

/// The state of a build whose enchantments keep no field beyond `Status`.
impl<V: ContentVersion> EnchantmentModel<V> for EnchantmentStatus {
    fn new(_enchantment: Option<V::Enchantment>) -> Self {
        EnchantmentStatus::Normal
    }

    fn status(&self) -> EnchantmentStatus {
        *self
    }

    fn set_status(&mut self, status: EnchantmentStatus) {
        *self = status;
    }
}

/// One `DamageResult`: what a hit did to one creature it reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageResult<V: ContentVersion> {
    pub receiver: crate::CombatId,
    /// Which of the attack's hits it came from, counted from zero.
    /// `AttackCommand.Results` holds a list of results for each hit.
    pub hit: u8,
    /// `TotalDamage`: what the block took and what the hit points lost.
    pub total: V::Number,
    /// `UnblockedDamage`: what the hit points lost.
    pub unblocked: V::Number,
    /// `OverkillDamage`: what the hit had left once the hit points ran out.
    pub overkill: V::Number,
    /// `WasTargetKilled`.
    pub killed: bool,
}

/// `AttackCommand.Results`: every hit's results, in the order they landed,
/// each naming its hit.
pub type AttackResults<V> = smallvec::SmallVec<[DamageResult<V>; 4]>;

/// The player's current and maximum hit points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerHealth<V: ContentVersion> {
    pub hp: V::Health,
    pub max_hp: V::Health,
}

/// `Owner`, as a relic reads it: the player's creature, and its health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelicOwner<V: ContentVersion> {
    /// `Owner.Creature`.
    pub creature: crate::CombatId,
    /// Its current and maximum hit points, which are the run's outside a
    /// fight.
    pub health: PlayerHealth<V>,
}

/// `cardSource`, the card whose number a damage hook is working out: which
/// copy, which card, and whether it is being played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardSource<V: ContentVersion> {
    pub id: crate::CardId<V>,
    pub card: V::Card,
    /// `Owner.Creature`, the player whose card it is.
    pub owner: crate::CombatId,
    /// `Pile.Type == PileType.Play`: the card stands in the play pile, as it
    /// does while it is played. A card in hand is being previewed.
    pub playing: bool,
    /// `IsUpgraded`.
    pub upgraded: bool,
    /// Whether an enchantment sits on it.
    pub enchanted: bool,
}

/// A number an enchantment works out for the card it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnchantHook {
    /// `EnchantDamageAdditive`, for a hit the card deals. `powered` is
    /// `props.IsPoweredAttack()`.
    DamageAdditive { powered: bool },
    /// `EnchantDamageMultiplicative`, over the hit after
    /// [`EnchantHook::DamageAdditive`].
    DamageMultiplicative { powered: bool },
    /// `EnchantBlockAdditive`, for block the card gives.
    BlockAdditive,
    /// `EnchantPlayCount`, over how many times the card's play runs.
    PlayCount,
}

/// What a table says about a relic, beyond its row.
pub trait RelicTable: Table {
    /// The pools a relic can be drawn from.
    type Pool: RelicPoolKinds + serde::Serialize;

    /// The hooks this relic's class overrides, as the hook dump measured them.
    /// See [`PowerTable::overrides`].
    fn overrides(self) -> Overrides;

    /// The relics `pool` drafts, in the pool's own order, which is
    /// `GenerateAllRelics`. A relic bag is filled in it before the shuffle.
    fn members(pool: Self::Pool) -> &'static [Self]
    where
        Self: Sized;

    /// How often it turns up.
    ///
    /// The one key on these traits carrying a shared vocabulary's bounds rather
    /// than a sweep's, because it appears in [`crate::Question`]. The crate
    /// README says why a reward screen puts it there.
    type Rarity: RarityKinds + serde::Serialize + serde::de::DeserializeOwned;

    /// Every pool that holds it. Empty for one nothing draws.
    fn pools(self) -> &'static [Self::Pool];

    /// Its rarity, which is also which screen offers it: a boss relic comes
    /// from a boss and nowhere else.
    fn rarity(self) -> Self::Rarity;

    /// Whether taking it does something there and then, which is
    /// `RelicModel.HasUponPickupEffect`.
    fn has_pickup_effect(self) -> bool;

    /// What a merchant charges before the haggle, or nothing for a relic no
    /// merchant stocks.
    ///
    /// `MerchantCost`, whose sentinel for the unsellable is absent here.
    fn merchant_price(self) -> Option<u16>;

    /// Whether a merchant is allowed to stock it, which is `IsAllowedInShops`.
    fn allowed_in_shops(self) -> bool;

    /// Where this relic sorts against every other: `ModelId.CompareTo`,
    /// ordinal over the id the wire spells. `StableShuffle` sorts by it.
    fn order(self) -> u16;

    /// `IsStackable`: whether a run can hold more than one. `RelicCmd.Obtain`
    /// takes one that is not out of both relic bags.
    fn stackable(self) -> bool;
}

/// What a table says about a potion, beyond its row.
pub trait PotionTable: Table {
    /// The pool a potion is drawn from.
    type Pool: Copy + serde::Serialize + 'static;
    /// What it can be pointed at, which is what a card is pointed at: a
    /// potion targets with the cards domain's own type.
    type Target: TargetKinds + serde::Serialize + 'static;

    /// The pool that holds it: shared, one character's, or one of the pools
    /// nothing in a run draws from.
    fn pool(self) -> Self::Pool;

    /// When it can be drunk at all.
    fn usage(self) -> PotionUsage;

    /// What it can be pointed at, or `None` for one that works it out from a
    /// fight rather than printing it, which
    /// [`Rules::computed_potion_target`] answers.
    fn target(self) -> Option<Self::Target>;

    /// Whether the potion's own check lets it be drunk.
    ///
    /// `PotionModel.PassesCustomUsabilityCheck`, which is `virtual`. `None`
    /// for the one potion that answers it from live state, which
    /// [`Rules::computed_potion_check`] answers.
    fn passes_own_check(self) -> Option<bool>;

    /// `PotionModel.CanBeGeneratedInCombat`, which a potion made mid-fight
    /// is drawn among.
    fn generated_in_combat(self) -> bool;

    /// The hooks this potion's class overrides, as the hook dump measured
    /// them. See [`PowerTable::overrides`].
    fn overrides(self) -> Overrides;
}

/// When a potion can be drunk, reduced to what a rule asks.
///
/// The game's `PotionUsage`, minus the zero that its own switch throws on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PotionUsage {
    /// In a fight, and only in a fight. Sixty of this build's sixty-five.
    CombatOnly,
    AnyTime,
    /// Drunk by the game itself, never by hand.
    Automatic,
}

/// What a table says about a character, beyond its row.
pub trait CharacterTable: Table {
    /// Offered by the character select screen.
    ///
    /// Three of this build's eight are not. Those are the deprecated
    /// placeholder, the Deprived, and the `random` pseudo-character. They are
    /// not runs a measurement counts itself as missing.
    fn playable(self) -> bool;
}

/// A set of flags over a generated enum, as much of one as a generic caller
/// needs.
///
/// Two methods, which let the wire layer build a set and a reader ask it. A
/// caller holding only a [`ContentVersion`] cannot reach the generated
/// inherent `new`.
///
/// Deliberately not the whole surface. The crate README says what is left out
/// and why.
pub trait FlagSet: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static {
    /// The enum the bits are over.
    type Member: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static;

    /// Every member the set has a bit for, in the enum's own order.
    ///
    /// What turns `contains` into "what is in here", for a caller that cannot
    /// enumerate the member enum. The digest is what wanted it, because a set
    /// has to be walked to be hashed.
    const MEMBERS: &'static [Self::Member];

    /// The set holding exactly these members. Repeats are one member.
    fn from_members(members: &[Self::Member]) -> Self;

    fn contains(self, member: Self::Member) -> bool;

    /// This set, and one more member.
    ///
    /// A set, so a member already in it is not added twice. Walks
    /// [`FlagSet::MEMBERS`] rather than asking for a bitwise `or`, because a
    /// set is generated per build and this is the same for all of them.
    fn with(self, member: Self::Member) -> Self {
        let held: smallvec::SmallVec<[Self::Member; MEMBERS_HELD]> = Self::MEMBERS
            .iter()
            .copied()
            .filter(|&held| held == member || self.contains(held))
            .collect();
        Self::from_members(&held)
    }

    /// This set, less one member, the way [`FlagSet::with`] adds one.
    fn without(self, member: Self::Member) -> Self {
        let held: smallvec::SmallVec<[Self::Member; MEMBERS_HELD]> = Self::MEMBERS
            .iter()
            .copied()
            .filter(|&held| held != member && self.contains(held))
            .collect();
        Self::from_members(&held)
    }
}

/// How many members one set holds while it is being rebuilt.
///
/// Every flag set this build generates is a byte wide, so eight covers them
/// with nothing to spare and nothing wasted.
const MEMBERS_HELD: usize = 8;

/// The nine kinds of node an act's map holds, as consts a rule can name.
///
/// The generated enum is per build. Code holding only a [`ContentVersion`]
/// cannot write `MapPointType::Elite`, and map generation has to: half its
/// rules say which kinds sit beside which.
///
/// Nine and not fewer. A build that drops or renames one fails generation
/// rather than mapping to the wrong node.
pub trait MapPointKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// The grid before the types are dealt.
    const UNASSIGNED: Self;
    const UNKNOWN: Self;
    const SHOP: Self;
    const TREASURE: Self;
    const REST_SITE: Self;
    const MONSTER: Self;
    const ELITE: Self;
    const BOSS: Self;
    /// The node an act starts on.
    const ANCIENT: Self;
}

/// The kinds of room run setup sorts an act's encounters into.
///
/// The same bridge as [`MapPointKinds`], for the three a roster is split on. A
/// treasure or a shop is a room a map point leads to and never an encounter.
pub trait RoomKinds: std::fmt::Debug + Copy + Eq + 'static {
    const MONSTER: Self;
    const ELITE: Self;
    const BOSS: Self;
    /// The four a refusal names, for the commands a room has to be the right
    /// kind for.
    const TREASURE: Self;
    const SHOP: Self;
    const EVENT: Self;
    const REST_SITE: Self;
}

/// What `CardRarity` names, as consts a rule can name. All ten, so a build
/// that adds one stops codegen.
///
/// Ordered by the game's own values, which a selection from the draw pile
/// sorts its cards by.
pub trait CardRarityKinds: std::fmt::Debug + Copy + Eq + Ord + 'static {
    const BASIC: Self;
    const COMMON: Self;
    const UNCOMMON: Self;
    const RARE: Self;
    const ANCIENT: Self;
    const EVENT: Self;
    const TOKEN: Self;
    const STATUS: Self;
    const CURSE: Self;
    const QUEST: Self;
}

/// What `CardTag` names, as consts a rule can name. All five, so a build
/// that adds one stops codegen.
pub trait CardTagKinds: std::fmt::Debug + Copy + Eq + 'static {
    const STRIKE: Self;
    const DEFEND: Self;
    const MINION: Self;
    const OSTY_ATTACK: Self;
    const SHIV: Self;
}

/// The monster the simulator names itself.
pub trait MonsterKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// `Osty`, the pet `OstyCmd.Summon` stands up.
    const OSTY: Self;
}

/// The power the simulator names itself.
pub trait PowerKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// `DieForYouPower`, which every Osty is summoned with.
    const DIE_FOR_YOU: Self;
    /// `DoomPower`, whose holders die together as their side's turn ends.
    const DOOM: Self;
    /// `ArtifactPower`, which `BeforePowerAmountChanged` reports its target
    /// holding.
    const ARTIFACT: Self;
}

/// The move names the simulator writes itself.
pub trait MoveKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// `UNSET_MOVE`, which a creature that has not rolled a move reports. A
    /// pet never rolls one.
    const UNSET: Self;
}

/// The sides of a fight, as consts a rule can name.
///
/// The same bridge as [`MapPointKinds`]. The game's enum has a third member, its
/// C# zero, which no creature in a fight is on.
pub trait SideKinds: std::fmt::Debug + Copy + Eq + 'static {
    const PLAYER: Self;
    const ENEMY: Self;
}

/// The phases of a player's turn, as consts a rule can name.
///
/// The same bridge as [`MapPointKinds`]. `PLAY` is the phase a decision is taken
/// in. The rest are the game working through its own queues, and a front end
/// meets them in an observation it cannot act on.
pub trait TurnPhaseKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// Between turns, and after a fight ends.
    const NONE: Self;
    const START: Self;
    const AUTO_PRE_PLAY: Self;
    const PLAY: Self;
    const AUTO_POST_PLAY: Self;
    const END: Self;
}

/// The rules a card in hand can fail, as consts a rule can name.
///
/// The same bridge as [`MapPointKinds`], over the members of
/// [`ContentVersion::UnplayableReason`]. Legality is decided by code holding
/// only a [`ContentVersion`], and it has to say which rule said no.
pub trait UnplayableReasonKinds: std::fmt::Debug + Copy + Eq + 'static {
    const HAS_UNPLAYABLE_KEYWORD: Self;
    /// Something else said no, and `preventer` names which model.
    const BLOCKED_BY_HOOK: Self;
    /// The card said no about itself.
    const BLOCKED_BY_CARD_LOGIC: Self;
    const ENERGY_COST_TOO_HIGH: Self;
    const STAR_COST_TOO_HIGH: Self;
    const NO_LIVING_ALLIES: Self;
}

/// What a card can be pointed at, as consts a rule can name.
///
/// The same bridge as [`MapPointKinds`]. Legality and previews both branch on
/// these. `CardModel.IsValidTarget` names two of them. A preview is computed
/// once per creature for the three that name one.
pub trait TargetKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// The C# zero, which a card that points at nothing carries.
    const NONE: Self;
    /// `TargetType.Self`, which is a Rust keyword.
    const SELF: Self;
    const ANY_ENEMY: Self;
    const ALL_ENEMIES: Self;
    const RANDOM_ENEMY: Self;
    const ANY_PLAYER: Self;
    const ANY_ALLY: Self;
    const ALL_ALLIES: Self;
    /// A target that is a place rather than a creature.
    const TARGETED_NO_CREATURE: Self;
    /// Ostrich targeting, which one card has.
    const OSTY: Self;
}

/// A random stream, by the name the game seeds it with.
///
/// `RunRngSet.CreateRng` is `new Rng(Seed, SnakeCase(rngType.ToString()))`, so
/// a stream's name is its member's, and the pair is not something an
/// implementation gets to invent. Before this it was a string written out
/// beside every draw, with nothing to say the two matched.
pub trait StreamKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// What the game hashes into this stream's seed.
    fn name(self) -> &'static str;
}

/// The card keywords a rule names, rather than a screen prints.
///
/// Most are read off the table alone, which [`CardTable::exhausts`] and its
/// three neighbours answer. A keyword something in a fight writes onto a copy
/// has to be nameable as a value, and this names it.
pub trait KeywordKinds<V: ContentVersion>: std::fmt::Debug + Copy + Eq + 'static {
    /// Starts the fight in hand, which `OnUpgrade` adds to three cards.
    const INNATE: Self;
    /// Stays in hand as the turn ends, which a power writes onto Shivs.
    const RETAIN: Self;
    /// Plays itself once discarded.
    const SLY: Self;
    /// Exhausts from hand as the turn ends, which a power writes onto cards
    /// it makes.
    const ETHEREAL: Self;
    /// Goes to the exhaust pile once played, which an upgrade can take off.
    const EXHAUST: Self;
    /// Cannot be played, which a card picking one to play reads.
    const UNPLAYABLE: Self;
}

/// The things a rest site can offer, as consts a rule can name.
///
/// The same bridge as [`MapPointKinds`]. `RestSiteOption.Generate` names two
/// of these, so a rule holding only a [`ContentVersion`] has to name them too.
///
/// All nine, and not only the two a single-player rest site opens with. A
/// build that adds an option stops codegen.
pub trait RestKinds<V: ContentVersion>: std::fmt::Debug + Copy + Eq + 'static {
    const CLONE: Self;
    const COOK: Self;
    const DIG: Self;
    const HATCH: Self;
    /// Rest, for three tenths of maximum health.
    const HEAL: Self;
    const KINDLE: Self;
    const LIFT: Self;
    /// The second player's rest, which a single-player run never offers.
    const MEND: Self;
    /// Upgrade a card the deck can still upgrade.
    const SMITH: Self;

    /// What a rest site offers a player of a run of `players`, in the
    /// game's own order.
    ///
    /// `RestSiteOption.Generate` opens with `HEAL` and `SMITH`, adds `MEND`
    /// where there are several players, and then runs
    /// `TryModifyRestSiteOptions`. That hook is why the list is the build's
    /// answer and not a constant. Its listeners add `added`, in the order
    /// they answered.
    fn offered(players: usize, added: &[Self]) -> Vec<Self>;

    /// `RestSiteOption.OnSelect` for an option whose selection is something
    /// the run carries out: `HATCH` obtains Byrdpip.
    ///
    /// # Panics
    ///
    /// An option the run carries out itself, or one nobody has ported.
    fn selected(self) -> Vec<HookEffect<V>>;
}

/// The streams a run keeps, as consts a rule can name.
///
/// The same bridge as [`MapPointKinds`]: a rule holding only a
/// [`ContentVersion`] cannot write `RunRngType::Niche`.
///
/// All twelve, and not only the ones a ported rule draws from. The count is the
/// game's, so a build that adds a stream stops codegen.
pub trait RunStreamKinds: StreamKinds {
    /// Drawn while a run or an act is set up, rather than while it is played.
    const UP_FRONT: Self;
    const SHUFFLE: Self;
    const UNKNOWN_MAP_POINT: Self;
    const COMBAT_CARD_GENERATION: Self;
    const COMBAT_POTION_GENERATION: Self;
    const COMBAT_CARD_SELECTION: Self;
    const COMBAT_ENERGY_COSTS: Self;
    const COMBAT_TARGETS: Self;
    const MONSTER_AI: Self;
    /// The stream for draws with no stream of their own.
    const NICHE: Self;
    const COMBAT_ORBS: Self;
    const TREASURE_ROOM_RELICS: Self;
}

/// The streams each player keeps, as consts a rule can name.
///
/// Beside the run's twelve, not among them: a multiplayer run has a set each.
pub trait PlayerStreamKinds: StreamKinds {
    const REWARDS: Self;
    const SHOPS: Self;
    const TRANSFORMATIONS: Self;
}

/// How much of a power is on a creature.
///
/// The game keeps one number and overloads its sign, so `PowerModel.Amount` is
/// a count for most powers and a flag for one. That is the game's spelling and
/// it stops at the wire. A rule reads which of the two this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PowerAmount<N> {
    /// A number that stacks, and counts down where the power counts down.
    Counted(N),
    /// One instance, with no number to run out. `PowerModel.IsInfinite`.
    ///
    /// Nothing to decrement, so an endless power sits out every tick.
    Endless,
}

/// `ITemporaryPower`, as the two facts a fight reads off one.
///
/// A temporary power applies a second power silently when it lands, and takes
/// that back when its owner's side ends a turn. `applies` is
/// `InternallyAppliedPower`, and `positive` is `IsPositive`, which is the sign
/// the second power is applied with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Temporary<P> {
    pub applies: P,
    pub positive: bool,
}

/// The relic pool run setup names.
///
/// The same bridge as [`MapPointKinds`]: a generated enum is per build, so a rule
/// holding only a [`ContentVersion`] cannot write `RelicPool::Shared`.
pub trait RelicPoolKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// The pool every character draws from, beside its own.
    const SHARED: Self;
}

/// The relic rarities a run's bag deals from.
///
/// `RelicGrabBag` keeps a set of four and drops every relic outside it, so a
/// starter, an event relic and an ancient never enter a bag. The four are what
/// this names, because the count of them is a rule and not a listing.
pub trait RarityKinds: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static {
    /// The four, in the game's own order.
    const DRAFTED: [Self; 4];

    /// The four again, one at a time.
    ///
    /// A pull names a rarity, and walks to the next one when the bag holds
    /// nothing at it. So the walk has to be able to write them down. Not the
    /// same fact as [`RarityKinds::DRAFTED`], which is a count and an order.
    const COMMON: Self;
    const UNCOMMON: Self;
    const RARE: Self;
    const SHOP: Self;

    /// The rarity that is not one: `RelicRarity.None`, which the tables spell
    /// `Undrafted`.
    ///
    /// A bag deals nothing at it. It is what a reward slot carries when the
    /// slot picks no rarity of its own. A screen reports it as the slot's
    /// rarity.
    const UNDRAFTED: Self;

    /// `RelicRarity.Starter`, a character's own relic, which a bag never
    /// deals.
    const STARTER: Self;
}

/// The ascension ladder, as much of it as a rule needs to ask about.
///
/// A run has one level and every rung at or below it applies. So this is a
/// scalar, and [`AscensionKinds::has`] is `>=` rather than equality.
///
/// The consts are the rungs a ported rule names. A rung with no const is one
/// nothing here reads yet, and adding one is how that changes.
pub trait AscensionKinds: std::fmt::Debug + Copy + Eq + 'static {
    /// No ascension at all, which is where a run starts.
    const NONE: Self;
    /// A merchant's card removal opens dearer and climbs faster.
    const INFLATION: Self;
    /// A reward card is upgraded half as often.
    const SCARCITY: Self;
    /// Elites go from five a map to eight.
    const SWARMING_ELITES: Self;
    /// An ancient heals four fifths of what is missing rather than all of it.
    const WEARY_TRAVELER: Self;
    /// Gold from a chest comes to three quarters.
    const POVERTY: Self;
    /// The run opens with one potion slot fewer.
    const TIGHT_BELT: Self;
    /// The last act fields a second boss after its first.
    const DOUBLE_BOSS: Self;

    /// Whether a run at this level has the rule `rung` names.
    fn has(self, rung: Self) -> bool;
}

/// One creature in a fight, as content behavior reads one.
///
/// The same narrowing as [`crate::EventView`]. `Creature` is the game's, and a
/// simulator's own creature answers these. Content then reads a fight without
/// knowing how one is stored. A member is added when a rule asks for it.
pub trait CreatureView<V: ContentVersion> {
    /// Stable for as long as the fight lasts, which a position is not.
    fn id(&self) -> crate::CombatId;

    /// Which side it fights on, not whether it is a monster.
    fn side(&self) -> V::CombatSide;

    fn alive(&self) -> bool;

    fn hp(&self) -> V::Health;

    fn max_hp(&self) -> V::Health;

    /// `Block`, as it stands.
    fn block(&self) -> V::Number;

    /// `PetOwner`: the player a pet stands beside. Nothing for a player or a
    /// monster.
    fn owner(&self) -> Option<crate::CombatId>;

    /// `Creature.Monster`, or nothing for a player.
    fn monster(&self) -> Option<V::Monster>;

    /// `Creature.GetPower<T>()?.Amount`, and `None` for one without it.
    ///
    /// Both of the questions the game asks a power from outside: whether the
    /// creature carries it, and the stack it carries it at.
    fn power(&self, power: V::Power) -> Option<PowerAmount<V::Number>>;

    /// `Monster.NextMove.Intents`, in the order the move declares them.
    /// Nothing for a creature with no move.
    fn intents(&self) -> impl Iterator<Item = V::IntentKind> + '_;
}

/// What a power keeps beside its model for as long as it lasts. That is the
/// creatures it waits on, and the cards it changed, each with a level.
///
/// `DampenPower`'s `casters` and `downgradedCardsToOldUpgradeLevels`. Every
/// fight a search makes copies a model, but not this. The fight holds it once
/// for each power that keeps any, and its copies share it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept<V: ContentVersion> {
    pub creatures: smallvec::SmallVec<[crate::CombatId; 2]>,
    pub cards: Vec<(crate::CardId<V>, u8)>,
}

impl<V: ContentVersion> Default for Kept<V> {
    fn default() -> Self {
        Kept {
            creatures: smallvec::SmallVec::new(),
            cards: Vec::new(),
        }
    }
}

/// The creatures in a fight, as a hook reads ones it does not own.
///
/// How a power asks about a dealer or a target. A narrowing like
/// [`CreatureView`], and just as small: a member is added when a rule asks.
pub trait FightView<V: ContentVersion> {
    /// `Creature.GetPower<T>()?.Amount`: the stack `creature` carries `power`
    /// at, or `None` without it.
    fn power(&self, creature: crate::CombatId, power: V::Power) -> Option<PowerAmount<V::Number>>;

    /// `Creature.PetOwner`: the player `creature` stands beside, or nothing
    /// for a creature that is no pet.
    fn pet_owner(&self, creature: crate::CombatId) -> Option<crate::CombatId>;

    /// `Creature.Monster`, or nothing for a player.
    fn monster(&self, creature: crate::CombatId) -> Option<V::Monster>;

    /// `Creature.IsPlayer`: whether `creature` is a player's own creature. A
    /// pet is not one.
    fn is_player(&self, creature: crate::CombatId) -> bool;

    /// `Creature.Side`, or nothing for a creature not in the fight.
    fn side_of(&self, creature: crate::CombatId) -> Option<V::CombatSide>;

    /// `CombatState.GetTeammatesOf(creature)`: its side's creatures, itself
    /// and dead ones included, in fight order.
    fn teammates(&self, creature: crate::CombatId) -> smallvec::SmallVec<[crate::CombatId; 8]>;

    /// The first of `AttackCommand.Results` whose receiver is `creature`, for
    /// the attack whose `AfterAttack` is being raised. Nothing outside one.
    fn attack_result(&self, creature: crate::CombatId) -> Option<DamageResult<V>>;

    /// `Creature.CurrentHp`, or nothing for a creature not in the fight.
    fn current_hp(&self, creature: crate::CombatId) -> Option<V::Health>;

    /// `CombatState.GetOpponentsOf(creature)` that are alive, in fight order.
    fn living_opponents(
        &self,
        creature: crate::CombatId,
    ) -> smallvec::SmallVec<[crate::CombatId; 8]>;

    /// `Creature.Player?.GetRelic<T>() != null`: whether `creature` is a player
    /// holding `relic`. A pet is no player, so it holds none.
    fn relic(&self, creature: crate::CombatId, relic: V::Relic) -> bool;

    /// What the fight keeps beside the instance of `power` that `holder`
    /// carries, or `None` while it keeps nothing.
    fn kept(&self, _holder: crate::CombatId, _power: V::Power) -> Option<&Kept<V>> {
        None
    }
}

/// A fight's type, named for any borrow of the fight.
///
/// A play that stops waits inside the fight, so it can hold no borrow of it.
/// It names the fight by its kind, and is lent the fight again at each step.
pub trait FightKind<V: ContentVersion>:
    std::fmt::Debug + Copy + Eq + std::hash::Hash + Send + Sync + 'static
{
    /// The fight, borrowed for `'a`.
    type Fight<'a>: Fight<V, Kind = Self>;
}

/// The fight, as a card's own behaviour reaches it.
///
/// A card's behaviour is ported code and lives per build, in `sts2-content`.
/// The fight it acts on lives in the engine. They meet here, at the game's
/// `ICombatState`.
///
/// Each method is a command, and arrives when a ported card needs it. What a
/// card prints is not here: that is static, and a card reads its own table.
pub trait Fight<V: ContentVersion> {
    /// This fight's type for any borrow of it.
    type Kind: FightKind<V>;

    /// An effect after this fight's current listeners have modified it.
    type ResolvedEffect: EffectResolution<V>;

    /// One creature in this fight, as [`Fight::creatures`] hands it over.
    type Creature<'a>: CreatureView<V>
    where
        Self: 'a;

    /// The creature the card was pointed at.
    ///
    /// A card whose target type names a creature is played at one. The play
    /// reaching behaviour has already been held to `CardModel.IsValidTarget`,
    /// so this is not a question the card asks and then answers.
    fn target(&self) -> crate::CombatId;

    /// The creature acting, which is `Owner.Creature`.
    ///
    /// The player for a card and the monster for a move. A card saying "gain"
    /// names this creature as what it gains for.
    fn actor(&self) -> crate::CombatId;

    /// Every creature the fight holds, in the order it holds them.
    ///
    /// `ICombatState.Creatures`, dead ones included. A rule that reads another
    /// creature reads it here, rather than take a list of ids and then ask
    /// after each one.
    fn creatures(&self) -> impl Iterator<Item = Self::Creature<'_>>;

    /// The side whoever is acting is on, which is `Owner.Creature.Side`.
    fn side(&self) -> V::CombatSide;

    /// The creatures across from whoever is acting, in command order.
    ///
    /// Pets are left out. A monster's move reaches `PlayerCreatures`, which
    /// is the players alone, and no pet stands against the player.
    ///
    /// `GetOpponentsOf`. A command snapshots this list before its first
    /// effect lands. Each effect still resolves immediately before it lands,
    /// because an earlier one can change the listeners the next one reaches.
    fn opponents(&self) -> Opponents {
        let side = self.side();
        self.creatures()
            .filter(|creature| {
                creature.side() != side && creature.alive() && creature.owner().is_none()
            })
            .map(|creature| creature.id())
            .collect()
    }

    /// Work an effect's number out and run it through the listeners the
    /// effect names, without applying it.
    ///
    /// One step, so no half-worked-out number is observable. A card's variable
    /// is worked out here, and the hooks run against the creatures the effect
    /// already names. The caller reads the resolved amount, keeps it, applies
    /// it, or any two of those.
    fn resolve(&self, effect: Effect<V>) -> Self::ResolvedEffect;

    /// Apply an effect whose listeners have already had their say.
    fn apply(&mut self, effect: Self::ResolvedEffect);

    /// A resolved hit, landed as `CreatureCmd.Damage` with
    /// `ValueProp.Unblockable`: what it worked out to meets no block.
    fn apply_unblockable(&mut self, _effect: Self::ResolvedEffect) {
        unimplemented!("an unblockable hit")
    }

    /// `PlayerCmd.GainStars(amount, Owner)`.
    ///
    /// A printed number, so signed. The pool it lands in is not.
    fn gain_stars(&mut self, amount: V::Number);

    /// `PlayerCmd.GainEnergy(amount)`, or `PlayerCmd.LoseEnergy` for a
    /// negative one.
    ///
    /// A printed number, so signed. The pool it lands in is not, and
    /// `PlayerCombatState` clamps it at zero. A loss larger than the pool
    /// empties the pool and leaves no debt.
    fn modify_energy(&mut self, amount: V::Number);

    /// `PowerCmd.Apply<P>(at, amount, Owner.Creature, this)`.
    ///
    /// The power is the caller's to name. A rule knows which power it applies,
    /// and the fight only has to put it somewhere.
    fn apply_power(&mut self, at: crate::CombatId, power: V::Power, amount: PowerAmount<V::Number>);

    /// `PowerCmd.Apply(power, at, amount, applier, this)`, for a copy whose
    /// applier is the original's rather than whoever is acting.
    fn apply_power_from(
        &mut self,
        _at: crate::CombatId,
        _power: V::Power,
        _amount: PowerAmount<V::Number>,
        _applier: Option<crate::CombatId>,
    ) {
        unimplemented!("a power applied on another creature's behalf")
    }

    /// `PowerCmd.Apply(power, at, amount, applier, this)` of `copy`, a clone
    /// the caller built and wrote on, as Misery's `IgnoreNextInstance`. The
    /// copy is what hears `BeforeApplied`.
    fn apply_power_copy(
        &mut self,
        _at: crate::CombatId,
        _copy: V::PowerState,
        _amount: PowerAmount<V::Number>,
        _applier: Option<crate::CombatId>,
    ) where
        V: Hooks,
    {
        unimplemented!("a power applied from a copy")
    }

    /// What the acting monster's move advertises it will hit for.
    ///
    /// A move's damage and the intent beside it are one number in the game:
    /// both read the monster's own property. The table carries it, so a move
    /// reads it here rather than print it a second time.
    fn intent_damage(&self) -> V::Number;

    /// How many times that hit lands, which is `WithHitCount`.
    ///
    /// The same count the intent advertises, for the same reason the damage
    /// is. A move that hits once reads one.
    fn intent_repeats(&self) -> V::Count;

    /// `AttackCommand.Execute` from the acting creature, whose hits are
    /// `hits`, and the command's `Results`.
    ///
    /// `BeforeAttack` before the first hit and `AfterAttack` after the last,
    /// once each however many hits land. A command whose attacker is dead, or
    /// that starts as the fight is ending, does neither and lands nothing.
    fn attack(&mut self, scaling: Scaling, hits: impl FnOnce(&mut Self)) -> AttackResults<V>
    where
        Self: Sized;

    /// [`Fight::attack`] with `attacker` in place of the acting creature,
    /// which is `DamageCmd.Attack(...).FromOsty` for an attack Osty makes
    /// for a card.
    fn attack_from(
        &mut self,
        _attacker: crate::CombatId,
        _scaling: Scaling,
        _hits: impl FnOnce(&mut Self),
    ) -> AttackResults<V>
    where
        Self: Sized,
    {
        unimplemented!("an attack from a creature other than the one acting")
    }

    /// The results of the card's own attack, once a question it left is
    /// answered and its hits have all landed.
    fn waited_attack_results(&mut self) -> AttackResults<V> {
        unimplemented!("reading an attack's results after a wait")
    }

    /// The results the attack command under way has gathered so far, which a
    /// hit inside it can read before the command ends.
    fn attack_results(&self) -> AttackResults<V> {
        unimplemented!("reading an attack's results as it lands")
    }

    /// `CreatureCmd.Damage` over many targets. Every hit `hits` makes lands
    /// before any of their hooks run. The creatures they kill die together
    /// once those are done.
    fn damage_command(&mut self, _hits: impl FnOnce(&mut Self))
    where
        Self: Sized,
    {
        unimplemented!("one damage command over many targets")
    }

    /// Perform the acting monster's advertised attack against every opponent.
    ///
    /// Each hit resolves immediately before it lands, so an earlier hit can
    /// change the listeners the next one reaches.
    fn attack_opponents(&mut self)
    where
        Self: Sized,
    {
        let damage = self.intent_damage();
        let repeats: usize = num_traits::cast(self.intent_repeats()).unwrap_or_else(|| {
            unimplemented!("a move that hits more times than a fight can count")
        });
        self.attack(Scaling::Dynamic, |fight| {
            for _ in 0..repeats {
                // `if (Attacker.IsDead) break;` before each hit. A hit the
                // attacker died during still lands on every target.
                let actor = fight.actor();
                let alive = (fight.creatures())
                    .find(|creature| creature.id() == actor)
                    .is_some_and(|creature| creature.alive());
                if !alive {
                    break;
                }
                let hit = |fight: &mut Self, target| {
                    let attack = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Number(damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(attack);
                };
                // Each hit is one `CreatureCmd.Damage` over them all. Over
                // one, that is the hit alone.
                let targets = fight.opponents();
                match targets[..] {
                    [target] => hit(fight, target),
                    _ => fight.damage_command(|fight| {
                        for target in targets {
                            hit(fight, target);
                        }
                    }),
                }
            }
        });
    }

    /// `PowerCmd.Apply<P>(targets, amount, ...)`, which lands on every
    /// creature on the other side.
    ///
    /// A monster's move points at the creatures across from it rather than at
    /// one, the way [`Fight::attack_opponents`] does.
    fn apply_to_opponents(&mut self, power: V::Power, amount: PowerAmount<V::Number>);

    /// `CardPileCmd.Draw(count, Owner)`, which hands back the copies it drew,
    /// in order, wherever they stand now.
    ///
    /// A printed number, so signed. A card printing a negative one draws
    /// nothing, which is what the game's own loop does.
    ///
    /// A shuffle inside it can ask, which [`Fight::waiting`] says. Nothing comes
    /// back here then, and every copy drawn comes with [`Taken::Drew`] once
    /// the draw is over.
    fn draw(&mut self, count: V::Number) -> Drew<V>;

    /// Where the copy `id` stands, or nothing for one the fight no longer
    /// holds.
    fn find_card(&self, _id: crate::CardId<V>) -> Option<crate::CardAt<V>> {
        unimplemented!("finding a copy by its identity")
    }

    /// `CardPileCmd.AddToCombatAndPreview<C>(targets, Discard, count)`.
    ///
    /// One copy per count, into the discard pile of every creature on the
    /// other side.
    fn give_cards(&mut self, card: V::Card, count: usize);

    /// A card's energy and star costs: its own modifiers alone, or with
    /// `global` every listener's too, which is `GetWithModifiers(All)`.
    fn card_costs(&self, _card: crate::CardAt<V>, _global: bool) -> (Cost, Cost) {
        unimplemented!("reading a card's costs")
    }

    /// One card's energy and star costs with no modifier at all, which is
    /// `GetWithModifiers(CostModifiers.None)` and `BaseStarCost`: what it
    /// prints, upgrades included.
    fn printed_costs(&self, _card: crate::CardAt<V>) -> (Cost, Cost) {
        unimplemented!("reading a card's printed costs")
    }

    /// `Keywords.Contains(keyword)` for one card: what its table prints and
    /// what the fight wrote on it, less what the fight took off.
    fn card_keyword(&self, _card: crate::CardAt<V>, _keyword: V::CardKeyword) -> bool {
        unimplemented!("reading a card's keywords")
    }

    /// `ShouldRetainThisTurn` or `IsSlyThisTurn` for one card: the keyword,
    /// or the single turn's flag `GiveSingleTurnRetain` or `GiveSingleTurnSly`
    /// set.
    fn card_keyword_this_turn(&self, _card: crate::CardAt<V>, _keyword: V::CardKeyword) -> bool {
        unimplemented!("reading a card's keywords for the turn")
    }

    /// `GetKeywordsWithSources(KeywordSources.Local).Contains(keyword)`: the
    /// words the card itself carries, without what a power grants it for the
    /// fight, as Hex grants Ethereal.
    fn card_local_keyword(&self, _card: crate::CardAt<V>, _keyword: V::CardKeyword) -> bool {
        unimplemented!("reading a card's own keywords")
    }

    /// `CardPileCmd.Shuffle`: the discard pile and the draw pile shuffled
    /// together into a new draw pile, then `AfterShuffle`.
    fn shuffle(&mut self) {
        unimplemented!("a shuffle asked for outside a draw")
    }

    /// `CardPileCmd.AddGeneratedCardToCombat(CreateClone(), pile)` for the
    /// card being played: a copy of all it holds now, marked a clone. Nothing
    /// once the fight is over.
    fn clone_this(&mut self, _pile: crate::Pile) -> Option<crate::CardAt<V>> {
        unimplemented!("a card cloning itself")
    }

    /// `CreateCloneForPlayer`, then `CardPileCmd.AddGeneratedCardToCombat`
    /// into `pile`, of the card being played, for each living player on its
    /// owner's side, the owner too, in order. Each clone is its player's,
    /// and its owner added it.
    fn clone_this_for_each_player(&mut self, _pile: crate::Pile) {
        unimplemented!("a card cloning itself for each player")
    }

    /// [`Fight::clone_this`] for another card in the fight, as it holds
    /// now.
    fn clone_card(
        &mut self,
        _card: crate::CardAt<V>,
        _pile: crate::Pile,
    ) -> Option<crate::CardAt<V>> {
        unimplemented!("a card cloned")
    }

    /// `CardPileCmd.Add` of a new card to one combat pile, made for the fight
    /// but not through `AddGeneratedCardToCombat`, so no generation hook
    /// hears it. Nothing once the fight is over.
    fn add_card(
        &mut self,
        _card: crate::CardIdent<V>,
        _pile: crate::Pile,
    ) -> Option<crate::CardAt<V>> {
        unimplemented!("a card added to the fight")
    }

    /// [`Fight::add_generated_cards`] with the player who made them named:
    /// `CardPileCmd.AddGeneratedCardsToCombat(cards, pile, creator)`.
    fn add_generated_cards_by(
        &mut self,
        _card: crate::CardIdent<V>,
        _pile: crate::Pile,
        _count: usize,
        _creator: crate::CombatId,
    ) -> Vec<crate::CardAt<V>> {
        unimplemented!("adding generated cards a named player made")
    }

    /// Add copies of a generated card to one combat pile.
    fn add_generated_cards(
        &mut self,
        _card: crate::CardIdent<V>,
        _pile: crate::Pile,
        _count: usize,
    ) -> Vec<crate::CardAt<V>> {
        unimplemented!("adding generated cards to a pile")
    }

    /// `CreatureCmd.Stun(target, nextMoveId)` on a monster: `stun` in place of
    /// the move it advertised, then `then`. A monster that has to perform
    /// the move it advertises keeps it.
    fn stun(
        &mut self,
        _target: crate::CombatId,
        _stun: &'static crate::NextMove<V>,
        _then: StunResume,
    ) {
        unimplemented!("a monster stunned by a card")
    }

    /// `CardPileCmd.Add(this, pile, position)`, for the card being played,
    /// which then stays there rather than going to its result pile.
    fn add_this(&mut self, _pile: crate::Pile, _position: PilePosition) {
        unimplemented!("a card moving itself as it is played")
    }

    /// `Keywords.Contains(keyword)` for the card being played.
    fn this_has_keyword(&self, _keyword: V::CardKeyword) -> bool {
        unimplemented!("reading the keywords of the card being played")
    }

    /// `act` on each of `cards`, from the pile each stands in. A Sly card
    /// discarded answers by playing itself.
    fn act_on(&mut self, _act: PileAct, _cards: &[crate::CardAt<V>]) {
        unimplemented!("acting on picked cards")
    }

    /// Write `change` on the card being played, which stands in the play pile.
    /// `EnergyCost.AddThisCombat` on itself, for one.
    fn modify_this(&mut self, _change: Change<V>, _lasting: Lasting) {
        unimplemented!("a card writing on itself as it is played")
    }

    /// `CombatState.HittableEnemies`: the opponents alive, less any a
    /// listener refuses a hit on, in command order.
    fn hittable_opponents(&self) -> Opponents {
        unimplemented!("the hittable opponents")
    }

    /// `PileType.<pile>.GetPile(Owner).Cards`, as it holds them now.
    ///
    /// Each is named by where it was found, the way a selection names one. A
    /// card that acts on a whole pile walks this.
    fn cards_in(&self, pile: crate::Pile) -> Vec<crate::CardAt<V>>;

    /// The identity carried by a card at one named place.
    fn card_ident(&self, _card: crate::CardAt<V>) -> crate::CardIdent<V> {
        unimplemented!("reading a card through its place")
    }

    /// Pick one candidate with the named run stream.
    fn random_card(
        &mut self,
        _cards: &[crate::CardAt<V>],
        _stream: V::RunRngType,
    ) -> Option<crate::CardAt<V>> {
        unimplemented!("choosing a random card from combat")
    }

    /// `TakeRandom(count)` over `cards` off `CombatCardSelection`: the list
    /// shuffled whole, then its first `count`, fewer where it is shorter.
    fn take_random_cards(
        &mut self,
        _cards: &mut [crate::CardAt<V>],
        _count: usize,
    ) -> Vec<crate::CardAt<V>> {
        unimplemented!("taking cards at random from the fight")
    }

    /// `CardPileCmd.RemoveFromCombat` for one named card, and
    /// `RemoveFromDeck` of its deck version where it has one.
    fn remove_card(&mut self, _card: crate::CardAt<V>) -> crate::CardIdent<V> {
        unimplemented!("removing a card from combat")
    }

    /// Write one change on the copy of a card at a named place.
    ///
    /// A number and a replay count go into what the copy prints, the way
    /// `DynamicVars.BaseValue` and `BaseReplayCount` are written. Neither
    /// wears off.
    ///
    /// A cost is kept beside the printed one. `CardEnergyCost` holds a list of
    /// those and folds it on every read.
    fn modify_card(&mut self, card: crate::CardAt<V>, change: Change<V>, lasting: Lasting);

    /// `CreatureCmd.Add<M>(CombatState, slot)` for this place in
    /// `Encounter.Slots`. `SortEnemiesBySlotName` then places it among the
    /// enemies by slot.
    fn summon_into_slot(&mut self, _monster: V::Monster, _slot: u8) -> crate::CombatId {
        unimplemented!("a monster added into a slot")
    }

    /// `CreatureCmd.Add<M>(CombatState, slot)` for the last of
    /// `Encounter.Slots` no enemy stands in. `SortEnemiesBySlotName` then
    /// places it among the enemies by slot. Nothing, with no slot free.
    ///
    /// The monster rolls its hit points when it arrives. It does not advertise
    /// a move until the enemy side next prepares for a turn.
    fn summon_into_last_free_slot(&mut self, _monster: V::Monster) -> Option<crate::CombatId> {
        unimplemented!("a monster added into a free slot")
    }

    /// `CombatState.CreateCreature` for an enemy, ahead of
    /// [`Fight::add_created`]. It takes the next id and rolls its hit points
    /// against the enemies in the fight, as one that joins does. Until it is
    /// added nothing targets it and its powers hear nothing, but a power can
    /// land on it.
    fn create_monster(&mut self, _monster: V::Monster) -> crate::CombatId {
        unimplemented!("a monster created before it joins")
    }

    /// `CreatureCmd.Add(creature)` for one [`Fight::create_monster`] made, at
    /// `position` among the enemies, with the powers it took meanwhile.
    fn add_created(&mut self, _created: crate::CombatId, _position: usize) {
        unimplemented!("a created monster joining")
    }

    /// `CreatureCmd.Add<M>(CombatState, position)`.
    ///
    /// The monster rolls its hit points when it arrives. One that joins on
    /// the player's turn rolls its move then too. One that joins on the
    /// enemies' turn advertises nothing until they next prepare for a turn.
    fn summon(&mut self, monster: V::Monster, position: usize) -> crate::CombatId;

    /// What the run is being played at.
    ///
    /// A monster's move prints numbers the level moves, which the table only
    /// carries for the ones an intent advertises. The rest are constants in
    /// the move, behind `AscensionHelper.GetValueIfAscension`.
    fn ascension(&self) -> V::AscensionLevel;

    /// One of the numbers the card being played prints right now.
    ///
    /// This copy's own, not the table's. A fight writes on a card, so the card
    /// that was forged twice prints more than the one beside it. A
    /// `CalculatedVar` is counted here, so this is what the card prints rather
    /// than the base it was built with.
    ///
    /// # Panics
    ///
    /// A number that runs through hooks, which is every
    /// [`CardNumberModifier`]. Those name creatures to run against, and
    /// [`Fight::resolve`] is where a number and its creatures arrive together.
    fn var(&self, key: V::CardVar) -> V::Number;

    /// `DynamicVars[key].BaseValue` handed on as a power's amount: what the
    /// copy prints, with no hook of the number's class run. The power runs
    /// its own hooks where it lands.
    fn power_var(&self, _key: V::CardVar) -> V::Number {
        unimplemented!("a card's number handed to a power")
    }

    /// `IsUpgraded` for the card being played.
    ///
    /// A card whose play changes with its upgrade rather than by printing a
    /// larger number. Begone upgrades the Minion Strike it creates, and
    /// prints no number at all.
    fn upgraded(&self) -> bool;

    /// `CombatManager.History`, as it stands.
    fn history(&self) -> History<'_, V> {
        unimplemented!("reading the fight's history")
    }

    /// `CombatManager.IsOverOrEnding`: a loss waits, or no primary enemy
    /// stands.
    fn over_or_ending(&self) -> bool {
        unimplemented!("asking whether the fight is ending")
    }

    /// `TargetType` of a card in this fight. Some cards work theirs out from
    /// it: a Shiv under Fan of Knives hits every enemy.
    fn card_target_type(&self, _card: crate::CardIdent<V>) -> V::TargetType {
        unimplemented!("a card's target type")
    }

    /// Whether a card this effect auto-played stopped on a question, which
    /// the rest of the effect waits behind. `CardCmd.AutoPlay` is awaited,
    /// so the game's own loop stands still there.
    fn waiting(&self) -> bool {
        unimplemented!("reading whether a question waits")
    }

    /// The copy of the card whose play is running, which is `this` to it.
    fn playing(&self) -> crate::CardId<V> {
        unimplemented!("naming the card being played")
    }

    /// What the copy whose play is running is, as it stands.
    fn playing_ident(&self) -> crate::CardIdent<V> {
        unimplemented!("reading the card being played")
    }

    /// `CardCmd.Transform(original, replacement)`.
    ///
    /// The replacement takes the original's place: the same pile, at the same
    /// index. It belongs to whoever owned the original, which is the creature
    /// acting.
    ///
    /// The original is named by a [`crate::CardAt`], so this looks straight
    /// where it says rather than read the piles through.
    fn transform(&mut self, card: crate::CardAt<V>, into: crate::CardIdent<V>);

    /// `CardCmd.Transform(transformations)`, several at once.
    ///
    /// Every original leaves its pile first, in the order given, and each
    /// notes its index as it leaves. The replacements then go in, sorted by
    /// that index, each at its noted index. A replacement whose original
    /// stood behind another one can so land a place early. Each then raises
    /// its hooks, and `AfterCardGeneratedForCombat` follows once all are in.
    fn transform_all(&mut self, _cards: &[(crate::CardAt<V>, crate::CardIdent<V>)]) {
        unimplemented!("transforming several cards at once")
    }

    /// `ForgeCmd.Forge`: `AddDamage(amount)` on every copy of one card.
    ///
    /// Every pile, the exhaust pile included, and one copy is created in hand
    /// when the fight holds none outside exhaust.
    fn forge(&mut self, card: V::Card, amount: V::Number);

    /// `OstyCmd.Summon`: the player's Osty gains `amount` maximum health, or
    /// stands up at that much, alive or anew.
    fn summon_osty(&mut self, _amount: V::Number) {
        unimplemented!("summoning Osty")
    }

    /// The player's Osty, while it stands. `Osty.CheckMissingWithAnim` is the
    /// question of whether this is nothing.
    fn osty(&self) -> Option<crate::CombatId> {
        unimplemented!("finding Osty")
    }

    /// `OrbModel.ModifyOrbValue`: one of an orb's numbers, as the fight's
    /// listeners leave it. `PassiveVal` and `EvokeVal` read theirs through
    /// this each time.
    fn orb_value(&self, orb: V::Orb, base: V::Number) -> V::Number;

    /// `CardPileCmd.AddGeneratedCardToCombat` of a new copy into `pile` at
    /// `CardPilePosition.Random`, and where it landed. Nothing, in a fight
    /// already over.
    fn add_at_random(&mut self, _card: V::Card, _pile: crate::Pile) -> Option<crate::CardAt<V>> {
        unimplemented!("a card added at a random place")
    }

    /// `PowerCmd.ModifyAmount` on `creature`'s `power`, by a signed amount,
    /// from `applier`.
    fn modify_power_amount(
        &mut self,
        _creature: crate::CombatId,
        _power: V::Power,
        _amount: V::Number,
        _applier: Option<crate::CombatId>,
    ) {
        unimplemented!("a power's amount moved")
    }

    /// `creature.Powers`: each instance it holds, in the order it holds them.
    fn powers_on(&self, _creature: crate::CombatId) -> Vec<HeldInstance<V>> {
        unimplemented!("listing a creature's powers")
    }

    /// `PowerCmd.Remove` of `power` from `creature`, where it holds one.
    fn remove_power(&mut self, _creature: crate::CombatId, _power: V::Power) {
        unimplemented!("removing a power")
    }

    /// `creature` onto what the fight keeps beside the instance of `power`
    /// that `holder` carries, in [`Kept::creatures`].
    fn keep_creature(
        &mut self,
        _holder: crate::CombatId,
        _power: V::Power,
        _creature: crate::CombatId,
    ) {
        unimplemented!("a creature kept beside a power")
    }

    /// `CreatureCmd.SetMaxHp`: the maximum, and the health down to it where
    /// it stood above.
    fn set_max_hp(&mut self, _creature: crate::CombatId, _hp: V::Health) {
        unimplemented!("setting a creature's maximum health")
    }

    /// `CreatureCmd.SetMaxAndCurrentHp`: both numbers at once.
    fn set_max_and_current_hp(&mut self, _creature: crate::CombatId, _hp: V::Health) {
        unimplemented!("setting a creature's hit points")
    }

    /// `Creature.ScaleHpForMultiplayer` of a monster's `hp`, for the fight's
    /// player count, truncated as `SetMaxHpInternal` takes a decimal. The
    /// same `hp` in a fight of one player.
    fn scaled_for_players(&self, _hp: V::Health) -> V::Health {
        unimplemented!("scaling health for the player count")
    }

    /// `RunRng.Niche.NextInt(min, max)`, which stops short of `max`.
    fn niche_int(&mut self, _min: V::Number, _max: V::Number) -> V::Number {
        unimplemented!("a draw off the niche stream")
    }

    /// `CreatureCmd.Escape` on the actor: its powers go with no hook, and it
    /// leaves the fight without dying.
    fn escape(&mut self) {
        unimplemented!("a creature escaping")
    }

    /// `CreatureCmd.Damage` on `target`, unpowered, from `dealer` or from
    /// nobody, once the damage hooks worked it out.
    fn damage_unpowered(
        &mut self,
        _dealer: Option<crate::CombatId>,
        _target: crate::CombatId,
        _amount: V::Number,
        _unblockable: bool,
    ) {
        unimplemented!("unpowered damage")
    }

    /// The number the card being resolved keeps on its own model, which
    /// [`HookEffect::NoteOnSelf`] wrote. Nothing written is zero.
    fn self_note(&self) -> V::Number {
        unimplemented!("a number a card keeps on itself")
    }

    /// Write the number the card being resolved keeps on its own model.
    fn set_self_note(&mut self, _note: V::Number) {
        unimplemented!("a number a card keeps on itself")
    }

    /// `CreatureCmd.Kill`: `creature` loses what hit points it has, and dies.
    fn kill(&mut self, _creature: crate::CombatId) {
        unimplemented!("a creature killed outright")
    }

    /// `Rng.CombatOrbGeneration.NextItem(orbs)`: one orb of the list,
    /// nothing for an empty one.
    fn random_orb(&mut self, _orbs: &[V::Orb]) -> Option<V::Orb> {
        unimplemented!("a random orb")
    }

    /// `CardCmd.Afflict` on each card `to` names.
    fn afflict(&mut self, _to: Writing<V>, _affliction: V::Affliction) {
        unimplemented!("afflicting the cards a fight holds")
    }

    /// `CardCmd.ClearAffliction` on each card `to` names.
    fn clear_affliction(&mut self, _to: Writing<V>) {
        unimplemented!("clearing the cards a fight holds")
    }

    /// `change` written on each card `to` names, for the rest of the fight.
    fn modify_cards(&mut self, _to: Writing<V>, _change: Change<V>) {
        unimplemented!("writing on the cards a fight holds")
    }

    /// `GetEnchantedReplayCount` of the card at one place: its replays,
    /// through its enchantment's `EnchantPlayCount`.
    fn card_replays(&self, _card: crate::CardAt<V>) -> V::Number {
        unimplemented!("reading a card's replays")
    }

    /// `CardModel.BaseReplayCount += amount` on each card `to` names.
    fn add_replays(&mut self, _to: Writing<V>, _amount: V::Number) {
        unimplemented!("replays added to cards")
    }

    /// `PlayerCombatState.Energy`: what the player has left to spend.
    fn energy(&self) -> V::Number {
        unimplemented!("the player's energy")
    }

    /// `CombatRoom.AddExtraReward` of a `CardReward` at
    /// `CardCreationOptions.ForRoom`, which a won fight offers after the
    /// room's own.
    fn add_card_reward(&mut self) {
        unimplemented!("an extra card reward")
    }

    /// The deck card `card` is a copy of, which is `CardModel.DeckVersion`.
    /// Nothing for a copy made in the fight.
    fn deck_version(&self, _card: crate::CardId<V>) -> Option<crate::DeckCardId> {
        unimplemented!("a card's deck version")
    }

    /// `write` on the deck version of the card being played, where it has
    /// one.
    fn write_deck_version(&mut self, _write: V::DeckWrite) {
        unimplemented!("a write on a deck version")
    }

    /// `PlayerCmd.EndTurn`: the player's turn ends once the play in
    /// progress is over.
    fn end_turn(&mut self) {
        unimplemented!("ending the turn from inside a play")
    }

    /// `DoomPower.DoomKill` of `creatures`: each killed outright, then
    /// `AfterDiedToDoom`. Nothing at all for none.
    fn doom_kill(&mut self, _creatures: &[crate::CombatId]) {
        unimplemented!("a Doom kill")
    }

    /// `creature.Powers.All(p => p.ShouldOwnerDeathTriggerFatal())`.
    fn death_triggers_fatal(&self, _creature: crate::CombatId) -> bool {
        unimplemented!("whether a death triggers Fatal")
    }

    /// `PotionCmd.TryToProcure` of `CreateRandomPotionInCombat` off
    /// `stream`, which the run carries out once the decision settles. The
    /// stream is the run's own, so no draw in the fight moves it.
    fn procure_random_potion(&mut self, _stream: V::RunRngType) {
        unimplemented!("a potion made mid-fight")
    }

    /// A number a card writes on the instance of `power` that `creature`
    /// carries, once `PowerCmd.Apply` hands that instance back, as
    /// `SetBlock` does. `Hooks::power_noted` says what it does there. Nothing
    /// where the creature carries none, which is `?.` on a power that did not
    /// land.
    fn note_on_power(&mut self, _creature: crate::CombatId, _power: V::Power, _note: V::Number) {
        unimplemented!("a card writing on a power it applied")
    }

    /// A clone of `card` as it stands, its affliction cleared, kept on the
    /// instance of `power` that `creature` carries last, as Nightmare's
    /// `SetSelectedCard` keeps it. `Hooks::power_kept_card` says where.
    fn keep_card_on_power(
        &mut self,
        _creature: crate::CombatId,
        _power: V::Power,
        _card: crate::CardAt<V>,
    ) {
        unimplemented!("a power keeping a card")
    }

    /// `CreatureCmd.GainMaxHp`: the maximum grows by `amount`, then a heal
    /// for as much.
    fn gain_max_hp(&mut self, _creature: crate::CombatId, _amount: V::Number) {
        unimplemented!("gaining maximum health in a fight")
    }

    /// `CreatureCmd.LoseMaxHp`: unblockable, unpowered damage from nobody for
    /// what the new maximum leaves above the current, then that maximum, at
    /// one or more.
    ///
    /// A hit that parks a question leaves the maximum as it was, and hands
    /// back the new one. `SetMaxHp` waits behind the answer, and
    /// [`Fight::set_max_hp`] is what sets it then.
    fn lose_max_hp(&mut self, _creature: crate::CombatId, _amount: V::Number) -> Option<V::Health> {
        unimplemented!("maximum health lost in a fight")
    }

    /// `PlayerCmd.LoseGold`: the player's gold down by `amount`, stopping at
    /// none.
    fn lose_gold(&mut self, _amount: V::Number) {
        unimplemented!("losing gold in a fight")
    }

    /// `PlayerCmd.GainGold`: `ModifyGoldGained` over `amount`, then the
    /// player's gold up by what is left, where any is.
    fn gain_gold(&mut self, _amount: V::Number) {
        unimplemented!("gaining gold in a fight")
    }

    /// `OrbCmd.Channel`: the orb joins the end of the player's queue.
    ///
    /// A player with no slots of their own gains one first. A full queue
    /// evokes its first orb to make room.
    fn channel(&mut self, _orb: V::Orb) {
        unimplemented!("channelling an orb")
    }

    /// `OrbCmd.EvokeNext`: the first orb in the queue does what it does on
    /// leaving, and leaves when `dequeue` says so. Nothing, with no orb.
    fn evoke_next(&mut self, _dequeue: bool) {
        unimplemented!("evoking an orb")
    }

    /// `OrbCmd.AddSlots`: `amount` more orb slots for the player, and never
    /// past ten. Nothing once the fight is ending.
    fn add_orb_slots(&mut self, _amount: V::Number) {
        unimplemented!("adding orb slots")
    }

    /// `OrbCmd.RemoveSlots`: `amount` fewer orb slots, and never below none.
    /// The orbs past the new capacity leave from the back, evoking nothing.
    /// Nothing once the fight is ending.
    fn remove_orb_slots(&mut self, _amount: V::Number) {
        unimplemented!("removing orb slots")
    }

    /// How many orbs the player holds, which is `OrbQueue.Orbs.Count`.
    fn orbs_held(&self) -> usize {
        unimplemented!("counting orbs")
    }

    /// How many orb slots the player has, which is `OrbQueue.Capacity`.
    fn orb_capacity(&self) -> usize {
        unimplemented!("counting orb slots")
    }

    /// `OrbCmd.Passive(orb, null)` on the orb at `place` in the queue, which
    /// does nothing once the fight is ending. The numbers it moves stay on
    /// it.
    fn orb_passive_at(&mut self, _place: usize) {
        unimplemented!("an orb's passive")
    }

    /// The player's orbs, front first, which is `OrbQueue.Orbs`.
    fn channelled(&self) -> Vec<crate::Channelled<V>> {
        unimplemented!("listing orbs")
    }

    /// `CreatureCmd.Damage(target, amount, Unblockable | Unpowered, card)`,
    /// from the card's owner, which is the acting creature.
    fn damage_unblockable(&mut self, _target: crate::CombatId, _amount: V::Number) {
        unimplemented!("unblockable damage")
    }

    /// `CreatureCmd.LoseBlock(creature, amount, remover)`: at most the block it holds,
    /// then `AfterBlockBroken` where that was the last of it. Nothing once
    /// the fight is ending or on a dead creature.
    fn lose_block(
        &mut self,
        _creature: crate::CombatId,
        _amount: V::Number,
        _remover: Option<crate::CombatId>,
    ) {
        unimplemented!("losing block")
    }

    /// `DynamicVars[key]` on the card at one place: its `BaseValue`, or
    /// `Calculate(null)` for a number the fight works out. A decimal, because
    /// Thrash writes a fraction on its own.
    fn card_number(&self, _card: crate::CardAt<V>, _key: V::CardVar) -> Decimal {
        unimplemented!("reading another card's number")
    }

    /// `Hook.ModifyDamage` for `damage` from the player against nobody, as a
    /// `Move` from the card at one place, before any flooring.
    fn card_damage(&self, _card: crate::CardAt<V>, _damage: Decimal) -> Decimal {
        unimplemented!("another card's damage")
    }

    /// `CardCmd.DiscardAndDraw(cards, draw)`: every card discarded, then the
    /// draw, and only then the Sly cards among them auto-played. Nothing, for
    /// no card.
    fn discard_and_draw(&mut self, _cards: &[crate::CardAt<V>], _draw: V::Number) {
        unimplemented!("discarding and drawing")
    }

    /// `EnergyCost.GetWithModifiers(CostModifiers.Local)` for a card: its own
    /// cost modifiers over what it prints, and no listener's.
    fn local_cost(&self, _card: crate::CardAt<V>) -> crate::Cost {
        unimplemented!("a card's own cost")
    }

    /// `CombatCardSelection.NextItem` over the cards in `pile`, or nothing for
    /// an empty one.
    fn random_card_in(&mut self, _pile: crate::Pile) -> Option<crate::CardAt<V>> {
        unimplemented!("a random card from a pile")
    }

    /// `CardPileCmd.AutoPlayFromDrawPile(count, Top, forceExhaust)`: the top
    /// `count` cards into the play pile, a shuffle first wherever the draw
    /// pile is empty, then each auto-played in turn.
    fn auto_play_from_draw(&mut self, _count: usize, _force_exhaust: bool) {
        unimplemented!("auto-playing from the draw pile")
    }

    /// [`Fight::auto_play_card`] with `target` named, which a card aimed at
    /// one enemy plays at rather than drawing one.
    fn auto_play_card_at(&mut self, _card: crate::CardAt<V>, _target: crate::CombatId) {
        unimplemented!("auto-playing a card at a named target")
    }

    /// [`Fight::auto_play_card`] at one of the enemies that can be hit, drawn
    /// off `CombatTargets` by `NextItem` as the play begins. With none to
    /// draw, it names no target.
    fn auto_play_card_at_random(&mut self, _card: crate::CardAt<V>) {
        unimplemented!("auto-playing a card at a random enemy")
    }

    /// `CardCmd.AutoPlay` of the card at one place, with no target named.
    /// Nothing once the fight is ending or its owner is dead.
    fn auto_play_card(&mut self, _card: crate::CardAt<V>) {
        unimplemented!("auto-playing a named card")
    }

    /// `CardCmd.AutoPlay` of a card that left the fight, as a power card one
    /// play already took out has, with no target named. The card is removed
    /// from the fight's state, so it does not play.
    fn auto_play_departed(&mut self, _card: crate::CardId<V>) {
        unimplemented!("auto-playing a card no pile holds")
    }

    /// `StableShuffle` over `cards` off the named run stream: put in the
    /// game's card order, then shuffled.
    fn stable_shuffle_cards(&mut self, _cards: &mut [crate::CardAt<V>], _stream: V::RunRngType) {
        unimplemented!("shuffling cards named in the fight")
    }

    /// `CardModel.GeneratePlayCount(null)` for the copy `card` names,
    /// wherever it stands: its replays and one, through
    /// `ModifyCardPlayCount`, then `AfterModifyingCardPlayCount`.
    fn generate_play_count(&mut self, _card: crate::CardId<V>) -> V::Number {
        unimplemented!("a play count worked out outside a play")
    }

    /// `RunRng.MonsterAi.NextItem(items)`, which draws nothing from an empty
    /// list.
    fn monster_ai_choose<T: Copy>(&mut self, _items: &[T]) -> Option<T> {
        unimplemented!("a monster choosing at random")
    }

    /// `CreatureCmd.Add` into `Encounter.GetNextSlot`: the first of the
    /// encounter's slots no enemy in the fight holds. Nothing where every
    /// slot is held.
    fn summon_into_first_free_slot(&mut self, _monster: V::Monster) -> Option<crate::CombatId> {
        unimplemented!("a monster summoned into the first free slot")
    }

    /// `CombatEnergyCosts.NextInt(below)`: a cost drawn at random.
    fn next_energy_cost(&mut self, _below: u8) -> V::Number {
        unimplemented!("a random energy cost")
    }

    /// `ResolveEnergyXValue` for the card being played: the X its play
    /// captured, which `Hook.ModifyXValue` can change.
    fn x_value(&self) -> V::Number {
        unimplemented!("an X-cost card's X")
    }

    /// `ResolveStarXValue` for the card being played: the stars its play
    /// spent, which `Hook.ModifyXValue` can change.
    fn star_x_value(&self) -> V::Number {
        unimplemented!("an X star cost's X")
    }

    /// One of the enemies that can be hit, drawn off `CombatTargets` by
    /// `NextItem`. Nothing when none can.
    fn random_opponent(&mut self) -> Option<crate::CombatId> {
        unimplemented!("a random target")
    }

    /// One hit's target in an attack `TargetingRandomOpponents`: a living
    /// opponent, hittable or not, drawn off `CombatTargets` by `NextItem`.
    /// Nothing when none is alive.
    fn random_target(&mut self) -> Option<crate::CombatId> {
        unimplemented!("a random attack's target")
    }

    /// `PowerCmd.Apply<P>(Owner.Creature, ...)`, on whoever is acting.
    ///
    /// The player for a card and the monster for a move, which is the creature
    /// a card saying "gain" means.
    fn apply_to_actor(&mut self, power: V::Power, amount: PowerAmount<V::Number>);

    /// `PowerCmd.Apply` of a fraction below one onto the actor. A fresh
    /// instance lands and holds nothing, since `ApplyInternal` floors the
    /// amount only after the zero check.
    fn apply_fraction_to_actor(&mut self, power: V::Power);

    /// `CreatureCmd.Heal(at, amount)`, which no power modifies.
    ///
    /// The health is capped at the recipient's maximum, and a dead creature
    /// healed above zero is alive again.
    fn heal(&mut self, at: crate::CombatId, amount: V::Number);

    /// Read or write the acting monster's own model, where the fight holds
    /// it. So a hook the move raises reads what the move has written, as the
    /// game's one model object does.
    ///
    /// A monster that leaves the fight during its own move keeps its model
    /// until the move is over.
    fn model<R>(&mut self, _with: impl FnOnce(&mut V::MonsterState) -> R) -> R
    where
        Self: Sized,
        V: Hooks,
    {
        unimplemented!("a monster's model read from a move")
    }

    /// Write the encounter's own model, as `CombatState.Encounter` is.
    fn write_encounter(&mut self, _write: impl FnOnce(&mut V::EncounterState))
    where
        Self: Sized,
        V: Hooks,
    {
        unimplemented!("an encounter's model written in a fight")
    }

    /// `Player.Gold`, which the fight holds while it runs.
    fn gold(&self) -> V::Number {
        unimplemented!("the player's gold read in a fight")
    }

    /// Read or write the model of every enemy in the fight, in fight order,
    /// as a move walks `CombatState.Enemies`.
    fn enemy_models(&mut self, _with: impl FnMut(&mut V::MonsterState))
    where
        Self: Sized,
        V: Hooks,
    {
        unimplemented!("the enemies' models read from a move")
    }

    /// Write the fields `power` saves on the creature `at`, from inside one
    /// of that power's own methods.
    ///
    /// A move a power builds for its owner, such as a revive, is the power's
    /// method in the game and writes the power's own data. Of an instanced
    /// power, the instance applied last. Nothing when the creature does not
    /// carry it.
    fn write_power(
        &mut self,
        at: crate::CombatId,
        power: V::Power,
        write: impl FnOnce(&mut V::PowerState),
    ) where
        V: Hooks;

    /// [`Fight::write_power`] of the `nth` instance of `power` the creature
    /// `at` carries, in the order they were applied.
    fn write_power_at(
        &mut self,
        _at: crate::CombatId,
        _power: V::Power,
        _nth: usize,
        _write: impl FnOnce(&mut V::PowerState),
    ) where
        V: Hooks,
    {
        unimplemented!("a power instance's model written in a fight")
    }

    /// [`Fight::modify_power_amount`] of the `nth` instance of `power` the
    /// creature `at` carries, in the order they were applied.
    fn modify_power_at(
        &mut self,
        _at: crate::CombatId,
        _power: V::Power,
        _nth: usize,
        _amount: V::Number,
        _applier: Option<crate::CombatId>,
    ) {
        unimplemented!("a power instance's amount moved")
    }

    /// `GetPowerInstances`: the `Target` and the stack of each instance of
    /// `power` the creature `at` carries, in the order they were applied.
    fn power_instances(&self, _at: crate::CombatId, _power: V::Power) -> PowerInstances<V> {
        unimplemented!("a power's instances read in a fight")
    }

    /// `PowerCmd.Apply` of a new instance of `power` on `at`, from
    /// `applier`, whose `Target` its applier set to `target` first.
    fn apply_aimed(
        &mut self,
        _at: crate::CombatId,
        _power: V::Power,
        _amount: PowerAmount<V::Number>,
        _applier: Option<crate::CombatId>,
        _target: crate::CombatId,
    ) {
        unimplemented!("an aimed power applied in a fight")
    }

    /// `CombatState.PlayerCreatures`: each player's creature, in seat order,
    /// living or not.
    fn players(&self) -> smallvec::SmallVec<[crate::CombatId; 4]> {
        unimplemented!("the fight's players")
    }

    /// `act`, with the actor's command reaching `player`: their piles and
    /// gold are what it reads and writes. The actor stays the actor.
    fn for_player(&mut self, _player: crate::CombatId, _act: impl FnOnce(&mut Self))
    where
        Self: Sized,
    {
        unimplemented!("acting for one of several players")
    }

    /// The character whose pool combat generation reads.
    fn character(&self) -> V::Character {
        unimplemented!("the character in this fight")
    }

    /// Whether the run has several players, which the pools combat
    /// generation reads deal the co-op cards from.
    fn multiplayer(&self) -> bool {
        unimplemented!("how many players this fight's run has")
    }

    /// One card of a combat-ready pool, `NextItem` off
    /// `CombatCardGeneration`, which is each draw `CardFactory.GetForCombat`
    /// makes. Nothing, for an empty pool.
    fn generate_card(&mut self, _cards: &[V::Card]) -> Option<V::Card> {
        unimplemented!("generating a card for combat")
    }

    /// Generate distinct cards from a combat-ready pool.
    fn generate_cards<const N: usize>(
        &mut self,
        _cards: &mut [V::Card],
        _stream: V::RunRngType,
    ) -> Result<[crate::CardIdent<V>; N], crate::rng::Error> {
        unimplemented!("generating cards for combat")
    }
}

/// Which of a card's two prices a temporary cost sets.
///
/// `CardModel` keeps them apart and names them separately, so an effect that
/// moves one says which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostPool {
    /// `CardModel.EnergyCost`.
    Energy,
    /// `CardModel.StarCost`, which only the Regent's cards carry.
    Star,
}

/// One thing an effect writes on a copy of a card.
///
/// The vocabulary an upgrade is already spelled in: `OnUpgrade` is a list of
/// these, and so is what a potion or a power writes. What differs is how long
/// each lasts, which the [`Lasting`] beside it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change<V: ContentVersion> {
    /// `CardCmd.Upgrade(card)`: whatever this card's own `OnUpgrade` does.
    ///
    /// It stands for the card's own list rather than naming one write. The
    /// list belongs to the card, and the effect asking does not know it. A
    /// card already at its maximum is left alone.
    Upgrade,
    /// `DynamicVars[key].UpgradeValueBy(amount)`, and `ForgeCmd`'s raise.
    Raise(V::CardVar, V::Number),
    /// `DynamicVars[key].BaseValue += amount` for a decimal amount, which
    /// Thrash writes: the hit it took from the card it exhausted.
    RaiseExactly(V::CardVar, Fixed),
    /// `LocalCostType.Relative`: the cost moves by an amount.
    ///
    /// `EnergyCost.UpgradeBy`, which an upgrade uses, and `AddThisTurn`.
    ChangeCost(CostPool, V::Number),
    /// `LocalCostType.Absolute`: the cost becomes a number.
    ///
    /// `EnergyCost.SetThisTurn` and its kin. `SetToFreeThisTurn` is this at
    /// zero on both pools, for [`Lasting::ThisTurn`].
    SetCost(CostPool, V::Number),
    /// `SetThisTurnOrUntilPlayed(cost, reduceOnly: true)` and its kind: the
    /// cost set to this where that lowers it, and left where it does not.
    LowerCost(CostPool, V::Number),
    /// `CardModel.AddKeyword`: the copy carries a word its table does not.
    /// The word is on the copy for as long as the copy lasts. Written to last
    /// the turn, it is `GiveSingleTurnSly`, which the turn's end clears.
    AddKeyword(V::CardKeyword),
    /// `CardModel.RemoveKeyword`: the copy no longer carries a word, whether
    /// its table prints it or something wrote it on.
    RemoveKeyword(V::CardKeyword),
    /// A method of the copy's own class, which writes a field the class adds
    /// as well as a number the copy prints. `Claw.BuffFromClawPlay` is one.
    Own(V::CardWrite),
}

/// How long a [`Change`] lasts.
///
/// `LocalCostModifierExpiration`, which is a flag set over two events. The
/// combinations the game names are these four.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lasting {
    /// Neither flag: as long as the fight does. `EndOfCombat`.
    ///
    /// What an upgrade is. A combat copy dies with the fight, so nothing a
    /// fight writes outlives one.
    ThisFight,
    /// `WhenPlayed`. `SetUntilPlayed`, which King's Gambit uses.
    UntilPlayed,
    /// `EndOfTurn`. `SetThisTurn`, which Bullet Time uses.
    ThisTurn,
    /// `EndOfTurn | WhenPlayed`, whichever comes first.
    /// `SetThisTurnOrUntilPlayed`, which `SetToFreeThisTurn` uses.
    ThisTurnOrUntilPlayed,
}

impl Lasting {
    /// `Expiration.HasFlag(EndOfTurn)`.
    pub const fn clears_when_turn_ends(self) -> bool {
        matches!(self, Self::ThisTurn | Self::ThisTurnOrUntilPlayed)
    }

    /// `Expiration.HasFlag(WhenPlayed)`.
    pub const fn clears_when_played(self) -> bool {
        matches!(self, Self::UntilPlayed | Self::ThisTurnOrUntilPlayed)
    }
}

/// How far a card's play got.
///
/// `CardModel.OnPlay` is an `async Task`, and a card that selects awaits in the
/// middle of one. A card that has to ask says where it stopped, `R`, and is
/// called back: [`ContentVersion::Resume`] for the fight's [`FightKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "a card play can stop on a question that must be carried"]
pub enum Played<V: ContentVersion, R> {
    /// The play ran to its end.
    Done,
    /// The play stopped on a `CardSelectCmd`, which is a question.
    Asking {
        /// The pile it selects from.
        pile: crate::Pile,
        /// How many it takes, which decides whether it asks at all.
        selection: Selection,
        /// Where the card stopped, in the card's own words.
        at: R,
    },
    /// The play stopped on a `CardSelectCmd` whose filter lets these cards
    /// through, which are the ones it offers. They stand in one pile, in its
    /// order.
    AskingAmong {
        cards: smallvec::SmallVec<[crate::CardAt<V>; MAX_CARDS_IN_HAND]>,
        /// How many it takes, which decides whether it asks at all.
        selection: Selection,
        at: R,
    },
    /// The play stopped on a choose-a-card screen.
    Choosing {
        offered: Offered<V>,
        skippable: bool,
        at: R,
    },
    /// A card the play auto-played stopped on a question, which
    /// [`Fight::waiting`] says. The play carries on from `at`, with
    /// [`Taken::Waited`], once that card's play is over.
    Waiting { at: R },
}

/// What a stopped effect resumes with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taken<V: ContentVersion> {
    Selected(SelectedCards<V>),
    Chosen(Option<crate::CardIdent<V>>),
    /// Nothing asked: the play waited on a card it auto-played.
    Waited,
    /// The play waited on its own draw, which a shuffle inside stood still.
    /// These are every copy it drew, in order.
    Drew(Drew<V>),
}

/// The copies one `CardPileCmd.Draw` drew, in order. A hand holds no more.
pub type Drew<V> = smallvec::SmallVec<[crate::CardId<V>; MAX_CARDS_IN_HAND]>;

/// The choose-a-card screen's fixed capacity.
pub const CHOICES: usize = 3;

/// The cards a choice offers, which is at most [`CHOICES`].
pub type Offered<V> = smallvec::SmallVec<[crate::CardIdent<V>; CHOICES]>;

/// `CardPile.MaxCardsInHand`: the most a hand holds.
pub const MAX_CARDS_IN_HAND: usize = 10;

/// How many cards a selection takes, which is `CardSelectorPrefs`'s pair of
/// counts.
///
/// The two differ in more than arithmetic. `RequireManualConfirmation` is set
/// when the counts differ, and that decides whether the selector asks at all.
///
/// The count is as wide as a pile, because the game writes "any number of
/// them" as a count: `CardSelectorPrefs(prompt, 0, 999999999)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// `CardSelectorPrefs(prompt, n)`, which sets both counts to `n`.
    ///
    /// Nothing has to be confirmed, so a pile holding no more than `n` is
    /// taken whole and the selector asks nothing.
    Exactly(nonmax::NonMaxUsize),
    /// `CardSelectorPrefs(prompt, 0, n)`, which sets the counts apart.
    ///
    /// Taking none is an answer, so the selector asks whenever the pile holds
    /// a card. Even one card is a question, because leaving it is a choice.
    UpTo(nonmax::NonMaxUsize),
}

/// Cards returned by a [`Selection`] after its cardinality was checked.
///
/// The contents stay opaque so card behaviour cannot be resumed with an
/// unchecked slice. The simulator constructs this at the boundary where it
/// handles either a player's answer or a selection taken without asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCards<V: ContentVersion>(Picked<V>);

/// The cards a selection names, before or after it is checked.
///
/// Inline to ten, which is what a hand holds. A selection has no bound of its
/// own: the game writes "any number" as a count, and one taken without asking
/// takes a pile whole. So a longer one reaches the heap.
pub type Picked<V> = smallvec::SmallVec<[crate::CardAt<V>; 10]>;

impl Selection {
    /// Validate a player's answer to this selection.
    pub fn validate_answer<V: ContentVersion>(self, cards: Picked<V>) -> Option<SelectedCards<V>> {
        let allowed = match self {
            Self::Exactly(count) => cards.len() == count.get(),
            Self::UpTo(count) => cards.len() <= count.get(),
        };
        allowed.then_some(SelectedCards(cards))
    }

    /// Validate cards taken without asking.
    ///
    /// An empty pile gives nothing without asking, which both
    /// `CardSelectCmd.FromHand` and `FromCombatPile` check first. An exact
    /// selection takes a pile whole when it holds no more than the requested
    /// count. An up-to selection asks otherwise.
    pub fn validate_automatic<V: ContentVersion>(
        self,
        cards: Picked<V>,
    ) -> Option<SelectedCards<V>> {
        match self {
            _ if cards.is_empty() => Some(SelectedCards(cards)),
            Self::Exactly(count) if cards.len() <= count.get() => Some(SelectedCards(cards)),
            Self::Exactly(_) | Self::UpTo(_) => None,
        }
    }
}

impl<V: ContentVersion> SelectedCards<V> {
    /// The selected cards, after cardinality validation.
    pub fn as_slice(&self) -> &[crate::CardAt<V>] {
        &self.0
    }

    /// The selected card for a question that takes at most one.
    ///
    /// Panics when card behaviour asks this of a multi-card selection.
    pub fn one_or_none(&self) -> Option<crate::CardAt<V>> {
        assert!(
            self.0.len() <= 1,
            "a single-card continuation received multiple cards"
        );
        self.0.first().copied()
    }
}

/// A combat operation before the fight's current listeners have modified it.
///
/// Every operation names the creatures it acts between. So resolving one is a
/// single step: the number it carries is worked out, and the hooks the
/// operation names run over it. Nothing in between is observable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect<V: ContentVersion> {
    /// `DamageCmd.Attack(damage).FromCard(this).Targeting(at)`.
    Attack {
        dealer: crate::CombatId,
        target: crate::CombatId,
        damage: Amount<V>,
        scaling: Scaling,
    },
    /// `CreatureCmd.GainBlock(recipient, amount, props, cardPlay)`.
    GainBlock {
        recipient: crate::CombatId,
        amount: Amount<V>,
        scaling: Scaling,
    },
}

/// Whether the powers that scale a number act on this one.
///
/// The game keeps this as `ValueProp`, a set of flags. `IsPoweredAttack` and
/// `IsPoweredCardOrMonsterMoveBlock` are both the `Move` flag without the
/// `Unpowered` one, so one answer serves a hit and a block.
///
/// A card or a monster move prints `Move`, and a relic prints `Unpowered`.
/// Entrench prints both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scaling {
    /// A relic's number, which lands as it reads.
    Static,
    /// A card's number printed `Unpowered | Move`: it lands as it reads, and
    /// `IsCardOrMonsterMove` still says yes.
    StaticMove,
    /// A card's number or a monster move's, which Strength and Dexterity
    /// move.
    Dynamic,
}

/// The number an effect carries, before the fight works it out.
///
/// A card names one of its own variables. A monster or a relic hands over a
/// number it already holds. The two differ in what resolution does first. A
/// variable can be a `CalculatedVar`, which counts something in the live fight
/// rather than reading the base it was built with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Amount<V: ContentVersion> {
    /// A number whoever asked already holds.
    Number(V::Number),
    /// One of the numbers on the card being played, by key.
    Var(V::CardVar),
}

/// What content behavior can read from an effect resolved by a fight.
pub trait EffectResolution<V: ContentVersion> {
    /// The amount the fight worked out, which every resolution has one of.
    ///
    /// Reading it is not applying it. A card that keeps a number for later,
    /// such as Glitterstream's block, resolves the gain and stores this rather
    /// than hand it to [`Fight::apply`].
    fn amount(&self) -> V::Number;

    /// Whether the game's decimal was above nothing, before the floor. A
    /// fraction below one is, though [`EffectResolution::amount`] is nothing.
    fn positive(&self) -> bool;
}

/// What a power does to a number a fight works out.
///
/// One shape for both of the game's modifier hooks. `ModifyDamage` and
/// `ModifyBlock` hand a power different things to read, and take the same
/// answer. Most powers override one, or neither.
///
/// One question rather than two. A power with no hook has neither an addition
/// nor a share to name, and [`Modify::Nothing`] is how it says so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Modify<N> {
    /// No hook of this kind, or one that does not act here.
    Nothing,
    /// The `Additive` half, which lands before anything multiplies.
    Added(N),
    /// The `Multiplicative` half, over what the additions left. Every share a
    /// hook hands back is a few hundredths, which a [`Fixed`] holds exactly.
    Scaled(Fixed),
    /// A ceiling on what is left. The listener returns a number of its own
    /// rather than a share of the one it was handed.
    ///
    /// `ModifyHpLost` is folded that way: each listener is handed what the one
    /// before it returned. Slippery hands back one, whatever it was given.
    AtMost(N),
    /// The whole number, which a `TryModify` hook that returns true hands
    /// back in place of the one it was handed.
    Replaced(N),
    /// A floor under what is left, once it is at least `from`. The Boot
    /// raises a loss of one to four to five, and leaves nothing at nothing.
    RaisedTo { floor: N, from: N },
    /// Less by this much, and never below nothing: `Math.Max(0, amount - n)`.
    /// Tungsten Rod takes one off that way, where a loss below nothing heals.
    Less(N),
    /// [`Modify::AtMost`] on anything but nothing, which stays nothing. Hardened
    /// Shell's cap, which can be below nothing, leaves a hit that took nothing
    /// alone.
    AtMostUnlessNothing(N),
}

/// A value-producing hook the simulator asks one content model to apply.
///
/// One per `AbstractModel` hook, each carrying the creatures the game hands
/// it. A listener reads whether it is one of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifyHook<V: ContentVersion> {
    /// `ModifyGoldGained`: the gold `player` is about to gain.
    GoldGained {
        player: crate::CombatId,
    },
    /// `ModifyRestSiteHealAmount`: what a rest site's heal gives the player,
    /// whose maximum health is `max_hp`.
    RestSiteHealAmount {
        max_hp: V::Number,
    },
    /// `ModifyOddsIncreaseForUnrolledRoomType`: how far an unknown room's
    /// odds of being `room` rise when it rolls something else.
    OddsIncreaseForUnrolledRoomType {
        room: V::RoomType,
    },
    /// `ModifyDamageAdditive`. The target is absent for a hit previewed
    /// against nobody.
    DamageAdditive {
        target: Option<crate::CombatId>,
        /// `dealer`: the creature swinging, or nothing for damage no creature
        /// deals, such as Poison's.
        dealer: Option<crate::CombatId>,
        /// `cardSource`: the card whose number this is, and which card it
        /// is. Nothing for a monster's move.
        card: Option<CardSource<V>>,
        /// `props.IsPoweredAttack()`.
        powered: bool,
    },
    /// `ModifyDamageMultiplicative`, over what the additions left.
    DamageMultiplicative {
        target: Option<crate::CombatId>,
        /// `dealer`: the creature swinging, or nothing for damage no creature
        /// deals, such as Poison's.
        dealer: Option<crate::CombatId>,
        /// `cardSource`, as [`ModifyHook::DamageAdditive`] carries it.
        card: Option<CardSource<V>>,
        /// `props.IsPoweredAttack()`.
        powered: bool,
    },
    /// `ModifyDamageCap`, the most a hit can do.
    DamageCap {
        target: Option<crate::CombatId>,
        /// `dealer`: the creature swinging, or nothing for damage no creature
        /// deals, such as Poison's.
        dealer: Option<crate::CombatId>,
        /// `props.IsPoweredAttack()`.
        powered: bool,
    },
    /// `ModifyBlockAdditive`, for block `target` is about to gain.
    BlockAdditive {
        target: crate::CombatId,
        /// `props.IsPoweredCardOrMonsterMoveBlock()`.
        powered: bool,
        /// `cardSource`: the card whose own play gives the block.
        card: Option<V::Card>,
        /// `cardSource.Owner.Creature`.
        player: Option<crate::CombatId>,
    },
    /// `ModifyBlockMultiplicative`, over what the additions left.
    BlockMultiplicative {
        target: crate::CombatId,
        /// `props.IsPoweredCardOrMonsterMoveBlock()`.
        powered: bool,
        /// `props.IsCardOrMonsterMove()`, the `Move` flag alone.
        moved: bool,
        /// `cardSource`, as [`ModifyHook::BlockAdditive`] carries it.
        card: Option<V::Card>,
        /// `cardSource.Owner.Creature`.
        player: Option<crate::CombatId>,
        /// `cardPlay`, as the play's [`PlayAt`].
        play: Option<PlayAt>,
        /// `cardSource`: which copy gives the block.
        source: Option<crate::CardId<V>>,
    },
    /// `ModifyHpLostBeforeOsty`, over what the block did not take.
    HpLostBeforeOsty {
        target: crate::CombatId,
    },
    HpLostBeforeOstyLate {
        target: crate::CombatId,
    },
    HpLostAfterOsty {
        target: crate::CombatId,
        /// `CombatManager.IsInProgress`.
        in_fight: bool,
    },
    HpLostAfterOstyLate {
        target: crate::CombatId,
        /// `dealer`, nothing for a loss no creature deals.
        dealer: Option<crate::CombatId>,
        /// `props.IsPoweredAttack()`.
        powered: bool,
    },
    /// `ModifyPowerAmountGivenAdditive`: what `giver` adds to `power` it
    /// applies to `target`.
    PowerAmountGivenAdditive {
        power: V::Power,
        giver: crate::CombatId,
        target: Option<crate::CombatId>,
    },
    /// `ModifyPowerAmountGivenMultiplicative`, over what the additions left,
    /// which is `amount`.
    PowerAmountGivenMultiplicative {
        power: V::Power,
        giver: crate::CombatId,
        target: Option<crate::CombatId>,
        amount: V::Number,
        /// `cardSource`: which copy gives the power.
        source: Option<crate::CardId<V>>,
    },
    /// `TryModifyPowerAmountReceived`: what `amount` of `power` lands on
    /// `target`, as an application or a change to one it carries.
    PowerAmountReceived {
        power: V::Power,
        target: crate::CombatId,
        amount: V::Number,
    },
    /// `ModifyHandDraw`: how many cards the player draws to open a turn, in a
    /// room of this type.
    HandDraw {
        player: crate::CombatId,
        count: V::Number,
        turn: V::Count,
        room: V::RoomType,
    },
    HandDrawLate {
        player: crate::CombatId,
        count: V::Number,
        turn: V::Count,
        room: V::RoomType,
    },
    /// `ModifyMerchantPrice`: what a merchant's shelf costs, which
    /// `MerchantEntry.Cost` asks every time it is read in a merchant's room.
    MerchantPrice,
    /// `TryModifyStarCost`, over a card's star cost once its own modifiers
    /// are folded in. Every cost the fight reads is of a card in hand or in
    /// play.
    StarCost {
        card: (crate::CardId<V>, V::Card),
        /// The card's `Owner.Creature`.
        player: crate::CombatId,
        /// `Pile.Type` for the copy, or nothing for the play pile.
        pile: Option<crate::Pile>,
    },
    /// `ModifyEnergyGain`: the energy `PlayerCmd.GainEnergy` gives `player`.
    EnergyGain {
        player: crate::CombatId,
    },
    /// `ModifyMaxEnergy`: the energy `player`'s turn resets to, which
    /// `PlayerCombatState.MaxEnergy` asks every time it is read.
    MaxEnergy {
        player: crate::CombatId,
        /// The player's `TurnNumber`, which a listener reads off the player.
        turn: V::Count,
    },
    /// `TryModifyEnergyCostInCombat`, over a card's cost once its own
    /// modifiers are folded in. Most costs the fight reads are of a card in
    /// hand or being played. An auto-play reads the pile the card stood in.
    EnergyCost {
        card: (crate::CardId<V>, V::Card),
        /// The card's `Owner.Creature`, which a listener compares with its
        /// own.
        player: crate::CombatId,
        /// `Keywords.Contains(Ethereal)` for the copy.
        ethereal: bool,
        /// The copy's `Affliction`.
        affliction: Option<V::Affliction>,
        /// `Pile.Type` for the copy, or nothing for the play pile, which the
        /// fight keeps apart from the four.
        pile: Option<crate::Pile>,
    },
    /// `TryModifyEnergyCostInCombatLate`, over what [`ModifyHook::EnergyCost`]
    /// left.
    EnergyCostLate {
        card: (crate::CardId<V>, V::Card),
        /// The card's `Owner.Creature`.
        player: crate::CombatId,
        ethereal: bool,
        affliction: Option<V::Affliction>,
        pile: Option<crate::Pile>,
    },
    /// `ModifyOrbPassiveTriggerCounts`: how many times one of `player`'s
    /// orbs triggers its passive. `first` is whether it is `Orbs[0]`.
    OrbPassiveTriggerCount {
        player: crate::CombatId,
        first: bool,
    },
    /// `ModifyXValue`: the X a card of `player`'s resolves to.
    XValue {
        player: crate::CombatId,
    },
    /// `ModifyOrbValue`: a number `player`'s orb shows, as its passive or its
    /// evoke reads it.
    OrbValue {
        player: crate::CombatId,
        orb: V::Orb,
    },
    /// `ModifyCardPlayCount`: how many times a card `player` plays runs its
    /// effect.
    CardPlayCount {
        player: crate::CombatId,
        card: V::Card,
        /// `CardPlay.Target`, which a listener can read.
        target: Option<crate::CombatId>,
    },
}

impl<V: ContentVersion> ModifyHook<V> {
    /// Whether `Hook` walks `IterateCombatHookListeners` for this, which
    /// hands a fight that is over or ending no listeners. The rest walk the
    /// run's, which keep answering.
    pub const fn combat_listeners(&self) -> bool {
        match self {
            ModifyHook::GoldGained { .. }
            | ModifyHook::RestSiteHealAmount { .. }
            | ModifyHook::OddsIncreaseForUnrolledRoomType { .. }
            | ModifyHook::DamageAdditive { .. }
            | ModifyHook::DamageMultiplicative { .. }
            | ModifyHook::DamageCap { .. }
            | ModifyHook::HpLostBeforeOsty { .. }
            | ModifyHook::HpLostBeforeOstyLate { .. }
            | ModifyHook::HpLostAfterOsty { .. }
            | ModifyHook::HpLostAfterOstyLate { .. }
            | ModifyHook::MerchantPrice => false,
            ModifyHook::BlockAdditive { .. }
            | ModifyHook::BlockMultiplicative { .. }
            | ModifyHook::PowerAmountGivenAdditive { .. }
            | ModifyHook::PowerAmountGivenMultiplicative { .. }
            | ModifyHook::PowerAmountReceived { .. }
            | ModifyHook::HandDraw { .. }
            | ModifyHook::HandDrawLate { .. }
            | ModifyHook::StarCost { .. }
            | ModifyHook::EnergyGain { .. }
            | ModifyHook::MaxEnergy { .. }
            | ModifyHook::EnergyCost { .. }
            | ModifyHook::EnergyCostLate { .. }
            | ModifyHook::OrbPassiveTriggerCount { .. }
            | ModifyHook::XValue { .. }
            | ModifyHook::OrbValue { .. }
            | ModifyHook::CardPlayCount { .. } => true,
        }
    }

    /// The `AbstractModel` hook this is raised as.
    pub fn model_hook(&self) -> ModelHook {
        match self {
            ModifyHook::DamageAdditive { .. } => ModelHook::ModifyDamageAdditive,
            ModifyHook::DamageMultiplicative { .. } => ModelHook::ModifyDamageMultiplicative,
            ModifyHook::DamageCap { .. } => ModelHook::ModifyDamageCap,
            ModifyHook::BlockAdditive { .. } => ModelHook::ModifyBlockAdditive,
            ModifyHook::BlockMultiplicative { .. } => ModelHook::ModifyBlockMultiplicative,
            ModifyHook::HpLostBeforeOsty { .. } => ModelHook::ModifyHpLostBeforeOsty,
            ModifyHook::HpLostBeforeOstyLate { .. } => ModelHook::ModifyHpLostBeforeOstyLate,
            ModifyHook::HpLostAfterOsty { .. } => ModelHook::ModifyHpLostAfterOsty,
            ModifyHook::HpLostAfterOstyLate { .. } => ModelHook::ModifyHpLostAfterOstyLate,
            ModifyHook::HandDraw { .. } => ModelHook::ModifyHandDraw,
            ModifyHook::PowerAmountGivenAdditive { .. } => {
                ModelHook::ModifyPowerAmountGivenAdditive
            }
            ModifyHook::PowerAmountGivenMultiplicative { .. } => {
                ModelHook::ModifyPowerAmountGivenMultiplicative
            }
            ModifyHook::PowerAmountReceived { .. } => ModelHook::TryModifyPowerAmountReceived,
            ModifyHook::HandDrawLate { .. } => ModelHook::ModifyHandDrawLate,
            ModifyHook::GoldGained { .. } => ModelHook::ModifyGoldGained,
            ModifyHook::RestSiteHealAmount { .. } => ModelHook::ModifyRestSiteHealAmount,
            ModifyHook::OddsIncreaseForUnrolledRoomType { .. } => {
                ModelHook::ModifyOddsIncreaseForUnrolledRoomType
            }
            ModifyHook::MaxEnergy { .. } => ModelHook::ModifyMaxEnergy,
            ModifyHook::EnergyGain { .. } => ModelHook::ModifyEnergyGain,
            ModifyHook::StarCost { .. } => ModelHook::TryModifyStarCost,
            ModifyHook::MerchantPrice => ModelHook::ModifyMerchantPrice,
            ModifyHook::EnergyCost { .. } => ModelHook::TryModifyEnergyCostInCombat,
            ModifyHook::EnergyCostLate { .. } => ModelHook::TryModifyEnergyCostInCombatLate,
            ModifyHook::CardPlayCount { .. } => ModelHook::ModifyCardPlayCount,
            ModifyHook::OrbValue { .. } => ModelHook::ModifyOrbValue,
            ModifyHook::OrbPassiveTriggerCount { .. } => ModelHook::ModifyOrbPassiveTriggerCounts,
            ModifyHook::XValue { .. } => ModelHook::ModifyXValue,
        }
    }
}

/// A yes-or-no hook the simulator asks one content model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionHook<V: ContentVersion> {
    /// `ShouldCreatureBeRemovedFromCombatAfterDeath`, about the creature that
    /// has just died.
    ShouldRemoveCreatureAfterDeath { creature: crate::CombatId },
    /// `PowerModel.OwnerIsSecondaryEnemy`, which the game reads off the
    /// powers on one creature rather than raising.
    ShouldCountForCombatEnd,
    /// `ShouldClearBlock`, as `creature` starts its turn.
    ShouldClearBlock { creature: crate::CombatId },
    /// `ShouldFlush`, as `player`'s turn ends. A hand any listener answers no
    /// for keeps every card.
    ShouldFlush {
        player: crate::CombatId,
        /// The player's `TurnNumber`, which a listener reads off the player.
        turn: V::Count,
    },
    /// `ShouldPlay`, asked of a card in hand as `CanPlay` looks it over. The
    /// first listener to answer no is the preventer, and the card is
    /// `BlockedByHook`.
    ShouldPlay {
        card: V::Card,
        /// `card.Owner.Creature`.
        player: crate::CombatId,
        affliction: Option<V::Affliction>,
        /// `autoPlayType != AutoPlayType.None`: the play is an auto-play.
        auto: bool,
    },
    /// `ShouldStopCombatFromEnding`, which any listener can answer yes to.
    ShouldStopCombatFromEnding,
    /// `ShouldDraw`, once for each `CardPileCmd.Draw`. A draw any listener
    /// answers no to draws nothing. `hand_draw` is `fromHandDraw`, which the
    /// turn's own draw is.
    /// `ShouldDie`, as `creature`'s hit points are gone, asked ahead of
    /// [`DecisionHook::ShouldDieLate`]. A listener compares it with its own
    /// owner.
    ShouldDie { creature: crate::CombatId },
    /// `ShouldDieLate`, as `creature`'s hit points are gone. A creature no
    /// listener lets die stays in the fight.
    ShouldDieLate { creature: crate::CombatId },
    /// `ShouldForcePotionReward`, as a won fight's rewards roll for a potion
    /// in a room of this type. One listener answering yes gives the potion.
    ShouldForcePotionReward {
        /// The player whose rewards these are.
        player: crate::CombatId,
        room: V::RoomType,
    },
    /// `ShouldTakeExtraTurn`, once the player's turn is over. `plays` is the
    /// player's plays finished this turn that no auto-play made. One
    /// listener answering yes keeps the turn with the player. `turn` is the
    /// player's `TurnNumber`.
    ShouldTakeExtraTurn {
        /// The player asked about.
        player: crate::CombatId,
        plays: u8,
        turn: V::Count,
    },
    /// `ShouldRefillMerchantEntry`, once a shelf's purchase went through. One
    /// listener answering yes restocks the shelf rather than emptying it.
    ShouldRefillMerchantEntry,
    /// `ShouldDisableRemainingRestSiteOptions`, once a rest site's option
    /// went through. One listener answering no keeps the other options up.
    ShouldDisableRemainingRestSiteOptions { player: crate::CombatId },
    /// `ShouldGenerateTreasure`, as a treasure room begins its relic picking.
    /// One listener answering no leaves the chest empty.
    ShouldGenerateTreasure,
    /// `ShouldPlayerResetEnergy`, as `player` sets up their turn `turn`. A
    /// player no listener lets reset adds their maximum to what they have.
    ShouldPlayerResetEnergy {
        player: crate::CombatId,
        turn: V::Count,
    },
    ShouldDraw {
        player: crate::CombatId,
        hand_draw: bool,
        /// `CombatState.CurrentSide`: whose turn it is.
        side: V::CombatSide,
    },
    /// `ShouldAllowHitting`, about `creature`. A creature no listener lets be
    /// hit is not hittable, and cannot receive powers.
    ShouldAllowHitting {
        creature: crate::CombatId,
        /// `IsAlive` for that creature, which a listener can read.
        alive: bool,
    },
    /// `PowerModel.ShouldPowerBeRemovedAfterOwnerDeath`, asked of a power
    /// about itself as its owner dies.
    ShouldRemoveSelfAfterOwnerDeath,
    /// `PowerModel.ShouldOwnerDeathTriggerFatal`, asked of a power about its
    /// owner's death.
    ShouldOwnerDeathTriggerFatal,
    /// `Hook.ShouldPowerBeRemovedOnDeath`, asked of everything on a dying
    /// creature about one of the powers on it.
    ///
    /// The other half of the same removal: a power can keep itself, and
    /// something else on the creature can keep it. Either is enough.
    ShouldRemovePowerOnDeath {
        creature: crate::CombatId,
        power: V::Power,
    },
    /// `ModifyUnblockedDamageTarget`, about damage past `target`'s block. A
    /// listener that answers yes takes it on the creature carrying it.
    ///
    /// Asked of each listener in turn, with what its own creature is: the
    /// player it is a pet of, and whether it still stands. A power's override
    /// reads both off `Owner`.
    TakesUnblockedDamage {
        target: crate::CombatId,
        /// `IsPoweredAttack`, which is `Scaling::Dynamic`.
        powered: bool,
        /// `Owner.PetOwner.Creature` of the listener's creature.
        pet_of: Option<crate::CombatId>,
        /// `!Owner.IsDead` for the listener's creature.
        alive: bool,
    },
    /// `ShouldAllowFreeTravel`: whether the run can go on to any point of
    /// the next row, rather than only those its point leads on to.
    ShouldAllowFreeTravel,
    /// `ShouldAllowMerchantCardRemoval`: whether a merchant's card removal
    /// is on sale. One no marks it used as its slot fills.
    ShouldAllowMerchantCardRemoval,
    /// `ShouldProcurePotion`: whether `player` takes `potion`. One no
    /// refuses it.
    ShouldProcurePotion {
        player: crate::CombatId,
        potion: V::Potion,
    },
}

impl<V: ContentVersion> DecisionHook<V> {
    /// The `AbstractModel` hook this is raised as.
    ///
    /// Nothing for a question the game asks of one power about itself, which
    /// is not raised.
    pub fn model_hook(&self) -> Option<ModelHook> {
        Some(match self {
            DecisionHook::ShouldRemoveCreatureAfterDeath { .. } => {
                ModelHook::ShouldCreatureBeRemovedFromCombatAfterDeath
            }
            DecisionHook::ShouldRemovePowerOnDeath { .. } => ModelHook::ShouldPowerBeRemovedOnDeath,
            DecisionHook::ShouldClearBlock { .. } => ModelHook::ShouldClearBlock,
            DecisionHook::ShouldFlush { .. } => ModelHook::ShouldFlush,
            DecisionHook::ShouldPlay { .. } => ModelHook::ShouldPlay,
            DecisionHook::TakesUnblockedDamage { .. } => ModelHook::ModifyUnblockedDamageTarget,
            DecisionHook::ShouldStopCombatFromEnding => ModelHook::ShouldStopCombatFromEnding,
            DecisionHook::ShouldAllowHitting { .. } => ModelHook::ShouldAllowHitting,
            DecisionHook::ShouldAllowFreeTravel => ModelHook::ShouldAllowFreeTravel,
            DecisionHook::ShouldAllowMerchantCardRemoval => {
                ModelHook::ShouldAllowMerchantCardRemoval
            }
            DecisionHook::ShouldProcurePotion { .. } => ModelHook::ShouldProcurePotion,
            DecisionHook::ShouldDraw { .. } => ModelHook::ShouldDraw,
            DecisionHook::ShouldDie { .. } => ModelHook::ShouldDie,
            DecisionHook::ShouldDieLate { .. } => ModelHook::ShouldDieLate,
            DecisionHook::ShouldPlayerResetEnergy { .. } => ModelHook::ShouldPlayerResetEnergy,
            DecisionHook::ShouldForcePotionReward { .. } => ModelHook::ShouldForcePotionReward,
            DecisionHook::ShouldTakeExtraTurn { .. } => ModelHook::ShouldTakeExtraTurn,
            DecisionHook::ShouldRefillMerchantEntry => ModelHook::ShouldRefillMerchantEntry,
            DecisionHook::ShouldDisableRemainingRestSiteOptions { .. } => {
                ModelHook::ShouldDisableRemainingRestSiteOptions
            }
            DecisionHook::ShouldGenerateTreasure => ModelHook::ShouldGenerateTreasure,
            DecisionHook::ShouldCountForCombatEnd
            | DecisionHook::ShouldRemoveSelfAfterOwnerDeath
            | DecisionHook::ShouldOwnerDeathTriggerFatal => return None,
        })
    }
}

/// A notification delivered to one content model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventHook<V: ContentVersion> {
    /// `AfterSideTurnStartLate`, once every listener has answered
    /// [`EventHook::SideTurnStart`].
    SideTurnStartLate { side: V::CombatSide },
    /// `BeforePotionUsed`, as the player drinks a potion in a fight, before
    /// it leaves the belt.
    PotionUsing {
        /// `potion.Owner.Creature`, its drinker.
        player: crate::CombatId,
        /// The creature it is aimed at, where it aims.
        target: Option<crate::CombatId>,
    },
    /// `AfterPotionUsed`, once a potion's use is done. `in_fight` is
    /// `CombatManager.IsInProgress`, and `holding_potions` is
    /// `Owner.Potions.Any()`.
    PotionUsed {
        /// `potion.Owner.Creature`, its drinker.
        player: crate::CombatId,
        in_fight: bool,
        holding_potions: bool,
    },
    /// `AfterPotionProcured`, once a potion is in a belt slot. `in_fight` is
    /// `CombatManager.IsInProgress`, and `holding_potions` is
    /// `Owner.Potions.Any()`.
    PotionProcured {
        in_fight: bool,
        holding_potions: bool,
    },
    /// `AfterPotionDiscarded`, once a potion leaves the belt unused.
    /// `in_fight` is `CombatManager.IsInProgress`, and `holding_potions` is
    /// `Owner.Potions.Any()`.
    PotionDiscarded {
        in_fight: bool,
        holding_potions: bool,
    },
    /// `AfterModifyingCardRewardOptions`, once cards made for a reward have
    /// had their late pass. `card_reward` is as for
    /// [`EventHook::CardRewardOption`].
    CardRewardOptionsModified { card_reward: bool },
    /// `TryModifyCardRewardOptionsLate`, for one card `CardFactory` made for
    /// a reward. A listener answers with what it does to the card.
    ///
    /// `card_reward` is `CardCreationFlags.IsCardReward`, which a
    /// `CardReward` sets and a card made for another use does not.
    CardRewardOption {
        card: crate::CardIdent<V>,
        card_reward: bool,
    },
    /// `TryModifyCardRewardOptionsLate` over a whole card reward, asked of a
    /// relic once it has answered for each card. A listener that picks among
    /// the cards answers here.
    CardRewardPick { card_reward: bool },
    /// `TryModifyCardBeingAddedToDeck`, for a card about to join the deck.
    CardBeingAddedToDeck { card: crate::CardIdent<V> },
    /// `ModifyMerchantCardCreationResults`, for one card a merchant stocks.
    MerchantCardCreated { card: crate::CardIdent<V> },
    /// `AfterCardPlayedLate`, once every `AfterCardPlayed` of a play has run.
    CardPlayedLate {
        card: V::Card,
        id: crate::CardId<V>,
        /// `CardPlay.Card.Owner.Creature`.
        player: crate::CombatId,
        /// `CardPlay.Resources.EnergyValue`: the energy the play spent.
        energy: V::Count,
    },
    /// `AfterModifyingHandDraw`, to each listener whose `ModifyHandDraw`
    /// changed the turn's hand, before it is drawn. The listener reads for
    /// itself whether it did.
    HandDrawModified,
    /// `AfterModifyingEnergyGain`, to each listener whose `ModifyEnergyGain`
    /// changed a gain, before the energy lands. The listener reads for itself
    /// whether it did.
    EnergyGainModified,
    /// `AfterStarsGained`, once `PlayerCmd.GainStars` has added `amount` to
    /// the player's stars.
    StarsGained {
        /// `gainer.Creature`.
        player: crate::CombatId,
        amount: V::Number,
    },
    /// `AfterBlockGained`, once `CreatureCmd.GainBlock` has given `creature`
    /// `amount`, which is what the modifiers left, and can be none.
    BlockGained {
        creature: crate::CombatId,
        amount: V::Number,
        /// Whether the game's decimal was above nothing. A fraction below one
        /// is, though `amount` floors it to nothing.
        positive: bool,
        /// `CombatState.CurrentSide`: whose turn the gain falls in.
        side: V::CombatSide,
    },
    /// `AfterPreventingDraw`, once a listener answered no to `ShouldDraw`.
    DrawPrevented,
    /// `AfterShuffle`, once `player`'s discard pile is shuffled into their
    /// draw pile.
    Shuffled { player: crate::CombatId },
    /// `ModifyShuffleOrder`, over the draw pile just shuffled. `initial` is
    /// the shuffle a fight opens on.
    ShuffleOrder { initial: bool },
    /// `ModifyUnknownMapPointRoomTypes`, as an unknown map point rolls. A
    /// listener answers with the types it takes out.
    UnknownRoomTypes {
        /// `RunState.CurrentActIndex`.
        act: usize,
    },
    /// `ModifyGeneratedMap`, as the act at this index has its map generated.
    GeneratedMap { act: usize },
    /// `ModifyGeneratedMapLate`, once every listener has heard
    /// [`EventHook::GeneratedMap`].
    GeneratedMapLate { act: usize },
    /// `ModifyCardRewardCreationOptions`, as a `CardReward` over the
    /// character's pool makes its cards. The run raises it only for one whose
    /// options take a change to their pools. `elite_encounter` is a won
    /// elite's reward: `Source` is `Encounter` and `RarityOdds` is
    /// `EliteEncounter`.
    CardRewardCreationOptions {
        /// The player the reward is for.
        player: crate::CombatId,
        elite_encounter: bool,
    },
    /// `ModifyMerchantCardPool`, as a merchant stocks a card from the
    /// character's own pool. The colorless shelves are left out, which every
    /// listener here leaves alone.
    MerchantCardPool,
    /// `AfterRoomEntered`, for a room of this type.
    RoomEntered {
        room: V::RoomType,
        /// Whether the map point entered is one the point before does not
        /// lead on to, which only free travel reaches.
        strayed: bool,
        /// `RunState.CurrentMapPoint.PointType`: the type of the map point the
        /// room stands on, whatever room it rolled. None off the map.
        point: Option<V::MapPointType>,
        /// How many cards the deck holds, which a relic reads off its owner.
        /// The walk fills in each listener's own player's.
        deck: usize,
        /// The player's hit points, which the walk fills in for each
        /// listener's owner. A modifier reads the first player's.
        player: PlayerHealth<V>,
        /// `room == RunState.BaseRoom`: not a room an event opened inside its
        /// own.
        base: bool,
    },
    /// `AfterItemPurchased`, once a merchant's entry is bought, with the gold
    /// it cost.
    ItemPurchased { gold: V::Number },
    /// `TryModifyRewards`, for rewards once they are filled in: a room's of
    /// this type, or a relic's with no room, in the act at this index.
    ModifyRewards {
        room: Option<V::RoomType>,
        act: usize,
        /// `RunState.Acts.Count`.
        acts: usize,
    },
    /// `TryModifyRewardsLate`, once `TryModifyRewards` is through, for the
    /// same rewards as [`EventHook::ModifyRewards`].
    ModifyRewardsLate { room: Option<V::RoomType> },
    /// `TryModifyCardRewardAlternatives`, for one card reward of `player`'s.
    CardRewardAlternatives { player: crate::CombatId },
    /// A card reward of `player`'s, its alternative chosen, which is the
    /// `OnSelect` of the model that added it. That model is `player`'s own,
    /// and only it hears this.
    CardRewardAlternativeChosen {
        alternative: crate::RewardAlternative,
        player: crate::CombatId,
    },
    /// `BeforeCombatStart`, once the fight is built and before it opens, in
    /// a room of this type. `holding_potions` is `Owner.Potions.Any()`.
    CombatStarting {
        room: V::RoomType,
        holding_potions: bool,
        /// `RunState.CurrentMapPoint.coord`, where the fight stands on the
        /// map.
        point: Option<crate::Coord>,
    },
    /// `BeforeCombatStartLate`, once every listener has heard
    /// `BeforeCombatStart`.
    CombatStartingLate { room: V::RoomType },
    /// `AfterCombatEnd`, for the fight the run has just finished, in a room
    /// of this type.
    CombatEnd { room: V::RoomType },
    /// `AfterRestSiteHeal`, once a rest site's heal has landed on `player`,
    /// whom a relic compares with its owner. `mimicked` for a heal an event
    /// gives the way a rest site does. `max_hp` is that player's maximum
    /// health then.
    RestSiteHeal {
        player: crate::CombatId,
        mimicked: bool,
        max_hp: V::Number,
    },
    /// `TryModifyRestSiteHealRewards`, once `AfterRestSiteHeal` has run. A
    /// listener answers with the rewards it adds.
    RestSiteHealRewards {
        /// The player healed.
        player: crate::CombatId,
        mimicked: bool,
    },
    /// `AfterCombatVictoryEarly`, raised on every listener before any hears
    /// `AfterCombatVictory`.
    CombatVictoryEarly { room: V::RoomType },
    /// `AfterCombatVictory`, raised after `AfterCombatEnd` for a fight the
    /// run won, in a room of this type.
    CombatVictory { room: V::RoomType },
    /// `AfterEnergyResetLate`, straight after `AfterEnergyReset`.
    EnergyResetLate {
        player: crate::CombatId,
        /// `PlayerCombatState.TurnNumber`.
        turn: V::Count,
    },
    /// `BeforeHandDraw`, once the player's energy is reset and before the
    /// turn's hand is drawn.
    BeforeHandDraw {
        player: crate::CombatId,
        /// The player's `TurnNumber`, which the turn about to be drawn for
        /// already carries.
        turn: V::Count,
    },
    /// `AfterPlayerTurnStart`, once the player has drawn and before the side
    /// turn starts.
    PlayerTurnStart {
        player: crate::CombatId,
        /// The player's `TurnNumber`.
        turn: V::Count,
    },
    /// `AfterPlayerTurnStartLate`, once every `AfterPlayerTurnStart` has run.
    PlayerTurnStartLate {
        player: crate::CombatId,
        /// The player's `TurnNumber`.
        turn: V::Count,
    },
    /// `BeforeSideTurnEndVeryEarly`, the first of the three before a side's
    /// turn ends.
    BeforeSideTurnEndVeryEarly {
        side: V::CombatSide,
        /// The player's `Block`, which a relic reads off its owner.
        block: V::Number,
    },
    /// The end of a command the model itself asked for, awaited inside its
    /// own hook, which the code after the `await` runs on. Asked of that
    /// model alone, even once a question inside the command is answered.
    OwnCommandOver,
    /// `AfterActEntered`, once the run stands in the act's first room: Neow's
    /// for the first act, and the map for each after.
    ActEntered,
    /// `ModifierModel.OnRunCreated`, once a new run's relic bags are filled:
    /// `ClearsPlayerDeck`, then `AfterRunCreated`. Asked of each modifier
    /// alone.
    RunCreated,
    /// `AfterObtained`, as the run picks the model up. `character` is its
    /// owner's, which a relic reads off `Owner.Character`.
    Obtained {
        character: V::Character,
        /// `RunState.CurrentActIndex`.
        act: usize,
        /// `Owner.PlayerCombatState?.TurnNumber`: the player's turn in the
        /// fight it stands in, or in the last one it fought, which a fight
        /// leaves behind. Nothing before the first.
        turn: Option<V::Count>,
    },
    /// The character a relic is set up for, before it is obtained, as the
    /// ancient that offers Sea Glass sets its `CharacterId`.
    CharacterNamed(V::Character),
    /// `AfterOstyRevived`, once a dead Osty stands back up.
    OstyRevived,
    /// `AfterCreatureAddedToCombat`, once `creature` stands in the fight on
    /// `side`.
    CreatureAdded {
        creature: crate::CombatId,
        side: V::CombatSide,
        /// `RunState.CurrentMapPoint.coord`, where the fight stands on the
        /// map.
        point: Option<crate::Coord>,
    },
    /// `AfterCardChangedPiles` for a card that joined the deck, outside a
    /// fight. `cloned` is `clonedBy` being set, for a copy a listener made.
    AddedToDeck {
        card: V::Card,
        /// `card.Owner.Creature`, whose deck it joined.
        player: crate::CombatId,
        id: crate::DeckCardId,
        cloned: bool,
        /// `CardCmd.Transform` landed it, which hands the hook the deck as
        /// `oldPileType`. Any other card came from no pile.
        transformed: bool,
    },
    /// The option `GenerateNeowOption` gave Neow for the modifier, chosen.
    /// Asked of that modifier alone.
    NeowOptionChosen,
    /// The copy `id` that the listener's own `CloneCard` made, before
    /// `CardPileCmd.Add` puts it in the deck. Asked of that listener alone.
    CopyMade { id: crate::DeckCardId },
    /// `AfterCardChangedPiles`, once a card lands in the pile it moved to.
    /// The fight raises it where a transform lands the replacement.
    CardChangedPiles {
        card: V::Card,
        id: crate::CardId<V>,
        /// `oldPileType`, nothing for a card that came from no pile.
        from: Option<crate::Pile>,
    },
    /// `AfterCurrentHpChanged`, once `creature`'s hit points moved by
    /// `delta`.
    CurrentHpChanged {
        creature: crate::CombatId,
        /// `Creature.Monster`, which is nothing for a player.
        monster: Option<V::Monster>,
        /// `PetOwner`, which is nothing for anything but a pet.
        pet_of: Option<crate::CombatId>,
        delta: V::Number,
        /// `CombatManager.IsInProgress`.
        in_fight: bool,
    },
    /// `AfterOrbEvoked`, once an orb's evoke has run. The creatures it hit
    /// stay with the fight, which [`HookEffect::DamageTargets`] reaches.
    OrbEvoked {
        /// The orb's `Owner.Creature`.
        player: crate::CombatId,
        orb: V::Orb,
    },
    /// `AfterCardExhausted`, once the card lies in the exhaust pile.
    CardExhausted {
        card: V::Card,
        id: crate::CardId<V>,
        /// `card.Owner.Creature`.
        player: crate::CombatId,
        /// `causedByEthereal`: Ethereal exhausted it as its turn ended.
        ethereal: bool,
    },
    /// `AfterCardPlayed`, carrying the card its owner just played.
    CardPlayed {
        card: V::Card,
        /// `CardPlay.Card`: which copy.
        id: crate::CardId<V>,
        /// `CardPlay.Card.Owner.Creature`, which a listener compares with
        /// its own.
        player: crate::CombatId,
        /// `CardPlay.Card.Affliction`.
        affliction: Option<V::Affliction>,
        /// `Keywords.Contains(Ethereal)` for the copy.
        ethereal: bool,
        /// `CardPlay.Resources.EnergyValue`: the energy this play spent.
        spent: V::Count,
        /// `CardPlay.IsAutoPlay`.
        auto: bool,
        /// `CardPlay.IsLastInSeries`: the last of the card's replays.
        last: bool,
    },
    /// `BeforeDamageReceived`, once the damage hooks worked the hit out and
    /// before it meets block.
    DamageReceiving {
        target: crate::CombatId,
        amount: V::Number,
        /// The creature that deals it, which is nothing for damage from
        /// nobody.
        dealer: Option<crate::CombatId>,
        /// `IsPoweredAttack`.
        powered: bool,
        /// The card that deals it, if a card does.
        card: Option<V::Card>,
    },
    /// `AfterModifyingPowerAmountReceived`, asked of each listener whose
    /// `TryModifyPowerAmountReceived` changed what `power` landed with.
    PowerAmountReceivedModified { power: V::Power },
    /// `AfterModifyingPowerAmountGiven`, asked of each listener whose
    /// `ModifyPowerAmountGiven` changed what `power` was applied with.
    PowerAmountGivenModified { power: V::Power },
    /// `AfterDamageReceived`, carrying the creature that has just been hit.
    ///
    /// `unblocked` is the `DamageResult`'s own, which is what came off the hit
    /// points after the block and the hooks that cap them.
    DamageReceived {
        target: crate::CombatId,
        unblocked: V::Number,
        /// The `DamageResult`'s `BlockedDamage`: what the block took, on the
        /// creature the hit was aimed at, and nothing on a pet that took the
        /// rest.
        blocked: V::Number,
        /// The creature that dealt it, which is nothing for damage from
        /// nobody.
        dealer: Option<crate::CombatId>,
        /// `IsPoweredAttack`.
        powered: bool,
        /// `cardSource`, as the copy being played that deals it.
        card: Option<crate::CardId<V>>,
        /// `CombatState.CurrentSide`: whose turn the hit lands in.
        side: V::CombatSide,
        /// `DamageResult.WasFullyBlocked`, which only the creature the hit was
        /// aimed at can be.
        fully_blocked: bool,
        /// Whether its props carry `Unblockable`.
        unblockable: bool,
        /// The player's `TurnNumber` as the hit lands, which its history entry
        /// keeps.
        turn: V::Count,
        /// `CombatManager.IsInProgress`, which `AfterRoomEntered` comes
        /// before.
        in_progress: bool,
    },
    /// `AfterDamageGiven`, carrying the creature that dealt the hit.
    DamageGiven {
        /// The creature that dealt it, or nothing for damage no creature
        /// deals.
        dealer: Option<crate::CombatId>,
        /// `Dealer.PetOwner.Creature`, for a pet's hit.
        pet_of: Option<crate::CombatId>,
        /// The creature this result is for.
        target: crate::CombatId,
        /// `DamageResult.TotalDamage`: what its block took and what its hit
        /// points lost.
        total: V::Number,
        /// `DamageResult.UnblockedDamage`: what its hit points lost.
        unblocked: V::Number,
        /// `IsPoweredAttack`.
        powered: bool,
        fully_blocked: bool,
        /// `DamageResult.WasBlockBroken`, on the creature the hit was aimed
        /// at.
        block_broken: bool,
    },
    /// `MonsterMoveState.OnPerform`, after the move's behavior runs.
    MonsterMovePerformed { move_id: V::MoveId },
    /// `BeforeCardPlayed`, before each play of a card, replays included. The
    /// card stands in the play pile.
    CardPlaying {
        card: V::Card,
        id: crate::CardId<V>,
        /// The card's `Owner.Creature`, which a listener compares with its
        /// own.
        player: crate::CombatId,
        /// The copy's `Affliction`.
        affliction: Option<V::Affliction>,
        /// `CardPlay.Target`.
        target: Option<crate::CombatId>,
        /// `Keywords.Contains(Ethereal)` for the copy.
        ethereal: bool,
        /// `EnergyCost.GetResolved()`: the cost with every modifier, or the
        /// X the play captured.
        resolved: V::Number,
        /// `CardPlay.Resources.EnergyValue`: the energy this play spent.
        spent: V::Number,
        /// `CombatState.CurrentSide`: whose turn the play starts in.
        side: V::CombatSide,
        /// `IsDupe`: the copy is a dupe, made to be played once.
        dupe: bool,
        /// `CardPlay.IsFirstInSeries`: the first of the card's replays.
        first: bool,
    },
    /// `AfterCardDrawnEarly`, the pass `Hook.AfterCardDrawn` makes over every
    /// listener before the one [`EventHook::CardDrawn`] names.
    CardDrawnEarly {
        card: V::Card,
        id: crate::CardId<V>,
        /// `card.Owner.Creature`.
        player: crate::CombatId,
        /// `fromHandDraw`.
        hand_draw: bool,
    },
    /// `AfterCardDrawn`, carrying the card that has just reached the hand.
    ///
    /// Raised once per card, inside the draw loop, on every listener. A card
    /// answering for its own draw compares `id` with its own, which is what
    /// every card that answers this hook does.
    CardDrawn {
        card: V::Card,
        id: crate::CardId<V>,
        /// `card.Owner.Creature`.
        player: crate::CombatId,
        /// `Keywords.Contains(Ethereal)` for the copy drawn, which a word
        /// written on it counts toward.
        ethereal: bool,
        /// `fromHandDraw`.
        hand_draw: bool,
        /// `CombatState.CurrentSide`: whose turn the draw falls in.
        side: V::CombatSide,
    },
    /// `BeforeApplied`, asked of a power on its way onto a creature.
    ///
    /// The power is not held yet, so `landing` is what is about to land
    /// rather than what the creature carries. A [`HookEffect::ApplyToOwner`]
    /// from here lands on the creature this one is landing on, which is the
    /// owner it is about to have.
    BeforeApplied { landing: PowerAmount<V::Number> },
    /// `BeforeSideTurnStart`, which runs before the side draws anything.
    /// `turn` is the player's `TurnNumber`, and `round` the fight's
    /// `RoundNumber`, which an extra turn does not move on.
    BeforeSideTurnStart {
        side: V::CombatSide,
        turn: V::Count,
        round: V::Count,
    },
    /// `AfterSideTurnStart`, which runs once the side has drawn, in a room of
    /// this type.
    SideTurnStart {
        side: V::CombatSide,
        turn: V::Count,
        /// `RoundNumber`, which an extra turn does not move on.
        round: V::Count,
        room: V::RoomType,
        /// `CombatManager.PlayersTakingExtraTurn` is not empty: the player
        /// takes another turn, and the enemy's turn did not run between.
        extra: bool,
        /// `RunState.Players.Count`, which an enemy's Plating loses each of
        /// its turns.
        players: u8,
    },
    BeforeSideTurnEndEarly {
        side: V::CombatSide,
        /// The player's plays finished this turn that no auto-play made,
        /// which a listener reads off `CardPlaysFinished`, at most 255.
        plays: u8,
        /// The player's `TurnNumber`.
        turn: V::Count,
    },
    /// `AfterTakingExtraTurn`, as the player's side keeps the turn.
    TakingExtraTurn { player: crate::CombatId },
    /// `BeforeSideTurnEnd`, the last of the three before a side's turn ends.
    BeforeSideTurnEnd {
        side: V::CombatSide,
        /// The player's `PlayerCombatState.Energy`, which a listener reads.
        energy: V::Count,
        /// How many cards the player's hand holds, which a card in it reads
        /// off its own pile.
        hand: usize,
    },
    SideTurnEnd {
        side: V::CombatSide,
        /// The player's `Creature.Block`, which a listener reads.
        block: V::Number,
    },
    /// `AfterSideTurnEndLate`, once every `AfterSideTurnEnd` has run.
    SideTurnEndLate { side: V::CombatSide },
    /// `AfterStarsSpent`: `SpendStars` took `amount` from `player`, which is
    /// more than none.
    StarsSpent {
        player: crate::CombatId,
        amount: V::Number,
    },
    /// `AfterHandEmptied`: `CheckForEmptyHand` found the player's hand
    /// empty once a play or a drink was over. `phase` is the player's
    /// `PlayerTurnPhase` then.
    HandEmptied {
        player: crate::CombatId,
        phase: V::PlayerTurnPhase,
    },
    /// `AfterAutoPrePlayPhaseEnteredEarly`, once the player's turn is set up,
    /// in the player's `TurnNumber`.
    AutoPrePlayPhaseEarly {
        /// The player whose turn it is.
        player: crate::CombatId,
        turn: V::Count,
    },
    /// `AfterAutoPrePlayPhaseEntered`, once every listener heard the early
    /// pass.
    AutoPrePlayPhase {
        player: crate::CombatId,
        turn: V::Count,
    },
    /// `AfterAutoPrePlayPhaseEnteredLate`, the last pass before the play
    /// phase.
    AutoPrePlayPhaseLate {
        player: crate::CombatId,
        turn: V::Count,
    },
    /// `AfterEnergySpent`, once a card's `SpendEnergy` took `amount` off the
    /// player's pool, which can be none.
    EnergySpent {
        /// The spending card's `Owner.Creature`.
        player: crate::CombatId,
        amount: V::Number,
    },
    /// `AfterForge`, once `ForgeCmd.Forge` raised the Sovereign Blades'
    /// damage by `amount`.
    Forged {
        amount: V::Number,
        /// `forger.Creature`.
        player: crate::CombatId,
        /// `source`, where a power's hook forged. Nothing for a card's or a
        /// potion's.
        source: Option<V::Power>,
    },
    /// `BeforeFlushLate`, the second pass of `Hook.BeforeFlush`, once
    /// `DoTurnEnd` is over and before `player`'s hand is flushed.
    FlushingLate { player: crate::CombatId },
    /// `AfterAutoPostPlayPhaseEntered`, as `EndPlayerTurnPhaseOne` opens and
    /// before `BeforeTurnEnd`, in the player's `TurnNumber`.
    AutoPostPlayPhase {
        /// The player whose turn is ending.
        player: crate::CombatId,
        turn: V::Count,
    },
    /// `AfterEnergyReset`, carrying the player's creature.
    EnergyReset {
        player: crate::CombatId,
        /// The player's `TurnNumber`.
        turn: V::Count,
    },
    /// `AfterBlockCleared`, carrying the creature whose block has just gone.
    ///
    /// Raised once for each creature starting its turn, once every one of
    /// them has cleared.
    BlockCleared {
        creature: crate::CombatId,
        /// The player's `TurnNumber`.
        turn: V::Count,
    },
    /// `AfterModifyingGoldGained`, asked of the models that changed a gain.
    GoldGainModified,
    /// `AfterGoldGained`, once gold `player` gained has landed.
    GoldGained { player: crate::CombatId },
    /// `AfterPreventingBlockClear`, asked of the one model whose
    /// `ShouldClearBlock` kept its owner's block, with the block kept.
    BlockClearPrevented { block: V::Number },
    /// `AfterFlush`, once `player`'s hand is flushed. The cards left in hand
    /// are the ones it retained.
    Flushed { player: crate::CombatId },
    /// `AfterOrbChanneled`, once an orb is in `player`'s queue.
    OrbChanneled { player: crate::CombatId },
    /// `AfterCardDiscarded`, once a card of `player`'s is in the discard
    /// pile. `side` is the side whose turn it is.
    CardDiscarded {
        player: crate::CombatId,
        side: V::CombatSide,
    },
    /// `BeforePowerAmountChanged`, as `amount` of `power` is about to land on
    /// `target`, or to change what it carries.
    PowerAmountChanging {
        power: V::Power,
        amount: V::Number,
        target: crate::CombatId,
        /// The side `target` fights on.
        target_side: V::CombatSide,
        /// Whether `target` holds Artifact.
        target_artifact: bool,
        applier: Option<crate::CombatId>,
        /// `cardSource`: which copy gives the power.
        source: Option<crate::CardId<V>>,
    },
    /// `AfterModifyingBlockAmount`, asked of each relic whose block hook
    /// changed a gain, with what the gain came to.
    BlockAmountModified {
        amount: V::Number,
        /// `cardSource`: which copy gives the block.
        source: Option<crate::CardId<V>>,
    },
    /// `AfterDiedToDoom`, once Doom killed its creatures. `fatal` counts those
    /// besides the player whose powers all let the death trigger Fatal.
    DiedToDoom { fatal: usize },
    /// `TryModifyRestSiteOptions`, as a rest site opens. A listener answers
    /// with each option it adds.
    RestSiteOptions,
    /// `TryModifyCardRewardOptions`, once a card reward has drawn its cards.
    /// `encounter` is `CardCreationSource.Encounter`, which a reward at a
    /// room's odds has. `from_combat` is v0.111.0's
    /// `CardCreationFlags.IsFromCombat`, which only a won fight's own rewards
    /// carry.
    CardRewardOptions { encounter: bool, from_combat: bool },
    /// The rest site option a relic offers, taken: the relic's own method,
    /// which the option's `OnSelect` calls.
    RestOptionTaken,
    /// `AfterPreventingDeath`, asked of the one model whose `ShouldDie` or
    /// `ShouldDieLate` kept its owner in the fight, with the owner's maximum
    /// hit points.
    DeathPrevented { max_hp: V::Number },
    /// `PowerModel.AfterRemoved`, asked of a power as it comes off the
    /// creature that carried it.
    Removed,
    /// `PowerModel.AfterApplied`, asked of a new instance once it is on, and
    /// before `AfterPowerAmountChanged`. A stack landing on one already
    /// there is not asked.
    Applied,
    /// `Hook.BeforeDeath`, carrying the creature about to die.
    ///
    /// Raised once its hit points are gone and before anything decides
    /// whether it dies.
    BeforeDeath { creature: crate::CombatId },
    /// `Hook.AfterBlockBroken`, carrying the creature whose block a hit has
    /// just taken the last of.
    BlockBroken {
        creature: crate::CombatId,
        /// The creature that broke it: the hit's dealer, or `LoseBlock`'s
        /// remover. v0.111.0's hook passes it. v0.107.1's passes the
        /// creature alone.
        breaker: Option<crate::CombatId>,
        /// The creature `breaker` is a pet of, where it is one.
        pet_of: Option<crate::CombatId>,
    },
    /// `Hook.AfterDeath`, carrying the dead creature and the side it fought
    /// on.
    ///
    /// Every model in the fight is asked, not only the dead creature's own. A
    /// power answering for its own holder's death compares `creature` with
    /// that holder. Gremlin Horn reads `side`, and pays for a kill and not for
    /// its own holder's death.
    AfterDeath {
        creature: crate::CombatId,
        side: V::CombatSide,
        /// `Creature.Monster`, which is nothing for the player.
        monster: Option<V::Monster>,
        /// `wasRemovalPrevented`: a `ShouldDie` listener kept the creature
        /// alive, as Die For You keeps Osty.
        removal_prevented: bool,
    },
    /// `AfterPowerAmountChanged`, raised on every listener once a power's
    /// amount has moved.
    ///
    /// `delta` is the amount that was applied, which is what the hook carries
    /// and not the total the creature now holds. A power reading this one
    /// compares the power named, and the creature carrying it, against
    /// itself.
    PowerAmountChanged {
        creature: crate::CombatId,
        power: V::Power,
        delta: V::Number,
        /// The creature whose command moved it, which a tick names nobody.
        applier: Option<crate::CombatId>,
    },
    /// `AfterCardEnteredCombat`, carrying the card that has just joined the
    /// fight.
    ///
    /// A card generated into a pile, or forged into one. Not a card moving
    /// between piles, which is `AfterCardChangedPiles`.
    ///
    /// Both where it landed and what it is: a listener decides on the identity
    /// and then writes on that one copy.
    CardEnteredCombat {
        at: crate::CardAt<V>,
        card: V::Card,
        /// `card.Owner`'s creature, which a listener compares with its own.
        player: crate::CombatId,
        /// `IsClone`: the card is a clone of another, which is `CloneOf`.
        clone: bool,
        /// The card has an `Affliction`, which a clone keeps from its
        /// original.
        afflicted: bool,
    },
    /// `AfterCardGeneratedForCombat`, once a card made in the fight has
    /// landed where `at` says.
    CardGeneratedForCombat {
        at: crate::CardAt<V>,
        card: V::Card,
        /// The player that made it. Nothing for a card a monster gave.
        creator: Option<crate::CombatId>,
    },
    /// `Hook.BeforeAttack`, as an attack command starts and before its first
    /// hit.
    BeforeAttack {
        command: AttackId,
        /// `Attacker`.
        attacker: crate::CombatId,
        /// `ModelSource`, which is the card for a card's attack and nothing
        /// for a monster's: which copy, which card it is, and its
        /// `Owner.Creature`.
        source: Option<(crate::CardId<V>, V::Card, crate::CombatId)>,
        /// `DamageProps.IsPoweredAttack()`.
        powered: bool,
    },
    /// `AfterModifyingCardPlayCount`, asked of each listener that changed a
    /// card's play count, once the count is worked out.
    PlayCountModified { card: V::Card },
    /// `AfterModifyingHpLostAfterOsty`, asked of each listener that changed
    /// what a hit takes off, once that is worked out.
    HpLostModified,
    /// `AfterModifyingCardPlayResultPileOrPosition`, asked of each listener
    /// that changed where a played card goes, once that is worked out.
    ResultPileModified { card: V::Card },
    /// `Hook.AfterAttack`, once the command's last hit has landed.
    AfterAttack {
        command: AttackId,
        /// `Attacker`.
        attacker: crate::CombatId,
        /// `Attacker.PetOwner.Creature`, for a pet's attack.
        pet_of: Option<crate::CombatId>,
        /// `ModelSource` where it is a card: which copy, and which card.
        source: Option<(crate::CardId<V>, V::Card)>,
        /// `Attacker.Monster`, which is nothing for the player.
        monster: Option<V::Monster>,
        /// `DamageProps.IsPoweredAttack()`.
        powered: bool,
    },
    /// `BeforeRoomEntered`, once the room is pushed and before it is entered.
    RoomEntering {
        /// `RunState.CurrentMapPoint.PointType`, whatever room it rolled. None
        /// off the map.
        point: Option<V::MapPointType>,
        /// `CurrentRoomCount == 1`: not a room an event opened inside its own.
        base: bool,
    },
    /// `BeforeCombatRewardOffered`, once a won fight's rewards are made and
    /// before they are offered. A hook v0.111.0 has and v0.107.1 lacks.
    CombatRewardOffering {
        /// `rewards.Player`: whose rewards they are.
        player: crate::CombatId,
        /// Whether a `CardReward` is among them.
        card_reward: bool,
    },
    /// `AfterMapGenerated`, once an act's map is made.
    MapGenerated {
        /// `RunState.CurrentActIndex`.
        act: usize,
    },
}

impl<V: ContentVersion> EventHook<V> {
    /// The `AbstractModel` hook this is raised as.
    ///
    /// Nothing for a notification the dump has no row for. `AfterObtained` is
    /// a relic's own virtual. `BeforeApplied`, `AfterApplied` and
    /// `AfterRemoved` are a power's, which [`EventHook::power_virtual`]
    /// names. A move being performed is the move state's. A model's overrides
    /// cannot say whether it answers one of those, so it is always asked.
    #[inline(always)]
    pub fn model_hook(&self) -> Option<ModelHook> {
        Some(match self {
            EventHook::ShuffleOrder { .. } => ModelHook::ModifyShuffleOrder,
            EventHook::Shuffled { .. } => ModelHook::AfterShuffle,
            EventHook::DrawPrevented => ModelHook::AfterPreventingDraw,
            EventHook::HandDrawModified => ModelHook::AfterModifyingHandDraw,
            EventHook::EnergyGainModified => ModelHook::AfterModifyingEnergyGain,
            EventHook::StarsGained { .. } => ModelHook::AfterStarsGained,
            EventHook::BlockGained { .. } => ModelHook::AfterBlockGained,
            EventHook::FlushingLate { .. } => ModelHook::BeforeFlushLate,
            EventHook::EnergySpent { .. } => ModelHook::AfterEnergySpent,
            EventHook::Forged { .. } => ModelHook::AfterForge,
            EventHook::PotionUsing { .. } => ModelHook::BeforePotionUsed,
            EventHook::PotionUsed { .. } => ModelHook::AfterPotionUsed,
            EventHook::PotionProcured { .. } => ModelHook::AfterPotionProcured,
            EventHook::PotionDiscarded { .. } => ModelHook::AfterPotionDiscarded,
            EventHook::CardRewardOption { .. } => ModelHook::TryModifyCardRewardOptionsLate,
            EventHook::CardRewardPick { .. } => ModelHook::TryModifyCardRewardOptionsLate,
            EventHook::CardRewardOptionsModified { .. } => {
                ModelHook::AfterModifyingCardRewardOptions
            }
            EventHook::CardBeingAddedToDeck { .. } => ModelHook::TryModifyCardBeingAddedToDeck,
            EventHook::MerchantCardCreated { .. } => ModelHook::ModifyMerchantCardCreationResults,
            EventHook::CardPlayedLate { .. } => ModelHook::AfterCardPlayedLate,
            EventHook::SideTurnStartLate { .. } => ModelHook::AfterSideTurnStartLate,
            EventHook::RoomEntered { .. } => ModelHook::AfterRoomEntered,
            EventHook::ItemPurchased { .. } => ModelHook::AfterItemPurchased,
            EventHook::RoomEntering { .. } => ModelHook::BeforeRoomEntered,
            EventHook::UnknownRoomTypes { .. } => ModelHook::ModifyUnknownMapPointRoomTypes,
            EventHook::GeneratedMap { .. } => ModelHook::ModifyGeneratedMap,
            EventHook::GeneratedMapLate { .. } => ModelHook::ModifyGeneratedMapLate,
            EventHook::MapGenerated { .. } => ModelHook::AfterMapGenerated,
            EventHook::ActEntered => ModelHook::AfterActEntered,
            EventHook::CardRewardCreationOptions { .. } => {
                ModelHook::ModifyCardRewardCreationOptions
            }
            EventHook::MerchantCardPool => ModelHook::ModifyMerchantCardPool,
            EventHook::ModifyRewards { .. } => ModelHook::TryModifyRewards,
            EventHook::ModifyRewardsLate { .. } => ModelHook::TryModifyRewardsLate,
            EventHook::CardRewardAlternatives { .. }
            | EventHook::CardRewardAlternativeChosen { .. } => {
                ModelHook::TryModifyCardRewardAlternatives
            }
            EventHook::CombatStarting { .. } => ModelHook::BeforeCombatStart,
            EventHook::CombatStartingLate { .. } => ModelHook::BeforeCombatStartLate,
            EventHook::RestSiteOptions => ModelHook::TryModifyRestSiteOptions,
            EventHook::Flushed { .. } => ModelHook::AfterFlush,
            EventHook::OrbChanneled { .. } => ModelHook::AfterOrbChanneled,
            EventHook::CardDiscarded { .. } => ModelHook::AfterCardDiscarded,
            EventHook::PowerAmountChanging { .. } => ModelHook::BeforePowerAmountChanged,
            EventHook::BlockAmountModified { .. } => ModelHook::AfterModifyingBlockAmount,
            EventHook::DiedToDoom { .. } => ModelHook::AfterDiedToDoom,
            EventHook::CardRewardOptions { .. } => ModelHook::TryModifyCardRewardOptions,
            EventHook::CombatVictoryEarly { .. } => ModelHook::AfterCombatVictoryEarly,
            EventHook::CombatEnd { .. } => ModelHook::AfterCombatEnd,
            EventHook::RestSiteHeal { .. } => ModelHook::AfterRestSiteHeal,
            EventHook::RestSiteHealRewards { .. } => ModelHook::TryModifyRestSiteHealRewards,
            EventHook::CombatVictory { .. } => ModelHook::AfterCombatVictory,
            EventHook::PlayerTurnStart { .. } => ModelHook::AfterPlayerTurnStart,
            EventHook::PlayerTurnStartLate { .. } => ModelHook::AfterPlayerTurnStartLate,
            EventHook::BeforeSideTurnEndVeryEarly { .. } => ModelHook::BeforeSideTurnEndVeryEarly,
            EventHook::BeforeHandDraw { .. } => ModelHook::BeforeHandDraw,
            EventHook::EnergyResetLate { .. } => ModelHook::AfterEnergyResetLate,
            EventHook::CardPlayed { .. } => ModelHook::AfterCardPlayed,
            EventHook::CardExhausted { .. } => ModelHook::AfterCardExhausted,
            EventHook::OrbEvoked { .. } => ModelHook::AfterOrbEvoked,
            EventHook::CurrentHpChanged { .. } => ModelHook::AfterCurrentHpChanged,
            EventHook::CardChangedPiles { .. } | EventHook::AddedToDeck { .. } => {
                ModelHook::AfterCardChangedPiles
            }
            EventHook::DamageReceiving { .. } => ModelHook::BeforeDamageReceived,
            EventHook::PowerAmountGivenModified { .. } => ModelHook::AfterModifyingPowerAmountGiven,
            EventHook::PowerAmountReceivedModified { .. } => {
                ModelHook::AfterModifyingPowerAmountReceived
            }
            EventHook::DamageReceived { .. } => ModelHook::AfterDamageReceived,
            EventHook::DamageGiven { .. } => ModelHook::AfterDamageGiven,
            EventHook::CardDrawn { .. } => ModelHook::AfterCardDrawn,
            EventHook::CardDrawnEarly { .. } => ModelHook::AfterCardDrawnEarly,
            EventHook::CardPlaying { .. } => ModelHook::BeforeCardPlayed,
            EventHook::BeforeSideTurnStart { .. } => ModelHook::BeforeSideTurnStart,
            EventHook::SideTurnStart { .. } => ModelHook::AfterSideTurnStart,
            EventHook::TakingExtraTurn { .. } => ModelHook::AfterTakingExtraTurn,
            EventHook::BeforeSideTurnEndEarly { .. } => ModelHook::BeforeSideTurnEndEarly,
            EventHook::BeforeSideTurnEnd { .. } => ModelHook::BeforeSideTurnEnd,
            EventHook::SideTurnEnd { .. } => ModelHook::AfterSideTurnEnd,
            EventHook::SideTurnEndLate { .. } => ModelHook::AfterSideTurnEndLate,
            EventHook::EnergyReset { .. } => ModelHook::AfterEnergyReset,
            EventHook::AutoPrePlayPhaseEarly { .. } => ModelHook::AfterAutoPrePlayPhaseEnteredEarly,
            EventHook::AutoPrePlayPhase { .. } => ModelHook::AfterAutoPrePlayPhaseEntered,
            EventHook::AutoPrePlayPhaseLate { .. } => ModelHook::AfterAutoPrePlayPhaseEnteredLate,
            EventHook::AutoPostPlayPhase { .. } => ModelHook::AfterAutoPostPlayPhaseEntered,
            EventHook::StarsSpent { .. } => ModelHook::AfterStarsSpent,
            EventHook::HandEmptied { .. } => ModelHook::AfterHandEmptied,
            EventHook::BlockCleared { .. } => ModelHook::AfterBlockCleared,
            EventHook::GoldGainModified => ModelHook::AfterModifyingGoldGained,
            EventHook::GoldGained { .. } => ModelHook::AfterGoldGained,
            EventHook::AfterDeath { .. } => ModelHook::AfterDeath,
            EventHook::BeforeDeath { .. } => ModelHook::BeforeDeath,
            EventHook::BlockBroken { .. } => ModelHook::AfterBlockBroken,
            EventHook::CombatRewardOffering { .. } => ModelHook::BeforeCombatRewardOffered,
            EventHook::PowerAmountChanged { .. } => ModelHook::AfterPowerAmountChanged,
            EventHook::CardEnteredCombat { .. } => ModelHook::AfterCardEnteredCombat,
            EventHook::CardGeneratedForCombat { .. } => ModelHook::AfterCardGeneratedForCombat,
            EventHook::BeforeAttack { .. } => ModelHook::BeforeAttack,
            EventHook::AfterAttack { .. } => ModelHook::AfterAttack,
            EventHook::PlayCountModified { .. } => ModelHook::AfterModifyingCardPlayCount,
            EventHook::HpLostModified => ModelHook::AfterModifyingHpLostAfterOsty,
            EventHook::OstyRevived => ModelHook::AfterOstyRevived,
            EventHook::CreatureAdded { .. } => ModelHook::AfterCreatureAddedToCombat,
            EventHook::ResultPileModified { .. } => {
                ModelHook::AfterModifyingCardPlayResultPileOrPosition
            }
            EventHook::Obtained { .. }
            | EventHook::OwnCommandOver
            | EventHook::CopyMade { .. }
            | EventHook::NeowOptionChosen
            | EventHook::RunCreated
            | EventHook::CharacterNamed(_)
            | EventHook::Removed
            | EventHook::Applied
            | EventHook::BlockClearPrevented { .. }
            | EventHook::DeathPrevented { .. }
            | EventHook::RestOptionTaken
            | EventHook::BeforeApplied { .. }
            | EventHook::MonsterMovePerformed { .. } => return None,
        })
    }

    /// The `PowerModel` virtual this is raised as, for the notifications a
    /// power alone is asked about itself.
    pub fn power_virtual(&self) -> Option<PowerVirtual> {
        match self {
            EventHook::BeforeApplied { .. } => Some(PowerVirtual::BeforeApplied),
            EventHook::Applied => Some(PowerVirtual::AfterApplied),
            EventHook::Removed => Some(PowerVirtual::AfterRemoved),
            _ => None,
        }
    }
}

/// A virtual `PowerModel` declares for itself, which the fight asks of a power
/// about itself. Every power is asked these, so a power whose class overrides
/// one answers for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerVirtual {
    BeforeApplied,
    AfterApplied,
    AfterRemoved,
}

/// [`Fight::power_instances`]: each instance's `Target` and stack.
pub type PowerInstances<V> = smallvec::SmallVec<
    [(
        Option<crate::CombatId>,
        PowerAmount<<V as ContentVersion>::Number>,
    ); 4],
>;

/// Whom a monster aims a starting power at, one instance for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    /// `CombatState.GetOpponentsOf`, pets included.
    EachOpponent,
    /// `CombatState.Players`.
    EachPlayer,
}

/// One power instance a creature holds, as a card that copies it reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldInstance<V: ContentVersion> {
    pub power: V::Power,
    pub amount: PowerAmount<V::Number>,
    /// `PowerModel.Applier`.
    pub applier: Option<crate::CombatId>,
    /// Whether its model holds nothing a new one does not. A new model is
    /// then the copy `ClonePreservingMutability` makes.
    pub fresh: bool,
}

/// What a card's own `IsPlayable` reads of the fight it is in.
///
/// Asked, not handed over, because the few cards that override `IsPlayable`
/// are the only readers. A hand of other cards reads none of it.
pub trait CardLogic {
    /// How many cards its owner's draw pile holds.
    fn draw_pile(&self) -> usize;
    /// Whether its owner's Osty stands, which `IsOstyMissing` negates.
    fn osty(&self) -> bool;
    /// Whether every card in its owner's hand is an Attack.
    fn hand_all_attacks(&self) -> bool;
}

/// Which cards a hook writes on, for [`HookEffect::AddReplays`] and
/// [`HookEffect::AddKeyword`].
///
/// `PowerModel` reaches cards two ways, and both are here. `AllCards` is the
/// combat state's own list. A listener that walks it names what it wants
/// rather than the copies, and the fight does the walking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writing<V: ContentVersion> {
    /// One copy, named where it sits.
    One(crate::CardAt<V>),
    /// Every copy of one card the fight holds, in every pile.
    EveryCopyOf(V::Card),
    /// Every card the fight holds that carries this tag, in every pile.
    EveryTagged(V::CardTag),
    /// Every card the fight holds of this type, in every pile.
    EveryOfKind(CardKind),
    /// Every card the fight holds, in every pile.
    Every,
    /// Every card the fight holds that carries this affliction, in every
    /// pile.
    EveryAfflictedWith(V::Affliction),
    /// One copy, named by its id wherever it stands.
    Named(crate::CardId<V>),
}

/// One `CardPlayStartedEntry` in `CombatManager.History`: a card's play that
/// began, and the energy it cost. A card with replays starts once for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayStarted<V: ContentVersion> {
    pub card: V::Card,
    /// `CardPlay.Player`: whose play it is, by their creature.
    pub player: crate::CombatId,
    /// Which copy it is, which `CardPlay.Card` is.
    pub id: crate::CardId<V>,
    /// `CardPlay.Resources.EnergyValue`: what the card costs, which an
    /// auto-play counts though it spends none of it.
    pub energy: V::Count,
    /// `CardPlay.IsAutoPlay`, whose `Resources.EnergySpent` is nothing.
    pub auto: bool,
    /// `EnergyCost.CapturedXValue`, for a card that costs X energy: what
    /// its play spent, or the energy in hand where it was auto-played.
    pub x: V::Count,
    /// `LastStarsSpent`: what its play spent in stars, or the stars in hand
    /// for an X star cost it was auto-played at.
    pub stars: V::Count,
    /// `CardPlay.IsFirstInSeries`: not a replay of the play before it.
    pub first: bool,
}

/// One `CardPlayFinishedEntry` in `CombatManager.History`: a card's play
/// that ran to its end. A card with replays finishes once for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayFinished<V: ContentVersion> {
    pub card: V::Card,
    /// `CardPlay.Player`: whose play it was, by their creature.
    pub player: crate::CombatId,
    /// Which copy it was, which `CardPlay.Card` is.
    pub id: crate::CardId<V>,
    /// `WasEthereal`: the copy was Ethereal as it was played.
    pub ethereal: bool,
    /// `CardPlay.IsAutoPlay`.
    pub auto: bool,
    /// `IsDupe`: the copy was a dupe, made to be played once.
    pub dupe: bool,
}

/// One entry in `CombatManager.History`, of the kinds a listener reads.
///
/// One list of every kind, as the game keeps it, so copying a turn copies
/// one list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryEntry<V: ContentVersion> {
    Started(PlayStarted<V>),
    Finished(PlayFinished<V>),
    /// An `EnergySpentEntry`, by its `Amount`, which `CardModel.SpendEnergy`
    /// records for a player's card whatever plays it. `player` is its
    /// `Actor`, the card's owner.
    EnergySpent {
        amount: V::Count,
        player: crate::CombatId,
    },
    /// A `CardDrawnEntry`, by the card drawn and `FromHandDraw`. `player` is
    /// its `Actor`, the card's owner.
    Drawn {
        card: V::Card,
        hand_draw: bool,
        player: crate::CombatId,
    },
    /// A `CreatureAttackedEntry`, by the attacker's `Monster`, nothing for a
    /// player, and the player whose pet it is, as Osty's `PetOwner`.
    Attacked {
        monster: Option<V::Monster>,
        pet_of: Option<crate::CombatId>,
    },
    BlockGained(BlockGained),
    /// A `StarsModifiedEntry`, by its `Amount`: the change to `player`'s
    /// stars.
    StarsModified {
        amount: V::Number,
        player: crate::CombatId,
    },
    /// A `CardAfflictedEntry`, by the affliction. Its `Actor` is the card's
    /// owner, `player`.
    Afflicted {
        affliction: V::Affliction,
        player: crate::CombatId,
    },
    /// A `CardDiscardedEntry`, by the card discarded, of `player`'s.
    Discarded {
        card: V::Card,
        player: crate::CombatId,
    },
    /// A `CardExhaustedEntry`, by the card exhausted, of `player`'s.
    Exhausted {
        card: V::Card,
        player: crate::CombatId,
    },
    /// A `CardGeneratedEntry`, by its `Creator`'s creature, nothing for a
    /// card no player made.
    Generated {
        creator: Option<crate::CombatId>,
    },
    /// A `DamageReceivedEntry`.
    DamageReceived(DamageReceived<V>),
    /// An `OrbChanneledEntry`, by the orb, of `player`'s.
    Channelled {
        orb: V::Orb,
        player: crate::CombatId,
    },
    /// A `PowerReceivedEntry`, by the power and who applied it.
    PowerReceived {
        power: V::Power,
        applier: Option<crate::CombatId>,
    },
}

/// One `DamageReceivedEntry` in `CombatManager.History`, as far as a
/// listener reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageReceived<V: ContentVersion> {
    pub receiver: crate::CombatId,
    pub dealer: Option<crate::CombatId>,
    /// `Result.UnblockedDamage`.
    pub unblocked: V::Number,
    /// `Result.Props.IsPoweredAttack()`.
    pub powered: bool,
}

/// One `BlockGainedEntry` in `CombatManager.History`, as far as a listener
/// reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockGained {
    /// `Receiver`.
    pub receiver: crate::CombatId,
    /// `Props.IsCardOrMonsterMove()`, the `Move` flag alone.
    pub moved: bool,
    /// `CardPlay`, as the play's [`PlayAt`].
    pub play: Option<PlayAt>,
}

/// One card play, by where its `CardPlayStartedEntry` stands in the turn's
/// entries. The game compares `CardPlay` objects, and each play and replay
/// starts once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayAt(pub u32);

/// One turn now over that had entries, in a fight's [`PastTurns`].
#[derive(Debug)]
pub struct TurnHistory<V: ContentVersion> {
    /// `RoundNumber` of that turn.
    pub round: V::Count,
    /// `CurrentSide` of that turn.
    pub side: V::CombatSide,
    /// The player's `TurnNumber` through that turn. It counts up as the
    /// player's side takes a turn, so the enemies' turn after a player's
    /// shares its number.
    pub player_turn: V::Count,
    /// Where its entries end among the fight's. They start where the turn
    /// before it ends them.
    end: u32,
}

impl<V: ContentVersion> Clone for TurnHistory<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: ContentVersion> Copy for TurnHistory<V> {}

/// The turns of a fight now over that had entries, and their entries, each
/// player's among them.
///
/// A turn is written onto it as the sides switch. Copies of a fight share
/// it, and a copy copies it only to end a turn. A fight that runs on alone
/// writes onto it in place.
#[derive(Debug)]
pub struct PastTurns<V: ContentVersion> {
    turns: Vec<TurnHistory<V>>,
    entries: Vec<HistoryEntry<V>>,
}

/// How many more turns, and entries, a fight's past makes room for at once.
/// A fight averages about a hundred entries over a dozen turns, and growing
/// a list a step at a time copies it each time.
const TURNS_AHEAD: usize = 32;
const ENTRIES_AHEAD: usize = 256;

impl<V: ContentVersion> Clone for PastTurns<V> {
    /// With room to write on, since a copy is made to end a turn on.
    fn clone(&self) -> Self {
        let mut turns = Vec::with_capacity(self.turns.len() + TURNS_AHEAD);
        turns.extend_from_slice(&self.turns);
        let mut entries = Vec::with_capacity(self.entries.len() + ENTRIES_AHEAD);
        entries.extend_from_slice(&self.entries);
        Self { turns, entries }
    }

    fn clone_from(&mut self, source: &Self) {
        self.turns.clone_from(&source.turns);
        self.entries.clone_from(&source.entries);
    }
}

impl<V: ContentVersion> Default for PastTurns<V> {
    fn default() -> Self {
        Self {
            turns: Vec::with_capacity(TURNS_AHEAD),
            entries: Vec::with_capacity(ENTRIES_AHEAD),
        }
    }
}

impl<V: ContentVersion> PastTurns<V> {
    /// The turn at `round` on `side`, in the player's turn `player_turn`, is
    /// over, with `entries`, oldest first.
    ///
    /// # Panics
    ///
    /// A fight with more entries than a `u32` counts.
    pub fn end_turn(
        &mut self,
        round: V::Count,
        side: V::CombatSide,
        player_turn: V::Count,
        entries: impl IntoIterator<Item = HistoryEntry<V>>,
    ) {
        self.entries.extend(entries);
        self.turns.push(TurnHistory {
            round,
            side,
            player_turn,
            end: u32::try_from(self.entries.len()).expect("a fight's entries fit a u32"),
        });
    }

    /// Each turn, latest first, with its entries, oldest first.
    pub fn latest_first(&self) -> impl Iterator<Item = (&TurnHistory<V>, &[HistoryEntry<V>])> {
        (0..self.turns.len()).rev().map(move |at| {
            let turn = &self.turns[at];
            let start = match at {
                0 => 0,
                _ => self.turns[at - 1].end,
            };
            (turn, &self.entries[start as usize..turn.end as usize])
        })
    }

    /// Every entry, the latest turn's first, each turn's oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &HistoryEntry<V>> {
        self.latest_first().flat_map(|(_, entries)| entries.iter())
    }
}

/// `CombatManager.History`, as far as a listener reads it.
#[derive(Debug, Clone, Copy)]
pub struct History<'a, V: ContentVersion> {
    /// The player whose card plays the reader counts as its owner's, by their
    /// creature. Each of the game's readers of `CardPlaysStarted` and
    /// `CardPlaysFinished` keeps its owner's alone, but Gold Axe.
    pub player: crate::CombatId,
    /// The turn the fight stands in, oldest first.
    pub this_turn: &'a [HistoryEntry<V>],
    /// The turns before it that had entries.
    pub before: Option<&'a PastTurns<V>>,
}

impl<V: ContentVersion> History<'_, V> {
    /// `CardPlaysFinished` where `HappenedThisTurn`, of the owner's plays.
    pub fn finished_this_turn(&self) -> impl Iterator<Item = V::Card> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::Finished(finished) if finished.player == self.player => {
                Some(finished.card)
            }
            _ => None,
        })
    }

    /// `CardPlaysFinished` where `HappenedLastPlayerTurn`, for a player now
    /// in its turn `turn`: the player's turn before, and the enemies' turn
    /// after it. The turn the fight stands in is the player's `turn` or later,
    /// so none of its entries is one.
    pub fn finished_last_player_turn(
        &self,
        turn: V::Count,
    ) -> impl Iterator<Item = PlayFinished<V>> + '_ {
        let last = turn - num_traits::one::<V::Count>();
        self.before
            .into_iter()
            .flat_map(PastTurns::latest_first)
            .take_while(move |(before, _)| before.player_turn >= last)
            .filter(move |(before, _)| before.player_turn == last)
            .flat_map(|(_, entries)| entries.iter())
            .filter_map(|entry| match entry {
                HistoryEntry::Finished(finished) if finished.player == self.player => {
                    Some(*finished)
                }
                _ => None,
            })
    }

    /// Every `CardPlaysFinished` entry of the fight, the turn it stands in
    /// first and then the turns before it, latest first, of the owner's
    /// plays.
    pub fn finished_this_fight(&self) -> impl Iterator<Item = PlayFinished<V>> + '_ {
        let player = self.player;
        self.finished_this_fight_by_all()
            .filter(move |finished| finished.player == player)
    }

    /// [`History::finished_this_fight`] of every player's plays, which Gold
    /// Axe counts.
    pub fn finished_this_fight_by_all(&self) -> impl Iterator<Item = PlayFinished<V>> + '_ {
        let before = self.before.into_iter().flat_map(PastTurns::entries);
        self.this_turn
            .iter()
            .chain(before)
            .filter_map(|entry| match entry {
                HistoryEntry::Finished(finished) => Some(*finished),
                _ => None,
            })
    }

    /// Each `CreatureAttackedEntry` where `HappenedThisTurn` whose attacker is
    /// the reader's player's pet, by the attacker's `Monster`.
    pub fn pet_attacked_this_turn(&self) -> impl Iterator<Item = Option<V::Monster>> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::Attacked { monster, pet_of } if *pet_of == Some(self.player) => {
                Some(*monster)
            }
            _ => None,
        })
    }

    /// `CardPlaysStarted` where `HappenedThisTurn`, of the owner's plays.
    pub fn started_this_turn(&self) -> impl Iterator<Item = PlayStarted<V>> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::Started(started) if started.player == self.player => Some(*started),
            _ => None,
        })
    }

    /// Each `CardDrawnEntry` where `HappenedThisTurn` of the reader's
    /// player's, by the card drawn.
    pub fn drawn_this_turn(&self) -> impl Iterator<Item = V::Card> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::Drawn { card, player, .. } if *player == self.player => Some(*card),
            _ => None,
        })
    }

    /// `EnergySpentEntry` where `HappenedThisTurn` of the reader's player's,
    /// summed.
    pub fn energy_spent_this_turn(&self) -> V::Count {
        self.this_turn
            .iter()
            .filter_map(|entry| match entry {
                HistoryEntry::EnergySpent { amount, player } if *player == self.player => {
                    Some(*amount)
                }
                _ => None,
            })
            .fold(num_traits::zero(), |spent, amount| spent + amount)
    }

    /// Each `PowerReceivedEntry` where `HappenedThisTurn`, by the power and
    /// who applied it.
    pub fn received_this_turn(
        &self,
    ) -> impl Iterator<Item = (V::Power, Option<crate::CombatId>)> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::PowerReceived { power, applier } => Some((*power, *applier)),
            _ => None,
        })
    }

    /// How many `OrbChanneledEntry` of `orb` of the reader's player's the
    /// fight holds.
    pub fn channelled_this_fight(&self, orb: V::Orb) -> usize {
        let before = self.before.into_iter().flat_map(PastTurns::entries);
        self.this_turn
            .iter()
            .chain(before)
            .filter(|entry| {
                matches!(
                    entry,
                    HistoryEntry::Channelled { orb: channelled, player }
                        if *channelled == orb && *player == self.player
                )
            })
            .count()
    }

    /// How many `CardDrawnEntry` of the reader's player's the fight holds.
    pub fn drawn_this_fight(&self) -> usize {
        let before = self.before.into_iter().flat_map(PastTurns::entries);
        self.this_turn
            .iter()
            .chain(before)
            .filter(|entry| matches!(entry, HistoryEntry::Drawn { player, .. } if *player == self.player))
            .count()
    }

    /// How many `CardDrawnEntry` where `HappenedThisTurn` of the reader's
    /// player's are not `FromHandDraw`.
    pub fn drawn_outside_hand_draw_this_turn(&self) -> usize {
        self.this_turn
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    HistoryEntry::Drawn {
                        hand_draw: false,
                        player,
                        ..
                    } if *player == self.player
                )
            })
            .count()
    }

    /// How many `CardAfflictedEntry` of `affliction` on the reader's
    /// player's cards are where `HappenedThisTurn`.
    pub fn afflicted_this_turn(&self, affliction: V::Affliction) -> usize {
        self.this_turn
            .iter()
            .filter(|entry| {
                matches!(
                    entry,
                    HistoryEntry::Afflicted { affliction: afflicted, player }
                        if *afflicted == affliction && *player == self.player
                )
            })
            .count()
    }

    /// Each `BlockGainedEntry` where `HappenedThisTurn` whose `CardPlay` is
    /// the reader's player's.
    pub fn block_gained_this_turn(&self) -> impl Iterator<Item = BlockGained> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::BlockGained(gained)
                if gained.play.is_some_and(|PlayAt(at)| {
                    matches!(
                        self.this_turn.get(at as usize),
                        Some(HistoryEntry::Started(started)) if started.player == self.player
                    )
                }) =>
            {
                Some(*gained)
            }
            _ => None,
        })
    }

    /// How many `CardDiscardedEntry` of the reader's player's are where
    /// `HappenedThisTurn`.
    pub fn discarded_this_turn(&self) -> usize {
        self.this_turn
            .iter()
            .filter(|entry| matches!(entry, HistoryEntry::Discarded { player, .. } if *player == self.player))
            .count()
    }

    /// Whether any `CardExhaustedEntry` of the reader's player's is where
    /// `HappenedThisTurn`.
    pub fn exhausted_this_turn(&self) -> bool {
        self.this_turn.iter().any(|entry| {
            matches!(entry, HistoryEntry::Exhausted { player, .. } if *player == self.player)
        })
    }

    /// How many `CardExhaustedEntry` the fight holds, every player's.
    pub fn exhausted_this_fight(&self) -> usize {
        let before = self.before.into_iter().flat_map(PastTurns::entries);
        self.this_turn
            .iter()
            .chain(before)
            .filter(|entry| matches!(entry, HistoryEntry::Exhausted { .. }))
            .count()
    }

    /// How many `CardGeneratedEntry` of the fight name the reader's player as
    /// their `Creator`.
    pub fn generated_by_player(&self) -> usize {
        let before = self.before.into_iter().flat_map(PastTurns::entries);
        self.this_turn
            .iter()
            .chain(before)
            .filter(|entry| {
                matches!(entry, HistoryEntry::Generated { creator } if *creator == Some(self.player))
            })
            .count()
    }

    /// Every `DamageReceivedEntry` of the fight, the turn it stands in first
    /// and then the turns before it, latest first.
    pub fn damage_received_this_fight(&self) -> impl Iterator<Item = DamageReceived<V>> + '_ {
        let before = self.before.into_iter().flat_map(PastTurns::entries);
        self.this_turn
            .iter()
            .chain(before)
            .filter_map(|entry| match entry {
                HistoryEntry::DamageReceived(received) => Some(*received),
                _ => None,
            })
    }

    /// Each `DamageReceivedEntry` where `HappenedThisTurn`.
    pub fn damage_received_this_turn(&self) -> impl Iterator<Item = DamageReceived<V>> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::DamageReceived(received) => Some(*received),
            _ => None,
        })
    }

    /// Each `StarsModifiedEntry` where `HappenedThisTurn` of the reader's
    /// player's, by its `Amount`.
    pub fn stars_modified_this_turn(&self) -> impl Iterator<Item = V::Number> + '_ {
        self.this_turn.iter().filter_map(|entry| match entry {
            HistoryEntry::StarsModified { amount, player } if *player == self.player => {
                Some(*amount)
            }
            _ => None,
        })
    }
}

/// Where a stunned monster goes once its stunned turn is over, which is
/// `CreatureCmd.Stun`'s `nextMoveId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunResume {
    /// None named: `StateLog.Last()`. That is the state the stun interrupted,
    /// unless a `SetMoveImmediate` forced that state, which writes nothing
    /// into the log.
    Interrupted,
    /// `StateLog.Last().GetNextState`: the state the interrupted move leaves
    /// for.
    AfterInterrupted,
    /// This state of the owner's graph.
    At(u8),
}

/// What the listeners an event reaches ask for, in the order they ask.
///
/// Inline to four. One notification rarely draws more than one answer from
/// the whole fight.
pub type HookEffects<V> = smallvec::SmallVec<[HookEffect<V>; HOOK_EFFECTS]>;

/// The creatures a command takes as its targets, in command order.
///
/// Inline to eight, which no encounter's side outnumbers.
pub type Opponents = smallvec::SmallVec<[crate::CombatId; 8]>;

/// How many effects a notification's answers hold before they reach the heap.
pub const HOOK_EFFECTS: usize = 4;

/// One `AttackCommand`, which a listener can tell from every other.
///
/// The fight counts them as they start. A listener keeps one to know the
/// command it bound to when that command ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackId(pub u32);

/// Which of the character's combat pool a generation effect draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolFilter {
    /// Every card of the pool.
    Every,
    /// The cards of one type.
    OfKind(CardKind),
    /// Every card but the `Basic` and `Ancient` ones.
    NotBasicOrAncient,
    /// The cards whose canonical cost is nothing, and not X.
    FreeFixedCost,
    /// The cards whose canonical `Keywords` hold Ethereal.
    Ethereal,
    /// The `Common` cards.
    Common,
}

/// `CardPilePosition`: which end of a pile a card goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PilePosition {
    Top,
    Bottom,
}

/// A card about to leave play, as `ModifyCardPlayResultPileTypeAndPosition`
/// hands it over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultPile<V: ContentVersion> {
    /// The card's owner.
    pub player: crate::CombatId,
    pub card: V::Card,
    /// `ResourceInfo.EnergyValue`, what the play cost.
    pub energy: V::Count,
    /// `IsDupe`: the copy is a dupe, made to be played once.
    pub dupe: bool,
    /// Where it goes so far. Nothing for a card that leaves the fight.
    pub pile: Option<crate::Pile>,
    pub position: PilePosition,
}

/// What a selection from the deck does to each card it takes.
///
/// Each is its own `CardSelectCmd` in the game, and each offers the deck
/// narrowed its own way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckAct<V: ContentVersion> {
    /// `FromDeckForRemoval`, then `CardPileCmd.RemoveFromDeck`. It offers
    /// the cards with no `Eternal`, curses first.
    Remove,
    /// `FromDeckForUpgrade`, then `CardCmd.Upgrade`. It offers the cards an
    /// upgrade can still raise.
    Upgrade,
    /// `FromDeckForTransformation`, then `CardCmd.TransformToRandom` off the
    /// named stream. It offers the cards that can be transformed.
    Transform(Stream<V>),
    /// The same, with each replacement upgraded before it lands, as
    /// `CardCmd.Upgrade` then `CardCmd.Transform`.
    TransformUpgraded(Stream<V>),
    /// `CardCmd.Transform` over the whole list at once, off the named
    /// stream. Its `PileIndexSort` lands the replacements in their
    /// originals' deck order. A transform a card at a time lands each in the
    /// order the cards were taken.
    TransformTogether(Stream<V>),
    /// `FromDeckForRemoval` over the deck's basic cards with this tag that
    /// can be removed, then `CardPileCmd.RemoveFromDeck`.
    RemoveBasicTagged(V::CardTag),
    /// `FromDeckGeneric` over the deck's basic cards that can be transformed,
    /// then `CardCmd.TransformTo` this card.
    TransformBasicInto(V::Card),
    /// `FromDeckForTransformation`, then `CardCmd.Transform` of each into this
    /// card. The replacement is upgraded where the original was, and carries
    /// the original's enchantment where that enchantment takes it.
    TransformInto(V::Card),
    /// `FromDeckForRemoval` over the deck's upgradable cards, then each
    /// removed and kept on the relic, in id order. Pael's Tooth's.
    RemoveOntoRelic(V::Relic),
    /// `FromDeckForEnchantment` over the cards the enchantment takes, of one
    /// type where `kind` names it, in deck order. Then `CardCmd.Enchant` by
    /// `amount`.
    Enchant {
        enchantment: V::Enchantment,
        amount: u8,
        kind: Option<CardKind>,
    },
}

/// One stream to draw from: one of the run's, one of the player's, or the
/// event's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream<V: ContentVersion> {
    Run(V::RunRngType),
    Player(V::PlayerRngType),
    /// `EventModel.Rng`, which the event the run stands in seeds for itself.
    Event,
}

/// One reward a relic's own screen offers, as `RewardsCmd.OfferCustom` is
/// handed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomReward<V: ContentVersion> {
    /// `SpecialCardReward` of this card.
    SpecialCard(V::Card),
    /// `RelicReward(rarity)`: the next relic of that rarity from the front
    /// of the player's bag, pulled as the reward fills in.
    Relic(<V::Relic as crate::RelicTable>::Rarity),
    /// `CardReward` over the character's own pool at `RegularEncounter` odds,
    /// from `CardCreationSource.Other`, offering `count` cards.
    /// `ForNonCombatWithDefaultOdds` sets `NoUpgradeRoll`, which clears
    /// `upgrade_roll`.
    CharacterCards { count: usize, upgrade_roll: bool },
    /// `PotionReward(player)`, a random potion out of combat.
    Potion,
    /// `PotionReward(potion, player)` for this potion.
    SpecificPotion(V::Potion),
    /// `PotionReward(potion, player)` for one `Rng.NextItem` over the potions
    /// of one rarity in the character's pool and the shared one. The potion
    /// is drawn as the reward is made, before any reward fills in.
    PotionOf(RewardRarity),
    /// `GoldReward(amount, player)`, whose `Populate` still draws once, over
    /// the range from the amount to the amount.
    Gold(V::Number),
    /// `RelicReward(player)`: the next relic from the front of the player's
    /// bag, at a rarity it rolls as it fills in.
    RolledRelic,
    /// `RelicReward(relic, player)` for `PullNextRelicFromFront(player)`,
    /// made wax. Toy Box pulls each before the set fills in, which draws as a
    /// set of [`CustomReward::RolledRelic`] alone does.
    WaxRelic,
    /// `PotionReward(potion, player)` for one `Rng.NextItem` over
    /// [`Rules::unlocked_potions`] off the rewards stream.
    UnlockedPotion,
    /// `CardReward` over the colourless pool at `RegularEncounter` odds, with
    /// no upgrade roll, offering this many cards.
    ColorlessCards(usize),
    /// `CardReward` at `CardCreationOptions.ForRoom(player, Monster)`: the
    /// three cards a monster fight's reward offers, at its odds.
    MonsterRoomCards,
    /// `CardReward` over a character's pool at uniform odds, of one rarity,
    /// with no rarity modification. `pool` names another character's pool,
    /// and nothing is the character's own. Glass Eye offers five.
    CardsOf {
        pool: Option<V::Character>,
        rarity: RewardRarity,
        count: usize,
        /// `RollForUpgrade` for each card, which `NoUpgradeRoll` leaves out.
        upgrade_roll: bool,
        /// `CardCreationFlags.NoCardPoolModifications`, which keeps
        /// `ModifyCardRewardCreationOptions` off the pool.
        no_pool_modifications: bool,
    },
    /// `CardReward` over the character's own pool at uniform odds, of one
    /// rarity and one type, and then every card it offers upgraded. The
    /// Future of Potions offers it.
    UpgradedCardsOf {
        rarity: RewardRarity,
        kind: CardKind,
        count: usize,
    },
    /// `RelicReward(relic, player)` for this relic.
    SpecificRelic(V::Relic),
}

/// How `RelicFactory.PullNextRelicFromFront` picks the next relic from the
/// front of the player's bag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelicPull<V: ContentVersion> {
    /// At a rarity rolled off the player's rewards stream, with no filter.
    Rolled,
    /// At this rarity, among the relics a merchant can stock, which is
    /// `IsAllowedInShops`.
    Stocked(<V::Relic as crate::RelicTable>::Rarity),
}

/// What happens to each card a [`HookEffect::SelectFromPile`] takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PileAct {
    /// `CardCmd.Exhaust`, one card at a time.
    Exhaust,
    /// `CardCmd.Discard` over all of them.
    Discard,
    /// `CardPileCmd.Add(cards, PileType.Hand)`.
    IntoHand,
    /// `CardPileCmd.Add(card, PileType.Draw, CardPilePosition.Top)`.
    OntoDrawTop,
    /// `CardPileCmd.Add(cards, PileType.Draw)`, which is its bottom.
    IntoDraw,
    /// `CardModel.GiveSingleTurnRetain`, which leaves the card where it is.
    RetainThisTurn,
    /// `CardCmd.TransformToRandom` off `CombatCardSelection`, one card at a
    /// time.
    TransformToRandom,
    /// `CardCmd.DiscardAndDraw(cards, cards.Count)`: every card discarded,
    /// then as many drawn, and only then the Sly ones among them played.
    DiscardAndDraw,
}

/// How [`HookEffect::ChooseCards`] draws each card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardDraw {
    /// `Uniform` odds at one rarity: one `NextItem` over what is left of it.
    Uniform(RewardRarity),
    /// `RegularEncounter` odds, which roll a rarity first.
    Regular,
    /// `RegularEncounter` odds rolled with `CardRarityOdds.Roll`, which moves
    /// the run's offset as a fight's reward does. `ForceRarityOddsChange`
    /// asks for it.
    Encounter,
}

/// A deck card a relic names rather than asks for.
///
/// Among the deck's `Basic` cards, the first or the last that carries a tag,
/// which is `FirstOrDefault` or `LastOrDefault` over them. A deck without one
/// names nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckPick<V: ContentVersion> {
    FirstBasic(V::CardTag),
    LastBasic(V::CardTag),
}

/// One command requested by a content model in response to an event hook.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HookEffect<V: ContentVersion> {
    /// `CardSelectCmd.FromDeckGeneric`, and a clone of what it took.
    ///
    /// The run asks for one card of its deck and adds a copy of it. A card
    /// whose type the filter names is offered by no such selection.
    CloneDeckCard {
        excluding: CardKind,
    },
    /// `CardSelectCmd.FromDeckFor*`: the player picks `count` cards of the
    /// deck, and `act` is what happens to each one.
    SelectFromDeck {
        act: DeckAct<V>,
        count: usize,
    },
    /// The same, with the cards on offer put through `UnstableShuffle` off
    /// `RunState.Rng.Niche` first. The selection shows them in deck order
    /// all the same.
    SelectFromShuffledDeck {
        act: DeckAct<V>,
        count: usize,
    },
    /// The same for `min` to `max` cards, with `RequireManualConfirmation`:
    /// the owner is asked whenever the deck offers a card at all.
    SelectFromDeckConfirmed {
        act: DeckAct<V>,
        min: usize,
        max: usize,
    },
    /// `CardCmd.Upgrade` or `CardCmd.Transform` on the deck cards the picks
    /// name, all picked before any is changed.
    OnDeckCards {
        act: DeckAct<V>,
        picks: &'static [DeckPick<V>],
    },
    /// `RelicReward(player)`, this many, into the rewards a hook is
    /// modifying. Each pulls from the front of the bag at a rolled rarity.
    AddRelicRewards(usize),
    /// `RewardsCmd.OfferCustom`: a screen of these rewards, filled in in this
    /// order, which the player takes from or skips.
    OfferRewards(&'static [CustomReward<V>]),
    /// `Reward.SelectUnsynchronized` on a reward no set holds, with `CanSkip`
    /// false. It fills in alone, so no set's hooks or sort run over it.
    SelectReward(CustomReward<V>),
    /// `RewardsCmd.OfferCustom` over the rewards the event built for itself,
    /// which [`Rules::event_rewards`] names. Each draws off the event's own
    /// stream.
    OfferEventRewards,
    /// A relic reward for each of the first `count` relics Neow can offer.
    /// They are shuffled off the player's rewards stream first, and the
    /// screen disallows skipping. The list is [`Rules::neow_options`] where
    /// `IsAllowedAtNeow` holds, less `except`.
    OfferNeowRelics {
        count: usize,
        except: V::Relic,
    },
    /// The event parks on a question of its own, which
    /// [`Rules::event_question`] asks.
    AskEvent,
    /// [`Rules::resume_event`]: the rest of the option's body, after the
    /// pickup before this has closed.
    ResumeEvent,
    /// `NextItem` over the deck's upgradable cards off `stream`, then
    /// `CardCmd.Upgrade` on the one it draws.
    UpgradeRandomDeckCard {
        stream: Stream<V>,
    },
    /// `NextItem` over the deck's upgraded cards off `stream`, then
    /// `CardCmd.Downgrade` on the one it draws, which sets its level to
    /// nothing.
    DowngradeRandomDeckCard {
        stream: Stream<V>,
    },
    /// `CardCmd.Upgrade` on every deck card an upgrade can still raise.
    UpgradeDeck,
    /// `RunState.CloneCard` of each card the deck held as this began, in deck
    /// order, each then `CardPileCmd.Add` to the deck.
    CloneDeck,
    /// `NAbandonRunConfirmPopup`, which Trial's Double Down opens. A yes is
    /// `RunManager.Abandon`, which kills the player and ends the run. A no
    /// closes it on the page it opened from.
    ConfirmAbandonRun,
    /// `HistoryCourse.AfterAutoPrePlayPhaseEntered`: a dupe of the last card
    /// its owner finished playing in its last turn, auto-played. The card is
    /// of one of `kinds`, and is no dupe itself.
    AutoPlayDupeOfLastPlay {
        kinds: &'static [CardKind],
    },
    /// `LordsParasol.PurchaseEverything`: every shelf of the merchant bought
    /// for nothing, in the shelves' order, then its card removal, which the
    /// player cannot cancel.
    PurchaseEverything,
    /// `Hook.AfterItemPurchased` for an entry bought outside a merchant's
    /// room, with the gold it spent.
    ItemPurchased(V::Number),
    /// `CreatureCmd.GainMaxHp` on the owner: the maximum grows, and the
    /// health with it.
    GainMaxHp(V::Number),
    /// `CreatureCmd.LoseMaxHp` on the owner. The health falls to the new
    /// maximum when it stood above it.
    LoseMaxHp(V::Number),
    /// `CreatureCmd.Damage(target, amount, Unpowered)` from nobody.
    DamageUnpowered {
        target: crate::CombatId,
        amount: V::Number,
    },
    /// `CreatureCmd.Add` of a monster the power built as its dying owner goes,
    /// in the owner's slot. `written` is what the power wrote on the new
    /// model, and the model opens on `opens_on` where it names one.
    SummonWritten {
        monster: V::Monster,
        written: V::Number,
        opens_on: Option<V::MoveId>,
    },
    /// `CreatureCmd.LoseMaxHp` on a creature the listener names.
    LoseMaxHpOf {
        target: crate::CombatId,
        amount: V::Number,
    },
    /// `CreatureCmd.Damage` on the owner, unpowered and from nobody.
    DamageOwner(V::Number),
    /// `CreatureCmd.Damage` on the listener's owner in a fight, unpowered.
    /// Unblockable where it says, and dealt by the owner itself where it says,
    /// or by nobody.
    DamageOwnerInFight {
        amount: V::Number,
        unblockable: bool,
        self_dealt: bool,
    },
    /// `CardFactory.CreateForReward` for one card of `rarity` from the
    /// character's pool, at `Uniform` odds and with no upgrade roll, into the
    /// deck.
    AddUniformCard {
        rarity: crate::RewardRarity,
    },
    /// `CardFactory.CreateForReward` for `count` cards of the character's
    /// pool, with no upgrade roll. A screen then takes between `min` and
    /// `max` of them into the deck, in the order picked.
    /// `FromChooseACardScreen` takes one or none, and
    /// `FromSimpleGridForRewards` its own count. `card_reward` is
    /// `CardCreationFlags.IsCardReward`, which the late hooks read.
    ChooseCards {
        draw: CardDraw,
        count: usize,
        min: usize,
        max: usize,
        card_reward: bool,
    },
    /// `CardCmd.Transform` on every deck card that `IsBasicStrikeOrDefend`
    /// and is removable, each into `CreateRandomCardForTransform` off the
    /// named stream, in deck order.
    TransformBasicStrikesAndDefends(Stream<V>),
    /// `RelicCmd.Replace` on the relic `from`, with `to` obtained into its
    /// slot.
    ReplaceRelic {
        from: V::Relic,
        to: V::Relic,
    },
    /// `RelicCmd.Replace` on the first starter relic the player holds, with
    /// the relic `upgrades` pairs it with, or `fallback` for one it names
    /// nowhere. The replacement takes the starter's slot.
    ReplaceStarterRelic {
        upgrades: &'static [(V::Relic, V::Relic)],
        fallback: V::Relic,
    },
    /// `CardFactory.CreateForReward` of one card at `CardRarityOddsType.Uniform`
    /// off the rewards stream, from the colorless pool or the character's
    /// own, with no upgrade roll. Then `CardPileCmd.Add` to the deck of
    /// `copies` of it, each its own `CloneCard`. `draws` times over, with no
    /// card kept out of a later draw.
    AddUniformToDeck {
        colorless: bool,
        draws: usize,
        copies: usize,
    },
    /// `RunState.CloneCard` of the card that just joined the deck, then
    /// `CardPileCmd.Add` of the copy to the deck's bottom with `clonedBy` the
    /// listener, this many times. [`EventHook::CopyMade`] is asked of the
    /// listener between the two.
    CopyIntoDeck(usize),
    /// `RunState.CloneCard` of each deck card carrying this enchantment, in
    /// deck order, each then `CardPileCmd.Add` to the deck's bottom.
    CloneEnchantedIntoDeck(V::Enchantment),
    /// `CardPileCmd.AutoPlayFromDrawPile(count, Top, forceExhaust: false)`
    /// for the listener's owner.
    AutoPlayFromDraw(V::Number),
    /// `count` times, `CardCmd.AutoPlay` of one card of the hand of `kind`
    /// that is not Unplayable, with no target named. Each is drawn by
    /// `NextItem` off `Shuffle` from those standing then, and none is played
    /// once there is none.
    AutoPlayRandomFromHand {
        kind: CardKind,
        count: V::Number,
    },
    /// `CardSelectCmd.FromHand` of up to this many of the cards that do not
    /// retain this turn, each of which `GiveSingleTurnRetain` then retains.
    PickToRetain(V::Number),
    /// `CardPileCmd.AutoPlayFromDrawPile(1, Top)`, when `card` stands on top
    /// of the draw pile. Nothing otherwise.
    AutoPlayFromDrawIfTop(crate::CardId<V>),
    /// `RelicCmd.Obtain` of `relic`, set up for `character` first.
    ObtainRelicFor {
        relic: V::Relic,
        character: V::Character,
    },
    /// `CardFactory.CreateForReward` three times over `pool`'s own card pool,
    /// at `Uniform` odds, with no upgrade roll: `per_rarity` commons, then
    /// uncommons, then rares. A grid then takes between `min` and `max` of
    /// them into the deck, in the order picked.
    ChooseCardsOfPool {
        pool: V::Character,
        per_rarity: usize,
        min: usize,
        max: usize,
    },
    /// `WithCardPools` over these characters' pools and then the reward's
    /// own, as `Union` joins them: in that order, each once.
    AddCharacterPools(&'static [V::Character]),
    /// `WithCardPools` over the reward's pools with the colorless pool after
    /// them, where it is not among them already.
    AddColorlessPool,
    /// `Concat` of this character's unlocked cards after the pool's own. A
    /// card reward drops repeats as it draws, and a merchant keeps them.
    ConcatCharacterPool(V::Character),
    /// `WithCardPools` over the reward's pools with this character's after
    /// them, as `Union` joins them. The reward's filter stays.
    UnionCharacterPool(V::Character),
    /// `RunManager.GenerateMap` for the act the run stands in, which the
    /// hooks can replace.
    RegenerateMap,
    /// A map of one column in place of the one generated: the ancient, each
    /// of `rooms` in turn, then the boss. Golden Compass's
    /// `GoldenPathActMap`.
    FixedPathMap(&'static [V::MapPointType]),
    /// One of the cards the relic keeps, drawn with `NextItem` off the
    /// rewards stream, back into the deck and upgraded where it can be.
    ReturnHeldCard(V::Relic),
    /// `CardCmd.Exhaust` of each card in the owner's hand, in hand order.
    ExhaustHand,
    /// `RelicCmd.Melt` of the owner's first wax relic not yet melted, where
    /// there is one.
    MeltFirstWax,
    /// `CanReroll = true` on each card reward of the set.
    RerollableCardRewards,
    /// Each gold reward of the set made again at twice its amount.
    DoubleGoldRewards,
    /// Each card reward of the set swapped, where it stands, for a relic
    /// reward of no set rarity.
    RelicRewardsForCardRewards,
    /// A `CardRewardAlternative` added to the card reward's list.
    OfferRewardAlternative(crate::RewardAlternative),
    /// `Rng.NextInt(below)` off a stream, drawn for its place in the stream
    /// alone, which a flavour line makes.
    DrawFromStream {
        stream: Stream<V>,
        below: u32,
    },
    /// `RelicCmd.Remove` on the first of the player's relics that is this
    /// one.
    RemoveRelic(V::Relic),
    /// `PlayerCmd.GainMaxPotionCount`.
    GainPotionSlots(usize),
    /// `PotionCmd.TryToProcure` of this potion, which a full belt refuses.
    ProcurePotion(V::Potion),
    /// A rest site option added to those a rest site offers.
    OfferRestOption(V::RestSiteOption),
    /// Every rest site option of this kind taken off those a rest site
    /// offers.
    WithdrawRestOption(V::RestSiteOption),
    /// `ModifyGeneratedMap` as Big Game Hunter answers it. The act's map is
    /// made again over `MapPointTypeCounts(map)`, with this many times its
    /// elites, rounded. The elites ignore the placement rules.
    ScaledElitesMap(Fixed),
    /// `WithCustomPool` of the rare cards among the reward's possible cards,
    /// or the character's own rares where it has none, at
    /// `CardRarityOddsType.Uniform`.
    RaresAtEvenOdds,
    /// `RelicGrabBag.Remove` of this relic from the player's bag and the
    /// shared one, so neither deals it.
    RemoveFromRelicBags(V::Relic),
    /// `UnknownMapPointOdds`: the odds an unknown room is `room` now, and the
    /// base they go back to once it is rolled. Negative is never.
    SetUnknownRoomOdds {
        room: V::RoomType,
        odds: Fixed,
        base: Fixed,
    },
    /// One more option on a card reward: a card of this kind from the pool
    /// the reward drew, by `CreateForReward` from `CardCreationSource.Other`.
    /// It is one the reward does not offer, unless it offers them all.
    AddCardRewardOption(CardKind),
    /// The `OnSelect` of a rest site option a relic offers, which calls a
    /// method on that relic: `Girya.Lift` or `PumpkinCandle.Rekindle`. The
    /// relic carries it out when told [`EventHook::RestOptionTaken`].
    RestOptionOfRelic(V::Relic),
    /// `PotionFactory.CreateRandomPotionsOutOfCombat` off the named stream,
    /// then `PotionCmd.TryToProcure` for each.
    ProcureRandomPotions {
        count: usize,
        stream: V::RunRngType,
        /// `CreateRandomPotionInCombat`, which draws among the potions
        /// `CanBeGeneratedInCombat` alone.
        in_combat: bool,
        /// Each into the slot at `originalSlotCount + i`: the last `count`
        /// slots, which a `GainMaxPotionCount` just before opened. Otherwise
        /// each into the first open slot.
        into_gained: bool,
    },
    /// `CardSelectCmd` over the run's upgradable cards, of one type where
    /// `kind` names it.
    UpgradeRandomDeckCards {
        count: usize,
        stream: Stream<V>,
        kind: Option<CardKind>,
    },
    /// The draw pile's upgradable cards, stably shuffled off
    /// `CombatCardSelection`, and `CardCmd.Upgrade` on the first this many.
    UpgradeRandomDrawCards(usize),
    /// Take a room type out of what an unknown map point can roll.
    RemoveRoomType(V::RoomType),
    /// `PlayerCmd.GainStars`.
    GainStars(V::Number),
    /// `CreatureCmd.Heal` on the listener's owner.
    Heal(V::Number),
    /// `OnUseWrapper` on the creature a potion kept alive: the potion leaves
    /// the belt, and its effect runs on that creature.
    UseOnSaved,
    /// `CreatureCmd.Heal` on the listener's owner for this many hundredths of
    /// its maximum health. The health keeps the whole part.
    HealPercent(V::Number),
    /// `CreatureCmd.Heal` on the listener's owner for what its health lacks
    /// of its maximum, as it stands when the heal is carried out.
    HealMissing,
    /// `OrbCmd.Channel` for the listener's owner.
    Channel(V::Orb),
    /// `OrbModel.GetRandomOrb` off `CombatOrbGeneration`, over `orbs`, then
    /// `OrbCmd.Channel` for the listener's owner, `count` times.
    ChannelRandomOrbs {
        orbs: &'static [V::Orb],
        count: V::Number,
    },
    /// `OstyCmd.Summon` for the listener's owner.
    Summon(V::Number),
    /// `CardPileCmd.AddGeneratedCardToCombat` of `count` clones of the card a
    /// power keeps, which [`Fight::keep_card_on_power`] put there, each into
    /// hand.
    CloneKeptIntoHand {
        kept: crate::CardId<V>,
        count: V::Number,
    },
    /// `CardPileCmd.AddGeneratedCardToCombat` of `count` clones of the copy
    /// `card`, each into hand. Each clone takes `writes`.
    CloneIntoHand {
        card: crate::CardId<V>,
        count: V::Number,
        writes: &'static [(Change<V>, Lasting)],
    },
    /// `CreatureCmd.Damage` on every enemy that can be hit, from the
    /// listener's owner, for a number no power scales.
    DamageOpponents(V::Number),
    /// `PowerCmd.Apply<P>(target, amount, applier)`, onto a creature the
    /// listener names, from the creature it names or from nobody.
    ApplyPower {
        target: crate::CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<crate::CombatId>,
    },
    /// `PowerCmd.Apply<P>` onto one enemy that can be hit, drawn by
    /// `NextItem` off `CombatTargets`, from the listener's owner. Nothing
    /// with no such enemy.
    ApplyToRandomOpponent {
        power: V::Power,
        amount: PowerAmount<V::Number>,
    },
    /// `CreatureCmd.Damage` on every enemy that can be hit, from the
    /// listener's owner, unblockable and unpowered.
    DamageOpponentsUnblockable(V::Number),
    /// `CreatureCmd.Damage` on one enemy that can be hit, drawn by `NextItem`
    /// off `CombatTargets`, unpowered. Unblockable where it says, and dealt by
    /// the listener's owner where it says, or by nobody. Nothing with no such
    /// enemy.
    DamageRandomOpponent {
        amount: V::Number,
        unblockable: bool,
        from_owner: bool,
    },
    /// `CreatureCmd.Damage` on the living creatures the event hit, from the
    /// listener's owner, unpowered.
    DamageTargets(V::Number),
    /// `change` written on each card `to` names, for the rest of the fight.
    ModifyCards {
        to: Writing<V>,
        change: Change<V>,
    },
    /// `CreatureCmd.Escape` on the listener's owner: its powers go with no
    /// hook, and it leaves the fight without dying.
    Escape,
    /// `CardCmd.Downgrade` on every upgraded card the power's owner holds in
    /// the fight. The fight keeps each copy and the level it had beside the
    /// power, in [`Kept::cards`].
    DowngradeUpgraded,
    /// `creature` off what the fight keeps beside the listener, in
    /// [`Kept::creatures`].
    ForgetCreature(crate::CombatId),
    /// `CardCmd.Upgrade` on one copy, `times` over, wherever it stands in
    /// the fight. Nothing for a copy that has left it.
    UpgradeCopy {
        card: crate::CardId<V>,
        times: u8,
    },
    /// `CardPileCmd.AddToCombatAndPreview(card)` into the discard pile of
    /// each player the attack hit: `per` for each of its hits on that player
    /// that took health.
    GiveCardsPerUnblockedHit {
        card: V::Card,
        per: V::Number,
    },
    /// `PowerCmd.Apply(power)` on the listener's owner: `per` for each of the
    /// attack's hits that took health. A hit a pet stood in for counts by the
    /// pet's result alone.
    ApplyToOwnerPerUnblockedHit {
        power: V::Power,
        per: V::Number,
    },
    /// `PotionFactory.CreateRandomPotionOutOfCombat` off the named stream and
    /// `PotionCmd.TryToProcure`, one at a time for as long as the belt has an
    /// open slot and the potion lands.
    FillPotionSlots {
        stream: V::RunRngType,
    },
    /// `PotionCmd.Discard` on the potion at this place in the belt, counting
    /// held potions in slot order.
    DiscardPotion(usize),
    /// `CardPileCmd.Add(this, PileType.Hand)` for the card listening, from
    /// whichever pile it stands in.
    ToHand,
    /// `CreatureCmd.Kill` on the listener's owner: what hit points it has
    /// are lost, and it dies.
    KillOwner,
    /// `DoomPower.BeforeSideTurnEnd`, once the owner's hit points are no more
    /// than its Doom. `DoomKill` takes every doomed creature on its side, at
    /// the first one's asking.
    DoomKill,
    /// `CreatureCmd.Damage` on every enemy that can be hit, from the
    /// listener's owner, unpowered, while the owner holds an `orb`.
    DamageOpponentsHolding {
        orb: V::Orb,
        amount: V::Number,
    },
    /// `CreatureCmd.Damage` on one creature, unpowered, from the listener's
    /// owner.
    Damage {
        target: crate::CombatId,
        amount: V::Number,
    },
    /// `CreatureCmd.GainBlock` on the listener's owner, unpowered: `amount`
    /// for each kind of orb its owner holds, which is `CoolantPower`'s.
    GainBlockPerOrbKind(V::Number),
    /// `OrbCmd.Passive` on the first orb the owner holds, `count` times, and
    /// nothing with no orb.
    FirstOrbPassive(V::Number),
    /// `OrbCmd.Passive` on each orb the owner holds, front first.
    EachOrbPassive,
    /// `count` times, one card of the character's combat pool that `filter`
    /// keeps, drawn by `CardFactory.GetDistinctForCombat` off
    /// `CombatCardGeneration`. Each takes `writes`, and they go into hand.
    GenerateFromPool {
        filter: PoolFilter,
        count: V::Number,
        writes: &'static [(Change<V>, Lasting)],
    },
    /// `count` distinct cards of the colorless pool, from one
    /// `CardFactory.GetDistinctForCombat` off `CombatCardGeneration`. It
    /// shuffles the pool once and takes the first `count`, or all of it if it
    /// is shorter. They go into hand.
    GenerateDistinctColorless {
        count: V::Number,
    },
    /// `OrbCmd.AddSlots` for the player.
    AddOrbSlots(V::Number),
    /// `CreatureCmd.SetCurrentHp` to `hp` on each enemy it can hit, or on
    /// the one `creature` names.
    SetEnemiesHp {
        creature: Option<crate::CombatId>,
        hp: V::Number,
    },
    /// `FurCoat.AddMarkedRooms`: `count` of the map's Monster and Elite
    /// points marked for `relic`, where its marks do not all stand on such
    /// points already. The points are `GetAllMapPoints` put through
    /// `UnstableShuffle` off a stream seeded with the relic's own id, `name`.
    MarkCombatPoints {
        relic: V::Relic,
        count: usize,
        name: &'static str,
    },
    /// Up to `left` times, the first card of the hand that `CanPlay`, paid
    /// for with `SpendResources` and then `CardCmd.AutoPlay`ed. An attack at
    /// any enemy goes at the leftmost one it can hit. The loop stops once the
    /// fight ends, the turn moves on, or no card can be played.
    AutoPlayHand {
        left: u8,
    },
    /// `count` distinct cards of the character's combat pool that `filter`
    /// keeps, from one `CardFactory.GetDistinctForCombat` off
    /// `CombatCardGeneration`, as `GenerateDistinctColorless` takes them.
    /// They go into hand.
    GenerateDistinctFromPool {
        filter: PoolFilter,
        count: V::Number,
    },
    /// `CardCmd.Afflict` with `affliction` on each card `to` names.
    Afflict {
        to: Writing<V>,
        affliction: V::Affliction,
    },
    /// `CardCmd.ClearAffliction` on each card `to` names.
    ClearAffliction(Writing<V>),
    /// `CardPileCmd.RemoveFromDeck` on the deck card that asked.
    RemoveFromDeck,
    /// `CardPileCmd.RemoveFromDeck` on the card at this place in the deck.
    RemoveDeckCardAt(usize),
    /// `CardCmd.Transform` on the first deck card that is the first of a
    /// pair, into its partner. The partner is upgraded where the card was,
    /// and carries its enchantment. It leaves its place and joins the deck's
    /// end.
    TransformFirstInDeck(&'static [(V::Card, V::Card)]),
    /// `CardCmd.TransformTo(into)` on each card of the deck that is `from`.
    /// Each leaves its place, and its replacement joins the deck's end.
    TransformInDeck {
        from: V::Card,
        into: V::Card,
    },
    /// `PlayerCmd.AddPet<M>`: the monster beside the player.
    AddPet(V::Monster),
    /// The card that asked, moved to the top of the draw pile it was
    /// shuffled into.
    ToTopOfDrawPile,
    /// `times` rounds of `CreatureCmd.Damage` on the listener's owner, each
    /// for its own stack as that round starts, unblockable and unpowered and
    /// from no creature. After each, one off the stack while the owner lives.
    /// `PoisonPower.AfterSideTurnStart`.
    DamageSelfByStack {
        times: V::Number,
    },
    /// `PowerCmd.Apply` on every hittable opponent of the listener's owner,
    /// from that owner.
    ApplyToOpponents(V::Power, PowerAmount<V::Number>),
    /// `PowerCmd.Apply` on every living opponent of the listener's owner,
    /// from nobody. Brimstone's.
    ApplyToLivingOpponents(V::Power, PowerAmount<V::Number>),
    /// `PowerCmd.Apply` on every creature in the fight, in its order, from
    /// nobody.
    ApplyToEveryCreature(V::Power, PowerAmount<V::Number>),
    /// `PowerCmd.Apply` on each player's creature, from nobody.
    ApplyToPlayers(V::Power, PowerAmount<V::Number>),
    /// `CardCmd.AutoPlay` of this copy, with no target named: the card plays
    /// for free, and one that targets an enemy picks it off `CombatTargets`.
    AutoPlay(crate::CardId<V>),
    /// `EnchantmentModel.Status = Disabled` on the enchantment of the card
    /// that asked.
    DisableOwnEnchantment,
    /// `Amount += by` on the enchantment of the card that asked, and `write`
    /// on that card's `DeckVersion` where it has one. Goopy's.
    RaiseOwnEnchantment {
        by: u8,
        write: V::DeckWrite,
    },
    /// `Card.EnergyCost.SetThisCombat` to a draw of `NextInt(below)` off
    /// `CombatEnergyCosts`, on the card whose enchantment asked. Slither's.
    RandomSelfCost {
        below: u8,
    },
    /// `card.EnergyCost.SetThisCombat` to a draw of `NextInt(below)` off
    /// `CombatEnergyCosts`. Confused's, for a card just drawn.
    RandomCost {
        card: crate::CardId<V>,
        below: u8,
    },
    /// One change written on the card that asked, as its own
    /// `EnergyCost.AddThisCombat` or kin write it.
    ModifySelf {
        change: Change<V>,
        lasting: Lasting,
    },
    /// `AggressionPower.BeforeSideTurnStart`: the attacks in the discard pile,
    /// shuffled off `CombatCardSelection`, and the first `count` of them into
    /// hand, each upgraded.
    AttacksFromDiscard(V::Number),
    /// `PlayerCmd.GainGold`, which a run carries out between rooms.
    GainGold(V::Number),
    /// `PlayerCmd.GainGold` of a `decimal` with a fraction. The gold hooks
    /// scale it whole, and `(int)` drops the fraction after them.
    GainGoldExact(Decimal),
    /// `CombatRoom.AddExtraReward` with a `GoldReward` of this much, which
    /// the fight's rewards offer beside its own.
    AddGoldReward(V::Number),
    /// `CardReward(CardCreationOptions.ForRoom(room), 3)`, added to a set's
    /// rewards: three cards at the odds that kind of room rolls.
    AddCardReward(V::RoomType),
    /// `CombatRoom.AddExtraReward` of a `CardRemovalReward`.
    AddCardRemovalReward,
    /// `PlayerCmd.LoseGold`, which stops at none.
    LoseGold(V::Number),
    /// `PlayerCmd.LoseGold(Owner.Gold)`: every coin its owner holds.
    LoseAllGold,
    /// `RewardsCmd.OfferCustom` over `rewards` card rewards, each holding one
    /// card from each of `pools` other characters' pools. The pools are a
    /// `StableShuffle` off `RunState.Rng.Niche` for each reward. Each card is
    /// its own `CardFactory.CreateForReward` at regular odds, whose pass is
    /// not a `CardReward`'s. Each reward then has the pass a `CardReward`
    /// holding cards set by hand has.
    OfferOtherPoolCards {
        rewards: usize,
        pools: usize,
    },
    /// `CardSelectCmd.FromChooseABundleScreen`, over `count` bundles drawn off
    /// the player's rewards stream. Each is `commons` commons of the
    /// character's reward pool, then `uncommons` uncommons, none already in a
    /// bundle. The cards of the one chosen go into the deck.
    ///
    /// `same_card` is `Some((card, percent))` where a `NextInt(100)` under
    /// `percent`, rolled before each bundle, makes it all `card` instead.
    OfferBundles {
        count: usize,
        commons: usize,
        uncommons: usize,
        same_card: Option<(V::Card, u32)>,
    },
    /// `SealOfGold.AfterSideTurnStart`: while the player holds at least
    /// `gold`, `PlayerCmd.GainEnergy` by `energy` and then
    /// `PlayerCmd.LoseGold` by `gold`.
    GainEnergyForGold {
        energy: V::Number,
        gold: V::Number,
    },
    /// `RelicCmd.Obtain`, and whatever the relic's pickup asks.
    ObtainRelic(V::Relic),
    /// `RelicFactory.PullNextRelicFromFront`: the next relic from the front
    /// of the player's bag, as the pull says, then `RelicCmd.Obtain`.
    ObtainNextRelic(RelicPull<V>),
    /// `EventModel.SetEventFinished`, the last thing an option that ends its
    /// event does. The room then offers nothing, and the run goes on.
    FinishEvent,
    /// `RunManager.EnterNextAct`, from the option that closes The Architect.
    /// It is the whole of that option.
    EnterNextAct,
    /// `PlayerCmd.MimicRestSiteHeal`: a rest site's heal, given by an event.
    MimicRestSiteHeal,
    /// `EventModel.EnterCombatWithoutExitingEvent`, which fights inside the
    /// event's room. It is the whole of the option that starts it. `rewards`
    /// is its extra rewards, in the order the fight's room adds them.
    ///
    /// `roster` is the monsters an encounter whose roster is configured opens
    /// on, and nothing for any other. `resume` is `shouldResumeAfterCombat`:
    /// the rewards' proceed goes back to the event, which
    /// [`crate::EventBehavior::resume_from_fight`] carries on.
    FightInEvent {
        encounter: V::Encounter,
        rewards: &'static [CustomReward<V>],
        roster: &'static [V::Monster],
        resume: bool,
        /// The event's own rewards follow `rewards`, which
        /// [`crate::EventBehavior::rewards`] lists from what the event holds.
        own_rewards: bool,
    },
    /// `CardPileCmd.Add(card, PileType.Deck)`, a new card into the run's deck.
    AddToDeck(V::Card),
    /// `count` of `cards` into the deck, each `NextItem` off `stream` over
    /// those not drawn already.
    AddDistinctToDeck {
        cards: &'static [V::Card],
        count: usize,
        stream: V::RunRngType,
    },
    /// The draw pile's cards whose energy cost, with their own modifiers
    /// alone, is a fixed nothing. They are `StableShuffle`d off
    /// `CombatCardSelection`, and `CardPileCmd.Add` puts the first `count`
    /// into the hand.
    ZeroCostFromDraw(usize),
    /// `NextItem` off `CombatCardSelection` over the draw pile's cards of
    /// `kind`, `SetToFreeThisTurn`, then `CardPileCmd.Add` into the hand.
    /// Nothing where the draw pile holds none. With `preferring_without`,
    /// the pick is over those of them without that keyword, where any are.
    FreeRandomFromDraw {
        kind: CardKind,
        preferring_without: Option<V::CardKeyword>,
    },
    /// `MummifiedHand.AfterCardPlayed`'s pick, off `CombatCardSelection`, and
    /// `SetToFreeThisTurn` on it. `NextItem` over the hand's cards that print
    /// a cost and cost something now. Failing that, over the hand's cards
    /// that cost something now, then those that print a cost, then the hand.
    FreeCostlyInHand,
    /// `Bookmark.AfterFlush`'s pick: `NextItem` off `CombatCardSelection`
    /// over the retained cards that cost more than nothing and not X, their
    /// own modifiers counted. The pick then costs one less until played.
    LowerRetainedCost,
    /// `CardCmd.Upgrade` on a new card, then `CardPileCmd.Add` to the deck.
    AddUpgradedToDeck(V::Card),
    /// `CardFactory.CreateForReward` of one card from the character's pool,
    /// narrowed by the filter, at `RegularEncounter` odds with no upgrade
    /// roll, into the deck.
    AddRegularCard(PoolFilter),
    /// `CreatureCmd.Kill(creature, force: true)`.
    KillCreature(crate::CombatId),
    /// `CardPileCmd.AddGeneratedCardToCombat` of `count` new copies into
    /// the player's draw pile, each at `CardPilePosition.Random`: the place
    /// `Rng.Shuffle.NextInt(count + 1)` names, from the top.
    AddToDrawAtRandom {
        card: V::Card,
        count: V::Number,
        /// `addedByPlayer` is the listener's owner, as a relic names it.
        /// Personal Hive names nobody, though its Dazed land in the
        /// player's pile.
        by_owner: bool,
    },
    /// `PlayerCmd.GainEnergy`.
    GainEnergy(V::Number),
    /// `GeneratePlayCount(null)` for the copy `card` names, then
    /// `PlayerCmd.GainEnergy(energy)` once for each play it counts.
    GainEnergyForEachPlay {
        card: crate::CardId<V>,
        energy: V::Number,
    },
    /// `PlayerCmd.LoseEnergy`.
    LoseEnergy(V::Number),
    /// `CardPileCmd.Draw` for the player.
    Draw(V::Number),
    /// `CardSelectCmd.FromHand` or `FromCombatPile`: the player picks cards
    /// of the pile as `selection` allows, and `act` is what happens to them.
    SelectFromPile {
        pile: crate::Pile,
        selection: Selection,
        act: PileAct,
    },
    /// `CardPileCmd.ShuffleIfNecessary`: the discard pile shuffled into an
    /// empty draw pile.
    ShuffleIfNecessary,
    /// `CardCmd.Exhaust` on the top card of the draw pile, or with
    /// `passing_innate` on the first there that is not innate, falling back
    /// to the top. Nothing, for an empty pile.
    ExhaustFromDrawTop {
        passing_innate: bool,
    },
    /// `CardFactory.GetForCombat` of `count` cards of one type from the
    /// character's combat pool: each `NextItem` off `CombatCardGeneration`,
    /// repeats allowed. All are made, then each goes into hand in turn.
    GenerateForCombat {
        kind: CardKind,
        count: V::Number,
    },
    /// `CardFactory.GetDistinctForCombat` of three colorless cards off
    /// `CombatCardGeneration`, then `CardSelectCmd.FromChooseACardScreen`.
    /// The one chosen goes into the hand through `AddGeneratedCardToCombat`.
    ChooseColorlessIntoHand,
    /// `CardFactory.GetDistinctForCombat` of `count` cards of the
    /// character's pool off `CombatCardGeneration`, each taking `writes`,
    /// then `CardSelectCmd.FromSimpleGrid` for one. The one chosen goes into
    /// the hand through `AddGeneratedCardToCombat`.
    ChooseFromPoolIntoHand {
        count: V::Number,
        writes: &'static [(Change<V>, Lasting)],
    },
    /// `CardPileCmd.AddGeneratedCardsToCombat`: this many new copies of a
    /// card into the player's hand, the way `Shiv.CreateInHand` makes them.
    GenerateInHand {
        card: V::Card,
        count: V::Number,
    },
    /// `ForgeCmd.Forge` from a power hook.
    Forge {
        card: V::Card,
        amount: V::Number,
    },
    /// `ForgeCmd.Forge` from a power hook for each living player but the
    /// power's owner, as `HammerTimePower.AfterForge` forges.
    ForgeOthers {
        card: V::Card,
        amount: V::Number,
    },
    /// `CreatureCmd.GainBlock` on the listener's owner.
    GainBlock(V::Number),
    /// `CreatureCmd.GainBlock(target, amount, Unpowered)` on a creature the
    /// listener names.
    GainBlockOf {
        target: crate::CombatId,
        amount: V::Number,
    },
    /// `CardCmd.Upgrade` over every card in the hand.
    UpgradeHand,
    /// `CardCmd.Upgrade` on a clone of the card the hook names, which takes
    /// its place.
    UpgradeCard,
    /// `CardCmd.Enchant` by `amount` on each deck card of `Basic` rarity
    /// carrying `tag` that the enchantment takes, in deck order.
    EnchantBasicTagged {
        tag: V::CardTag,
        enchantment: V::Enchantment,
        amount: u8,
    },
    /// `CardCmd.Enchant` by `amount` on each deck card the enchantment takes,
    /// in deck order.
    EnchantDeck {
        enchantment: V::Enchantment,
        amount: u8,
    },
    /// `CardCmd.Enchant` by `amount` on a clone of one card of a reward,
    /// which takes its place. The card is `Rng.Niche.NextItem` over those
    /// the enchantment can enchant, and nothing where none can take it.
    EnchantOneRewardCard {
        enchantment: V::Enchantment,
        amount: u8,
    },
    /// `CardCmd.Enchant` by `amount` on a clone of the card the hook names,
    /// which takes its place.
    EnchantCard {
        enchantment: V::Enchantment,
        amount: u8,
    },
    /// Keep this number on the card that asked, as a field of its own model
    /// that it prints nowhere.
    NoteOnSelf(V::Number),
    /// `CardPileCmd.Add(this, PileType.Hand)` for the card that asked, where
    /// it stands in `only_from`, or anywhere but the hand with none named.
    AddSelfToHand {
        only_from: Option<crate::Pile>,
    },
    /// `PowerCmd.Apply<P>(Owner, amount, Owner)`, on the creature the listener
    /// is carried by.
    ApplyToOwner(V::Power, PowerAmount<V::Number>),
    /// `CardModel.BaseReplayCount += amount`.
    ///
    /// The extra times a card's own play runs. The two hooks that hand replays
    /// out reach different cards: one walks `AllCards`, and the other is given
    /// the single card that just arrived.
    AddReplays {
        to: Writing<V>,
        amount: V::Number,
    },
    /// `CardCmd.ApplyKeyword`, which writes a word onto each card named.
    AddKeyword {
        to: Writing<V>,
        keyword: V::CardKeyword,
    },
    TickDownDuration,
    /// `PowerCmd.Decrement(this)`: one off the amount, and the instance goes
    /// once it reaches zero.
    ///
    /// Unlike [`HookEffect::TickDownDuration`], nothing is skipped. A power
    /// that counts down outside a turn's own tick asks for this.
    Decrement,
    /// `PowerCmd.ModifyAmount(this, offset)`, by a signed offset. The instance
    /// goes once it reaches zero.
    ModifyAmount(V::Number),
    /// `PowerModel.SetAmount` on this instance, which no hook hears.
    SetAmount(V::Number),
    GainEnergyAndRemove(V::Number),
    GainBlockAndRemove(V::Number),
    /// `PowerCmd.Remove(this)`: the power takes itself off its owner.
    ///
    /// Its own effect rather than a suffix on another. A power that acts and
    /// then leaves asks for both, in the order it wants them.
    Remove,
    /// `SetMoveImmediate` onto a state the owner's own graph already holds.
    ///
    /// The machine stands on that state, so the graph walks on from it. A
    /// segment sent to its dead move reaches the reattach that follows.
    /// [`HookEffect::InstallMove`] is the other half of the same command, for
    /// the states a power builds rather than names.
    ///
    /// `force` is `forceTransition`, which skips the guard
    /// [`Rules::can_leave_current_move`] stands for.
    EnterMove {
        to: V::MoveId,
        force: bool,
    },
    /// [`HookEffect::EnterMove`] of `to` for the listener's own monster, where
    /// the move it advertises is `from`.
    EnterMoveFrom {
        from: V::MoveId,
        to: V::MoveId,
    },
    ///
    /// A power builds the same state every time. So this and the two moves
    /// below point at one the build keeps. A move is the widest thing an
    /// effect names, and an effect stays as narrow as a pointer.
    InstallMove(&'static crate::NextMove<V>),
    /// `CreatureCmd.SetMaxAndCurrentHp` on the listener's owner.
    SetMaxAndCurrentHp(V::Number),
    /// `CardFactory.CreateForReward` as a custom card reward rolls its cards,
    /// each then added to the deck rather than offered.
    AddRolledToDeck(CustomReward<V>),
    /// This power's `AfterDeath` over its own holder, which
    /// [`Hooks::perform_power_death`] runs against the fight.
    PerformDeath(V::Power),
    /// `CreatureCmd.Stun` on the owner: a stunned turn in place of the move
    /// it advertised.
    ///
    /// `then` is `nextMoveId`, as the index of that state in the owner's
    /// graph. Nothing is the state the stun interrupted.
    StunOwner {
        stun: &'static crate::NextMove<V>,
        then: StunResume,
    },
    /// `CreatureCmd.LoseBlock` on the owner, for all of it.
    LoseBlock,
    /// `OrbCmd.EvokeLast` this many times, each on the last orb then in the
    /// player's queue.
    EvokeLast(V::Number),
    /// `CreatureCmd.LoseBlock` on the owner, for this much of it.
    LoseSomeBlock(V::Number),
    /// A card the fight took from the deck, handed back as a reward once the
    /// fight is won. `SwipePower.BeforeDeath`'s `SpecialCardReward`.
    ReturnCard(crate::DeckCardId),
    /// [`Hooks::write_deck_card`] on this card of the deck, which the run
    /// carries out as the decision settles.
    WriteDeckCard {
        card: crate::DeckCardId,
        write: V::DeckWrite,
    },
    /// Remove another power from the same owner.
    RemoveFromOwner(V::Power),
    /// `CreatureCmd.Add<M>` once per count, in the dead owner's place.
    SummonAfterOwnerDeath {
        monster: V::Monster,
        count: V::Number,
        opening: &'static crate::NextMove<V>,
    },
    /// `CardCmd.TransformTo`, on the deck card asking: it becomes a new
    /// card of this kind.
    TransformTo(V::Card),
    /// `RewardsCmd.OfferCustom` of these rewards and the `ExtraRewards` of
    /// the fight the event resumes after, once there is any. An event that
    /// resumes asks it.
    OfferRewardsWithRoomExtras(&'static [CustomReward<V>]),
    /// `CardCmd.Enchant` by `amount` on `count` deck cards the enchantment
    /// takes. `TakeRandom` off `stream` picks them: it shuffles those cards,
    /// in deck order, unstably, and takes the first.
    EnchantRandomDeckCards {
        enchantment: V::Enchantment,
        amount: u8,
        count: usize,
        stream: Stream<V>,
    },
    /// `ModifyGeneratedMap` putting a `SpoilsActMap` in place of the act's
    /// own: the paths narrow to one treasure seven rows from the top, off the
    /// `spoils_map` stream.
    SpoilsMap,
    /// `ModifyGeneratedMapLate` setting the Spoils Map's `SpoilsCoord` to the
    /// map's first Treasure point, where its quest pays `gold`.
    SpoilsQuest {
        card: V::Card,
        gold: V::Number,
    },
    /// `ModifyUnknownMapPointRoomTypes` leaving an unknown room nothing but
    /// an event.
    EventRoomsOnly,
    /// `CardFactory.CreateForReward` for `count` cards of the colorless pool
    /// at `RegularEncounter` odds, the upgrade roll included. A
    /// `FromChooseACardScreen` then takes one or none into the deck.
    ChooseColorlessCards {
        count: usize,
    },
    /// `CardFactory.CreateForReward` for `count` cards of the player's
    /// character pool and the colorless pool, at `RegularEncounter` odds, the
    /// upgrade roll included. Only the cards a run of several players alone
    /// deals are drawn. `colorless_first` lists the colorless pool first. A
    /// `FromChooseACardScreen` then takes one or none into the deck.
    ChooseMultiplayerCards {
        count: usize,
        colorless_first: bool,
    },
    /// A potion thrown at the event's merchant, which
    /// [`crate::EventBehavior::potion_thrown`] carries out. It is the whole
    /// of the potion's use.
    PotionThrownAtEvent(V::Potion),
    /// `CreateCloneForPlayer(Owner.Player)` of the card about to be played,
    /// as it stands, kept outside every pile by the power that asked.
    /// [`Hooks::power_kept_card`] gives that power the clone's id.
    KeepCloneOfPlaying,
    /// `PowerCmd.Decrement(this)`, then `CardCmd.AutoPlay` of the clone the
    /// power keeps as `kept`, with no target named. The play pile takes the
    /// clone first, since it stands in no pile.
    DecrementAndAutoPlayKept(crate::CardId<V>),
}

/// One build's relic models, which is what a run holds a relic as.
///
/// `RelicModel` is an instance per relic held, and this is the type of that
/// instance. The two things a run asks one without giving it a hook: build it,
/// and read the number it shows.
/// What a run modifier's model saves.
pub trait ModifierModel<V: Hooks>: Clone + std::fmt::Debug + PartialEq + Eq + Sized {
    /// The model as the run is started with it.
    fn new(modifier: V::Modifier) -> Self;
}

pub trait RelicModel<V: Hooks>: Clone + std::fmt::Debug + PartialEq + Eq + Sized {
    /// The model as `RelicCmd.Obtain` builds it.
    ///
    /// # Panics
    ///
    /// A relic nobody has ported. A run carrying one skips it at every later
    /// hook, which is a divergence with no name on it.
    fn new(relic: V::Relic) -> Self;

    /// `RelicModel.DisplayAmount`, which is the number the relic shows.
    /// `fighting` is `CombatManager.IsInProgress`, which a fight's end and a
    /// player's death both clear. `turn` is its owner's
    /// `PlayerCombatState?.TurnNumber`, which outlives a fight until the
    /// next one starts, and is nothing before the first.
    ///
    /// # Panics
    ///
    /// A relic that works its number out and nobody has ported.
    fn display_amount(&self, relic: V::Relic, fighting: bool, turn: Option<V::Count>) -> V::Number;

    /// `RelicModel.IsUsedUp`.
    ///
    /// # Panics
    ///
    /// A relic that works it out and nobody has ported.
    fn used_up(&self, relic: V::Relic) -> bool;

    /// The deck cards the relic keeps off the deck, which Pael's Tooth's
    /// `SerializableCards` are. Nothing for any other relic.
    fn held_cards(&self) -> &[DeckCard<V>] {
        &[]
    }

    /// `cards` added to the ones the relic keeps, in order.
    ///
    /// # Panics
    ///
    /// A relic that keeps no cards.
    fn hold_cards(&mut self, _cards: &[DeckCard<V>]) {
        unimplemented!("a relic keeping deck cards it has no place for");
    }

    /// The map points the relic marked, which Fur Coat's `FurCoatCoordCols`
    /// and `FurCoatCoordRows` are. Nothing for any other relic.
    fn marked_points(&self) -> &[crate::Coord] {
        &[]
    }

    /// The map points the relic marks, in place of any it marked before.
    ///
    /// # Panics
    ///
    /// A relic that marks no points.
    fn mark_points(&mut self, _points: &[crate::Coord]) {
        unimplemented!("a relic marking map points it has no place for");
    }

    /// The card the relic keeps at `at`, handed back and gone from it.
    ///
    /// # Panics
    ///
    /// A relic that keeps no cards.
    fn release_card(&mut self, _at: usize) -> DeckCard<V> {
        unimplemented!("a relic handing back a card it cannot keep");
    }
}

/// One deck card's own model, as the run's deck holds one.
///
/// A card saves a few fields between fights, like Guilty's `CombatsSeen`. No
/// observation shows them, so they sit beside a card's identity rather than
/// in it.
pub trait CardModel<V: ContentVersion>:
    Copy + std::fmt::Debug + PartialEq + Eq + std::hash::Hash + Sized
{
    /// The model as a card joins the deck, with nothing saved yet.
    fn new(card: V::Card) -> Self;

    /// What its saved fields write onto the copy a fight makes of it, where
    /// they write something. A copy carries its saved fields, and their
    /// setters write numbers the card prints.
    fn written(&self) -> Option<Change<V>>;
}

/// The fields one fight copy's own class adds to `CardModel`, where its
/// `AfterDowngraded` reads them.
///
/// A copy carries them through a downgrade, which prints its numbers afresh
/// and then has the class write back what they say.
pub trait CardFields<V: Hooks>:
    Copy + std::fmt::Debug + PartialEq + Eq + std::hash::Hash + 'static
{
    /// The fields of a fresh copy of `card`.
    fn new(card: V::Card) -> Self;

    /// `write` on these fields, and what it writes on the numbers the copy
    /// prints, which `printed` holds as they stand.
    ///
    /// # Panics
    ///
    /// A write the class has no method for.
    fn write<P: Printed<V> + ?Sized>(&mut self, write: V::CardWrite, printed: &P) -> Change<V>;

    /// `SovereignBlade.AddDamage`, which `ForgeCmd.Forge` calls on every
    /// blade.
    fn add_damage<P: Printed<V> + ?Sized>(&mut self, amount: V::Number, printed: &P) -> Change<V>;

    /// `CardModel.AfterDowngraded`: what the class writes back on a copy
    /// whose downgrade printed its numbers afresh, which `printed` holds. No
    /// write is a class that does not override it.
    fn after_downgraded<P: Printed<V> + ?Sized>(&self, printed: &P) -> Option<Change<V>>;
}

/// A card in the run's deck: which one, what it is, and what its own model
/// saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeckCard<V: Hooks> {
    pub id: crate::DeckCardId,
    pub ident: crate::CardIdent<V>,
    pub state: V::CardState,
}

impl<V: Hooks> DeckCard<V> {
    /// `ident` as it joins the deck as `id`, with nothing saved yet.
    pub fn new(id: crate::DeckCardId, ident: crate::CardIdent<V>) -> Self {
        Self {
            id,
            ident,
            state: <V::CardState as CardModel<V>>::new(ident.card),
        }
    }
}

/// What one monster's own model saves between its moves.
///
/// `EncounterModel`'s own fields, which the fight writes and the gold
/// proportion reads.
pub trait EncounterModel<V: ContentVersion>:
    Copy + std::fmt::Debug + PartialEq + Eq + Sized
{
    /// The model as the fight builds the encounter.
    fn new(encounter: V::Encounter) -> Self;
}

/// `MonsterModel`'s fields, which a move writes and a later move or a
/// conditional state reads. The fight keeps one beside the monster's move
/// graph.
pub trait MonsterModel<V: ContentVersion>: Copy + std::fmt::Debug + PartialEq + Eq + Sized {
    /// The model as a monster enters a fight.
    fn new(monster: V::Monster) -> Self;

    /// The model a power builds for a monster it adds, with `written` on the
    /// one field the power writes. `StockPower` writes `StockAmount`.
    fn written(monster: V::Monster, written: V::Number) -> Self;
}

/// One power's own model, as the creature carrying it holds one.
///
/// `PowerModel`'s subclasses read their own fields off `this`, the way a
/// relic's do. The stack is the amount the creature carries, and everything
/// beside it a power saves for itself is here.
pub trait PowerModel<V: ContentVersion>: Copy + std::fmt::Debug + PartialEq + Eq + Sized {
    /// The model as `PowerCmd.Apply` builds one.
    ///
    /// Every power reaches this, because every power that lands is an
    /// instance. A power that saves nothing still has a model.
    fn new(power: V::Power) -> Self;

    /// Which power this is the model of.
    fn power(&self) -> V::Power;

    /// Whether `PowerModel.Type` is `Debuff`, which a power whose table cell
    /// is computed works out for itself.
    ///
    /// # Panics
    ///
    /// A computed type nobody has ported.
    fn debuff(&self) -> bool;

    /// Whether the effects this power's own hook asked for are being carried
    /// out, which the fight sets around them. A power that guards against
    /// hearing itself, as Beacon of Hope's `HasAlreadyBeenGivenBlock` does,
    /// keeps it. Any other ignores it.
    fn mark_busy(&mut self, _busy: bool) {}
}

/// What a card reads off its owner's side of the fight.
#[derive(Debug, Clone, Copy)]
pub struct CardOwner<'a, V: ContentVersion> {
    /// The player's own powers.
    pub powers: &'a [crate::Stack<V>],
    /// `Owner.Osty.CurrentHp`, while Osty is alive.
    pub osty: Option<V::Health>,
    /// `Owner.Osty.MaxHp`, while Osty is alive.
    pub osty_max_hp: Option<V::Health>,
    /// How many cards the owner's draw pile holds.
    pub draw: usize,
    /// How many cards the owner's discard pile holds.
    pub discard: usize,
    /// The cards in the owner's exhaust pile.
    pub exhaust: &'a [V::Card],
    /// How many cards the owner's hand holds besides this one.
    pub hand_besides: usize,
    /// The powers of every living enemy, one creature's after another's.
    pub enemy_powers: &'a [crate::Stack<V>],
    /// How many of the owner's cards in the fight, every pile included, are
    /// tagged Strike. `PlayerCombatState.AllCards`.
    pub strikes: usize,
    /// The owner's creature.
    pub player: crate::CombatId,
    /// The creature the card is pointed at, where it is pointed at one.
    pub targeted: Option<crate::CombatId>,
    /// The powers of the creature the card is pointed at, where it is pointed
    /// at one.
    pub target: Option<&'a [crate::Stack<V>]>,
    /// `CombatManager.History`.
    pub history: History<'a, V>,
    /// `Owner.Creature.Block`.
    pub block: V::Number,
    /// `EnergyCost.GetWithModifiers(CostModifiers.All)`, while the card
    /// stands in the play pile, which it does while it is played.
    pub cost_in_play: Option<crate::Cost>,
    /// How many of the owner's cards in the fight are statuses, the exhaust
    /// pile left out.
    pub statuses: usize,
    /// `PlayerCombatState.OrbQueue.Orbs`, in queue order.
    pub orbs: &'a [crate::Channelled<V>],
    /// The cards in the owner's hand, in hand order.
    pub hand: &'a [V::Card],
    /// How many of the owner's cards in the fight, every pile included,
    /// print a star cost, which is `CanonicalStarCost >= 0 || HasStarCostX`.
    pub star_cards: usize,
    /// How many of the owner's cards in the fight, every pile included, are
    /// tagged `OstyAttack`, this one left out.
    pub osty_attacks_besides: usize,
}

/// What one copy of a card prints, as a rule reads it.
///
/// `CardModel` saves its fields in the numbers on its own face. A card that
/// raised its own damage as it was drawn has raised what this answers.
///
/// One method, because a rule asks for one number by name and never for the
/// set. The table already says which numbers a card prints.
pub trait Printed<V: ContentVersion> {
    /// The number this copy prints for `key`.
    ///
    /// # Panics
    ///
    /// A key the card does not print. That is a mistake in the port, the same
    /// way a card asking for a number no card prints is.
    fn face(&self, key: V::CardVar) -> V::Number;
}

/// One model instance a hook is delivered to, with what that instance saves.
///
/// `AbstractModel`'s subclasses read their own fields off `this`. A power's
/// are the stack the creature carries it as, a relic's are the model the run
/// carries. The hook carries what the fight knows, and this carries the rest.
///
/// A kind of model gets a variant when something asks it a hook.
pub enum Listener<'a, V: Hooks, P: Printed<V> + ?Sized> {
    Power {
        power: V::Power,
        amount: PowerAmount<V::Number>,
        /// The side its owner is on, which is `Owner.Creature.Side`. A hook
        /// naming a side asks the instance whether it is one of them.
        owner: V::CombatSide,
        /// The creature carrying it, which a hook naming a creature compares
        /// with.
        holder: crate::CombatId,
        /// `PowerModel.Applier`, the creature whose command put it on.
        applier: Option<crate::CombatId>,
        /// `PowerModel.Target`, the creature a monster aimed it at.
        target: Option<crate::CombatId>,
        /// `Owner.Monster`, which is nothing for a power on the player.
        monster: Option<V::Monster>,
        /// `PowerModel.AmountOnTurnStart`: the stack as its holder's side
        /// last began a turn, or zero before one has.
        on_turn_start: PowerAmount<V::Number>,
        /// What this instance saves for itself, which is every field of its
        /// own model. A power writes here rather than ask the fight to.
        state: &'a V::PowerState,
        /// What the fight has done so far, which a power reads rather than
        /// counts for itself.
        history: History<'a, V>,
        /// The fight's creatures, for a power that asks about one other than
        /// its own.
        fight: &'a dyn FightView<V>,
    },
    /// A potion in the belt, which the fight asks after the relics. `owner`
    /// is the creature of the player whose belt holds it.
    Potion {
        potion: V::Potion,
        owner: crate::CombatId,
    },
    /// A modifier the run was started with. A fight asks the modifiers last,
    /// after every creature's listeners.
    Modifier(V::Modifier),
    Relic {
        state: &'a V::RelicState,
        /// `Owner`, as its creature, which a hook naming a player compares
        /// with.
        owner: crate::CombatId,
        /// The fight's creatures, for a relic that asks about one. Nothing
        /// outside a fight.
        fight: Option<&'a dyn FightView<V>>,
        /// `Owner.Relics`: the belt it sits on, itself among them.
        belt: &'a [V::Relic],
    },
    /// A monster's own model, which the fight asks after the powers on its
    /// creature.
    Monster {
        monster: V::Monster,
        creature: crate::CombatId,
    },
    /// A card in the fight, and what this copy of it prints.
    Card {
        card: V::Card,
        /// Which copy this is.
        id: crate::CardId<V>,
        /// `Owner.Creature`, the player whose card it is.
        owner: crate::CombatId,
        printed: &'a P,
        /// `Pile.Type`, and nothing for the play pile.
        pile: Option<crate::Pile>,
        /// `CombatManager.History`.
        history: History<'a, V>,
    },
}

impl<V: Hooks, P: Printed<V> + ?Sized> Clone for Listener<'_, V, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Hooks, P: Printed<V> + ?Sized> Copy for Listener<'_, V, P> {}

/// A power as an event hook reaches it.
pub struct EventPower<'a, V: Hooks> {
    pub power: V::Power,
    pub amount: PowerAmount<V::Number>,
    pub owner: V::CombatSide,
    /// The creature carrying it, which a hook naming a creature compares with.
    pub holder: crate::CombatId,
    /// `PowerModel.Applier`, the creature whose command put it on.
    pub applier: Option<crate::CombatId>,
    /// `PowerModel.Target`, the creature a monster aimed it at.
    pub target: Option<crate::CombatId>,
    /// `Owner.Monster`, which is nothing for a power on the player.
    pub monster: Option<V::Monster>,
    /// `PowerModel.AmountOnTurnStart`.
    pub on_turn_start: PowerAmount<V::Number>,
    pub state: &'a mut V::PowerState,
    /// What the fight has done so far.
    pub history: History<'a, V>,
    /// The fight's creatures, for a power that asks about one other than its
    /// own.
    pub fight: &'a dyn FightView<V>,
}

/// The build-specific behavior behind the game's hook dispatcher.
///
/// The enums begin with only the hook shapes the simulator can execute. They
/// grow as behavior reaches another hook, without widening this trait.
///
/// An event hook has an answer for each kind of listener. A walk knows which
/// kind it is asking, so each has a method of its own.
pub trait Hooks: ContentVersion {
    /// What one relic's own model saves, which the run carries for it.
    type RelicState: RelicModel<Self>;

    /// What one run modifier's own model saves, which the run carries for
    /// it.
    type ModifierState: ModifierModel<Self>;

    /// What one power's own model saves, which the creature carries for it.
    type PowerState: PowerModel<Self>;

    /// What one monster's own model saves, which the fight carries beside
    /// its move graph.
    type MonsterState: MonsterModel<Self>;

    /// What the encounter's own model saves while the fight lasts.
    type EncounterState: EncounterModel<Self>;

    /// What one deck card's own model saves, its `[SavedProperty]` fields,
    /// which the run carries for it between fights.
    type CardState: CardModel<Self>;

    /// The fields one fight copy's own class adds to `CardModel`.
    type CardFields: CardFields<Self>;

    /// What a card's note does to the model of a power it just applied,
    /// which [`Fight::note_on_power`] hands it.
    fn power_noted(state: &mut Self::PowerState, note: Self::Number);

    /// Where a power's model keeps the clone `kept` of a card, which
    /// [`Fight::keep_card_on_power`] hands it.
    fn power_kept_card(state: &mut Self::PowerState, kept: crate::CardId<Self>);

    /// `PowerModel.GetScaledAmountForMultiplayer`: the amount of `power`
    /// that lands on an enemy in a fight of `players`, where `factor` is the
    /// act's. `None` for a power whose `ShouldScaleInMultiplayer` is false,
    /// which the game does not ask.
    fn power_scaled_for_multiplayer(
        power: Self::Power,
        amount: crate::Decimal,
        players: usize,
        factor: crate::Decimal,
    ) -> Option<crate::Decimal>;

    fn modify_hook<P: Printed<Self> + ?Sized>(
        listener: Listener<'_, Self, P>,
        hook: ModifyHook<Self>,
    ) -> Modify<Self::Number>;

    fn decide_hook<P: Printed<Self> + ?Sized>(
        listener: Listener<'_, Self, P>,
        hook: DecisionHook<Self>,
    ) -> crate::HookResponse<bool>;

    /// `EnchantDamageAdditive` and its kin, answered by the enchantment on a
    /// card, which is in `state`. [`Modify::Nothing`] for one whose class
    /// does not override the hook.
    fn enchant_modify(
        enchanted: crate::Enchanted<Self>,
        state: &Self::EnchantmentState,
        hook: EnchantHook,
    ) -> Modify<Self::Number>;

    /// What an enchantment asks for when `hook` reaches the card `id` it is
    /// on, pushed onto `asked`. Asked only of an enchantment whose class
    /// overrides the hook.
    fn enchantment_event_hook(
        enchanted: crate::Enchanted<Self>,
        id: crate::CardId<Self>,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// [`Hooks::power_event_hook`] for `affliction` on the copy `id`, pushed
    /// onto `asked`. Asked only of an affliction whose class overrides the
    /// hook.
    fn affliction_event_hook(
        affliction: Self::Affliction,
        id: crate::CardId<Self>,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// `TryModifyKeywordsInCombat` of the power that afflicts cards with
    /// `affliction`: the keyword it gives each card that carries it. Nothing
    /// for an affliction whose power gives none.
    ///
    /// `HexPower` gives Hexed cards Ethereal, and takes Hexed off every card
    /// as it goes. So a card carries the keyword exactly while it carries the
    /// affliction.
    fn keyword_while_afflicted(affliction: Self::Affliction) -> Option<Self::CardKeyword>;

    /// Whether `card` carries `Eternal`: its table prints it, or its
    /// enchantment's `OnEnchant` adds it. No removal takes an eternal card,
    /// and no transform takes it from the deck.
    fn card_eternal(card: &crate::CardIdent<Self>) -> bool;

    /// What an enchantment's `OnEnchant` writes on the card it goes on, which
    /// every copy of that card carries from then on. Nothing for an
    /// enchantment whose `OnEnchant` writes nothing.
    fn enchanted_changes(enchantment: Self::Enchantment) -> &'static [Change<Self>];

    /// `CardModel.GetResultPileTypeForCardPlay`, or v0.111.0's
    /// `GetResultLocationForCardPlay`, for a card whose class overrides it:
    /// the pile the card goes to once its play is over, and where in it,
    /// given what the base class answers.
    fn card_result_pile(
        card: Self::Card,
        base: (Option<crate::Pile>, PilePosition),
    ) -> crate::HookResponse<(Option<crate::Pile>, PilePosition)>;

    /// `write` carried out on the deck card a fight's copy came from.
    fn write_deck_card(card: &mut DeckCard<Self>, write: Self::DeckWrite);

    /// `CardModel.IsPlayable`, for a card whose class overrides it: whether
    /// its own logic lets it be played in the fight `fight` shows.
    fn card_is_playable(card: Self::Card, fight: &impl CardLogic) -> crate::HookResponse<bool>;

    /// `ModifyCardPlayResultPileTypeAndPosition`: where a played card goes
    /// once its play is over, given where it goes so far.
    fn result_pile<P: Printed<Self> + ?Sized>(
        listener: Listener<'_, Self, P>,
        hook: ResultPile<Self>,
    ) -> crate::HookResponse<(Option<crate::Pile>, PilePosition)>;

    /// What a power asks for when `hook` reaches it, pushed onto `asked` in
    /// the order it asks. The other kinds of listener have their own below.
    ///
    /// Onto a list the caller owns, because a hook reaches every listener in
    /// the fight and most answer nothing. An answer of nothing then costs
    /// nothing.
    fn power_event_hook(
        power: EventPower<'_, Self>,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// [`Hooks::power_event_hook`] for a card in the run's deck, by what its
    /// model saves. `RunState.IterateHookListeners` walks the deck's cards
    /// ahead of the fight's.
    fn deck_card_event_hook(
        card: Self::Card,
        state: &mut Self::CardState,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// [`Hooks::power_event_hook`] for a run modifier, with what it was set
    /// up with and what its model saves.
    fn modifier_event_hook(
        modifier: crate::RunModifier<Self>,
        state: &mut Self::ModifierState,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// Whether this build ports every rule of `modifier`. Some rules run
    /// from no hook the fight or the run raises, such as `AfterRunCreated`.
    /// So a run started with a modifier this says no to panics as it begins.
    fn modifier_ported(modifier: Self::Modifier) -> bool;

    /// [`Hooks::power_event_hook`] for a relic, by what its model saves.
    /// `belt` is `Owner.Relics`, itself among them.
    fn relic_event_hook(
        state: &mut Self::RelicState,
        belt: &[Self::Relic],
        owner: RelicOwner<Self>,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// [`Hooks::power_event_hook`] for a potion in the belt, which saves
    /// nothing of its own.
    fn potion_event_hook(
        potion: Self::Potion,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// [`Hooks::power_event_hook`] for a monster's own model, on `creature`.
    fn monster_event_hook(
        monster: Self::Monster,
        model: &mut Self::MonsterState,
        creature: crate::CombatId,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );

    /// [`Hooks::power_event_hook`] for the copy `id` of `card`, printed as
    /// `printed`.
    ///
    /// `history` is the fight's, which a card reads the way a power does.
    /// `pile` is where the copy stands, and nothing in the play pile.
    /// `owner` is `Owner.Creature`, the player whose card it is.
    #[allow(clippy::too_many_arguments)]
    fn card_event_hook<P: Printed<Self> + ?Sized>(
        card: Self::Card,
        id: crate::CardId<Self>,
        owner: crate::CombatId,
        pile: Option<crate::Pile>,
        printed: &P,
        history: History<'_, Self>,
        hook: EventHook<Self>,
        asked: &mut HookEffects<Self>,
    );
}

/// What a power does when one side's turn ends, which is its
/// `AfterSideTurnEnd`.
///
/// Most powers have no override: two hundred of this build's two hundred and
/// sixty. What the other sixty do varies, from `Decrement` and `Remove` to
/// damage, healing and a draw.
///
/// So this names what the ported ones do. Three powers tick a duration down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideTurnEnd {
    /// No `AfterSideTurnEnd`, or one that does not act at this side's end.
    Nothing,
    /// `PowerCmd.TickDownDuration`: one off the amount, and the instance goes
    /// once it reaches zero.
    ///
    /// Unlike `Decrement`, it passes over an instance whose
    /// `SkipNextDurationTick` is set, and clears the flag instead.
    TicksDownDuration,
}

/// What an encounter fixed about a monster when it spawned one.
///
/// An encounter builds identical monsters and flags one. `NibbitsNormal` marks
/// the front Nibbit, `InkletsNormal` the middle Inklet. A graph branches on
/// that flag, and no table holds it.
///
/// The position is where the monster stood when the roster was built, which a
/// death later makes ambiguous in the living list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonsterSpawn<E> {
    pub encounter: E,
    /// Where it stood in the roster, counting from the front.
    pub position: nonmax::NonMaxU8,
    /// How many the encounter built. Never zero: a fight has a monster in it.
    pub roster_size: nonmax::NonMaxU8,
    /// `GetTeammatesOf(Creature).Count(IsAlive)`: how many on its side stand
    /// as it rolls, itself included.
    pub standing: u8,
    /// `Creature.CurrentHp` as it rolls.
    pub hp: u32,
    /// `Creature.MaxHp` as it rolls.
    pub max_hp: u32,
    /// Whether `Encounter.GetNextSlot` names a slot as it rolls: one of
    /// `Encounter.Slots` that no enemy in the fight holds.
    pub next_slot: bool,
}

/// One creature an encounter's roster names, as `GenerateMonsters` returns it.
///
/// The game builds a mutable monster model per slot and writes onto it what
/// that roster decided. Every field written this way is read in one of two
/// places. One is a `ConditionalBranchState` the graph asks mid-fight, which
/// [`MonsterSpawn`] answers. The other is the state the machine is constructed
/// on, which is this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spawned<V: ContentVersion> {
    pub monster: V::Monster,
    /// `GenerateMoveStateMachine`'s initial state, when the roster names one.
    ///
    /// `None` is the monster's own table entry, which is what a roster that
    /// writes nothing leaves the machine to open on.
    pub opens_on: Option<V::MoveId>,
    /// `SlotName`, as its place in `Encounter.Slots`, where the roster names
    /// one.
    pub slot: Option<u8>,
    /// The number the roster writes onto the monster's model, which
    /// [`MonsterModel::written`] builds the model with.
    pub written: Option<V::Number>,
}

impl<V: ContentVersion> Spawned<V> {
    /// A monster whose roster wrote nothing onto it.
    pub fn new(monster: V::Monster) -> Self {
        Self {
            monster,
            opens_on: None,
            slot: None,
            written: None,
        }
    }

    /// The same, opening on the move the roster named.
    pub fn opening_on(monster: V::Monster, opens_on: V::MoveId) -> Self {
        Self {
            monster,
            opens_on: Some(opens_on),
            slot: None,
            written: None,
        }
    }
}

/// Where a monster's move graph stands.
///
/// `MonsterMoveStateMachine`, which no observation reports: an observation
/// says what a monster does next, not how its graph arrived there. Two runs
/// that agree on every intent so far can still part on the next one.
///
/// The numbers index the build's own state list, so only that build's walk
/// reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveMachine {
    /// The state the machine stands on.
    pub at: u8,
    /// `_performedAtLeastOnce` on that state, which is what lets the machine
    /// leave a move the graph pins it to.
    pub performed: bool,
    /// `_performedFirstMove` on the machine. The machine stays on the move it
    /// started on until this is set.
    pub ever_performed: bool,
    /// `StateLog`: every move state entered, in order. A random branch's
    /// repeat rules read it.
    pub log: MoveLog,
    /// Whether the machine stands on a state a caller built.
    ///
    /// `Creature.Stun` and `IllusionPower` both build a `MoveState` rather
    /// than name one, and hand it to `SetMoveImmediate`. Each sets
    /// `MustPerformOnceBeforeTransitioning`, and each points `FollowUpStateId`
    /// at the last state in the log. That state is [`MoveMachine::at`]. So a
    /// built state needs no index of its own, and `at` is where it goes.
    pub built: bool,
}

/// `StateLog`, as the three questions a repeat rule asks of it.
///
/// A rule asks whether a state was ever entered, what the last few entries
/// were, and how many entries there are. So this keeps the set of states
/// entered, the latest [`RECENT`] entries, and the count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveLog {
    /// One bit per state index.
    entered: u64,
    /// The latest entries, the newest at `count % RECENT` less one.
    recent: [u8; RECENT],
    count: u32,
}

/// How far back a repeat rule reads. A build's `RepeatCount` stops at eight.
pub const RECENT: usize = 8;

impl MoveLog {
    /// # Panics
    ///
    /// A state index past what the set holds.
    pub fn push(&mut self, at: u8) {
        if at >= 64 {
            unimplemented!("a move state at index {at}, past what a log holds");
        }
        self.entered |= 1 << at;
        self.recent[self.count as usize % RECENT] = at;
        self.count += 1;
    }

    /// Whether the machine has ever entered `at`.
    pub fn contains(&self, at: u8) -> bool {
        at < 64 && self.entered & (1 << at) != 0
    }

    /// How many entries the log has.
    pub fn len(&self) -> usize {
        self.count as usize
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The latest `n` entries, newest first.
    ///
    /// # Panics
    ///
    /// More than [`RECENT`].
    pub fn recent(&self, n: usize) -> impl Iterator<Item = u8> + '_ {
        assert!(n <= RECENT, "a log reads back {RECENT} entries, not {n}");
        let count = self.count as usize;
        (1..=n.min(count)).map(move |back| self.recent[(count - back) % RECENT])
    }
}

impl MoveMachine {
    /// Where a monster's graph starts, which is `initial` in its table.
    ///
    /// `MonsterMoveStateMachine` records the state it starts on. A monster's
    /// opening move is therefore in its history when the first branch reads
    /// it. `logged` is that state's `ShouldAppearInLogs`. Branch states answer
    /// false.
    pub fn new(at: u8, logged: bool) -> Self {
        let mut log = MoveLog::default();
        if logged {
            log.push(at);
        }
        Self {
            at,
            log,
            ..Self::default()
        }
    }

    /// Go to a state, which is `SetCurrentState`.
    ///
    /// Leaving a state clears its own performed flag. So a move the graph
    /// pins until it is performed is pinned again next time round.
    pub fn enter(&mut self, at: u8) {
        self.at = at;
        self.performed = false;
    }

    /// `PerformMove` and `OnMovePerformed`, which happen together.
    pub fn performed(&mut self) {
        self.performed = true;
        self.ever_performed = true;
    }

    /// `SetMoveImmediate` onto a state the caller built.
    ///
    /// The machine leaves the state it stands on without logging the new
    /// one. `at` stays where it is, because that is what the built state
    /// follows on to.
    pub fn interpose(&mut self) {
        self.built = true;
        self.performed = false;
    }

    /// [`MoveMachine::interpose`], for a built state whose `FollowUpStateId`
    /// names `follow_up` rather than the state it interrupted.
    pub fn interpose_to(&mut self, follow_up: u8) {
        self.interpose();
        self.at = follow_up;
    }

    /// Leave a built state for the one it follows on to.
    ///
    /// `FindNextMoveState` enters that state and writes it into the log, so a
    /// move a monster is stunned out of is logged twice.
    pub fn resume(&mut self) {
        self.built = false;
        let at = self.at;
        self.enter(at);
        self.log.push(at);
    }
}

/// One game build's content, as the types that name it.
///
/// The seam: name the build once, as a type parameter, and every id follows
/// from it. [`ContentVersion::VERSION`] is what makes that more than typing,
/// because a client can check it against the build it is talking to.
///
/// The crate README says why the bounds and supertraits are spelled out.
pub trait ContentVersion: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static {
    /// The build these types were generated from.
    const VERSION: Version;

    /// This build's `StringHelper.GetDeterministicHashCode`.
    ///
    /// The number a name contributes to the seed of the stream it names.
    /// `v0.107.1` folds UTF-16 code units in wrapping `i32` and keeps the low
    /// 32 bits. `v0.111.0` takes `XxHash64` over the UTF-8 bytes and keeps all
    /// 64. A run seeded by one lands on streams the other never reaches.
    fn name_hash(name: &str) -> u64;

    /// The seed this build's `Rng` takes, from the sum of its parts.
    ///
    /// `Rng`'s seed is `uint` on `v0.107.1`, so a sum wider than 32 bits
    /// truncates before it seeds anything. It is `ulong` on `v0.111.0`, where
    /// nothing truncates. Both add first and narrow after, which is what this
    /// is applied to.
    fn seed_from(parts: u64) -> u64;

    type EventOptionId: crate::EventOptionKinds<Self>;

    /// An option an event's page names, as this build generates them.
    type EventOption: Copy + Eq + std::hash::Hash + std::fmt::Debug + 'static;

    /// A signed amount in this build's live game state.
    type Number: num_traits::PrimInt
        + num_traits::Signed
        + From<Self::Health>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::fmt::Debug
        + std::fmt::Display
        + std::hash::Hash;

    /// Hit points carried by players and creatures in this build.
    type Health: crate::rng::RngInteger
        + From<Self::Number>
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::fmt::Debug
        + std::fmt::Display
        + std::hash::Hash;

    /// A nonnegative quantity in this build's live game state.
    type Count: num_traits::PrimInt
        + num_traits::Unsigned
        + serde::Serialize
        + serde::de::DeserializeOwned
        + std::fmt::Debug
        + std::fmt::Display
        + std::hash::Hash;

    /// The number a fight names one of its cards by.
    ///
    /// Wider than a pile is deep. A fight mints one per card it holds, and one
    /// more for every card it makes. It reuses none of them. So this counts
    /// every card a combat ever had, not the ones it holds at once.
    type CardSerial: num_traits::PrimInt
        + num_traits::Unsigned
        // A fight that runs out of numbers has to say so rather than hand out
        // one it already gave away.
        + num_traits::CheckedAdd
        + std::fmt::Debug
        + std::fmt::Display
        + std::hash::Hash;

    /// A method of a card's own class, which writes the fields the class adds.
    ///
    /// One per method, and nothing for a build whose cards keep no field.
    type CardWrite: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static;

    /// What a fight's card writes on its deck version, `CardModel.DeckVersion`.
    ///
    /// One per kind of write, and nothing for a build whose cards write none.
    type DeckWrite: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static;

    /// Where a card's own play stopped to ask something.
    ///
    /// One per card that stops, and nothing for a build whose cards never do.
    /// A card names its own stopping points, so no card can be resumed at
    /// another card's.
    ///
    /// For a fight of kind `K`: a stopped play holds the rest of itself, and
    /// that rest reaches the fight through `K`'s type.
    type Resume<K: FightKind<Self>>: std::fmt::Debug + Clone + Eq + std::hash::Hash + 'static;

    /// Where a card sits in one of a fight's piles.
    ///
    /// Named here so the repository picks the width once. A pile is as large
    /// as the deck. The cards that copy a deck take one into the thousands, so
    /// this is not the byte an id is.
    ///
    /// Not [`ContentVersion::Count`]. A count is a quantity a card prints.
    type Index: num_traits::PrimInt
        + num_traits::Unsigned
        + std::fmt::Debug
        + std::fmt::Display
        + std::hash::Hash;

    /// One rung of the ascension ladder.
    ///
    /// Version-dependent in a way that is easy to miss. Builds move what a
    /// level means, so the same number is a different rule across two of them.
    type AscensionLevel: AscensionKinds
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Send
        + Sync;

    /// Where in a run a room sits.
    type Act: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + ActTable
        + 'static;

    /// A debuff attached to one card.
    type Affliction: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + AfflictionTable
        + 'static;

    /// Every card, across all twelve pools.
    ///
    /// The pool-wrapped `Cards`, which is `#[serde(untagged)]`, so it
    /// still spells `STRIKE_IRONCLAD` on the wire.
    type Card: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + CardTable<Target = Self::TargetType, Var = Self::CardVar, Tag = Self::CardTag>
        + 'static;

    /// What a relic or card reads a card as, such as a Strike.
    type CardTag: CardTagKinds + std::hash::Hash + 'static;

    /// A word a card carries, which a rule can read and an upgrade can add.
    type CardKeyword: KeywordKinds<Self>
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// The words one card carries, as the set the build generates for them.
    ///
    /// A set rather than a list, because that is what a card holds: a word
    /// already on one is not added twice. The build spells it as a bitfield,
    /// which is what makes a copy cheap to carry.
    type CardKeywordSet: FlagSet<Member = Self::CardKeyword> + PartialEq + Eq + 'static;

    /// One of the numbers this build's cards print, by the game's key for it.
    type CardVar: CardVars<Power = Self::Power>
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// A playable character.
    ///
    /// Shared between threads, because a co-op lobby's seats begin at once.
    type Character: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + CharacterTable
        + Send
        + Sync
        + 'static;

    /// One enchantment on one copy, with the fields its class keeps.
    type EnchantmentState: EnchantmentModel<Self>;

    /// A modifier stuck to a card.
    type Enchantment: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + EnchantmentTable
        + 'static;

    /// What starts a fight.
    type Encounter: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + EncounterTable<
            Act = Self::Act,
            Room = Self::RoomType,
            Monster = Self::Monster,
            MoveId = Self::MoveId,
        > + 'static;

    /// A room that is a decision rather than a fight.
    ///
    /// Present for the same reason [`EventTable`] is. The observation names the
    /// event a run is standing in. Asking which events a fuzzer has never
    /// walked into needs both the name and the list.
    type Event: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + EventTable<Act = Self::Act, Encounter = Self::Encounter>
        + 'static;

    /// A run-wide rule chosen before the run starts.
    type Modifier: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + ModifierTable
        + Send
        + Sync
        + 'static;

    /// One creature in a fight.
    type Monster: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + MonsterTable<Act = Self::Act>
        + MonsterKinds
        + 'static;

    /// A Defect orb.
    type Orb: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// A potion.
    type Potion: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + PotionTable<Target = Self::TargetType>
        + 'static;

    /// A buff or debuff on a creature.
    type Power: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + PowerTable
        + PowerKinds
        + 'static;

    /// A permanent passive.
    type Relic: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + RelicTable
        + 'static;

    /// What kind of room a map point is.
    type RoomType: RoomKinds + std::hash::Hash + serde::Serialize + serde::de::DeserializeOwned;

    /// What a node on the act's map leads to.
    ///
    /// Not [`ContentVersion::RoomType`]: this is what the map *shows*, and an
    /// `Unknown` node rolls the room it becomes on arrival.
    type MapPointType: MapPointKinds
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned;

    /// The map one act generates, which is `ActModel.GetMapPointTypes` beside
    /// the room count the act's own row carries.
    /// Which side of a fight a creature is on.
    type CombatSide: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + SideKinds
        + 'static;
    /// Where in their turn a player is.
    type PlayerTurnPhase: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + TurnPhaseKinds
        + 'static;
    /// One rule a card in hand fails.
    type UnplayableReason: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + UnplayableReasonKinds
        + 'static;
    /// Every rule a card fails at once, which is how the game reports it.
    type UnplayableReasons: FlagSet<Member = Self::UnplayableReason>;
    /// One of the random streams a run keeps.
    type RunRngType: std::fmt::Debug
        + Copy
        + Ord
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + RunStreamKinds
        + 'static;
    /// One of the random streams each player keeps.
    type PlayerRngType: std::fmt::Debug
        + Copy
        + Ord
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + PlayerStreamKinds
        + 'static;
    /// What a monster telegraphs before a move resolves.
    type IntentKind: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;
    /// The name of a move a monster can make.
    ///
    /// One enum across the build rather than one per monster, because that is
    /// the precision the wire has. An observation reports a chosen move by name
    /// alone, and the monster beside it is what makes the pair exact.
    type MoveId: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + MoveKinds
        + 'static;
    /// One of the things a rest site can offer.
    ///
    /// Not a model. The game spells these as `RestSiteOption` subclasses, and
    /// the name is `OptionId` on each, so the build says nothing about one
    /// beyond that it exists.
    type RestSiteOption: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + crate::RestKinds<Self>
        + 'static;
    /// What a card can be pointed at.
    type TargetType: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + TargetKinds
        + 'static;
}

/// The engine rules of one build whose game code changed between builds, as
/// the simulator asks them.
///
/// These belong to the game's engine: its commands, odds, actions and
/// entities, not its content models. Each build implements them in its own
/// `engine` module. A rule both builds spell the same way stays in the
/// simulator.
pub trait Engine {
    /// `CardRarityOdds.RollWithBaseOdds`, given what `NextFloat` drew and the
    /// base odds of a rare and of an uncommon for the reward's type.
    fn roll_with_base_odds(rolled: f32, rare: f32, uncommon: f32) -> RewardRarity;

    /// `PotionRewardOdds.Roll`: whether a fight's rewards pay a potion, and
    /// the odds it leaves. `current` is `CurrentValue`, `elite` the room's
    /// bonus, `forced` what `ShouldForcePotionReward` said, and `next_float`
    /// the draw, which a build makes or skips.
    fn roll_potion_reward(
        current: f32,
        elite: bool,
        forced: bool,
        next_float: impl FnOnce() -> f32,
    ) -> (bool, f32);

    /// `CardSelectCmd.FromHand` and `FromCombatPile` signal the choice before
    /// they list the cards, even where nothing is left to ask. A turn's setup
    /// pauses at the signal, and `AfterSideTurnStart` runs on meanwhile, so a
    /// card added then is among the choices. A build whose commands list the
    /// cards first, and signal only for a question, says no.
    fn signals_choice_before_listing() -> bool;

    /// `CardModel.OnPlayWrapper`'s loop over a card's plays stops before a
    /// replay once the fight is over or ending. A build whose loop plays
    /// each replay regardless says no.
    fn replays_stop_once_ending() -> bool;

    /// `Creature.LoseHpInternal` takes a loss below nothing as nothing. A
    /// build that takes it as it stands, so that a cap a hit went past
    /// heals, says no.
    fn hp_loss_floors_at_nothing() -> bool;

    /// `RunAutoPrePlayPhase` raises `AfterHandEmptied` for an empty hand as
    /// it enters the phase, before the phase's own hooks. A build that waits
    /// for a play or a potion to empty it says no.
    fn checks_empty_hand_entering_pre_play() -> bool;

    /// `CardSelectCmd`'s selections call `UndoEndTurnIfNecessary`, which takes
    /// back a player's ended turn and lifts `PlayerActionsDisabled`. So a
    /// selection asked while the turn ends leaves potions drinkable. A build
    /// whose selections leave the ended turn alone says no.
    fn selection_undoes_ended_turn() -> bool;

    /// `CardCreationOptions.WithCardPools` clears its pools before it reads
    /// the ones it is handed. Prismatic Gem hands it a `Union` over those same
    /// pools, which then finds only the character pools it has just added. So
    /// a pool an earlier listener added, as Dingy Rug adds the colorless one,
    /// is lost. A build that copies the pools before it clears them says no.
    fn card_pools_cleared_before_read() -> bool;
}

/// The rules of one build, as the simulator asks them.
///
/// [`ContentVersion`] is the vocabulary a build's tables are written in. This
/// is what no table answers: what a card does, how a monster picks its next
/// move, which stream a roll comes from.
///
/// A build pinned only for its tables implements `ContentVersion` and stops
/// there. The simulator will not take it, which is the error worth having.
pub trait Rules: Hooks + Engine {
    /// The relic pool a character drafts from, beside the shared one.
    ///
    /// Here rather than on [`CharacterTable`], because the pool's type belongs
    /// to the relic table. Tying the two together anywhere else needs an
    /// equality bound at every call to say they are the same pool.
    fn relic_pool(character: Self::Character) -> <Self::Relic as RelicTable>::Pool;

    /// The cards a character opens a run with, in deck order.
    fn starting_deck(character: Self::Character) -> Vec<Self::Card>;

    /// What one act's map is made of, which is `ActModel.GetMapPointTypes`
    /// together with the room count its table carries.
    fn act_layout(act: Self::Act) -> Layout;

    /// The relics a character opens a run with, in panel order.
    fn starting_relics(character: Self::Character) -> Vec<Self::Relic>;

    /// The unmodified hit points and gold a character opens a run with.
    fn starting_hp(character: Self::Character) -> Self::Health;
    fn starting_gold(character: Self::Character) -> u16;

    /// How many potion slots an ordinary run opens with.
    fn starting_potion_slots() -> usize;

    /// `AscensionManager.ApplyEffectsTo`'s card, which an ascension adds to
    /// the opening deck: Ascender's Bane from its rung up.
    fn ascension_deck(ascension: Self::AscensionLevel) -> Option<Self::Card>;

    /// The energy a character's turn begins with, before a relic moves it.
    fn max_energy(character: Self::Character) -> u8;

    /// `BaseOrbSlotCount`: the orb slots a fight opens with.
    fn orb_slots(character: Self::Character) -> u8;

    /// An orb as it joins the queue, with the numbers it shows there.
    fn channelled(orb: Self::Orb) -> crate::Channelled<Self>;

    /// `OrbModel.BeforeTurnEndOrbTrigger`, once for each trigger.
    /// An orb whose numbers move writes them on `orb`, which the fight
    /// keeps.
    fn orb_turn_end<F: Fight<Self>>(orb: &mut crate::Channelled<Self>, fight: &mut F);

    /// `OrbModel.AfterTurnStartOrbTrigger`, once for each trigger.
    fn orb_turn_start<F: Fight<Self>>(orb: &mut crate::Channelled<Self>, fight: &mut F);

    /// `OrbModel.Evoke`: what the orb does as it leaves.
    ///
    /// The creatures it hit, which `AfterOrbEvoked` names.
    fn orb_evoke<F: Fight<Self>>(orb: &crate::Channelled<Self>, fight: &mut F) -> Opponents;

    /// `EnchantmentModel.OnPlay`, after each play of the card it is on.
    /// `state` is this copy's enchantment, which the play reads and writes:
    /// its `Status`, set when it disables itself, and its class's own fields.
    fn enchantment_on_play<F: Fight<Self>>(
        enchanted: crate::Enchanted<Self>,
        state: &mut Self::EnchantmentState,
        fight: &mut F,
    );

    /// `OrbModel.Passive` with no target named, which `OrbCmd.Passive` runs.
    fn orb_passive<F: Fight<Self>>(orb: &mut crate::Channelled<Self>, fight: &mut F);

    /// `OrbModel.PassiveVal`: the passive number the orb shows and uses.
    /// `modify` is `ModifyOrbValue`, which the getter runs or leaves out.
    fn orb_passive_val(
        orb: &crate::Channelled<Self>,
        modify: impl FnOnce(Self::Number) -> Self::Number,
    ) -> Self::Number;

    /// `OrbModel.EvokeVal`: the evoke number the orb shows and uses.
    /// `modify` is `ModifyOrbValue`, which the getter runs or leaves out.
    fn orb_evoke_val(
        orb: &crate::Channelled<Self>,
        modify: impl FnOnce(Self::Number) -> Self::Number,
    ) -> Self::Number;

    /// Neow's event id, which the opening room reports.
    fn neow_event() -> Self::Event;

    /// The colorless pool, split by rarity in the game's enumeration order.
    ///
    /// `multiplayer` is whether the run has several players, which deals the
    /// cards only such a run does: `FilterForPlayerCount`.
    fn colorless_cards(multiplayer: bool) -> Drafted<'static, Self::Card>;

    /// A card rarity reduced to the three-way reward rule.
    fn reward_rarity(card: Self::Card) -> RewardRarity;

    /// What a card's `OnUpgrade` writes, as the list of changes it is.
    ///
    /// Declared rather than applied: the fight owns the copy, and an upgrade
    /// is the same vocabulary any other effect writes in.
    fn card_upgrade(card: Self::Card) -> &'static [Change<Self>];

    /// `CardModel.OnPlay`, as a direct call on the hot path.
    ///
    /// A card either finishes or names the question on which it stopped.
    /// Panics for a card nobody has ported.
    fn on_play<'a, K: FightKind<Self>>(
        card: Self::Card,
        fight: &mut K::Fight<'a>,
    ) -> Played<Self, Self::Resume<K>>;

    /// `CardModel.OnTurnEnd`, for a card that remains in the hand.
    fn on_card_turn_end<F: Fight<Self>>(card: Self::Card, fight: &mut F);

    /// Continue a card that stopped, with the answer it awaited.
    ///
    /// `at` identifies the card grouping and the point where it stopped.
    /// `chosen` is what the selection took. A card that stops again says so.
    fn resume_play<'a, K: FightKind<Self>>(
        fight: &mut K::Fight<'a>,
        at: Self::Resume<K>,
        chosen: Taken<Self>,
    ) -> Played<Self, Self::Resume<K>>;

    /// The cards a character's own pool offers, split by rarity.
    ///
    /// `Character.CardPool`, which is what a fight's card reward draws from.
    /// [`Rules::colorless_cards`] is the other pool, and no character holds it.
    ///
    /// `multiplayer` as [`Rules::colorless_cards`] takes it.
    fn reward_cards(character: Self::Character, multiplayer: bool) -> Drafted<'static, Self::Card>;

    /// The character's pool narrowed by `CardFactory.FilterForCombat`, in the
    /// order a combat generation effect shuffles it.
    ///
    /// `multiplayer` as [`Rules::colorless_cards`] takes it.
    fn combat_cards(character: Self::Character, multiplayer: bool) -> &'static [Self::Card];

    /// The colorless pool's `GetUnlockedCards`, every rarity, in the pool's
    /// order, `multiplayer` as [`Rules::colorless_cards`] takes it.
    fn unlocked_colorless_cards(multiplayer: bool) -> &'static [Self::Card];

    /// `Character.CardPool.GetUnlockedCards`, every rarity, in the pool's
    /// order, `multiplayer` as [`Rules::colorless_cards`] takes it.
    fn unlocked_cards(character: Self::Character, multiplayer: bool) -> &'static [Self::Card];

    /// The colorless pool narrowed by `CardFactory.FilterForCombat`, in the
    /// order a combat generation effect shuffles it.
    ///
    /// `multiplayer` as [`Rules::colorless_cards`] takes it.
    fn combat_colorless_cards(multiplayer: bool) -> &'static [Self::Card];

    /// The cards one of a merchant's character shelves draws from.
    ///
    /// `Character.CardPool` narrowed to one type and one rarity, which is what
    /// `CreateForMerchant` filters it down to before it picks. Basic cards are
    /// out, because the merchant drops them first.
    ///
    /// [`Rules::merchant_colorless_cards`] is the other pool a merchant reads.
    fn merchant_cards(
        character: Self::Character,
        kind: CardKind,
        rarity: RewardRarity,
        multiplayer: bool,
    ) -> Vec<Self::Card>;

    /// `CardModel.Type`, or nothing for the one card that works it out from
    /// the fight it is in.
    ///
    /// A pickup that filters the deck by type reads this. A run stands
    /// outside a fight, so the card with no answer is in no such selection.
    fn card_kind(card: Self::Card) -> Option<CardKind>;

    /// The cards a merchant's two colourless shelves draw from.
    ///
    /// The same pool narrowed the same way, at one rarity. A colourless shelf
    /// stocks by rarity alone, so it asks for no card type.
    fn merchant_colorless_cards(rarity: RewardRarity, multiplayer: bool) -> Vec<Self::Card>;

    /// The cards a transform of `card` draws its replacement from, in the
    /// order `Rng.NextItem` reads them.
    ///
    /// `CardFactory.GetDefaultTransformationOptions`, which is the card's own
    /// pool, unlocked. It keeps common, uncommon and rare cards unless the card
    /// is a status or a curse, and it leaves the card itself out. `in_combat`
    /// keeps the cards `CanBeGeneratedInCombat`.
    fn transform_options(card: Self::Card, in_combat: bool, multiplayer: bool) -> Vec<Self::Card>;

    /// `EnchantmentModel.CanEnchant`, for a card in the deck.
    fn can_enchant(enchantment: Self::Enchantment, card: &crate::CardIdent<Self>) -> bool;

    /// `CardCmd.Enchant` by `amount` on a card that takes it, which is the
    /// enchantment's own `OnEnchant` as well.
    fn enchant(enchantment: Self::Enchantment, amount: u8, card: &mut crate::CardIdent<Self>);

    /// `EnchantmentModel.ShouldStartAtBottomOfDrawPile`, which sinks its card
    /// as a fight's first hand is drawn.
    fn should_start_at_bottom_of_draw_pile(
        enchantment: Self::Enchantment,
    ) -> crate::HookResponse<bool>;

    /// The potions a reward draws from, split by rarity.
    ///
    /// `Character.PotionPool` and the shared pool together, which is
    /// `PotionFactory.GetPotionOptions`.
    fn reward_potions(character: Self::Character) -> Drafted<'static, Self::Potion>;

    /// `PotionPoolModel.GetUnlockedPotions` over the character's pool, then
    /// over the shared one, each in its pool's order. A run has every potion
    /// unlocked.
    fn unlocked_potions(character: Self::Character) -> &'static [Self::Potion];

    /// What winning an encounter pays, which is `MinGoldReward` to
    /// `MaxGoldReward`.
    fn encounter_gold(
        encounter: Self::Encounter,
        ascension: Self::AscensionLevel,
    ) -> crate::rng::Interval<u16>;

    /// Resolve a monster’s unmodified HP range at this ascension.
    /// `EncounterModel.CalculateGoldProportion`, for an encounter that
    /// overrides it. `escaped` is `CombatState.EscapedCreatures`' monsters.
    ///
    /// `None` is an encounter that does not override it.
    fn encounter_gold_proportion(
        encounter: Self::Encounter,
        state: &Self::EncounterState,
        escaped: &[Self::Monster],
    ) -> Option<f32>;

    /// A power's `AfterDeath` on the creature carrying it, which runs its
    /// commands against the fight the way a monster's move does.
    /// [`HookEffect::PerformDeath`] asks for it.
    fn perform_power_death<F: Fight<Self>>(
        power: Self::Power,
        holder: crate::CombatId,
        fight: &mut F,
    );

    /// `MinInitialHp` and `MaxInitialHp`, which a monster's model can move:
    /// a respawned Axebot's do in v0.111.0.
    fn monster_hp_bounds(
        monster: Self::Monster,
        model: &Self::MonsterState,
        ascension: Self::AscensionLevel,
    ) -> crate::rng::Interval<Self::Health>;

    /// `AfterAddedToRoom`'s rewrite of the health a monster was built with.
    ///
    /// `rolled` is what its own range gave it, and `teammates` is what the
    /// others on its side stand at.
    ///
    /// `None` is a monster that does not override it, which is nearly all of
    /// them. The rewrite is a `SetMaxAndCurrentHp`, and nothing calls one.
    /// `scaled` is `Creature.ScaleHpForMultiplayer` for the fight, which a
    /// rewrite can bound itself by.
    fn monster_health_after_added(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        rolled: Self::Health,
        teammates: &[Self::Health],
        scaled: impl Fn(Self::Health) -> crate::Decimal,
    ) -> Option<Self::Health>;

    /// `AfterAddedToRoom`'s `SetCurrentHpInternal`, which leaves the maximum
    /// where it stands. `hp` is what the monster stands at as it enters.
    ///
    /// `None` is a monster that does not override it.
    fn monster_hp_after_added(
        monster: Self::Monster,
        model: &Self::MonsterState,
        hp: Self::Health,
    ) -> Option<Self::Health>;

    /// Walk a monster's state graph to the move it makes next, which is
    /// `MonsterMoveStateMachine.RollMove`.
    ///
    /// `machine` is where the graph stands, and the walk leaves it where it
    /// ended. The first call answers the opening intent.
    ///
    /// `None` is a machine on a built state it has yet to perform, so the
    /// monster keeps advertising that.
    ///
    /// `advertised` is whether another on its side has a move as `NextMove`.
    #[allow(clippy::too_many_arguments)]
    fn roll_monster_move<'a, R: crate::rng::GameRng, P: Printed<Self> + ?Sized + 'a>(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        spawn: MonsterSpawn<Self::Encounter>,
        powers: impl IntoIterator<Item = crate::Listener<'a, Self, P>>,
        advertised: impl Fn(Self::MoveId) -> bool,
        state: &Self::MonsterState,
        machine: &mut MoveMachine,
        rng: &mut R,
    ) -> Result<Option<crate::NextMove<Self>>, ContentError>;

    /// `StateLog.Last().GetNextState`: the state the move the machine stands
    /// on leaves for.
    ///
    /// # Panics
    ///
    /// A machine on a branch or a built state, whose next state no port has
    /// asked for.
    fn follow_up(monster: Self::Monster, machine: &MoveMachine) -> u8;

    /// `NextMove.CanTransitionAway` for the state the machine stands on.
    ///
    /// The guard `SetMoveImmediate` carries. A move that has to be performed
    /// first is not replaced, whether by a state the graph holds or by one a
    /// power built. A forced [`Rules::set_move_immediate`] skips it.
    ///
    /// The machine-wide pin a fresh graph carries is not part of this. That
    /// one holds `RollMove`, and this command is not one.
    fn can_leave_current_move(monster: Self::Monster, machine: &MoveMachine) -> bool;

    /// `MonsterModel.SetMoveImmediate`: stand the graph on a state it holds.
    ///
    /// The machine enters that state without logging it. What the monster
    /// advertises becomes that state's move, and the graph walks on from
    /// there, so the state's own follow-up is what comes next.
    ///
    /// `force` is `forceTransition`, which replaces even a move that has to
    /// be performed first, built or named.
    ///
    /// # Errors
    ///
    /// [`MovePinned`], which is the guard the command carries when it is not
    /// forced. The graph is left where it stood.
    ///
    /// # Panics
    ///
    /// A move the named monster's graph does not hold.
    fn set_move_immediate(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        move_id: Self::MoveId,
        force: bool,
        machine: &mut MoveMachine,
    ) -> Result<crate::NextMove<Self>, MovePinned>;

    /// A monster's move graph, as it stands before the fight opens.
    ///
    /// The machine's constructor records its starting state. Only the table
    /// says whether the log carries that state, so a bare index is not enough.
    ///
    /// `opens_on` is what the encounter's roster wrote onto this one, which
    /// `GenerateMoveStateMachine` takes in place of the table's own entry.
    ///
    /// # Panics
    ///
    /// A move the named monster's graph does not hold.
    fn opening_move_machine(
        monster: Self::Monster,
        opens_on: Option<Self::MoveId>,
    ) -> crate::MoveMachine;

    /// Powers a monster applies to itself when it enters combat.
    ///
    /// The level is read the way a move's own numbers are: a monster whose
    /// `AfterAddedToRoom` scales what it applies asks for it.
    ///
    /// `side` is `CombatState.CurrentSide` as the monster arrives.
    fn monster_starting_powers(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        side: Self::CombatSide,
        model: &Self::MonsterState,
    ) -> impl Iterator<Item = (Self::Power, PowerAmount<Self::Number>)>;

    /// Whom a monster's `AfterAddedToRoom` aims a starting power at, which
    /// lands on the monster once for each, with its `Target` set. `None` is
    /// a power it applies once, aimed at nobody.
    fn monster_starting_power_aim(monster: Self::Monster, power: Self::Power) -> Option<Aim>;

    /// Powers a monster's `AfterAddedToRoom` applies to each of its
    /// opponents, from itself.
    fn monster_starting_opponent_powers(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
    ) -> impl Iterator<Item = (Self::Power, PowerAmount<Self::Number>)>;

    /// What one potion does when it is drunk, which is `PotionModel.OnUse`.
    ///
    /// `OnUseWrapper` calls `RemoveBeforeUse` first, so the potion has left
    /// the belt before this runs and does not see itself in it.
    ///
    /// Panics for a potion nobody has ported, as [`Rules::on_play`] does for a
    /// card.
    fn on_use<'a, K: FightKind<Self>>(
        potion: Self::Potion,
        fight: &mut K::Fight<'a>,
    ) -> Played<Self, Self::Resume<K>>;

    /// What `PotionModel.OnUse` asks of the run rather than of a fight.
    /// Outside a fight, where `AnyTime` lets a player drink, that is all of
    /// it. Inside one, it is any part touching the belt. The potion has left
    /// the belt, as [`Rules::on_use`] says.
    ///
    /// # Panics
    ///
    /// A potion nobody has ported outside a fight.
    fn on_use_on_run(
        potion: Self::Potion,
        run: &impl crate::EventView<Self>,
        in_fight: bool,
    ) -> HookEffects<Self>;

    /// What a card that works its own target out from the fight names.
    ///
    /// `CardModel.TargetType`, overridden. The player's powers are what the
    /// four cards doing this read, so they are what is handed over.
    ///
    /// # Panics
    ///
    /// A card whose override nobody has ported.
    fn computed_card_target(card: Self::Card, powers: &[crate::Stack<Self>]) -> Self::TargetType;

    /// `ModifyNextEvent` for a card in the deck: the event it puts in place
    /// of `event`, which is `event` where it changes nothing.
    ///
    /// # Panics
    ///
    /// A card whose override nobody has ported.
    fn deck_card_next_event(card: Self::Card, act: usize, event: Self::Event) -> Self::Event;

    /// `PotionModel.TargetType`, overridden, for a potion that reads whether a
    /// fight is in progress, which `in_combat` says.
    ///
    /// # Panics
    ///
    /// A potion whose override nobody has ported.
    fn computed_potion_target(potion: Self::Potion, in_combat: bool) -> Self::TargetType;

    /// `PotionModel.PassesCustomUsabilityCheck`, overridden. `in_combat` is
    /// whether a fight is in progress. `merchant` is whether
    /// `FoulPotion.GetFoulPotionMerchantTarget` finds a merchant's button in
    /// the room.
    ///
    /// # Panics
    ///
    /// A potion whose override nobody has ported.
    fn computed_potion_check(potion: Self::Potion, in_combat: bool, merchant: bool) -> bool;

    /// What a card works out from the fight for a number it prints.
    ///
    /// `CalculatedVar.Calculate`, which is a base plus an extra times a
    /// multiplier the card names. Both halves are on the card's own face, so
    /// this asks the copy rather than the table.
    ///
    /// # Panics
    ///
    /// A number nobody has ported the calculation for.
    fn calculated_card_var<P: Printed<Self> + ?Sized>(
        card: Self::Card,
        key: Self::CardVar,
        printed: &P,
        owner: CardOwner<'_, Self>,
    ) -> Self::Number;

    /// What one of a monster's moves does, which is its `onPerform`. The move
    /// reads and writes its monster's model through [`Fight::model`]. A move
    /// that asks the player something hands the question back, and the rest
    /// of its move resumes through [`Rules::resume_play`].
    ///
    /// # Panics
    ///
    /// A move nobody has ported.
    fn perform_move<'a, K: FightKind<Self>>(
        monster: Self::Monster,
        move_id: Self::MoveId,
        fight: &mut K::Fight<'a>,
    ) -> Played<Self, Self::Resume<K>>;

    /// The hit count `move_id`'s `MultiAttackIntent` reads off its monster's
    /// model now, as the intent's lambda does each time it is shown:
    /// `TestSubject.MultiClawTotalCount`. Nothing for a count the table
    /// prints.
    fn intent_repeats_now(
        monster: Self::Monster,
        move_id: Self::MoveId,
        model: &Self::MonsterState,
    ) -> Option<Self::Count>;

    /// The damage `move_id`'s attack intent reads off its monster's model
    /// now, as the intent's lambda does each time it is shown:
    /// `WaterfallGiant.CurrentPressureGunDamage`. Nothing for damage the
    /// table prints, or that the monster reads off its powers.
    fn intent_damage_now(
        monster: Self::Monster,
        move_id: Self::MoveId,
        ascension: Self::AscensionLevel,
        model: &Self::MonsterState,
    ) -> Option<i32>;

    /// The rest of a move that handed back [`Played::Waiting`], from `at`,
    /// once what it waited on is over.
    fn resume_move<'a, K: FightKind<Self>>(
        monster: Self::Monster,
        move_id: Self::MoveId,
        at: Self::Resume<K>,
        fight: &mut K::Fight<'a>,
    ) -> Played<Self, Self::Resume<K>>;

    /// Generate an encounter whose roster is code rather than table data.
    /// An unported build-specific rule returns a typed error.
    fn random_encounter_monsters<R: crate::rng::GameRng>(
        encounter: Self::Encounter,
        rng: &mut R,
    ) -> Result<Vec<crate::Spawned<Self>>, ContentError>;

    /// Every encounter an act fields, in the act's own order.
    ///
    /// Not the sorted set [`EncounterTable::acts`] projects. Room generation
    /// fills its grab bags by walking this. An entry's weight lands at the
    /// index it was added at, so the order decides what a draw picks.
    fn encounters(act: Self::Act) -> &'static [Self::Encounter];

    /// The events an act offers, in the act's own order.
    ///
    /// Not sorted, and not the ancients. Room generation shuffles this list
    /// with the shared pool concatenated on, and a shuffle permutes what it was
    /// handed.
    fn events(act: Self::Act) -> &'static [Self::Event];

    /// The ancients an act offers, in the act's own order.
    fn ancients(act: Self::Act) -> &'static [Self::Event];

    /// The events every act offers, concatenated onto its own list.
    fn shared_events() -> &'static [Self::Event];

    /// The ancients the run deals out to the acts after the first.
    fn shared_ancients() -> &'static [Self::Event];

    /// The Architect, which the run enters past its last act's boss. Its
    /// room is the run's victory room.
    fn the_architect() -> Self::Event;

    type EventState: Clone + std::fmt::Debug;

    fn event_allowed(event: Self::Event, run: &impl crate::EventView<Self>) -> bool;

    fn enter_event<R: crate::rng::GameRng>(
        event: Self::Event,
        run: &impl crate::EventView<Self>,
        rng: &mut R,
    ) -> Self::EventState;

    /// [`crate::EventBehavior::set_up`] for the event entered, off the
    /// player's rewards stream.
    fn set_up_event<R: crate::rng::GameRng>(
        state: &mut Self::EventState,
        run: &impl crate::EventView<Self>,
        rewards: &mut R,
    );

    /// [`crate::EventBehavior::relics_pulled`] for the event entered.
    fn event_relics_pulled(_state: &Self::EventState) -> &'static [RelicPull<Self>] {
        &[]
    }

    /// [`crate::EventBehavior::hand_pulled_relics`] for the event entered.
    fn hand_event_pulled_relics(_state: &mut Self::EventState, _relics: &[Self::Relic]) {}

    fn observe_event(
        state: &Self::EventState,
        run: &impl crate::EventView<Self>,
    ) -> crate::Room<Self>;

    /// Whether the event's `BeforeEventStarted` sets `CanRemovePotions` off,
    /// which its `OnEventFinished` puts back.
    fn event_holds_potions(state: &Self::EventState) -> bool;

    fn choose_event<R: crate::rng::GameRng>(
        state: &mut Self::EventState,
        option: u8,
        run: &impl crate::EventView<Self>,
        rng: &mut R,
    ) -> crate::HookEffects<Self>;

    /// [`crate::EventBehavior::stock`] for the event entered.
    fn stock_event<R: crate::rng::GameRng>(_state: &mut Self::EventState, _shops: &mut R) {}

    /// [`crate::EventBehavior::buy`] for the event entered.
    fn buy_in_event(_state: &mut Self::EventState, index: u8) -> crate::HookEffects<Self> {
        unimplemented!("buying entry {index} from an event")
    }

    /// [`crate::EventBehavior::question`] for the event entered.
    fn event_question(_state: &Self::EventState) -> Option<crate::Question<Self>> {
        None
    }

    /// [`crate::EventBehavior::answer`] for the event entered.
    fn answer_event(
        _state: &mut Self::EventState,
        answer: &crate::Answer,
    ) -> crate::HookEffects<Self> {
        unimplemented!("an event answered with {answer:?}")
    }

    /// [`crate::EventBehavior::resume`] for the event entered.
    fn resume_event<R: crate::rng::GameRng>(
        _state: &mut Self::EventState,
        _run: &impl crate::EventView<Self>,
        _rng: &mut R,
    ) -> crate::HookEffects<Self> {
        unimplemented!("an event resumed after its pickup")
    }

    /// [`crate::EventBehavior::resume_from_fight`] for the event entered.
    fn resume_event_from_fight(
        _state: &mut Self::EventState,
        _escaped: bool,
    ) -> crate::HookEffects<Self> {
        unimplemented!("an event resumed after its fight")
    }

    /// [`crate::EventBehavior::rewards`] for the event entered.
    fn event_rewards(_state: &Self::EventState) -> Vec<CustomReward<Self>> {
        unimplemented!("an event with rewards of its own")
    }

    /// [`crate::EventBehavior::merchant`] for the event entered.
    fn event_merchant(_state: &Self::EventState) -> bool {
        false
    }

    /// [`crate::EventBehavior::potion_thrown`] for the event entered.
    fn event_potion_thrown(
        _state: &mut Self::EventState,
        potion: Self::Potion,
    ) -> crate::HookEffects<Self> {
        unimplemented!("{potion:?} thrown at an event")
    }

    /// The curses Neow offers, in the game's own order.
    fn neow_curses() -> &'static [Self::Relic];

    /// The blessings Neow offers, before the coin flips add to them.
    fn neow_positives() -> &'static [Self::Relic];

    /// The three pairs a coin flip adds one of, in the order they are added.
    fn neow_flips() -> &'static [(Self::Relic, Self::Relic)];

    /// The curses that take a blessing off the table with them.
    fn neow_pairs() -> &'static [(Self::Relic, Self::Relic)];

    /// `Neow.AllPossibleOptions`, as the relics its options offer, in its
    /// order.
    fn neow_options() -> impl Iterator<Item = Self::Relic>;

    /// The curse that suppresses the first of [`Rules::neow_flips`].
    fn neow_suppresses_flip() -> Self::Relic;

    /// Whether Neow can offer this relic to a run of this shape.
    ///
    /// `RelicModel.IsAllowedAtNeow`, which four of the twenty-seven override.
    /// Two turn on the player count. The other two ask what the profile has
    /// unlocked, and a run unlocks everything.
    ///
    /// A stopgap. Relic behaviour belongs beside the rules, and it is here
    /// because naming a relic from a rule needs something this crate does not
    /// offer yet.
    fn allowed_at_neow(relic: Self::Relic, players: usize) -> bool;

    /// `RelicModel.IsAllowed`, for a run `floor` floors in, which is its
    /// `TotalFloor`. A relic bag drops, for good, each relic that fails it as
    /// the bag is pulled from. A run's floor only climbs, so a relic that
    /// fails it once fails it for the rest of the run.
    fn relic_allowed(relic: Self::Relic, floor: usize, players: usize) -> bool;

    /// Every run stream, in the order the game creates them.
    fn run_rng_types() -> &'static [Self::RunRngType];

    /// Every per-player stream, in the order the game creates them.
    fn player_rng_types() -> &'static [Self::PlayerRngType];

    /// The side a monster fights on.
    fn enemy_side() -> Self::CombatSide;
}

/// Which build a content module came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// The build's own version string. A label, not an identity.
    pub name: &'static str,
    /// The commit the release was cut from. A label too, and the one that
    /// distinguishes two builds sharing a version string.
    pub commit: &'static str,
    /// The hash shipped in the game's `release_info.json`, identifying the
    /// release. This moves on any code change at all.
    ///
    /// Needed beside [`Version::content_hash`], not instead of it. A rebalance
    /// that moved a card's damage leaves the content hash equal and every
    /// generated number wrong. This is the hash that notices.
    ///
    /// `i64` rather than the game's `int`, so it compares against a
    /// `Greeting`'s field without a cast.
    pub assembly_hash: i64,
    /// XxHash32 over the model-id namespace: the *set of ids* and nothing else.
    ///
    /// This is the identity of the content, and it is what the game itself uses
    /// to refuse a replay recorded against different content. It moves when
    /// content is added, removed or renamed. It does not move when a value on a
    /// model changes, which is why [`Version::assembly_hash`] sits beside it.
    pub content_hash: u32,
}

/// What a card costs to play, before anything in a run touches it.
///
/// Three states rather than a number and a flag. Only [`Cost::Fixed`] takes
/// arithmetic, because the game's own `CardEnergyCost` lets only one of the
/// three carry a number.
///
/// Serves both pools a card is priced in. Star cost has the same three states
/// and the same `-1` sentinel, read where the game is parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cost {
    /// Spends everything left in the pool. Ten cards cost X energy on this
    /// build, and an X star cost resolves to the stars in hand.
    X,
    /// A fixed cost, and the only variant cost modifiers act on.
    Fixed(nonmax::NonMaxU8),
    /// Not priced in this pool at all. The default for statuses, curses and
    /// quests, and for the star cost of every card outside the Regent's.
    ///
    /// Not the same as `Fixed(0)`, and the difference is load-bearing. A
    /// negative base makes `GetWithModifiers` return early, `SetThisTurn(0)`
    /// a no-op, and Confused skip the card entirely. It still resolves to
    /// spending zero, because `GetAmountToSpend` clamps at zero.
    None,
}

impl Cost {
    /// A fixed cost known not to collide with the largest-byte sentinel.
    pub const fn fixed(value: u8) -> Self {
        match nonmax::NonMaxU8::new(value) {
            Some(value) => Self::Fixed(value),
            None => panic!("the largest byte is not a fixed cost"),
        }
    }
}
