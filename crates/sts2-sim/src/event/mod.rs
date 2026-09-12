//! Event rules.

mod neow;

pub use neow::Blessing;

use sts2_core::Rules;
use sts2_core::rng::{GameRng, name_hash};

pub(crate) fn stream<V: Rules, R: GameRng>(seed: u32, slot: u32, event: V::Event) -> R {
    let id = serde_json::to_value(event).expect("an event id serializes");
    let id = id.as_str().expect("an event id is a string");
    R::new(seed.wrapping_add(slot).wrapping_add(name_hash(id) as u32))
}
