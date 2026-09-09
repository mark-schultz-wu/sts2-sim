//! Counting what a decision asks of the allocator.
//!
//! Time is the number that matters and the number that wobbles. Two builds of
//! the same code measured ten percent apart on this machine, which is enough
//! to hide a real three percent win. Allocation count does not wobble at all:
//! the same steps make the same calls, so a change either moves the number or
//! it does not.
//!
//! It is also the number that has been worth moving. Every cut so far was a
//! `Vec` that did not need to exist. One per node in an act, one per stream a
//! run names, one per stretch a prune walks.
//!
//! A `GlobalAlloc` can only be declared by a binary, so the counters live here
//! and each bench or test that wants them says
//!
//! ```text
//! #[global_allocator]
//! static ALLOCATOR: Counted = Counted::system();
//! ```

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

/// Trips to the allocator, and the bytes asked for.
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

/// What a piece of work asked of the allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocations {
    /// How many times it asked, which is what a trip to the allocator costs.
    ///
    /// A grow counts, because a `Vec` that reallocates twice on its way to
    /// three items has been to the allocator three times.
    pub calls: u64,
    /// How many bytes it asked for, across those calls.
    pub bytes: u64,
}

/// An allocator that counts, wrapping one that allocates.
pub struct Counted<A = System> {
    inner: A,
}

impl Counted<System> {
    /// Counting, over the system allocator.
    pub const fn system() -> Self {
        Counted { inner: System }
    }
}

// Counting is two relaxed adds on the way through. It is not free, so nothing
// times a build that has this installed -- the allocation counts are the
// answer such a build is for.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Counted<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout);
        unsafe { self.inner.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout);
        unsafe { self.inner.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { self.inner.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.inner.dealloc(ptr, layout) }
    }
}

fn count(layout: Layout) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
}

/// What one piece of work asked of the allocator.
///
/// Only meaningful in a binary that installed [`Counted`], and only on one
/// thread at a time: the counters are global, the way the allocator is.
pub fn allocations<T>(work: impl FnOnce() -> T) -> (T, Allocations) {
    let before = (CALLS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed));
    let answer = work();
    let after = (CALLS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed));
    (
        answer,
        Allocations {
            calls: after.0 - before.0,
            bytes: after.1 - before.1,
        },
    )
}
