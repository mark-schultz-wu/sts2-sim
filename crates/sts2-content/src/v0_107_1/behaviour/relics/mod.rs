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
mod arcane_scroll;
mod bone_tea;
mod burning_blood;
mod cracked_core;
mod cursed_pearl;
mod divine_right;
mod dollys_mirror;
mod golden_pearl;
mod gorget;
mod gremlin_horn;
mod hefty_tablet;
mod lead_paperweight;
mod mercury_hourglass;
mod neows_torment;
mod new_leaf;
mod permafrost;
mod phial_holster;
mod pocketwatch;
mod pomander;
mod precise_scissors;
mod ring_of_the_snake;
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
    ArcaneScroll,
    BoneTea(bone_tea::State),
    BurningBlood,
    CrackedCore,
    CursedPearl,
    DivineRight,
    DollysMirror,
    GoldenPearl,
    Gorget,
    GremlinHorn,
    HeftyTablet,
    LeadPaperweight,
    MercuryHourglass,
    NeowsTorment,
    NewLeaf,
    Permafrost(permafrost::State),
    PhialHolster,
    Pocketwatch(pocketwatch::State),
    Pomander,
    PreciseScissors,
    RingOfTheSnake,
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
        State::ArcaneScroll => arcane_scroll::event_hook(hook, asked),
        State::BoneTea(state) => bone_tea::event_hook(state, hook, asked),
        State::BurningBlood => burning_blood::event_hook(hook, asked),
        State::CrackedCore => cracked_core::event_hook(hook, asked),
        State::CursedPearl => cursed_pearl::event_hook(hook, asked),
        State::DivineRight => divine_right::event_hook(hook, asked),
        State::DollysMirror => dollys_mirror::event_hook(hook, asked),
        State::GoldenPearl => golden_pearl::event_hook(hook, asked),
        State::Gorget => gorget::event_hook(hook, asked),
        State::GremlinHorn => gremlin_horn::event_hook(hook, asked),
        State::HeftyTablet => hefty_tablet::event_hook(hook, asked),
        State::LeadPaperweight => lead_paperweight::event_hook(hook),
        State::MercuryHourglass => mercury_hourglass::event_hook(hook, asked),
        State::NeowsTorment => neows_torment::event_hook(hook, asked),
        State::NewLeaf => new_leaf::event_hook(hook, asked),
        State::Permafrost(state) => permafrost::event_hook(state, hook, asked),
        State::PhialHolster => phial_holster::event_hook(hook, asked),
        State::Pocketwatch(state) => pocketwatch::event_hook(state, hook),
        State::Pomander => pomander::event_hook(hook, asked),
        State::PreciseScissors => precise_scissors::event_hook(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it
        // rather than tells it. `AfterObtained` reaches every relic, and its
        // own does nothing.
        State::RingOfTheSnake => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what RingOfTheSnake does for {hook:?}"),
        },
        State::SandCastle => sand_castle::event_hook(hook, asked),
    }
}

/// Ask one relic to change a value.
pub(crate) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match state {
        State::Pocketwatch(state) => pocketwatch::modify_hook(state, hook),
        State::RingOfTheSnake => ring_of_the_snake::modify_hook(hook),
        // The fight asks only a relic that overrides the hook, and none of
        // the rest overrides one of these.
        State::Anchor
        | State::ArcaneScroll
        | State::BoneTea(_)
        | State::BurningBlood
        | State::CrackedCore
        | State::CursedPearl
        | State::DivineRight
        | State::DollysMirror
        | State::GoldenPearl
        | State::Gorget
        | State::GremlinHorn
        | State::HeftyTablet
        | State::LeadPaperweight
        | State::MercuryHourglass
        | State::NeowsTorment
        | State::NewLeaf
        | State::Permafrost(_)
        | State::PhialHolster
        | State::Pomander
        | State::PreciseScissors
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
            Relic::ArcaneScroll => State::ArcaneScroll,
            Relic::BoneTea => State::BoneTea(bone_tea::State::default()),
            Relic::BurningBlood => State::BurningBlood,
            Relic::CrackedCore => State::CrackedCore,
            Relic::CursedPearl => State::CursedPearl,
            Relic::DivineRight => State::DivineRight,
            Relic::DollysMirror => State::DollysMirror,
            Relic::GoldenPearl => State::GoldenPearl,
            Relic::Gorget => State::Gorget,
            Relic::GremlinHorn => State::GremlinHorn,
            Relic::HeftyTablet => State::HeftyTablet,
            Relic::LeadPaperweight => State::LeadPaperweight,
            Relic::MercuryHourglass => State::MercuryHourglass,
            Relic::NeowsTorment => State::NeowsTorment,
            Relic::NewLeaf => State::NewLeaf,
            Relic::Permafrost => State::Permafrost(permafrost::State::default()),
            Relic::PhialHolster => State::PhialHolster,
            Relic::Pocketwatch => State::Pocketwatch(pocketwatch::State::default()),
            Relic::Pomander => State::Pomander,
            Relic::PreciseScissors => State::PreciseScissors,
            Relic::RingOfTheSnake => State::RingOfTheSnake,
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
