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

    /// Whether an ordinary run is played through this act.
    ///
    /// The table holds an alternative first act and a deprecated one, and
    /// `ActModel.GetDefaultList` is this filter.
    fn played(self) -> bool;
}

/// How one act rolls the rest sites on its map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rests {
    /// `NextGaussianInt(mean, 1, min, max)`, which three of the four acts use.
    Around { mean: i32, min: i32, max: i32 },
    /// `NextInt(min, max)`, which the last act uses.
    Between { min: i32, max: i32 },
}

/// What one act's map is made of, before any of it is rolled.
///
/// The acts differ in three numbers and in nothing else, so the generator
/// takes them rather than knowing about acts. Which numbers an act uses is
/// `BaseNumberOfRooms` and `GetMapPointTypes`, both of them per build, which
/// is why [`ContentVersion::act_layout`] is what answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// `BaseNumberOfRooms`, which is one less than the grid's height.
    pub rooms: usize,
    pub rests: Rests,
    /// Taken off the standard question mark roll. Zero in the first act.
    pub fewer_unknowns: i32,
}

/// A content rule that cannot produce a value from static state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContentError {
    #[error("{0} has not been ported")]
    Unported(&'static str),
    #[error("{0} depends on live combat state")]
    CombatDependent(&'static str),
    #[error("monster state index {index} is outside its graph")]
    InvalidMonsterState { index: u8 },
    #[error("a random monster state has invalid weights")]
    InvalidMonsterWeights,
}

/// How an encounter chooses the creatures that open its fight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncounterRoster<T> {
    Fixed(T),
    Random,
    Configured,
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

    /// Whether the two field a creature family in common.
    ///
    /// Room generation places no encounter next to one this holds for. So a
    /// run does not meet the weak slimes and then the full ones.
    fn shares_family_with(self, other: Self) -> bool;

    /// The monsters this encounter can field, in table order.
    fn possible_monsters(self) -> impl Iterator<Item = Self::Monster>;

    /// How the encounter chooses its opening roster.
    fn roster(self) -> EncounterRoster<impl Iterator<Item = Self::Monster>>;

    /// The encounter entry’s hash, already cast for wrapping seed arithmetic.
    fn seed_salt(self) -> u32;
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
}

/// What a table says about a monster, beyond its row.
pub trait MonsterTable: Table {
    /// Where a monster can appear.
    type Act: Copy + serde::Serialize + 'static;

    /// The acts that field it. Empty for one only another monster summons.
    fn acts(self) -> &'static [Self::Act];
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
    type Rarity: Copy + serde::Serialize + 'static;
    /// What it can be pointed at.
    type Target: TargetKinds + serde::Serialize + 'static;
    /// Which of the build's printed numbers one of its rows is.
    type Var: CardVars;
    /// The exact numeric representation used by this build's card variables.
    ///
    /// Not a live game number. This is what the card's face prints, which one
    /// build spells as an exact decimal. [`num_traits::ToPrimitive`] is how a
    /// printed number reaches the run: it yields nothing for one no integer
    /// can hold.
    type Number: std::fmt::Debug + Copy + Eq + std::hash::Hash + num_traits::ToPrimitive + 'static;

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

    /// Whether it carries the keyword that exhausts it from hand at the end
    /// of a turn.
    fn ethereal(self) -> bool;

    /// Whether an unplayed copy stays in hand when the turn ends.
    fn retains(self) -> bool;

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
    fn vars(self) -> &'static [Printed<Self::Var, Self::Number>];
}

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
pub struct Printed<K, N> {
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
    /// Every key, once each.
    const ALL: &'static [Self];

    /// The one key the engine's own rules name.
    ///
    /// Damage is the only printed number a fight modifies before a card runs.
    /// A preview against a target has to find it without naming a build. Every
    /// other key is named by the ported behaviour that reads it.
    const DAMAGE: Self;

    /// The game's own key for it, which is what an observation carries.
    fn name(self) -> &'static str;
}

/// What a table says about a power, beyond its row.
///
/// One question, because one caller has one. A front end drawing a power on a
/// creature has to know which way it points, and the game's own tables say. Not
/// derived here from a name or a suffix, for the reason the crate README gives.
pub trait PowerTable: Table {
    /// Whether the game counts this as a debuff.
    ///
    /// `None` for a power that decides at the time. The game's own
    /// `TypeForCurrentAmount` is this together with the stacking rule and
    /// whether negatives are allowed. A debuff that forbids negatives reads as
    /// a buff while its amount is below zero. So this is the static half, and a
    /// caller wanting the exact answer needs the creature too.
    fn debuff(self) -> Option<bool>;
}

/// What a table says about a relic, beyond its row.
pub trait RelicTable: Table {
    /// The pools a relic can be drawn from.
    type Pool: RelicPoolKinds + serde::Serialize;

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
    /// fight rather than printing it.
    fn target(self) -> Option<Self::Target>;

    /// Whether the potion's own check lets it be drunk.
    ///
    /// `PotionModel.PassesCustomUsabilityCheck`, which is `virtual`. `None`
    /// for the one potion that answers it from live state.
    fn passes_own_check(self) -> Option<bool>;
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
}

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

/// The meaning of a power application, before it is lowered to the game's
/// overloaded numeric amount.
///
/// The widths are the game's own. A duration is a count of side turns and a
/// counter is a signed magnitude, and neither is near sixteen bits. Both are
/// carried natively rather than converted here, so a caller already holding a
/// narrow number hands one straight over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAmount {
    /// A single instance that does not count down.
    Permanent,
    /// A power lasting this many side turns.
    Duration(std::num::NonZeroU16),
    /// A signed counter whose magnitude changes the effect.
    Intensity(std::num::NonZeroI16),
}

impl PowerAmount {
    /// The numeric representation used by the game's combat observation.
    pub fn observed(self) -> i32 {
        match self {
            Self::Permanent => -1,
            Self::Duration(amount) => i32::from(amount.get()),
            Self::Intensity(amount) => i32::from(amount.get()),
        }
    }
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

    /// Whether a run at this level has the rule `rung` names.
    fn has(self, rung: Self) -> bool;
}

/// The fight, as a card's own behaviour reaches it.
///
/// A card's behaviour is ported code and lives per build, in `sts2-content`.
/// The fight it acts on lives in the engine. They meet here, which is what
/// `ICombatState` is in the game.
///
/// Each method is one of the game's commands, named after it. One the engine
/// cannot honour records a fault and does nothing.
pub trait Fight<V: ContentVersion> {
    /// The creature the card was pointed at.
    ///
    /// Present for every card whose target type names one. A play that reaches
    /// behaviour has already been checked against `CardModel.IsValidTarget`.
    fn target(&self) -> Option<u32>;

    /// One of the card's own printed numbers, by the build's key for it.
    ///
    /// `DynamicVars.Damage.BaseValue` and its neighbours. A key the card does
    /// not print answers zero and records a fault. A card asking for a number
    /// it has no row for is a mistake in the port.
    fn var(&mut self, key: V::CardVar) -> V::Number;

    /// `DamageCmd.Attack(damage).FromCard(this).Targeting(at)`.
    fn attack(&mut self, at: u32, damage: V::Number);

    /// `CreatureCmd.GainBlock(Owner.Creature, amount, cardPlay)`.
    fn gain_block(&mut self, amount: V::Number);

    /// `PlayerCmd.GainStars(amount, Owner)`.
    ///
    /// A printed number, so signed. The pool it lands in is not.
    fn gain_stars(&mut self, amount: V::Number);

    /// `PowerCmd.Apply<P>(at, amount, Owner.Creature, this)`.
    fn apply_power(&mut self, at: u32, power: V::Power, amount: V::Number);
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

    /// A signed amount in this build's live game state.
    type Number: num_traits::PrimInt
        + num_traits::Signed
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

    /// One rung of the ascension ladder.
    ///
    /// Version-dependent in a way that is easy to miss. Builds move what a
    /// level means, so the same number is a different rule across two of them.
    type AscensionLevel: AscensionKinds
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned;

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
        + CardTable<Target = Self::TargetType, Var = Self::CardVar>
        + 'static;

    /// One of the numbers this build's cards print, by the game's key for it.
    type CardVar: CardVars
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// A playable character.
    type Character: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + CharacterTable
        + 'static;

    /// A modifier stuck to a card.
    type Enchantment: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// What starts a fight.
    type Encounter: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + EncounterTable<Act = Self::Act, Room = Self::RoomType, Monster = Self::Monster>
        + 'static;

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
        + EventTable<Act = Self::Act>
        + 'static;

    /// A run-wide rule chosen before the run starts.
    type Modifier: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// One creature in a fight.
    type Monster: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + MonsterTable<Act = Self::Act>
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
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + StreamKinds
        + 'static;
    /// One of the random streams each player keeps.
    type PlayerRngType: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + StreamKinds
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

/// The rules of one build, as the simulator asks them.
///
/// [`ContentVersion`] is the vocabulary a build's tables are written in. This
/// is what no table answers: what a card does, how a monster picks its next
/// move, which stream a roll comes from.
///
/// A build pinned only for its tables implements `ContentVersion` and stops
/// there. The simulator will not take it, which is the error worth having.
pub trait Rules: ContentVersion {
    /// The relic pool a character drafts from, beside the shared one.
    ///
    /// Here rather than on [`CharacterTable`], because the pool's type belongs
    /// to the relic table. Tying the two together anywhere else needs an
    /// equality bound at every call to say they are the same pool.
    fn relic_pool(character: Self::Character) -> <Self::Relic as RelicTable>::Pool;

    /// The cards a character opens a run with, in deck order.
    fn starting_deck(character: Self::Character) -> Vec<Self::Card>;

    /// The relics a character opens a run with, in panel order.
    fn starting_relics(character: Self::Character) -> Vec<Self::Relic>;

    /// The unmodified hit points and gold a character opens a run with.
    fn starting_hp(character: Self::Character) -> u16;
    fn starting_gold(character: Self::Character) -> u16;

    /// How many potion slots an ordinary run opens with.
    fn starting_potion_slots() -> usize;

    /// The energy a character's turn begins with, before a relic moves it.
    fn max_energy(character: Self::Character) -> u8;

    /// Neow's event id, which the opening room reports.
    fn neow_event() -> Self::Event;

    /// The colorless pool, split by rarity in the game's enumeration order.
    fn colorless_cards() -> Drafted<'static, Self::Card>;

    /// A card rarity reduced to the three-way reward rule.
    fn reward_rarity(card: Self::Card) -> RewardRarity;

    /// The relic whose pickup opens the two-card colorless choice.
    fn lead_paperweight() -> Self::Relic;

    /// The Regent's starting relic, which opens every fight with three stars.
    fn divine_right() -> Self::Relic;

    /// What one card does when it is played, which is `CardModel.OnPlay`.
    ///
    /// Generic over the fight rather than a trait object, because playing a
    /// card is the hot path.
    ///
    /// False for a card nobody has ported. The engine reports that rather than
    /// play a card whose text it does not know, and a card answering false has
    /// issued no command.
    fn on_play<F: Fight<Self>>(card: Self::Card, fight: &mut F) -> bool;

    /// Resolve a monster’s unmodified HP range at this ascension.
    fn monster_hp_bounds(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
    ) -> crate::rng::Interval<u16>;

    /// Roll the monster’s first visible move from its dumped state graph.
    /// A combat-dependent graph returns a typed error.
    fn initial_monster_move<R: crate::rng::GameRng>(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        rng: &mut R,
    ) -> Result<crate::NextMove<Self>, ContentError>;

    /// Generate an encounter whose roster is code rather than table data.
    /// An unported build-specific rule returns a typed error.
    fn random_encounter_monsters<R: crate::rng::GameRng>(
        encounter: Self::Encounter,
        rng: &mut R,
    ) -> Result<Vec<Self::Monster>, ContentError>;

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

    /// The curses Neow offers, in the game's own order.
    fn neow_curses() -> &'static [Self::Relic];

    /// The blessings Neow offers, before the coin flips add to them.
    fn neow_positives() -> &'static [Self::Relic];

    /// The three pairs a coin flip adds one of, in the order they are added.
    fn neow_flips() -> &'static [(Self::Relic, Self::Relic)];

    /// The curses that take a blessing off the table with them.
    fn neow_pairs() -> &'static [(Self::Relic, Self::Relic)];

    /// The curse that suppresses the first of [`ContentVersion::neow_flips`].
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

    /// Every run stream, in the order the game creates them.
    fn run_rng_types() -> &'static [Self::RunRngType];

    /// The run stream consumed while the run is built.
    fn up_front_rng() -> Self::RunRngType;

    /// Every per-player stream, in the order the game creates them.
    fn player_rng_types() -> &'static [Self::PlayerRngType];

    /// The per-player stream card rewards draw from.
    fn rewards_rng() -> Self::PlayerRngType;

    /// The side a monster fights on.
    fn enemy_side() -> Self::CombatSide;

    /// The stream that rolls monster health.
    fn niche_rng() -> Self::RunRngType;

    /// The stream that chooses monster moves.
    fn monster_ai_rng() -> Self::RunRngType;

    /// The stream that shuffles a combat's draw pile.
    fn shuffle_rng() -> Self::RunRngType;
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
    Fixed(u8),
    /// Not priced in this pool at all. The default for statuses, curses and
    /// quests, and for the star cost of every card outside the Regent's.
    ///
    /// Not the same as `Fixed(0)`, and the difference is load-bearing. A
    /// negative base makes `GetWithModifiers` return early, `SetThisTurn(0)`
    /// a no-op, and Confused skip the card entirely. It still resolves to
    /// spending zero, because `GetAmountToSpend` clamps at zero.
    None,
}
