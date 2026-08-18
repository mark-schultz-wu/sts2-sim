//! Game content, one module per build.
//!
//! Content is the part that moves between game versions: which cards exist and
//! what they cost, which monsters exist and how they choose a move, what a
//! power stacks as, what starts a fight. What any of it *does* is code, ported
//! by hand and living elsewhere; what it *is* is a table, generated from the
//! game's own dumps so that nobody types a number the game already knows.
//!
//! A version is identified by [`Version::content_hash`], not by its name.
//! Names repeat across builds and say nothing about whether ids still line up;
//! the hash is XxHash32 over the game's own model-id namespace, and it moves
//! when and only when content moves. The game uses the same value to refuse a
//! replay recorded against different content, and `sts2-headless` reports it in
//! the greeting — so a table can be checked against the build it is talking to
//! before anything trusts it.

#[cfg(feature = "v0_107_1")]
pub mod v0_107_1;

/// A field that is usually a constant, but which some content works out from
/// combat state.
///
/// Every domain has a few: a card overriding `Type`, a monster's branch weight
/// written as a lambda, a power whose `StackType` follows the sign of its
/// amount. [`Field::Computed`] is the simulator's cue to ask rather than look
/// up, and the dumps err towards it — marking a constant as computed costs one
/// method, marking a computed value as constant costs a divergence nobody can
/// see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field<T> {
    /// Fixed for this card, whatever the run is doing.
    Constant(T),
    /// The card computes it — `Shiv` widens to all enemies under Fan of Knives,
    /// `Mad Science` becomes whatever it stored. Ask; do not look up.
    Computed,
}

/// A number that ascension changes.
///
/// Ascension is a scalar, not a set of flags: `AscensionManager.HasLevel` is
/// `_level >= (int)level`, so each `AscensionLevel` is a threshold and any
/// value behind `AscensionHelper.GetValueIfAscension` is a step function of the
/// run's level.
///
/// One threshold, not a list of them, because `GetValueIfAscension(level,
/// ascensionValue, fallbackValue)` takes exactly one — a second would mean a
/// monster nesting two calls, which nothing in this build does. Codegen refuses
/// rather than truncating if one ever appears, so this is checked rather than
/// assumed. It is also what keeps the type small: 433 intents carry two of
/// these each, and a slice would cost 16 bytes apiece to say "no threshold"
/// 319 times over.
///
/// The thresholds themselves are measured rather than read out of the source —
/// the dump reads every value at every level and records where it actually
/// moves. That is how Gremlin Merc turned up: its damage steps at
/// `ToughEnemies`, where every other monster's steps at `DeadlyEnemies`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scaled<T> {
    /// The value below the threshold — ascension 0.
    pub base: T,
    /// The level at which it changes, and what it changes to.
    pub step: Option<(u8, T)>,
}

impl<T: Copy> Scaled<T> {
    /// The value at a run's ascension level.
    ///
    /// Any level is meaningful, including one past the highest the game
    /// allows: the game's own test is `>=`, so a larger number simply lands on
    /// the step.
    pub const fn at(&self, ascension: u8) -> T {
        if let Some((level, stepped)) = self.step
            && ascension >= level
        {
            return stepped;
        }
        self.base
    }

    /// Whether ascension moves this at all.
    pub const fn is_flat(&self) -> bool {
        self.step.is_none()
    }
}

/// A short list, laid out where it is used.
///
/// The generated tables are full of "at most a handful": an encounter fields at
/// most four monsters and could ever field at most six. A `&'static [T]` spells
/// one of those as a pointer and a length — sixteen bytes to reach a thing that
/// is usually smaller than the pointer, plus an indirection to follow and a
/// relocation to keep. `Few` is the same list inline.
///
/// `N` is a cap the generator checks rather than a guess: a build that grows a
/// fifth spawn fails generation instead of truncating, and [`Few::new`] is a
/// `const fn` called from a `const`, so overflowing it is a compile error and
/// not a panic in a run.
///
/// Items are always a prefix — `new` fills from the front — so a `None` is the
/// end of the list and never a hole in it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Few<T, const N: usize>([Option<T>; N]);

impl<T: Copy, const N: usize> Few<T, N> {
    pub const fn new(items: &[T]) -> Self {
        assert!(items.len() <= N, "more items than this Few can hold");

        let mut slots = [None; N];
        let mut i = 0;
        while i < items.len() {
            slots[i] = Some(items[i]);
            i += 1;
        }

        Few(slots)
    }

    pub const fn len(self) -> usize {
        let mut n = 0;
        while n < N {
            if self.0[n].is_none() {
                return n;
            }
            n += 1;
        }
        N
    }

    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub const fn get(self, i: usize) -> Option<T> {
        if i < N { self.0[i] } else { None }
    }

    pub fn iter(self) -> impl Iterator<Item = T> {
        self.0.into_iter().flatten()
    }
}

impl<T: Copy + std::fmt::Debug, const N: usize> std::fmt::Debug for Few<T, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// What a card costs to play, before anything in a run touches it.
///
/// Three states rather than a number and a flag, because the game's own
/// `CardEnergyCost` only lets one of the three carry a number.
/// `Canonical = CostsX ? 0 : canonicalCost` throws the number away for X-cost
/// cards, and `GetWithModifiers` returns the base unchanged — skipping every
/// local and global modifier — when the card costs X or the base is negative.
/// So only [`Cost::Energy`] is a cost that arithmetic applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cost {
    /// Spends all remaining energy. Ten cards on this build.
    X,
    /// A fixed cost, and the only variant cost modifiers act on.
    Energy(u8),
    /// No cost — the default for statuses, curses and quests, which the dump
    /// spells `-1`.
    ///
    /// Not the same as `Energy(0)`, and the difference is load-bearing: a
    /// negative base makes `GetWithModifiers` return early, `SetThisTurn(0)`
    /// a no-op, and Confused skip the card entirely. It still resolves to
    /// spending zero, because `GetAmountToSpend` clamps at zero.
    None,
}

/// One game build's content, as the types that name it.
///
/// The generated tables are concrete types per build, so anything that works
/// against content — the oracle client, the simulator — would otherwise be
/// pinned to one of them. This is the seam: name the build once, as a type
/// parameter, and every id follows from it.
///
/// The bounds are spelled out on each associated type rather than hidden behind
/// a blanket-implemented alias. An alias would abbreviate and nothing else: it
/// constrains nothing the bounds do not, and it makes a missing bound report as
/// `V::Card: Id` instead of naming the trait actually missing. The one thing
/// that would earn a real trait here is a `wire()` accessor, and that is
/// deliberately absent everywhere — the serde rename is the only spelling of an
/// id, because two spellings are how they come to disagree.
///
/// The supertraits are what a `#[derive]` on a generic holding these asks for:
/// `Command<V>` derives `Debug`, `Clone`, `Eq` and `Serialize`, and each of
/// those writes a bound on `V` itself even though `V` is a zero-sized marker.
///
/// [`ContentVersion::VERSION`] is what makes the parameter more than typing: a
/// client can check the build it is talking to against the build its tables
/// describe, and refuse rather than quietly drive the wrong game.
pub trait ContentVersion: std::fmt::Debug + Copy + Eq + std::hash::Hash + 'static {
    /// The build these types were generated from.
    const VERSION: Version;

    /// One rung of the ascension ladder.
    ///
    /// Version-dependent in a way that is easy to miss: builds move what a level
    /// means, so the same number is a different rule across two of them.
    type AscensionLevel: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// Where in a run a room sits.
    type Act: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
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
        + 'static;

    /// A playable character.
    type Character: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
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
        + 'static;

    /// A buff or debuff on a creature.
    type Power: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// A permanent passive.
    type Relic: std::fmt::Debug
        + Copy
        + Eq
        + std::hash::Hash
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static;

    /// What kind of room a map point is.
    type RoomType: std::fmt::Debug
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
    /// Needed beside [`Version::content_hash`], not instead of it: a rebalance
    /// that changed a card's damage without adding or removing content leaves
    /// the content hash equal and every generated number wrong. This is the
    /// hash that notices. `sts2_determinism` pins the same value before it will
    /// run at all.
    ///
    /// `i64` rather than the game's `int` so it compares against a
    /// `Greeting`'s field without a cast.
    pub assembly_hash: i64,
    /// XxHash32 over the model-id namespace — the *set of ids* and nothing
    /// else.
    ///
    /// This is the identity of the content, and it is what the game itself uses
    /// to refuse a replay recorded against different content. It moves when
    /// content is added, removed or renamed; it does not move when a value on a
    /// model changes, which is why [`Version::assembly_hash`] sits beside it.
    pub content_hash: u32,
}
