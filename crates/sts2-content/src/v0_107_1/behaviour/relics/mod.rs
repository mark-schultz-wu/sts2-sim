//! What a relic does, ported.
//!
//! One module per relic, the way `RelicModel`'s subclasses are one file each,
//! and one function here per hook. A relic is ported when a run can pick it
//! up. The game gives hooks to every relic in the belt.
//!
//! [`new`] is the one place a relic becomes a model, and a relic nobody has
//! ported panics there. The belt a run carries is then the list of relics
//! this tree has read.
//!
//! Every hook dispatches on the state alone. The state names the relic that
//! saved it, so no caller can pair a relic with another relic's model.

mod anchor;
mod bone_tea;
mod divine_right;
mod lead_paperweight;
mod pocketwatch;

use sts2_core::{EventHook, Modify, ModifyHook, Table};

use super::super::V0_107_1 as V;
use super::super::generated::relics::Relic;

use crate::Field;

pub(crate) type Number = <V as sts2_core::ContentVersion>::Number;
type Effects = Vec<sts2_core::HookEffect<V>>;

/// One relic's own model, as the run carries it.
///
/// A variant per ported relic, holding what that relic's `[SavedProperty]`
/// fields hold. A relic that saves nothing still has a variant of its own.
/// That variant is what says the port has read the relic and found no state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Anchor,
    BoneTea(bone_tea::State),
    DivineRight,
    LeadPaperweight,
    Pocketwatch(pocketwatch::State),
}

/// One relic's model as the run picks it up.
///
/// # Panics
///
/// A relic nobody has ported. Every later hook skips such a relic in silence,
/// which is a divergence with no name on it.
pub(crate) fn new(relic: Relic) -> State {
    match relic {
        Relic::Anchor => State::Anchor,
        Relic::BoneTea => State::BoneTea(bone_tea::State::default()),
        Relic::DivineRight => State::DivineRight,
        Relic::LeadPaperweight => State::LeadPaperweight,
        Relic::Pocketwatch => State::Pocketwatch(pocketwatch::State::default()),
        relic => unimplemented!("what {relic:?} does in a run that carries it"),
    }
}

/// Deliver a notification to one relic.
pub(crate) fn event_hook(state: &mut State, hook: EventHook<V>) -> Effects {
    match state {
        State::Anchor => anchor::event_hook(hook),
        State::BoneTea(state) => bone_tea::event_hook(state, hook),
        State::DivineRight => divine_right::event_hook(hook),
        State::LeadPaperweight => lead_paperweight::event_hook(hook),
        State::Pocketwatch(state) => pocketwatch::event_hook(state, hook),
    }
}

/// Ask one relic to change a value.
pub(crate) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match state {
        State::Pocketwatch(state) => pocketwatch::modify_hook(state, hook),
        // The rest change no value a hook asks about.
        State::Anchor | State::BoneTea(_) | State::DivineRight | State::LeadPaperweight => {
            Modify::Nothing
        }
    }
}

/// The number a relic shows, which is `RelicModel.DisplayAmount`.
///
/// The table answers for a relic that prints a constant, and the rest work it
/// out from what they saved.
///
/// # Panics
///
/// A relic that works its number out and nobody has ported.
pub(crate) fn display_amount(relic: Relic, state: &State) -> Number {
    match Table::data(relic).display_amount {
        // The table holds the column's own width, and a relic counts in the
        // build's game number. Widening here is the only place the two meet.
        Field::Constant(amount) => Number::from(amount),
        Field::Computed => match state {
            State::BoneTea(state) => bone_tea::display_amount(state),
            State::Pocketwatch(state) => pocketwatch::display_amount(state),
            _ => unimplemented!("the number {relic:?} shows"),
        },
    }
}
