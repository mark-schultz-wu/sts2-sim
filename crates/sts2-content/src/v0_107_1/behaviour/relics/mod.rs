//! What a relic does, ported.
//!
//! One module per relic, the way `RelicModel`'s subclasses are one file each,
//! and one function here per hook. A relic gets an arm when a run can carry it
//! into the state the hook fires from.
//!
//! A relic with no arm asks for nothing and saves nothing. A run carries every
//! relic it picks up, so an unported one sits in the belt and does nothing.
//! The golden replay is what catches the fight where that silence is wrong.

mod bone_tea;
mod divine_right;
mod pocketwatch;

use sts2_core::{EventHook, Modify, ModifyHook, Table};

use crate::Field;

use super::super::V0_107_1 as V;
use super::super::generated::relics::Relic;

pub(crate) type Number = <V as sts2_core::ContentVersion>::Number;
type Effects = Vec<sts2_core::HookEffect<V>>;

/// What one relic's own model saves.
///
/// One variant per relic that saves anything, holding the fields that relic's
/// `[SavedProperty]` declares. `Nothing` is every other relic, which is most
/// of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Nothing,
    BoneTea(bone_tea::State),
    Pocketwatch(pocketwatch::State),
}

/// One relic's model as the run picks it up.
pub(crate) fn new(relic: Relic) -> State {
    match relic {
        Relic::BoneTea => State::BoneTea(bone_tea::State::default()),
        Relic::Pocketwatch => State::Pocketwatch(pocketwatch::State::default()),
        _ => State::Nothing,
    }
}

/// Deliver a notification to one relic.
pub(crate) fn event_hook(relic: Relic, state: &mut State, hook: EventHook<V>) -> Effects {
    match (relic, state) {
        (Relic::DivineRight, _) => divine_right::event_hook(hook),
        (Relic::BoneTea, State::BoneTea(state)) => bone_tea::event_hook(state, hook),
        (Relic::Pocketwatch, State::Pocketwatch(state)) => pocketwatch::event_hook(state, hook),
        _ => Vec::new(),
    }
}

/// Ask one relic to change a value.
pub(crate) fn modify_hook(relic: Relic, state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match (relic, state) {
        (Relic::Pocketwatch, State::Pocketwatch(state)) => pocketwatch::modify_hook(state, hook),
        _ => Modify::Nothing,
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
            State::Nothing => unimplemented!("the number {relic:?} shows"),
        },
    }
}
