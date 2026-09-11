//! Argument types that carry their own validity.
//!
//! The game throws when draw arguments violate their bounds. [`Bound`] and
//! [`Span`] encode those invariants before a draw begins.
//!
//! Integer spans are half-open and reject `min >= max`. Float and Gaussian
//! bounds allow one point and reject only `min > max`. Constructors require the
//! valid comparison to hold, which also rejects unordered floating-point values
//! such as `NaN`.

/// Why an argument was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A bound outside `1..=i32::MAX`. Zero makes a draw's range empty, and
    /// the game throws on it.
    #[error("a draw bound must be in 1..=i32::MAX")]
    Bound,

    /// A half-open span with nothing in it: `min >= max`, or a `NaN`.
    #[error("a half-open span must contain something")]
    Empty,

    /// A closed interval whose ends are inverted: `min > max`, or a `NaN`.
    #[error("an interval's end must not precede its start")]
    Inverted,

    /// A counter asked to move backwards. A stream is a position in a
    /// sequence, so rewinding is not a slower version of seeking, it is
    /// meaningless.
    #[error("cannot rewind a stream from {from} to {to}")]
    Rewind { from: i32, to: i32 },
}

/// An exclusive upper bound in `1..=i32::MAX`.
///
/// Not [`core::num::NonZeroU32`], which still admits values above `i32::MAX`.
/// The game's parameter is an `int`, and scaling a double by something larger
/// truncates to nonsense rather than failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bound(i32);

impl Bound {
    /// `const`, so a literal bound is checked at compile time.
    pub const fn new(value: i32) -> Option<Self> {
        if value >= 1 { Some(Bound(value)) } else { None }
    }

    pub const fn get(self) -> i32 {
        self.0
    }
}

/// A slice whose length is a valid bound for the game's choice operation.
#[derive(Debug, Clone, Copy)]
pub struct Choices<'a, T> {
    items: &'a [T],
    bound: Bound,
}

impl<'a, T> Choices<'a, T> {
    pub fn new(items: &'a [T]) -> Option<Self> {
        let count = i32::try_from(items.len()).ok()?;
        let bound = Bound::new(count)?;
        Some(Choices { items, bound })
    }

    pub(crate) const fn items(&self) -> &'a [T] {
        self.items
    }

    pub(crate) const fn bound(&self) -> Bound {
        self.bound
    }
}

impl TryFrom<i32> for Bound {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self, Error> {
        Bound::new(value).ok_or(Error::Bound)
    }
}

/// A half-open span `min..max`, known to contain something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span<T> {
    min: T,
    max: T,
}

impl<T: PartialOrd> Span<T> {
    pub fn new(min: T, max: T) -> Result<Self, Error> {
        if min < max {
            Ok(Span { min, max })
        } else {
            Err(Error::Empty)
        }
    }
}

impl<T: Copy> Span<T> {
    pub const fn min(&self) -> T {
        self.min
    }

    pub const fn max(&self) -> T {
        self.max
    }
}

/// A closed interval `min..=max`, known ordered. A single point is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Interval<T> {
    bounds: (T, T),
}

impl<T: PartialOrd> Interval<T> {
    pub fn new(min: T, max: T) -> Self {
        assert!(min <= max, "an interval must be ordered");
        Interval { bounds: (min, max) }
    }

    pub fn try_new(min: T, max: T) -> Result<Self, Error> {
        if min <= max {
            Ok(Interval { bounds: (min, max) })
        } else {
            Err(Error::Inverted)
        }
    }
}

impl<T: Copy> Interval<T> {
    pub const fn min(&self) -> T {
        self.bounds.0
    }

    pub const fn max(&self) -> T {
        self.bounds.1
    }
}

impl Interval<u16> {
    /// The number of integers in this closed interval.
    pub const fn cardinality(&self) -> Bound {
        // Every `u16` interval has between one and 65,536 members.
        let count = self.max() as i32 - self.min() as i32 + 1;
        Bound(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bound_must_be_positive_and_fit_an_i32() {
        assert!(Bound::new(1).is_some());
        assert!(Bound::new(i32::MAX).is_some());
        assert!(Bound::new(0).is_none());
        assert!(Bound::new(-1).is_none());
    }

    #[test]
    fn a_span_must_contain_something() {
        assert!(Span::new(0, 1).is_ok());
        assert_eq!(Span::new(5, 5).unwrap_err(), Error::Empty);
        assert_eq!(Span::new(5, 4).unwrap_err(), Error::Empty);
    }

    #[test]
    fn an_interval_may_be_a_point_but_not_inverted() {
        assert_eq!(Interval::new(5, 5), Interval { bounds: (5, 5) });
        assert_eq!(Interval::try_new(5, 4).unwrap_err(), Error::Inverted);
    }

    #[test]
    fn choices_must_be_nonempty() {
        assert!(Choices::<u8>::new(&[]).is_none());
        assert_eq!(Choices::new(&[10, 20]).unwrap().bound().get(), 2);
    }

    #[test]
    fn nan_is_rejected_by_construction() {
        assert_eq!(
            Interval::try_new(f64::NAN, 1.0).unwrap_err(),
            Error::Inverted
        );
        assert_eq!(
            Interval::try_new(0.0, f64::NAN).unwrap_err(),
            Error::Inverted
        );
        assert_eq!(Span::new(f64::NAN, 1.0).unwrap_err(), Error::Empty);

        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        {
            let (min, max) = (f64::NAN, 1.0);
            assert!(!(min > max), "the negated form accepts NaN");
        }
    }
}
