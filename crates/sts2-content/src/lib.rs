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

/// A `u8` that is never `u8::MAX`, so `Option<NonMaxU8>` costs one byte.
///
/// Re-exported because it appears in the generated tables' public types — an
/// orb value is `Option<NonMaxU8>` — and a caller matching on one should not
/// have to name the crate it came from.
///
/// It is here rather than a plain `Option<u8>` because that would be two bytes,
/// one of them spent saying whether the other counts, when the range of a byte
/// already has room to say it. The niche is at `u8::MAX` rather than at zero,
/// which matters: zero is a real value in the tables that use this — a mock
/// orb's, a relic that costs nothing — and a zero-niche type would read it as an
/// absence.
///
/// `NonMaxU8::new` answers `None` for 255, the one number it cannot hold, which
/// is the same `None` a table uses for "no number here". Codegen refuses to emit
/// a real 255 rather than letting the two meanings collide.
pub use nonmax::NonMaxU8;

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

/// Which build a content module came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// The build's own version string. A label, not an identity.
    pub name: &'static str,
    /// XxHash32 over the model-id namespace. This is the identity.
    pub content_hash: u32,
}

#[cfg(test)]
mod tests {
    use super::NonMaxU8;

    /// The reason the tables use this type: the `Option` is free.
    ///
    /// A guarantee `nonmax` makes and this crate leans on, so it is asserted
    /// rather than assumed — a version bump that lost the niche would otherwise
    /// grow every table that carries one, silently.
    #[test]
    fn an_optional_non_max_costs_no_more_than_the_number() {
        assert_eq!(size_of::<Option<NonMaxU8>>(), 1);
        assert_eq!(size_of::<Option<u8>>(), 2, "which is what it is instead of");
    }

    /// Zero is a number, and 255 is the one that is not.
    ///
    /// The reason the niche is at the top of the range rather than the bottom:
    /// the tables that use this have real zeroes in them.
    #[test]
    fn zero_survives_and_the_max_does_not() {
        assert_eq!(NonMaxU8::new(0).map(|n| n.get()), Some(0));
        assert_eq!(NonMaxU8::new(u8::MAX), None);
    }
}
