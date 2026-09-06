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
    type Data;

    /// Every value of this kind, once each, in the game's own order.
    const ALL: &'static [Self];

    /// This value's row.
    fn data(self) -> Self::Data;
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

/// What a table says about an encounter, beyond its row.
///
/// The two keys a coverage measurement cuts on. A flat count over 96 encounters
/// measures the wrong thing, for the reason the crate README gives.
pub trait EncounterTable: Table {
    /// Where an encounter can appear.
    type Act: Copy + serde::Serialize + 'static;
    /// Monster, elite or boss.
    type Room: Rooms + serde::Serialize;

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

/// The three rarities an ordinary card reward rolls, plus everything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardRewardRarity {
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

    /// The character whose pool holds it, or `None` for one held by no
    /// character: colourless, curses, statuses, tokens, quest and event cards.
    fn pool(self) -> Option<Self::Pool>;

    /// Its rarity, which for the poolless cards is also what they are.
    fn rarity(self) -> Self::Rarity;
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
    type Pool: RelicPools + serde::Serialize;

    /// How often it turns up.
    ///
    /// The one key on these traits carrying a shared vocabulary's bounds rather
    /// than a sweep's, because it appears in [`crate::Question`]. The crate
    /// README says why a reward screen puts it there.
    type Rarity: Rarities + serde::Serialize + serde::de::DeserializeOwned;

    /// Every pool that holds it. Empty for one nothing draws.
    fn pools(self) -> &'static [Self::Pool];

    /// Its rarity, which is also which screen offers it: a boss relic comes
    /// from a boss and nowhere else.
    fn rarity(self) -> Self::Rarity;
}

/// What a table says about a potion, beyond its row.
pub trait PotionTable: Table {
    /// The pool a potion is drawn from.
    type Pool: Copy + serde::Serialize + 'static;

    /// The pool that holds it: shared, one character's, or one of the pools
    /// nothing in a run draws from.
    fn pool(self) -> Self::Pool;
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
pub trait MapPoints: std::fmt::Debug + Copy + Eq + 'static {
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
/// The same bridge as [`MapPoints`], for the three a roster is split on. A
/// treasure or a shop is a room a map point leads to and never an encounter.
pub trait Rooms: std::fmt::Debug + Copy + Eq + 'static {
    const MONSTER: Self;
    const ELITE: Self;
    const BOSS: Self;
}

/// The relic pool run setup names.
///
/// The same bridge as [`MapPoints`]: a generated enum is per build, so a rule
/// holding only a [`ContentVersion`] cannot write `RelicPool::Shared`.
pub trait RelicPools: std::fmt::Debug + Copy + Eq + 'static {
    /// The pool every character draws from, beside its own.
    const SHARED: Self;
}

/// The relic rarities a run's bag deals from.
///
/// `RelicGrabBag` keeps a set of four and drops every relic outside it, so a
/// starter, an event relic and an ancient never enter a bag. The four are what
/// this names, because the count of them is a rule and not a listing.
pub trait Rarities: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static {
    /// The four, in the game's own order.
    const DRAFTED: [Self; 4];
}

/// The ascension ladder, as much of it as a rule needs to ask about.
///
/// A run has one level and every rung at or below it applies. So this is a
/// scalar, and [`Ladder::has`] is `>=` rather than equality.
///
/// The consts are the rungs a ported rule names. A rung with no const is one
/// nothing here reads yet, and adding one is how that changes.
pub trait Ladder: std::fmt::Debug + Copy + Eq + 'static {
    /// No ascension at all, which is where a run starts.
    const NONE: Self;
    /// Elites go from five a map to eight.
    const SWARMING_ELITES: Self;

    /// Whether a run at this level has the rule `rung` names.
    fn has(self, rung: Self) -> bool;
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

    /// One rung of the ascension ladder.
    ///
    /// Version-dependent in a way that is easy to miss. Builds move what a
    /// level means, so the same number is a different rule across two of them.
    type AscensionLevel: Ladder + std::hash::Hash + serde::Serialize + serde::de::DeserializeOwned;

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
        + CardTable
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
        + EncounterTable<Act = Self::Act, Room = Self::RoomType>
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
        + PotionTable
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
    type RoomType: Rooms + std::hash::Hash + serde::Serialize + serde::de::DeserializeOwned;

    /// What a node on the act's map leads to.
    ///
    /// Not [`ContentVersion::RoomType`]: this is what the map *shows*, and an
    /// `Unknown` node rolls the room it becomes on arrival.
    type MapPointType: MapPoints + std::hash::Hash + serde::Serialize + serde::de::DeserializeOwned;

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

    /// Neow's event id, which the opening room reports.
    fn neow_event() -> Self::Event;

    /// The colorless pool in the order the game enumerates it.
    fn colorless_cards() -> Vec<Self::Card>;

    /// A card rarity reduced to the three-way reward rule.
    fn reward_rarity(card: Self::Card) -> CardRewardRarity;

    /// The relic whose pickup opens the two-card colorless choice.
    fn lead_paperweight() -> Self::Relic;

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

    /// Which side of a fight a creature is on.
    type CombatSide: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// Where in their turn a player is.
    type PlayerTurnPhase: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// One rule a card in hand fails.
    type UnplayableReason: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
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
        + 'static;

    /// Every run stream, in the order the game creates them.
    fn run_rng_types() -> &'static [Self::RunRngType];

    /// The run stream consumed while the run is built.
    fn up_front_rng() -> Self::RunRngType;

    /// One of the random streams each player keeps.
    type PlayerRngType: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// Every per-player stream, in the order the game creates them.
    fn player_rng_types() -> &'static [Self::PlayerRngType];

    /// The per-player stream card rewards draw from.
    fn rewards_rng() -> Self::PlayerRngType;

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
        + 'static;
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
