//! The hooks a relic is asked on its own rather than raised on the fight.
//!
//! A relic answers as its own model, and writes the state the run carries for
//! it. Every hook the fight raises reaches the belt through the walk.

use sts2_core::{EventHook, Rules};

/// `AfterObtained`, which `RelicCmd.Obtain` gives the relic it just handed
/// over and to no other, once it is on the belt.
pub(crate) fn obtained<V: Rules>(
    (state, belt): (&mut V::RelicState, &[V::Relic]),
    character: V::Character,
    act: usize,
    turn: Option<V::Count>,
) -> sts2_core::HookEffects<V> {
    let mut asked = sts2_core::HookEffects::new();
    V::relic_event_hook(
        state,
        belt,
        EventHook::Obtained {
            character,
            act,
            turn,
        },
        &mut asked,
    );
    asked
}

/// The code after the `await` on a command the relic in `slot` asked for,
/// once the command is over.
///
/// # Panics
///
/// A belt that changed under the command, or a relic that asks for more.
pub(crate) fn own_command_over<V: Rules>(
    belt: &mut crate::run::Belt<V>,
    slot: u8,
    relic: V::Relic,
) {
    let slot = usize::from(slot);
    if belt.get(slot).map(|held| held.relic) != Some(relic) {
        unimplemented!("the belt changing under {relic:?}'s own command");
    }
    let (state, belt) = belt.state_beside(slot);
    let mut asked = sts2_core::HookEffects::new();
    V::relic_event_hook(state, belt, EventHook::OwnCommandOver, &mut asked);
    if !asked.is_empty() {
        unimplemented!("{relic:?} asking for {asked:?} once its own command is over");
    }
}

/// Where the relic whose model is `state` sits: its seat, and its slot on
/// that belt. A walk hands back a relic's model and not its place.
///
/// # Panics
///
/// A model on no belt.
pub(crate) fn place_of<V: Rules>(
    belts: &crate::run::Belts<V>,
    state: *const V::RelicState,
) -> (sts2_core::SeatIndex, usize) {
    belts
        .seated()
        .find_map(|(seat, belt)| {
            let slot = belt
                .iter()
                .position(|held| std::ptr::eq(&held.state, state))?;
            Some((seat, slot))
        })
        .expect("a relic asked is on a belt")
}
