//! Event rules.

mod neow;

pub use neow::Blessing;

use sts2_core::rng::GameRng;
use sts2_core::{EventTable, Rules};

pub(crate) fn stream<V: Rules, R: GameRng>(seed: u32, slot: u32, event: V::Event) -> R {
    R::new(seed.wrapping_add(slot).wrapping_add(event.event_id_hash()))
}
