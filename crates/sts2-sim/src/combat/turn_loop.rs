//! The turn loop as a coroutine, from the player's turn end to the start of
//! the player's next turn. It runs `EndPlayerTurnPhaseOne`,
//! `EndPlayerTurnPhaseTwo`, `SwitchSides` and the enemies' turn.
//!
//! The post-play phase, `BeforeSideTurnEnd`, `DoTurnEnd`, `BeforeFlushLate`,
//! `AfterTurnEnd` and each enemy's move can stop on a question. `diapause`
//! turns the straight-line code below into the state it waits in, which
//! [`Live::deferred`](crate::combat::live::Live) holds and clones. The
//! player's next turn starts once it is over.

use smallvec::SmallVec;
use sts2_core::{CombatId, Rules, SideKinds};

use super::{Error, Fighting};
use crate::combat::board::Board;
use crate::combat::live::{CREATURES, Paused, Skips};
use crate::combat::setup::Lent;
use crate::combat::turn::{InHand, Moved, PassRest, Stood, TurnEndPass};

/// How the turn loop came out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Looped<V: Rules> {
    /// The fight ended, and the turn loop stops.
    Ended(crate::combat::Ended),
    /// The player's next turn starts, with what skips the tick this turn.
    /// `extra` is a turn `ShouldTakeExtraTurn` gave.
    PlayerTurn { skipping: Skips<V>, extra: bool },
}

/// What a hook that waits for its listeners came to. It holds how the hook
/// ended the fight, and the tasks of the listeners that paused, in order.
type Waits<V> = (Option<crate::combat::Ended>, Option<Vec<Paused<V>>>);

/// The turn loop, from the player's turn end to the start of the next.
///
/// It yields on each question it stops on, which stands in
/// [`Live::asking`](crate::combat::live::Live). Some hooks wait for their
/// paused listeners. At each of those, it yields the listeners' tasks in
/// the order they paused, and they go on before the turn loop does.
#[diapause::coroutine(yield = Option<Vec<Paused<V>>>, resume = for<'f> Lent<'f, V>)]
#[derive(Debug, Clone, PartialEq)]
pub(super) fn turn_loop<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
) -> Looped<V> {
    // The post-play phase. An auto-play there that asks stands the turn's
    // end still behind the answer.
    let mut pass: PassRest<V> = post_play_pass::<V>(seat, resumed!());
    if post_play::<V>(seat, resumed!(), &mut pass) {
        loop {
            yield_!(None);
            if let Some(ended) = resumed!().ended {
                return Looped::Ended(ended);
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
            return Looped::Ended(ended);
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
        Err(ended) => return Looped::Ended(ended),
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
                return Looped::Ended(ended);
            }
            done = in_hand_from::<V>(seat, ascension, resumed!(), held, &mut skipping);
            if let InHand::Done(Some(by_hand)) = done {
                resumed!().live.skip_tick = skipping;
                return Looped::Ended(by_hand);
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
            return Looped::Ended(ended);
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
    if let Some(by_turn_end) = ended {
        let lent = resumed!();
        lent.live.skip_tick = skipping;
        crate::combat::turn::ended_at_turn_end::<V>(seat, lent.live, by_turn_end);
        return Looped::Ended(by_turn_end);
    }
    if extra_turn::<V>(seat, resumed!()) {
        return Looped::PlayerTurn {
            skipping,
            extra: true,
        };
    }

    // `SwitchSides` to the enemy and its `StartTurn`, then
    // `ExecuteEnemyTurn`: each enemy that acts performs what it advertised,
    // in turn. Each is held against the fight again before it acts, since
    // an earlier move can take a later one out. A move that stands still on
    // a question stands the turn still with it.
    let acting: SmallVec<[CombatId; CREATURES]> =
        match enemy_side_starts::<V>(seat, ascension, resumed!()) {
            Ok(acting) => acting,
            Err(at_start) => {
                resumed!().live.skip_tick = skipping;
                return Looped::Ended(at_start);
            }
        };
    let mut next: usize = 0;
    let mut moved: Moved<V> = enemies_act::<V>(resumed!(), &mut skipping, &acting, &mut next);
    if let Moved::Stood(_) = moved {
        loop {
            let stood: Stood<V> = match std::mem::replace(&mut moved, Moved::Over(None)) {
                Moved::Stood(stood) => stood,
                // `CheckWinCondition` after each enemy's turn: an ended
                // fight gives the enemies after it no turn.
                Moved::Over(by_move) => {
                    if by_move.is_some() || next >= acting.len() {
                        moved = Moved::Over(by_move);
                        break;
                    }
                    moved = enemies_act::<V>(resumed!(), &mut skipping, &acting, &mut next);
                    continue;
                }
            };
            resumed!().live.skip_tick = skipping;
            yield_!(None);
            skipping = std::mem::take(&mut resumed!().live.skip_tick);
            moved = move_goes_on::<V>(seat, resumed!(), &mut skipping, stood);
        }
    }
    let over: Option<crate::combat::Ended> = match moved {
        Moved::Over(by_enemies) => by_enemies,
        Moved::Stood(_) => unreachable!("a move standing still after the enemies' turn"),
    };
    // `EndEnemyTurn` is gated on `IsInProgress`, which both endings clear.
    // So a fight that ended during the enemy turn ends the turn with it.
    if let Some(by_enemies) = over {
        resumed!().live.skip_tick = skipping;
        return Looped::Ended(by_enemies);
    }
    if let Some(at_enemy_end) = enemy_turn_ends::<V>(seat, ascension, resumed!(), &mut skipping) {
        resumed!().live.skip_tick = skipping;
        return Looped::Ended(at_enemy_end);
    }
    Looped::PlayerTurn {
        skipping,
        extra: false,
    }
}

/// Whether the player takes an extra turn, with it set up where so.
fn extra_turn<V: Rules>(seat: sts2_core::SeatIndex, lent: &mut Lent<'_, V>) -> bool {
    let (live, _, belts) = lent.parts();
    crate::combat::turn::takes_extra_turn::<V>(seat, live, belts)
}

/// The enemy side's turn start, and the enemies that act in it.
fn enemy_side_starts<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
) -> Result<SmallVec<[CombatId; CREATURES]>, crate::combat::Ended> {
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::enemy_side_starts::<V>(seat, live, rngs, belts, ascension)
}

/// The enemies from `next` on act, until a move stands still.
fn enemies_act<V: Rules>(
    lent: &mut Lent<'_, V>,
    skipping: &mut Skips<V>,
    acting: &[CombatId],
    next: &mut usize,
) -> Moved<V> {
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::enemies_act::<V>(live, rngs, belts, skipping, acting, next)
}

/// A move that stood still, once what it stood still on is over.
fn move_goes_on<V: Rules>(
    seat: sts2_core::SeatIndex,
    lent: &mut Lent<'_, V>,
    skipping: &mut Skips<V>,
    stood: Stood<V>,
) -> Moved<V> {
    let answer = lent.taken.take();
    let ended = lent.ended;
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::move_goes_on::<V>(seat, live, rngs, belts, skipping, stood, answer, ended)
}

/// The enemies' turn end and the switch back to the player.
fn enemy_turn_ends<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
    skipping: &mut Skips<V>,
) -> Option<crate::combat::Ended> {
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::enemy_turn_ends::<V>(seat, live, rngs, belts, ascension, skipping)
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
    let mut looping = turn_loop::<V>(seat, ascension);
    let state = looping.start_with(Lent {
        live: &mut *fighting.live,
        rngs: fighting.streams.combat(),
        belts: &mut *fighting.belts,
        ended: None,
        taken: None,
    });
    match state {
        diapause::CoroutineState::Yielded(tasks) => {
            park(seat, fighting, max_energy, Box::new(looping), tasks);
            Ok(None)
        }
        diapause::CoroutineState::Complete(done) => {
            complete(seat, fighting, ascension, max_energy, done)
        }
    }
}

/// The turn loop from where `looping` stopped, once what it waited for is
/// over. `ended` is how that ended the fight, and `taken` is the player's
/// answer where it waited on its own question.
pub(super) fn finish<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
    mut looping: Box<turn_loop::State<V>>,
    ended: Option<crate::combat::Ended>,
    taken: Option<sts2_core::Taken<V>>,
) -> Result<Option<crate::combat::Ended>, Error> {
    let state = diapause::Coroutine::resume(
        &mut *looping,
        Lent {
            live: &mut *fighting.live,
            rngs: fighting.streams.combat(),
            belts: &mut *fighting.belts,
            ended,
            taken,
        },
    );
    match state {
        diapause::CoroutineState::Yielded(tasks) => {
            park(seat, fighting, max_energy, looping, tasks);
            Ok(None)
        }
        diapause::CoroutineState::Complete(done) => {
            complete(seat, fighting, ascension, max_energy, done)
        }
    }
}

/// The turn loop waiting, behind the tasks of the listeners that paused
/// where it yielded some. It is `Task.WhenAll`: the tasks go on in the
/// order they paused, then the turn loop.
fn park<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    max_energy: V::Count,
    looping: Box<turn_loop::State<V>>,
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
        .defer(crate::combat::live::Deferred::TurnLoop {
            looping,
            max_energy,
        });
    if let Some(first) = first {
        fighting.live.rejoin(seat, first);
    }
}

/// The turn loop over: the fight's end, or the player's next turn.
fn complete<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
    done: Looped<V>,
) -> Result<Option<crate::combat::Ended>, Error> {
    match done {
        Looped::Ended(ended) => Ok(Some(ended)),
        Looped::PlayerTurn { skipping, extra } => crate::combat::turn::start_player_turn(
            seat, fighting, ascension, max_energy, skipping, extra,
        ),
    }
}
