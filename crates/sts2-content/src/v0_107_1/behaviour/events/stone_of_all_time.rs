//! Stone of All Time eligibility.

use sts2_core::EventView;

use super::super::super::V0_107_1 as V;

pub(super) fn allowed(run: &impl EventView<V>) -> bool {
    run.act() == 1 && run.has_potion()
}
