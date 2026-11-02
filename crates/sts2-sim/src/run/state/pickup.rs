//! A pickup as a coroutine.
//!
//! A relic's pickup, an event's option and a rest site's option hand the
//! run a list of effects. The first that asks a question stands the rest
//! still until the answer, as the game's `await`s do. `diapause` turns the
//! loop below into the state it waits in, which the room holds and clones.
//! An effect that runs a list of its own delegates to another pickup, so a
//! question inside a question is a coroutine inside a coroutine.

use sts2_core::{Answer, Rules};

use super::{After, Pickup, RunWalking, State, Stepped, Steps, Then};

/// The run, lent to a pickup for one step, and the answer to the question
/// the pickup asked, where it asked one.
pub(in crate::run) struct Lent<'a, V: Rules> {
    pub(in crate::run) run: &'a mut State<V>,
    pub(in crate::run) answer: Option<Answer>,
}

/// `seat`'s pickup of `steps`, in order. It yields each question it asks.
///
/// A pickup inside it is driven here, a step at a time, with the run lent
/// again for each step. `yield_all!` starts its delegate with nothing lent,
/// and a pickup's first step needs the run.
#[diapause::coroutine(yield = Pickup<V>, resume = for<'f> Lent<'f, V>)]
#[derive(Debug, Clone, PartialEq)]
pub(in crate::run) fn pickup<V: Rules>(seat: sts2_core::SeatIndex, steps: Steps<V>) {
    let mut at: usize = 0;
    while at < steps.len() {
        let then: Then<V> = steps[at].clone();
        at += 1;
        let mut stepped: Stepped<V> = resumed!().run.step(seat, then);
        loop {
            match std::mem::replace(&mut stepped, Stepped::Done) {
                Stepped::Done => break,
                Stepped::Ask(question) => {
                    let asked: Pickup<V> = question;
                    yield_!(asked.clone());
                    stepped = answered::<V>(seat, resumed!(), asked);
                }
                // A walk over a hook's listeners. What each asks for runs
                // before the next is asked.
                Stepped::Walk(walking) => {
                    let mut walk: RunWalking<V> = walking;
                    stepped = match resumed!().run.walk_next(&mut walk) {
                        None => Stepped::Done,
                        Some((owner, effects)) => {
                            Stepped::Nested(owner, effects, After::Walk(walk))
                        }
                    };
                }
                Stepped::Nested(owner, inner, after) => {
                    let then: After<V> = after;
                    let mut sub: Box<pickup::State<V>> = Box::new(pickup::<V>(owner, inner));
                    let mut asked: Option<Pickup<V>> = drive::<V>(&mut sub, false, resumed!());
                    loop {
                        let Some(question) = asked.take() else {
                            break;
                        };
                        yield_!(question);
                        asked = drive::<V>(&mut sub, true, resumed!());
                    }
                    stepped = resumed!().run.after_nested(seat, then);
                }
            }
        }
    }
}

/// One step of a pickup inside another, with the run lent again: its
/// start, or once `started`, its resume. The question it asks, if any.
fn drive<V: Rules>(
    sub: &mut pickup::State<V>,
    started: bool,
    lent: &mut Lent<'_, V>,
) -> Option<Pickup<V>> {
    let lent = Lent {
        run: &mut *lent.run,
        answer: lent.answer.take(),
    };
    let state = match started {
        false => sub.start_with(lent),
        true => diapause::Coroutine::resume(sub, lent),
    };
    match state {
        diapause::CoroutineState::Yielded(question) => Some(question),
        diapause::CoroutineState::Complete(()) => None,
    }
}

/// What the answer to `asked` comes to.
fn answered<V: Rules>(
    seat: sts2_core::SeatIndex,
    lent: &mut Lent<'_, V>,
    asked: Pickup<V>,
) -> Stepped<V> {
    let answer = lent
        .answer
        .take()
        .unwrap_or_else(|| unreachable!("a pickup resumed with the answer to its question"));
    lent.run.answered(seat, asked, &answer)
}
