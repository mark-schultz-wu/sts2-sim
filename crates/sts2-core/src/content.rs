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

/// `CardType`, which a merchant's shelves and a relic's filter both name.
///
/// The game's own six. `_coloredCardTypes` names the first three, and a shelf
/// is stocked by filtering the character's pool down to one of them.
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

    /// Whether the table prints the keyword that opens a fight in hand.
    ///
    /// What the table prints, which an upgrade can add to. A copy is what
    /// carries the answer, because `AddKeyword` writes on one.
    fn innate(self) -> bool;

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
    /// The same, for the owner's Osty rather than the owner.
    OstyDamage,
    /// `Hook.ModifyBlock`, for the creature about to gain it.
    Block,
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

    /// The game's own key for it, which is what an observation carries.
    fn name(self) -> &'static str;
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

    /// What a rest site offers this run, in the game's own order.
    ///
    /// `RestSiteOption.Generate` opens with `HEAL` and `SMITH`, adds `MEND`
    /// for a second player, and then runs `TryModifyRestSiteOptions`. That
    /// hook is why the list is the build's answer and not a constant. Both the
    /// belt and the deck listen on it.
    fn offered(relics: &[V::Relic], deck: &[crate::CardIdent<V>]) -> Vec<Self>;
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

    /// `Creature.GetPower<T>()?.Amount`, and `None` for one without it.
    ///
    /// Both of the questions the game asks a power from outside: whether the
    /// creature carries it, and the stack it carries it at.
    fn power(&self, power: V::Power) -> Option<PowerAmount<V::Number>>;
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
    /// An effect after this fight's current listeners have modified it.
    type ResolvedEffect: EffectResolution<V>;

    /// One creature in this fight, as [`Fight::creatures`] hands it over.
    type Creature: CreatureView<V>;

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
    fn creatures(&self) -> impl Iterator<Item = &Self::Creature>;

    /// The side whoever is acting is on, which is `Owner.Creature.Side`.
    fn side(&self) -> V::CombatSide;

    /// The creatures across from whoever is acting, in command order.
    ///
    /// `GetOpponentsOf`. A command snapshots this list before its first
    /// effect lands. Each effect still resolves immediately before it lands,
    /// because an earlier one can change the listeners the next one reaches.
    fn opponents(&self) -> Vec<crate::CombatId> {
        let side = self.side();
        self.creatures()
            .filter(|creature| creature.side() != side && creature.alive())
            .map(CreatureView::id)
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

    /// Perform the acting monster's advertised attack against every opponent.
    ///
    /// Each hit resolves immediately before it lands, so an earlier hit can
    /// change the listeners the next one reaches.
    fn attack_opponents(&mut self) {
        let damage = self.intent_damage();
        let repeats: usize = num_traits::cast(self.intent_repeats()).unwrap_or_else(|| {
            unimplemented!("a move that hits more times than a fight can count")
        });
        for _ in 0..repeats {
            for target in self.opponents() {
                let attack = self.resolve(Effect::Attack {
                    dealer: self.actor(),
                    target,
                    damage: Amount::Number(damage),
                    scaling: Scaling::Dynamic,
                });
                self.apply(attack);
            }
        }
    }

    /// `PowerCmd.Apply<P>(targets, amount, ...)`, which lands on every
    /// creature on the other side.
    ///
    /// A monster's move points at the creatures across from it rather than at
    /// one, the way [`Fight::attack_opponents`] does.
    fn apply_to_opponents(&mut self, power: V::Power, amount: PowerAmount<V::Number>);

    /// `CardPileCmd.Draw(count, Owner)`.
    ///
    /// A printed number, so signed. A card printing a negative one draws
    /// nothing, which is what the game's own loop does.
    fn draw(&mut self, count: V::Number);

    /// `CardPileCmd.AddToCombatAndPreview<C>(targets, Discard, count)`.
    ///
    /// One copy per count, into the discard pile of every creature on the
    /// other side.
    fn give_cards(&mut self, card: V::Card, count: usize);

    /// Add copies of a generated card to one combat pile.
    fn add_generated_cards(
        &mut self,
        _card: crate::CardIdent<V>,
        _pile: crate::Pile,
        _count: usize,
    ) -> Vec<crate::CardAt<V>> {
        unimplemented!("adding generated cards to a pile")
    }

    /// `CardCmd.Discard(this)`, for the card whose behavior is running.
    fn discard_this(&mut self);

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

    /// `CardPileCmd.RemoveFromCombat` for one named card.
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

    /// `CreatureCmd.Add<M>(CombatState, position)`.
    ///
    /// The monster rolls its hit points when it arrives. It does not advertise
    /// a move until the enemy side next prepares for a turn.
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

    /// `IsUpgraded` for the card being played.
    ///
    /// A card whose play changes with its upgrade rather than by printing a
    /// larger number. Begone upgrades the Minion Strike it creates, and
    /// prints no number at all.
    fn upgraded(&self) -> bool;

    /// `CardCmd.Transform(original, replacement)`.
    ///
    /// The replacement takes the original's place: the same pile, at the same
    /// index. It belongs to whoever owned the original, which is the creature
    /// acting.
    ///
    /// The original is named by a [`crate::CardAt`], so this looks straight
    /// where it says rather than read the piles through.
    fn transform(&mut self, card: crate::CardAt<V>, into: crate::CardIdent<V>);

    /// `ForgeCmd.Forge`: raise a number on every copy of one card.
    ///
    /// Every pile, the exhaust pile included, and one copy is created in hand
    /// when the fight holds none outside exhaust.
    fn forge(&mut self, card: V::Card, key: V::CardVar, amount: V::Number);

    /// `PowerCmd.Apply<P>(Owner.Creature, ...)`, on whoever is acting.
    ///
    /// The player for a card and the monster for a move, which is the creature
    /// a card saying "gain" means.
    fn apply_to_actor(&mut self, power: V::Power, amount: PowerAmount<V::Number>);

    /// `CreatureCmd.Heal(at, amount)`, which no power modifies.
    ///
    /// The health is capped at the recipient's maximum, and a dead creature
    /// healed above zero is alive again.
    fn heal(&mut self, at: crate::CombatId, amount: V::Number);

    /// The character whose pool combat generation reads.
    fn character(&self) -> V::Character {
        unimplemented!("the character in this fight")
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
    /// `LocalCostType.Relative`: the cost moves by an amount.
    ///
    /// `EnergyCost.UpgradeBy`, which an upgrade uses, and `AddThisTurn`.
    ChangeCost(CostPool, V::Number),
    /// `LocalCostType.Absolute`: the cost becomes a number.
    ///
    /// `EnergyCost.SetThisTurn` and its kin. `SetToFreeThisTurn` is this at
    /// zero on both pools, for [`Lasting::ThisTurn`].
    SetCost(CostPool, V::Number),
    /// `CardModel.AddKeyword`: the copy carries a word its table does not.
    ///
    /// The word is on the copy for as long as the copy lasts. `RemoveKeyword`
    /// exists and nothing on this build calls it.
    AddKeyword(V::CardKeyword),
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
/// middle of one. Nothing here is a coroutine, so a card that has to ask says
/// where it stopped and is called back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a card play can stop on a question that must be carried"]
pub enum Played<V: ContentVersion> {
    /// The play ran to its end.
    Done,
    /// The play stopped on a `CardSelectCmd`, which is a question.
    Asking {
        /// The pile it selects from.
        pile: crate::Pile,
        /// How many it takes, which decides whether it asks at all.
        selection: Selection,
        /// Where the card stopped, in the card's own words.
        at: V::Resume,
    },
    /// The play stopped on a choose-a-card screen.
    Choosing {
        offered: [crate::CardIdent<V>; CHOICES],
        skippable: bool,
        at: V::Resume,
    },
}

/// What a stopped effect resumes with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taken<V: ContentVersion> {
    Selected(SelectedCards<V>),
    Chosen(Option<crate::CardIdent<V>>),
}

/// The choose-a-card screen's fixed capacity.
pub const CHOICES: usize = 3;

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
    /// Taking none is an answer, so the selector always asks. Even one card in
    /// the pile is a question, because leaving it is a choice.
    UpTo(nonmax::NonMaxUsize),
}

/// Cards returned by a [`Selection`] after its cardinality was checked.
///
/// The contents stay opaque so card behaviour cannot be resumed with an
/// unchecked slice. The simulator constructs this at the boundary where it
/// handles either a player's answer or a selection taken without asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCards<V: ContentVersion>(Box<[crate::CardAt<V>]>);

impl Selection {
    /// Validate a player's answer to this selection.
    pub fn validate_answer<V: ContentVersion>(
        self,
        cards: Box<[crate::CardAt<V>]>,
    ) -> Option<SelectedCards<V>> {
        let allowed = match self {
            Self::Exactly(count) => cards.len() == count.get(),
            Self::UpTo(count) => cards.len() <= count.get(),
        };
        allowed.then_some(SelectedCards(cards))
    }

    /// Validate cards taken without asking.
    ///
    /// An exact selection takes a pile whole when it holds no more than the
    /// requested count. An up-to selection always asks, including for none.
    pub fn validate_automatic<V: ContentVersion>(
        self,
        cards: Box<[crate::CardAt<V>]>,
    ) -> Option<SelectedCards<V>> {
        match self {
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
/// The game keeps this as `ValueProp`, a set of flags. It asks the same
/// question of a hit and of a block: `IsPoweredAttack` and
/// `IsPoweredCardOrMonsterMoveBlock` are both the `Move` flag without the
/// `Unpowered` one. So one answer serves both.
///
/// A card or a monster move prints `Move`, and a relic prints `Unpowered`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scaling {
    /// A relic's number, which lands as it reads.
    Static,
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
    /// The `Multiplicative` half, over what the additions left.
    Scaled(f64),
    /// A ceiling on what is left. The listener returns a number of its own
    /// rather than a share of the one it was handed.
    ///
    /// `ModifyHpLost` is folded that way: each listener is handed what the one
    /// before it returned. Slippery hands back one, whatever it was given.
    AtMost(N),
}

/// A value-producing hook the simulator asks one content model to apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifyHook<V: ContentVersion> {
    /// The model can change a hit dealt or received by its owner.
    Damage { holding: Holding },
    /// The model can change block gained by its owner.
    Block,
    /// The model can change the hit points its owner is about to lose, which
    /// is `ModifyHpLost` over what the block did not take.
    HpLost,
    /// The model can change how many cards its owner draws to open a turn,
    /// which is `ModifyHandDraw`.
    HandDraw { count: V::Number, turn: V::Count },
}

/// A yes-or-no hook the simulator asks one content model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionHook<V: ContentVersion> {
    ShouldRemoveCreatureAfterDeath,
    ShouldCountForCombatEnd,
    /// `PowerModel.ShouldPowerBeRemovedAfterOwnerDeath`, asked of a power
    /// about itself as its owner dies.
    ShouldRemoveSelfAfterOwnerDeath,
    /// `Hook.ShouldPowerBeRemovedOnDeath`, asked of everything on a dying
    /// creature about one of the powers on it.
    ///
    /// The other half of the same removal: a power can keep itself, and
    /// something else on the creature can keep it. Either is enough.
    ShouldRemovePowerOnDeath {
        power: V::Power,
    },
}

/// A notification delivered to one content model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventHook<V: ContentVersion> {
    /// `AfterRoomEntered`, for a room the run fights in.
    CombatRoomEntered,
    /// `BeforeCombatStart`, once the fight is built and before it opens.
    CombatStarting,
    /// `AfterCombatEnd`, for the fight the run has just finished.
    CombatEnd,
    /// `AfterObtained`, as the run picks the model up.
    Obtained,
    /// `AfterCardPlayed`, carrying the card its owner just played.
    CardPlayed {
        card: V::Card,
    },
    /// `AfterDamageReceived`, for the creature that has just been hit.
    ///
    /// `unblocked` is the `DamageResult`'s own, which is what came off the hit
    /// points after the block and the hooks that cap them.
    DamageReceived {
        unblocked: V::Number,
    },
    /// `AfterDamageGiven`, for the creature that dealt the hit.
    DamageGiven {
        fully_blocked: bool,
    },
    /// `MonsterMoveState.OnPerform`, after the move's behavior runs.
    MonsterMovePerformed {
        move_id: V::MoveId,
    },
    /// `AfterCardDrawn`, carrying the card that has just reached the hand.
    ///
    /// Raised once per card, inside the draw loop. A listener that is itself
    /// the drawn card compares the two, which is what every card that answers
    /// this hook does.
    CardDrawn {
        card: V::Card,
    },
    /// `BeforeApplied`, asked of a power on its way onto a creature.
    ///
    /// The power is not held yet, so `landing` is what is about to land
    /// rather than what the creature carries. A [`HookEffect::ApplyToOwner`]
    /// from here lands on the creature this one is landing on, which is the
    /// owner it is about to have.
    BeforeApplied {
        landing: PowerAmount<V::Number>,
    },
    /// `BeforeSideTurnStart`, which runs before the side draws anything.
    BeforeSideTurnStart {
        side: V::CombatSide,
        turn: V::Count,
    },
    /// `AfterSideTurnStart`, which runs once the side has drawn.
    SideTurnStart {
        side: V::CombatSide,
        turn: V::Count,
    },
    BeforeSideTurnEndEarly {
        side: V::CombatSide,
    },
    SideTurnEnd {
        side: V::CombatSide,
    },
    EnergyReset,
    BlockCleared,
    /// `Hook.AfterDeath`, carrying the side the dead creature fought on.
    ///
    /// Every model in the fight is asked, not only the dead creature's own.
    /// So a model whose rule is about the other side reads this. Gremlin Horn
    /// pays for a kill, and not for its own holder's death.
    AfterDeath {
        side: V::CombatSide,
    },
    /// `AfterPowerAmountChanged`, raised on every listener once a power's
    /// amount has moved.
    ///
    /// `delta` is the amount that was applied, which is what the hook carries
    /// and not the total the creature now holds. A power reading this one
    /// compares the power named against itself.
    PowerAmountChanged {
        power: V::Power,
        delta: V::Number,
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
    },
}

/// Which cards a [`HookEffect::AddReplays`] writes on.
///
/// `PowerModel` reaches cards two ways, and both are here. `AllCards` is the
/// combat state's own list. A listener that walks it names the card it wants
/// rather than the copies, and the fight does the walking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replaying<V: ContentVersion> {
    /// One copy, named where it sits.
    One(crate::CardAt<V>),
    /// Every copy of one card the fight holds, in every pile.
    EveryCopyOf(V::Card),
}

/// One command requested by a content model in response to an event hook.
#[derive(Debug, Clone, PartialEq)]
pub enum HookEffect<V: ContentVersion> {
    /// `CardSelectCmd.FromDeckGeneric`, and a clone of what it took.
    ///
    /// The run asks for one card of its deck and adds a copy of it. A card
    /// whose type the filter names is offered by no such selection.
    CloneDeckCard {
        excluding: CardKind,
    },
    /// `CardSelectCmd` over the run's upgradable cards.
    UpgradeRandomDeckCards {
        count: usize,
        stream: V::RunRngType,
    },
    /// `PlayerCmd.GainStars`.
    GainStars(V::Number),
    /// `PlayerCmd.GainEnergy`.
    GainEnergy(V::Number),
    /// `PlayerCmd.LoseEnergy`.
    LoseEnergy(V::Number),
    /// `CardPileCmd.Draw` for the player.
    Draw(V::Number),
    /// `ForgeCmd.Forge` from a power hook.
    Forge {
        card: V::Card,
        key: V::CardVar,
        amount: V::Number,
    },
    /// `CreatureCmd.GainBlock` on the listener's owner.
    GainBlock(V::Number),
    /// `CardCmd.Upgrade` over every card in the hand.
    UpgradeHand,
    /// `PowerCmd.Apply<P>(Owner, amount, Owner)`, on the creature the listener
    /// is carried by.
    ApplyToOwner(V::Power, PowerAmount<V::Number>),
    /// `CardModel.BaseReplayCount += amount`.
    ///
    /// The extra times a card's own play runs. The two hooks that hand replays
    /// out reach different cards: one walks `AllCards`, and the other is given
    /// the single card that just arrived.
    AddReplays {
        to: Replaying<V>,
        amount: V::Number,
    },
    TickDownDuration,
    /// `PowerCmd.Decrement(this)`: one off the amount, and the instance goes
    /// once it reaches zero.
    ///
    /// Unlike [`HookEffect::TickDownDuration`], nothing is skipped. A power
    /// that counts down outside a turn's own tick asks for this.
    Decrement,
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
    EnterMove(V::MoveId),
    InstallMove(crate::NextMove<V>),
    /// Replace the owner's advertised move with a stunned turn.
    StunOwner(crate::NextMove<V>),
    /// Remove another power from the same owner.
    RemoveFromOwner(V::Power),
    /// `CreatureCmd.Add<M>` once per count, in the dead owner's place.
    SummonAfterOwnerDeath {
        monster: V::Monster,
        count: V::Number,
        opening: crate::NextMove<V>,
    },
}

/// One build's relic models, which is what a run holds a relic as.
///
/// `RelicModel` is an instance per relic held, and this is the type of that
/// instance. The two things a run asks one without giving it a hook: build it,
/// and read the number it shows.
pub trait RelicModel<V: ContentVersion>: Clone + std::fmt::Debug + PartialEq + Eq + Sized {
    /// The model as `RelicCmd.Obtain` builds it.
    ///
    /// # Panics
    ///
    /// A relic nobody has ported. A run carrying one skips it at every later
    /// hook, which is a divergence with no name on it.
    fn new(relic: V::Relic) -> Self;

    /// `RelicModel.DisplayAmount`, which is the number the relic shows.
    ///
    /// # Panics
    ///
    /// A relic that works its number out and nobody has ported.
    fn display_amount(&self, relic: V::Relic) -> V::Number;
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
        /// What this instance saves for itself, which is every field of its
        /// own model. A power writes here rather than ask the fight to.
        state: &'a V::PowerState,
    },
    Relic(&'a V::RelicState),
    /// A card in the fight, and what this copy of it prints.
    Card {
        card: V::Card,
        printed: &'a P,
    },
}

pub enum EventListener<'a, V: Hooks, P: Printed<V> + ?Sized> {
    Power {
        power: V::Power,
        amount: PowerAmount<V::Number>,
        owner: V::CombatSide,
        state: &'a mut V::PowerState,
    },
    Relic(&'a mut V::RelicState),
    Card {
        card: V::Card,
        printed: &'a P,
    },
}

/// The build-specific behavior behind the game's hook dispatcher.
///
/// The enums begin with only the hook shapes the simulator can execute. They
/// grow as behavior reaches another hook, without widening this trait.
pub trait Hooks: ContentVersion {
    /// What one relic's own model saves, which the run carries for it.
    type RelicState: RelicModel<Self>;

    /// What one power's own model saves, which the creature carries for it.
    type PowerState: PowerModel<Self>;

    fn modify_hook<P: Printed<Self> + ?Sized>(
        listener: Listener<'_, Self, P>,
        hook: ModifyHook<Self>,
    ) -> Modify<Self::Number>;

    fn decide_hook<P: Printed<Self> + ?Sized>(
        listener: Listener<'_, Self, P>,
        hook: DecisionHook<Self>,
    ) -> crate::HookResponse<bool>;

    fn event_hook<P: Printed<Self> + ?Sized>(
        listener: EventListener<'_, Self, P>,
        hook: EventHook<Self>,
    ) -> Vec<HookEffect<Self>>;
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

/// Which side of a hit the creature holding a power is on.
///
/// A power that modifies damage asks this. Vulnerable acts when its owner is
/// hit, Weak when its owner is doing the hitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Holding {
    /// The power's owner is the one taking the hit.
    Target,
    /// The power's owner is the one dealing it.
    Dealer,
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
}

impl<V: ContentVersion> Spawned<V> {
    /// A monster whose roster wrote nothing onto it.
    pub fn new(monster: V::Monster) -> Self {
        Self {
            monster,
            opens_on: None,
        }
    }

    /// The same, opening on the move the roster named.
    pub fn opening_on(monster: V::Monster, opens_on: V::MoveId) -> Self {
        Self {
            monster,
            opens_on: Some(opens_on),
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
#[derive(Debug, PartialEq, Eq, Default)]
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
    ///
    /// Inline, because one monster logs one state a turn and a fight is
    /// shorter than this. A branch of a run copies every monster's log.
    pub log: smallvec::SmallVec<[u8; LOG_INLINE]>,
    /// Whether the machine stands on a state a caller built.
    ///
    /// `Creature.Stun` and `IllusionPower` both build a `MoveState` rather
    /// than name one, and hand it to `SetMoveImmediate`. Each sets
    /// `MustPerformOnceBeforeTransitioning`, and each points `FollowUpStateId`
    /// at the last state in the log. That state is [`MoveMachine::at`]. So a
    /// built state needs no index of its own, and `at` is where it goes.
    pub built: bool,
}

/// How many states one monster's log holds before reaching the heap.
///
/// A monster enters about one a turn. The longest fight in the recording logs
/// twenty, so this is that with room over. A longer one spills, as every log
/// used to.
pub const LOG_INLINE: usize = 24;

impl Clone for MoveMachine {
    /// Copies the log rather than walking it.
    ///
    /// `SmallVec` clones a list one item at a time, because its `Clone` cannot
    /// see that the item is `Copy`. A state index is a byte.
    fn clone(&self) -> Self {
        Self {
            at: self.at,
            performed: self.performed,
            ever_performed: self.ever_performed,
            log: smallvec::SmallVec::from_slice(&self.log),
            built: self.built,
        }
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
        Self {
            at,
            log: if logged {
                smallvec::smallvec![at]
            } else {
                smallvec::SmallVec::new()
            },
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

    /// Where a card's own play stopped to ask something.
    ///
    /// One per card that stops, and nothing for a build whose cards never do.
    /// A card names its own stopping points, so no card can be resumed at
    /// another card's.
    type Resume: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static;

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

/// The rules of one build, as the simulator asks them.
///
/// [`ContentVersion`] is the vocabulary a build's tables are written in. This
/// is what no table answers: what a card does, how a monster picks its next
/// move, which stream a roll comes from.
///
/// A build pinned only for its tables implements `ContentVersion` and stops
/// there. The simulator will not take it, which is the error worth having.
pub trait Rules: Hooks {
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

    /// What a card's `OnUpgrade` writes, as the list of changes it is.
    ///
    /// Declared rather than applied: the fight owns the copy, and an upgrade
    /// is the same vocabulary any other effect writes in.
    fn card_upgrade(card: Self::Card) -> &'static [Change<Self>];

    /// `CardModel.OnPlay`, as a direct call on the hot path.
    ///
    /// A card either finishes or names the question on which it stopped.
    /// Panics for a card nobody has ported.
    fn on_play<F: Fight<Self>>(card: Self::Card, fight: &mut F) -> Played<Self>;

    /// `CardModel.OnTurnEnd`, for a card that remains in the hand.
    fn on_card_turn_end<F: Fight<Self>>(card: Self::Card, fight: &mut F);

    /// Continue a card that stopped, with the answer it awaited.
    ///
    /// `at` identifies the card grouping and the point where it stopped.
    /// `chosen` is what the selection took. A card that stops again says so.
    fn resume_play<F: Fight<Self>>(
        fight: &mut F,
        at: Self::Resume,
        chosen: Taken<Self>,
    ) -> Played<Self>;

    /// The cards a character's own pool offers, split by rarity.
    ///
    /// `Character.CardPool`, which is what a fight's card reward draws from.
    /// [`Rules::colorless_cards`] is the other pool, and no character holds it.
    fn reward_cards(character: Self::Character) -> Drafted<'static, Self::Card>;

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
    fn merchant_colorless_cards(rarity: RewardRarity) -> Vec<Self::Card>;

    /// The potions a reward draws from, split by rarity.
    ///
    /// `Character.PotionPool` and the shared pool together, which is
    /// `PotionFactory.GetPotionOptions`.
    fn reward_potions(character: Self::Character) -> Drafted<'static, Self::Potion>;

    /// What winning an encounter pays, which is `MinGoldReward` to
    /// `MaxGoldReward`.
    fn encounter_gold(
        encounter: Self::Encounter,
        ascension: Self::AscensionLevel,
    ) -> crate::rng::Interval<u16>;

    /// Resolve a monster’s unmodified HP range at this ascension.
    fn monster_hp_bounds(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
    ) -> crate::rng::Interval<Self::Health>;

    /// `AfterAddedToRoom`'s rewrite of the health a monster was built with.
    ///
    /// `rolled` is what its own range gave it, and `teammates` is what the
    /// others on its side stand at.
    ///
    /// `None` is a monster that does not override it, which is nearly all of
    /// them. The rewrite is a `SetMaxAndCurrentHp`, and nothing calls one.
    fn monster_health_after_added(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        rolled: Self::Health,
        teammates: &[Self::Health],
    ) -> Option<Self::Health>;

    /// Walk a monster's state graph to the move it makes next, which is
    /// `MonsterMoveStateMachine.RollMove`.
    ///
    /// `machine` is where the graph stands, and this leaves it where the walk
    /// ended. A machine on its own initial state has not walked, so the first
    /// call answers the opening intent.
    ///
    /// `None` is a machine on a built state it has yet to perform. Nothing
    /// here names that state, so the monster keeps advertising it.
    fn roll_monster_move<'a, R: crate::rng::GameRng, P: Printed<Self> + ?Sized + 'a>(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        spawn: MonsterSpawn<Self::Encounter>,
        powers: impl IntoIterator<Item = crate::Listener<'a, Self, P>>,
        machine: &mut MoveMachine,
        rng: &mut R,
    ) -> Result<Option<crate::NextMove<Self>>, ContentError>;

    /// `NextMove.CanTransitionAway` for the state the machine stands on.
    ///
    /// The guard `SetMoveImmediate` carries. A move that has to be performed
    /// first is not replaced, whether by a state the graph holds or by one a
    /// power built. Three callers force the transition and skip it.
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
    /// # Errors
    ///
    /// [`MovePinned`], which is the guard the command carries. The graph is
    /// left where it stood.
    ///
    /// # Panics
    ///
    /// A move the named monster's graph does not hold.
    fn set_move_immediate(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        move_id: Self::MoveId,
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
    fn monster_starting_powers(
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
    fn on_use<F: Fight<Self>>(potion: Self::Potion, fight: &mut F) -> Played<Self>;

    /// What a card that works its own target out from the fight names.
    ///
    /// `CardModel.TargetType`, overridden. The player's powers are what the
    /// four cards doing this read, so they are what is handed over.
    ///
    /// # Panics
    ///
    /// A card whose override nobody has ported.
    fn computed_card_target(card: Self::Card, powers: &[crate::Stack<Self>]) -> Self::TargetType;

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
        powers: &[crate::Stack<Self>],
    ) -> Self::Number;

    /// What one of a monster's moves does, which is its `onPerform`.
    ///
    /// # Panics
    ///
    /// A move nobody has ported.
    fn perform_move<F: Fight<Self>>(monster: Self::Monster, move_id: Self::MoveId, fight: &mut F);

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

    type EventState: Clone + std::fmt::Debug;

    fn event_allowed(event: Self::Event, run: &impl crate::EventView<Self>) -> bool;

    fn enter_event<R: crate::rng::GameRng>(
        event: Self::Event,
        run: &impl crate::EventView<Self>,
        rng: &mut R,
    ) -> Self::EventState;

    fn observe_event(
        state: &Self::EventState,
        run: &impl crate::EventView<Self>,
    ) -> crate::Room<Self>;

    fn choose_event<R: crate::rng::GameRng, C: crate::EventContext<Self>>(
        state: &mut Self::EventState,
        option: u8,
        run: &mut C,
        rng: &mut R,
    ) -> crate::EventFlow<Self>;

    /// The curses Neow offers, in the game's own order.
    fn neow_curses() -> &'static [Self::Relic];

    /// The blessings Neow offers, before the coin flips add to them.
    fn neow_positives() -> &'static [Self::Relic];

    /// The three pairs a coin flip adds one of, in the order they are added.
    fn neow_flips() -> &'static [(Self::Relic, Self::Relic)];

    /// The curses that take a blessing off the table with them.
    fn neow_pairs() -> &'static [(Self::Relic, Self::Relic)];

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
