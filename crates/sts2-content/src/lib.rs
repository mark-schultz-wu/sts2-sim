//! Game content, one module per build.
//!
//! Content is the part that moves between game versions. That is which cards
//! exist and what they cost, and which monsters exist and how they choose a
//! move. Then what a power stacks as, and what starts a fight. What it *does* is
//! code, ported by hand and living elsewhere. What it *is* is a table,
//! generated from the game's own dumps, so that nobody types a number the game
//! already knows.
//!
//! A version is identified by [`Version::content_hash`](sts2_core::Version::content_hash), not by its name.
//! Names repeat across builds and say nothing about whether ids still line up.
//! The hash is XxHash32 over the game's own model-id namespace, and it moves
//! when and only when content moves. The game uses the same value to refuse a
//! replay recorded against different content. `sts2-headless` reports it in the
//! greeting, so a table can be checked against the build it is talking to
//! before anything trusts it.
//!
//! The abstract shape these tables fill in lives in `sts2-core`. That is
//! [`sts2_core::ContentVersion`] and the [`sts2_core::Table`] traits. This
//! crate is the builds themselves: generated tables, the dumps they came from,
//! and the behaviour ported beside them.
//!
//! The shapes a row is *written* in stay here, because no trait in `sts2-core`
//! names one. Those are [`Field`], [`Scaled`], [`Few`] and [`Cost`]. `Table`
//! exposes a row through an associated type, so what a row is made of is this
//! crate's business.

pub mod v0_107_1;
pub mod v0_111_0;

/// A field that is usually a constant, but which some content works out from
/// combat state.
///
/// Every domain has a few. [`Field::Computed`] is the simulator's cue to ask
/// rather than look up, and the dumps err towards it. The crate README says
/// What a card costs to play. Defined in `sts2-core`, because a cost is a fact
/// about the game rather than about one build's tables.
pub use sts2_core::Cost;

/// what each way of being wrong costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field<T> {
    /// Fixed for this card, whatever the run is doing.
    Constant(T),
    /// The card computes it. `Shiv` widens to all enemies under Fan of Knives,
    /// and `Mad Science` becomes whatever it stored. Ask, do not look up.
    Computed,
}

/// A number that ascension changes.
///
/// Ascension is a scalar, not a set of flags. Each `AscensionLevel` is a
/// threshold, and a value behind `AscensionHelper.GetValueIfAscension` is a
/// step function of the run's level.
///
/// One threshold, not a list, because `GetValueIfAscension` takes exactly one.
/// The crate README says how the thresholds are measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scaled<T> {
    /// The value below the threshold, at ascension 0.
    pub base: T,
    /// The level at which it changes, and what it changes to.
    pub step: Option<(u8, T)>,
}

impl<T: Copy> Scaled<T> {
    /// The value at a run's ascension level.
    ///
    /// Any level is meaningful, including one past the highest the game allows.
    /// The game's own test is `>=`, so a larger number lands on the step.
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
/// The generated tables are full of "at most a handful". `Few` is one of those
/// inline rather than behind a pointer, and the crate README does the
/// arithmetic.
///
/// `N` is a cap the generator checks rather than a guess. Items are always a
/// prefix, so a `None` is the end of the list and never a hole in it.
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
