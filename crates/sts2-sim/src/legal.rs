//! Every decision a run accepts right now, read off the run itself.
//!
//! The observation carries the same answer, and a front end reads it there.
//! A search asks at every node, so the simulator answers without building one.
//! Each phase reads the facts its report reads, and runs the same rules.
//!
//! The order is part of the answer. A driver that picks by index from a seeded
//! die plays the same run only when the list comes back the same way. So it is
//! the protocol's order. A pending question is answered alone. Otherwise the
//! belt comes first, then a room's options, then the fight or the map.

use smallvec::SmallVec;
use sts2_core::{Answer, BeltIndex, CombatId, ContentVersion, Decision, HandIndex, Picks};

/// The decisions a run accepts, in the order a driver picks from.
///
/// Inline to thirty-two. A position in the recordings offers at most nineteen,
/// and eight in ten offer nine or fewer. A full hand aimed at a full board can
/// offer more, and the list then moves to the heap rather than being cut.
pub type Legal<V> = SmallVec<[Decision<V>; LEGAL_INLINE]>;

/// How many decisions a [`Legal`] holds before it reaches the heap.
const LEGAL_INLINE: usize = 32;

/// One decision per creature it can be aimed at, or one aimed at nothing.
pub(crate) fn aimed<V: ContentVersion, I: Copy>(
    index: I,
    targets: &[CombatId],
    moves: &mut Legal<V>,
    decision: impl Fn(I, Option<CombatId>) -> Decision<V>,
) {
    if targets.is_empty() {
        moves.push(decision(index, None));
    } else {
        moves.extend(targets.iter().map(|&target| decision(index, Some(target))));
    }
}

/// The next clicks a card selection allows. Each card not yet picked, while
/// the selection takes more. Taking back the last pick. Handing the picks
/// over, once there are enough.
///
/// A click at a time, because the order of the picks is part of the answer.
/// Purity exhausts its picks in that order, and Astrolabe transforms them in
/// it, one draw of the RNG each.
pub(crate) fn picks<V: ContentVersion>(
    min: usize,
    max: usize,
    offered: usize,
    picked: &Picks,
    moves: &mut Legal<V>,
) {
    if picked.len() < max {
        moves.extend(
            (0..offered)
                .filter(|&at| !picked.iter().any(|taken| taken == at))
                .map(|at| Decision::Answer(Answer::Pick(pick_index(at)))),
        );
    }
    if !picked.is_empty() {
        moves.push(Decision::Answer(Answer::Undo));
    }
    if picked.len() >= min {
        moves.push(Decision::Answer(Answer::Done));
    }
}

/// An option's position, as a pick names it.
///
/// # Panics
///
/// A selection offering more cards than a pick can name.
fn pick_index(at: usize) -> u16 {
    u16::try_from(at).unwrap_or_else(|_| unimplemented!("a pick of option {at}"))
}

/// A hand position, as a play names it.
///
/// # Panics
///
/// A hand past what a play can name.
pub(crate) fn hand_index(at: usize) -> HandIndex {
    u8::try_from(at)
        .ok()
        .and_then(|at| HandIndex::try_new(at).ok())
        .unwrap_or_else(|| unimplemented!("a card at hand position {at}"))
}

/// A belt position, as a drink names it.
///
/// # Panics
///
/// A belt past what a drink can name.
pub(crate) fn belt_index(at: usize) -> BeltIndex {
    u8::try_from(at)
        .ok()
        .and_then(|at| BeltIndex::try_new(at).ok())
        .unwrap_or_else(|| unimplemented!("a potion at belt position {at}"))
}
