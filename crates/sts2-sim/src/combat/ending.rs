//! The player's turn end as a coroutine: `EndPlayerTurnPhaseOne` and
//! `EndPlayerTurnPhaseTwo`.
//!
//! The post-play phase, `BeforeSideTurnEnd`, `DoTurnEnd`, `BeforeFlushLate`
//! and `AfterTurnEnd` can each stop on a question. `diapause` turns the
//! straight-line code below into the state it waits in, which
//! [`Live::deferred`](crate::combat::live::Live) holds and clones. The
//! enemies' turn and the player's next run once it is over.

use sts2_core::{Rules, SideKinds};

use super::{Error, Fighting};
use crate::combat::board::Board;
use crate::combat::live::{Paused, Skips};
use crate::combat::setup::Lent;
use crate::combat::turn::{InHand, PassRest, TurnEndPass};

/// How the player's turn end came out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum TurnEnd<V: Rules> {
    /// The fight ended before the flush, and the turn's end stops.
    Ended(crate::combat::Ended),
    /// `AfterTurnEnd` over, with how it ended the fight and what skips
    /// the tick this turn.
    AfterTurnEnd {
        ended: Option<crate::combat::Ended>,
        skipping: Skips<V>,
    },
}

/// What a hook that waits for its listeners came to. It holds how the hook
/// ended the fight, and the tasks of the listeners that paused, in order.
type Waits<V> = (Option<crate::combat::Ended>, Option<Vec<Paused<V>>>);

/// The player's turn end, to the end of `AfterTurnEnd`.
///
/// It yields on each question it stops on, which stands in
/// [`Live::asking`](crate::combat::live::Live). Some hooks wait for their
/// paused listeners. At each of those, it yields the listeners' tasks in
/// the order they paused, and they go on before the turn's end does.
#[diapause::coroutine(yield = Option<Vec<Paused<V>>>, resume = for<'f> Lent<'f, V>)]
#[derive(Debug, Clone, PartialEq)]
pub(super) fn player_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
) -> TurnEnd<V> {
    // The post-play phase. An auto-play there that asks stands the turn's
    // end still behind the answer.
    let mut pass: PassRest<V> = post_play_pass::<V>(seat, resumed!());
    if post_play::<V>(seat, resumed!(), &mut pass) {
        loop {
            yield_!(None);
            if let Some(ended) = resumed!().ended {
                return TurnEnd::Ended(ended);
            }
            if !post_play::<V>(seat, resumed!(), &mut pass) {
                break;
            }
        }
    }

    // `BeforeSideTurnEnd`, whose `WhenAll` waits for every listener that
    // paused, each in the order it paused.
    let mut skipping: Skips<V> = std::mem::take(&mut resumed!().live.skip_tick);
    let hooks: Waits<V> = match before_turn_end::<V>(seat, ascension, resumed!()) {
        Ok(hooks) => hooks,
        Err(ended) => {
            resumed!().live.skip_tick = skipping;
            return TurnEnd::Ended(ended);
        }
    };
    let mut over: Option<crate::combat::Ended> = hooks.0;
    if hooks.1.is_some() {
        yield_!(hooks.1);
        over = over.or(resumed!().ended);
    }

    // `CheckWinCondition`, whatever ended the fight, then `DoTurnEnd`. A
    // card's effect that asks stands the rest still behind the answer.
    let mut done: InHand<V> = match turn_end::<V>(seat, ascension, over, resumed!(), &mut skipping)
    {
        Ok(done) => done,
        Err(ended) => return TurnEnd::Ended(ended),
    };
    if let InHand::Parked { .. } = done {
        loop {
            let held: (sts2_core::CardId<V>, crate::combat::turn::Rest<V>) =
                match std::mem::replace(&mut done, InHand::Done(None)) {
                    InHand::Parked { card, rest: left } => (card, left),
                    InHand::Done(_) => break,
                };
            yield_!(None);
            if let Some(ended) = resumed!().ended {
                return TurnEnd::Ended(ended);
            }
            done = in_hand_from::<V>(seat, ascension, resumed!(), held, &mut skipping);
            if let InHand::Done(Some(by_hand)) = done {
                resumed!().live.skip_tick = skipping;
                return TurnEnd::Ended(by_hand);
            }
        }
    }

    // `Hook.BeforeFlush`'s late pass, whose `WhenAll` waits for every
    // listener that paused on its pick.
    let picking: Option<Vec<Paused<V>>> = {
        let (live, rngs, belts) = resumed!().parts();
        let tasks = crate::combat::turn::before_flush_late::<V>(seat, live, rngs, belts);
        (!tasks.is_empty()).then_some(tasks)
    };
    if picking.is_some() {
        yield_!(picking);
        if let Some(ended) = resumed!().ended {
            return TurnEnd::Ended(ended);
        }
    }

    // `EndPlayerTurnPhaseTwo`: the flush and the cards' cleanup, then
    // `Hook.AfterTurnEnd`. Its `WhenAll` waits for each pass's listeners
    // that paused before the next pass, and a fight that ended skips the
    // late pass.
    flush::<V>(seat, ascension, resumed!());
    let main: Waits<V> = after_turn_end::<V>(
        seat,
        ascension,
        resumed!(),
        &mut skipping,
        TurnEndPass::Main,
    );
    let mut ended: Option<crate::combat::Ended> = main.0;
    if main.1.is_some() {
        yield_!(main.1);
        ended = ended.or(resumed!().ended);
    }
    if ended.is_none() {
        let late: Waits<V> = after_turn_end::<V>(
            seat,
            ascension,
            resumed!(),
            &mut skipping,
            TurnEndPass::Late,
        );
        ended = late.0;
        if late.1.is_some() {
            yield_!(late.1);
            ended = ended.or(resumed!().ended);
        }
    }
    TurnEnd::AfterTurnEnd { ended, skipping }
}

/// The hand's flush, then the cards' cleanup.
fn flush<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
) {
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::flush_hand::<V>(seat, live, rngs, belts, ascension);
    crate::combat::turn::end_of_turn_cleanup::<V>(seat, live);
}

/// The player's pass `pass` of `AfterTurnEnd`, and the tasks of its
/// listeners that paused on a draw's question.
fn after_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
    skipping: &mut Skips<V>,
    pass: TurnEndPass,
) -> Waits<V> {
    let (live, rngs, belts) = lent.parts();
    let side = <V::CombatSide as SideKinds>::PLAYER;
    let (ended, listening) = crate::combat::turn::after_turn_end_pass::<V>(
        seat, live, rngs, belts, ascension, skipping, side, pass, true,
    );
    let tasks = listening.into_tasks();
    (ended, (!tasks.is_empty()).then_some(tasks))
}

/// The post-play phase's walk, with the phase turned to it.
fn post_play_pass<V: Rules>(seat: sts2_core::SeatIndex, lent: &mut Lent<'_, V>) -> PassRest<V> {
    let (live, _, belts) = lent.parts();
    live.players[seat].phase = <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::AUTO_POST_PLAY;
    let turn = live.turn;
    PassRest::walk(
        live,
        belts,
        sts2_core::EventHook::AutoPostPlayPhase { turn },
    )
}

/// The post-play phase from `pass`: true where an auto-play asked.
fn post_play<V: Rules>(
    seat: sts2_core::SeatIndex,
    lent: &mut Lent<'_, V>,
    pass: &mut PassRest<V>,
) -> bool {
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::post_play_until_asked::<V>(seat, live, rngs, belts, pass)
}

/// The win check once the post-play phase is over, then `BeforeSideTurnEnd`.
/// The post-play hooks run as an action, and `ActionExecutor` checks the win
/// after each. So an auto-play there that killed the last enemy ends the
/// fight before the turn-end hooks, which hear nothing.
fn before_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
) -> Result<Waits<V>, crate::combat::Ended> {
    let (live, mut rngs, belts) = lent.parts();
    let mut board = Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
    board.end_if_ending();
    if let Some(ended) = board.ended() {
        return Err(ended);
    }
    if live.lost {
        return Err(crate::combat::Ended::Lost);
    }
    let (over, listening) = crate::combat::turn::before_turn_end::<V>(
        seat,
        live,
        rngs,
        belts,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
    );
    let tasks = listening.into_tasks();
    if let (Some(ended), false) = (over, tasks.is_empty()) {
        unimplemented!("a turn-end listener paused in a fight that ended {ended:?}");
    }
    Ok((over, (!tasks.is_empty()).then_some(tasks)))
}

/// `CheckWinCondition` with `over`, how the turn-end hooks ended the fight,
/// then `DoTurnEnd`, with what it put on the player merged into `skipping`.
fn turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    over: Option<crate::combat::Ended>,
    lent: &mut Lent<'_, V>,
    skipping: &mut Skips<V>,
) -> Result<InHand<V>, crate::combat::Ended> {
    let (live, mut rngs, belts) = lent.parts();
    let ended = over.or_else(|| {
        let mut board = Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
        board.end_if_ending();
        board.ended()
    });
    if let Some(ended) = ended {
        live.skip_tick = *skipping;
        return Err(ended);
    }
    let done = crate::combat::turn::turn_end::<V>(seat, live, rngs, belts, ascension);
    merge_landed::<V>(live, skipping);
    match done {
        InHand::Done(Some(ended)) => {
            live.skip_tick = *skipping;
            Err(ended)
        }
        done => Ok(done),
    }
}

/// `DoTurnEnd` from the end of the parked card's wrapper, once its question
/// is answered, with what it put on the player merged into `skipping`.
fn in_hand_from<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
    held: (sts2_core::CardId<V>, crate::combat::turn::Rest<V>),
    skipping: &mut Skips<V>,
) -> InHand<V> {
    let (live, rngs, belts) = lent.parts();
    let done =
        crate::combat::turn::in_hand_from::<V>(seat, live, rngs, belts, ascension, held.0, &held.1);
    merge_landed::<V>(live, skipping);
    done
}

/// What `DoTurnEnd` put on the player skips this turn's tick too.
fn merge_landed<V: Rules>(live: &mut crate::combat::live::Live<V>, skipping: &mut Skips<V>) {
    let landed = std::mem::take(&mut live.skip_tick);
    crate::combat::play::merge::<V>(skipping, landed.iter().copied().collect());
}

/// End the player's turn, then run the enemies' turn and the player's next.
pub(super) fn begin<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
) -> Result<Option<crate::combat::Ended>, Error> {
    let mut ending = player_turn_end::<V>(seat, ascension);
    let state = ending.start_with(Lent {
        live: &mut *fighting.live,
        rngs: fighting.streams.combat(),
        belts: &mut *fighting.belts,
        ended: None,
    });
    match state {
        diapause::CoroutineState::Yielded(tasks) => {
            park(seat, fighting, max_energy, Box::new(ending), tasks);
            Ok(None)
        }
        diapause::CoroutineState::Complete(done) => {
            complete(seat, fighting, ascension, max_energy, done)
        }
    }
}

/// The player's turn end from where `ending` stopped, once what it waited
/// for is over. `ended` is how that ended the fight.
pub(super) fn finish<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
    mut ending: Box<player_turn_end::State<V>>,
    ended: Option<crate::combat::Ended>,
) -> Result<Option<crate::combat::Ended>, Error> {
    let state = diapause::Coroutine::resume(
        &mut *ending,
        Lent {
            live: &mut *fighting.live,
            rngs: fighting.streams.combat(),
            belts: &mut *fighting.belts,
            ended,
        },
    );
    match state {
        diapause::CoroutineState::Yielded(tasks) => {
            park(seat, fighting, max_energy, ending, tasks);
            Ok(None)
        }
        diapause::CoroutineState::Complete(done) => {
            complete(seat, fighting, ascension, max_energy, done)
        }
    }
}

/// The turn's end waiting, behind the tasks of the listeners that paused
/// where it yielded some. It is `Task.WhenAll`: the tasks go on in the
/// order they paused, then the turn's end.
fn park<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    max_energy: V::Count,
    ending: Box<player_turn_end::State<V>>,
    tasks: Option<Vec<Paused<V>>>,
) {
    let mut tasks = tasks.into_iter().flatten();
    let first = tasks.next();
    for task in tasks {
        fighting
            .live
            .defer(crate::combat::live::Deferred::Rejoin(Box::new(task)));
    }
    fighting
        .live
        .defer(crate::combat::live::Deferred::EndingTurn { ending, max_energy });
    if let Some(first) = first {
        fighting.live.rejoin(seat, first);
    }
}

/// The turn's end over: the fight's end, or the enemies' turn and the
/// player's next.
fn complete<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
    done: TurnEnd<V>,
) -> Result<Option<crate::combat::Ended>, Error> {
    match done {
        TurnEnd::Ended(ended) => Ok(Some(ended)),
        TurnEnd::AfterTurnEnd { ended, skipping } => crate::combat::turn::after_player_turn_end(
            seat, fighting, ascension, max_energy, skipping, ended,
        ),
    }
}
