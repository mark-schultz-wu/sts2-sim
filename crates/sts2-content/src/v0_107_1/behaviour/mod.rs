//! What this build's content does, as ported code.
//!
//! The generated tables beside this say what the game's data holds. This says
//! what its code does with it. A row is dumped. A rule is read and written out
//! by hand, against the decompiled source.
//!
//! Filed the way `generated/` is, so a thing's behaviour sits under the same
//! name as its row.

pub(crate) mod cards;
pub mod events;
pub(crate) mod monsters;
pub(crate) mod powers;
pub(crate) mod relics;

use sts2_core::{Fight, Played, Taken};

use super::V0_107_1 as V;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Card(cards::Resume),
    Potion(crate::behaviour::potions::Resume),
    Monster(monsters::Resume),
}

impl From<crate::behaviour::potions::Resume> for Resume {
    fn from(at: crate::behaviour::potions::Resume) -> Self {
        Resume::Potion(at)
    }
}

pub(crate) fn resume<F: Fight<V>>(fight: &mut F, at: Resume, answer: Taken<V>) -> Played<V> {
    match (at, answer) {
        (Resume::Card(at), Taken::Selected(cards)) => cards::resume_play(fight, at, cards),
        (Resume::Card(at), Taken::Chosen(card)) => cards::resume_chosen(fight, at, card),
        (Resume::Card(at), Taken::Waited) => cards::resume_waited(fight, at),
        (Resume::Card(at), Taken::Drew(drew)) => cards::resume_drew(fight, at, &drew),
        (Resume::Potion(at), answer) => crate::behaviour::potions::resume(fight, at, answer),
        (Resume::Monster(at), Taken::Chosen(card)) => monsters::resume(fight, at, card),
        (at, answer) => unimplemented!("{at:?} resumed with {answer:?}"),
    }
}

impl crate::build::OwnRules for V {
    fn own_relic_event_hook(
        state: &mut crate::behaviour::relics::State<V>,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) {
        relics::own_relic_event_hook(state, hook, asked);
    }

    fn own_power(power: <V as sts2_core::ContentVersion>::Power) -> bool {
        powers::own_power(power)
    }

    fn own_power_state(
        power: <V as sts2_core::ContentVersion>::Power,
    ) -> Option<crate::behaviour::powers::State<V>> {
        powers::own_power_state(power)
    }

    fn own_power_modify_hook(
        power: <V as sts2_core::ContentVersion>::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, V>,
        state: &crate::behaviour::powers::State<V>,
        hook: sts2_core::ModifyHook<V>,
    ) -> Option<sts2_core::Modify<i32>> {
        powers::own_power_modify_hook(power, amount, held, state, hook)
    }

    fn own_power_event_hook(
        power: <V as sts2_core::ContentVersion>::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, V>,
        state: &mut crate::behaviour::powers::State<V>,
        hook: sts2_core::EventHook<V>,
    ) -> Option<Option<sts2_core::HookEffect<V>>> {
        powers::own_power_event_hook(power, amount, held, state, hook)
    }
}
