//! Event rules.

mod neow;

pub use neow::Blessing;

use sts2_core::rng::GameRng;
use sts2_core::{EventTable, Rules};

/// An event's own stream. `EventModel.BeginEvent`.
///
/// The run's seed, the player's slot and the event's id, added and then taken
/// at the width this build's `Rng` seeds from.
pub(crate) fn stream<V: Rules, R: GameRng>(seed: u64, slot: u64, event: V::Event) -> R {
    R::new(V::seed_from(
        seed.wrapping_add(slot).wrapping_add(event.event_id_hash()),
    ))
}
