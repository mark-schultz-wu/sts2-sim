//! The potions whose rules this build ports for itself, which the shared
//! potions hand over through [`crate::build::OwnRules`]. This build has none.

use super::super::V0_107_1 as V;
use super::super::generated::potions::Potion;

/// What a potion of this build's own does when it is drunk in a fight.
pub(crate) fn own_potion_use<F: sts2_core::Fight<V>>(
    _potion: Potion,
    _fight: &mut F,
) -> Option<crate::behaviour::Played<V, F::Kind>> {
    None
}

/// What a potion of this build's own asks of the run.
pub(crate) fn own_potion_use_on_run(
    _potion: Potion,
    _run: &impl sts2_core::EventView<V>,
    _in_fight: bool,
) -> Option<sts2_core::HookEffects<V>> {
    None
}
