//! The powers the creatures in a fight carry, as it asks them things.
//!
//! `Hook` walks every model in the fight and gives each the notification,
//! which for the powers is what this does. A power answers as its own model,
//! reading the stack its creature carries it as and the fields it saves.

use sts2_core::{EventHook, Rules};

use super::live::Live;

/// `MonsterMoveState.OnPerform`, which the move state gives the powers on
/// the creature that performed it.
///
/// # Panics
///
/// A power asking for something once the move is performed, which nothing
/// here carries out.
pub(crate) fn move_performed<V: Rules>(
    live: &mut Live<V>,
    creature: sts2_core::CombatId,
    move_id: V::MoveId,
) {
    let hook = EventHook::MonsterMovePerformed { move_id };
    let mut asked = sts2_core::HookEffects::new();
    let Some(performer) = live.creatures.iter_mut().find(|c| c.id == creature) else {
        return;
    };
    let carrier = performer.carrier();
    for held in performer.powers.iter_mut() {
        <V as sts2_core::Hooks>::event_hook(held.event_listener(carrier), hook, &mut asked);
    }
    if let Some(effect) = asked.first() {
        unimplemented!("a power asking for {effect:?} after a monster move")
    }
}
