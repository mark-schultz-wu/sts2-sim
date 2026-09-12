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
    /// A bound that is not positive or does not fit the RNG integer type.
    #[error("a draw bound must be positive and fit the RNG integer type")]
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

/// A positive exclusive upper bound in the RNG's integer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bound<I>(I);

impl<I: RngInteger> Bound<I> {
    pub fn try_new(value: I) -> Result<Self, Error> {
        if value > I::zero() {
            Ok(Bound(value))
        } else {
            Err(Error::Bound)
        }
    }

    pub const fn get(self) -> I {
        self.0
    }
}

/// Arithmetic required by the game's integer RNG operations.
pub trait RngInteger:
    num_traits::PrimInt
    + num_traits::Signed
    + From<i32>
    + Into<i32>
    + Into<f64>
    + TryFrom<usize>
    + TryInto<usize>
{
    const ZERO: Self;
    const ONE: Self;
    fn scaled(unit: f64, bound: Self) -> Self;
    fn in_half_open(unit: f64, min: Self, max: Self) -> Self;
    fn from_f64(value: f64) -> Self;
}

impl RngInteger for i32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;

    fn scaled(unit: f64, bound: Self) -> Self {
        (unit * bound as f64) as i32
    }

    fn in_half_open(unit: f64, min: Self, max: Self) -> Self {
        let width = max as i64 - min as i64;
        if width <= i32::MAX as i64 {
            ((unit * width as f64) as i32).wrapping_add(min)
        } else {
            (((unit * width as f64) as i64).wrapping_add(min as i64)) as i32
        }
    }

    fn from_f64(value: f64) -> Self {
        value as i32
    }
}

/// A slice whose length is a valid bound for the game's choice operation.
#[derive(Debug, Clone, Copy)]
pub struct Choices<'a, T, I> {
    items: &'a [T],
    bound: Bound<I>,
}

impl<'a, T, I: RngInteger> Choices<'a, T, I> {
    pub fn try_new(items: &'a [T]) -> Result<Self, Error> {
        let count = I::try_from(items.len()).map_err(|_| Error::Bound)?;
        let bound = Bound::try_new(count)?;
        Ok(Choices { items, bound })
    }

    pub(crate) const fn items(&self) -> &'a [T] {
        self.items
    }

    pub(crate) const fn bound(&self) -> Bound<I> {
        self.bound
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

impl<T: num_traits::PrimInt> Interval<T> {
    /// Every integer in this closed interval, in order.
    pub fn values(&self) -> IntervalValues<T> {
        IntervalValues {
            next: Some(self.min()),
            max: self.max(),
        }
    }
}

/// The integers in a closed interval.
pub struct IntervalValues<T> {
    next: Option<T>,
    max: T,
}

impl<T: num_traits::PrimInt> Iterator for IntervalValues<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let value = self.next?;
        self.next = if value == self.max {
            None
        } else {
            value.checked_add(&T::one())
        };
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bound_must_be_positive_and_fit_an_i32() {
        assert!(Bound::try_new(1).is_ok());
        assert!(Bound::try_new(i32::MAX).is_ok());
        assert_eq!(Bound::try_new(0), Err(Error::Bound));
        assert_eq!(Bound::try_new(-1), Err(Error::Bound));
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
    fn an_integer_interval_iterates_over_its_members() {
        assert_eq!(
            Interval::new(3_i32, 5).values().collect::<Vec<_>>(),
            [3, 4, 5]
        );
    }

    #[test]
    fn choices_must_be_nonempty() {
        assert_eq!(Choices::<u8, i32>::try_new(&[]).unwrap_err(), Error::Bound);
        assert_eq!(
            Choices::<_, i32>::try_new(&[10, 20]).unwrap().bound().get(),
            2
        );
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
