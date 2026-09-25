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
mod dollys_mirror;
mod gremlin_horn;
mod lead_paperweight;
mod permafrost;
mod pocketwatch;
mod sand_castle;

use sts2_core::{EventHook, Modify, ModifyHook, Table};

use super::super::V0_107_1 as V;
use super::super::generated::relics::Relic;

use crate::Field;

pub(crate) type Number = <V as sts2_core::ContentVersion>::Number;

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
    DollysMirror,
    GremlinHorn,
    LeadPaperweight,
    Permafrost(permafrost::State),
    Pocketwatch(pocketwatch::State),
    SandCastle,
}

/// Deliver a notification to one relic.
pub(crate) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match state {
        State::Anchor => anchor::event_hook(hook, asked),
        State::BoneTea(state) => bone_tea::event_hook(state, hook, asked),
        State::DivineRight => divine_right::event_hook(hook, asked),
        State::DollysMirror => dollys_mirror::event_hook(hook, asked),
        State::GremlinHorn => gremlin_horn::event_hook(hook, asked),
        State::LeadPaperweight => lead_paperweight::event_hook(hook),
        State::Permafrost(state) => permafrost::event_hook(state, hook, asked),
        State::Pocketwatch(state) => pocketwatch::event_hook(state, hook),
        State::SandCastle => sand_castle::event_hook(hook, asked),
    }
}

/// Ask one relic to change a value.
pub(crate) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match state {
        State::Pocketwatch(state) => pocketwatch::modify_hook(state, hook),
        // The fight asks only a relic that overrides the hook, and none of
        // the rest overrides one of these.
        State::Anchor
        | State::BoneTea(_)
        | State::DivineRight
        | State::DollysMirror
        | State::GremlinHorn
        | State::LeadPaperweight
        | State::Permafrost(_)
        | State::SandCastle => unimplemented!("what {state:?} does for {hook:?}"),
    }
}

impl sts2_core::RelicModel<V> for State {
    /// One relic's model as the run picks it up.
    ///
    /// # Panics
    ///
    /// A relic nobody has ported.
    fn new(relic: Relic) -> Self {
        match relic {
            Relic::Anchor => State::Anchor,
            Relic::BoneTea => State::BoneTea(bone_tea::State::default()),
            Relic::DivineRight => State::DivineRight,
            Relic::DollysMirror => State::DollysMirror,
            Relic::GremlinHorn => State::GremlinHorn,
            Relic::LeadPaperweight => State::LeadPaperweight,
            Relic::Permafrost => State::Permafrost(permafrost::State::default()),
            Relic::Pocketwatch => State::Pocketwatch(pocketwatch::State::default()),
            Relic::SandCastle => State::SandCastle,
            relic => unimplemented!("what {relic:?} does in a run that carries it"),
        }
    }

    /// `DisplayAmount`. The table answers for a relic that prints a constant,
    /// and the rest work it out from what they saved.
    ///
    /// # Panics
    ///
    /// A relic that works its number out and nobody has ported.
    fn display_amount(&self, relic: Relic) -> Number {
        match Table::data(relic).display_amount {
            // The table holds the column's own width, and a relic counts in
            // the build's game number. Widening here is the only place the two
            // meet.
            Field::Constant(amount) => Number::from(amount),
            Field::Computed => match self {
                State::BoneTea(state) => bone_tea::display_amount(state),
                State::Pocketwatch(state) => pocketwatch::display_amount(state),
                _ => unimplemented!("the number {relic:?} shows"),
            },
        }
    }
}
