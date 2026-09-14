//! The relics a run carries, as a fight asks them things.
//!
//! `RelicManager` walks the belt in panel order and gives every relic the
//! hook, which is what these do. A relic answers as its own model. A
//! notification writes the state the run carries for it, and a value hook
//! reads that state and leaves it.

use sts2_core::{EventHook, Fight, HookEffect, Listener, ModifyHook, Rules};

use crate::run::HeldRelic;

/// Deliver one notification to every relic, and gather what they ask for.
///
/// The effects come back in belt order, which is the order the game applies
/// them in.
pub(crate) fn notify<V: Rules>(
    relics: &mut [HeldRelic<V>],
    hook: EventHook<V>,
) -> Vec<HookEffect<V>> {
    relics
        .iter_mut()
        .flat_map(|held| V::event_hook(Listener::Relic(&mut held.state), hook))
        .collect()
}

/// Ask every relic to change one number.
pub(crate) fn modified<V: Rules>(
    relics: &mut [HeldRelic<V>],
    hook: ModifyHook<V>,
    amount: V::Number,
) -> V::Number {
    super::board::hooked::<V>(
        amount,
        relics
            .iter_mut()
            .map(|held| V::modify_hook(Listener::Relic(&mut held.state), hook)),
    )
}

/// `AfterCombatEnd`, which every relic gets once the fight is over.
///
/// The relics that counted something in the fight forget it here. The belt a
/// run carries out of a fight is then the belt the game's screens show.
pub(crate) fn combat_ended<V: Rules>(relics: &mut [HeldRelic<V>]) {
    asks_for_nothing::<V>(notify::<V>(relics, EventHook::CombatEnd), "as a fight ends");
}

/// `AfterCardPlayed`, for a card the player has just finished playing.
pub(crate) fn card_played<V: Rules>(
    relics: &mut [HeldRelic<V>],
    card: V::Card,
) -> Vec<HookEffect<V>> {
    notify::<V>(relics, EventHook::CardPlayed { card })
}

/// `BeforeCombatStart`, once the fight is built and before it opens.
pub(crate) fn combat_starting<V: Rules>(relics: &mut [HeldRelic<V>]) -> Vec<HookEffect<V>> {
    notify::<V>(relics, EventHook::CombatStarting)
}

/// `AfterObtained`, which `RelicCmd.Obtain` gives the relic it just handed
/// over and to no other.
pub(crate) fn obtained<V: Rules>(held: &mut HeldRelic<V>) {
    asks_for_nothing::<V>(
        notify::<V>(std::slice::from_mut(held), EventHook::Obtained),
        "as it is obtained",
    );
}

/// `BeforeSideTurnStart` then, once the hand is dealt, `AfterSideTurnStart`.
///
/// Only the player's side turn. The relics this build fields act on their
/// owner's turn, and the simulator has nowhere to put the enemy's yet.
pub(crate) fn turn_starting<V: Rules>(relics: &mut [HeldRelic<V>], turn: V::Count) {
    let asked = notify::<V>(
        relics,
        EventHook::BeforeSideTurnStart {
            side: <V::CombatSide as sts2_core::SideKinds>::PLAYER,
            turn,
        },
    );
    asks_for_nothing::<V>(asked, "before a turn starts");
}

/// What a hook nothing acts on came back with, which is nothing.
///
/// # Panics
///
/// A relic that asked for something here. The caller has no way to carry it
/// out, and a request dropped in silence is a divergence somewhere else.
fn asks_for_nothing<V: Rules>(asked: Vec<HookEffect<V>>, when: &str) {
    if let Some(effect) = asked.first() {
        unimplemented!("a relic asking for {effect:?} {when}")
    }
}

/// `AfterSideTurnStart`, which is the first hook to see the hand just dealt.
pub(crate) fn turn_started<V: Rules>(
    relics: &mut [HeldRelic<V>],
    turn: V::Count,
) -> Vec<HookEffect<V>> {
    notify::<V>(
        relics,
        EventHook::SideTurnStart {
            side: <V::CombatSide as sts2_core::SideKinds>::PLAYER,
            turn,
        },
    )
}

/// Carry out what a relic asked for, through the fight it asked in.
///
/// # Panics
///
/// An effect no fight can carry out here.
pub(crate) fn apply<V: Rules, F: Fight<V>>(fight: &mut F, asked: Vec<HookEffect<V>>) {
    for effect in asked {
        match effect {
            HookEffect::GainBlock(block) => fight.gain_block(block),
            other => unimplemented!("a relic asking for {other:?} inside a fight"),
        }
    }
}
