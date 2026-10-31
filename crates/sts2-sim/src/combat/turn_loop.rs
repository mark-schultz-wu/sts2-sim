//! The turn loop as a coroutine, from the player's turn end to the play
//! phase of the player's next turn. It runs `EndPlayerTurnPhaseOne`,
//! `EndPlayerTurnPhaseTwo`, `SwitchSides`, the enemies' turn and the
//! player's `StartTurn`. A fight's first turn runs the last of those alone.
//!
//! The post-play phase, `BeforeSideTurnEnd`, `DoTurnEnd`, `BeforeFlushLate`,
//! `AfterTurnEnd`, each enemy's move, `SetupPlayerTurn` and the pre-play
//! phase can stop on a question. `diapause` turns the straight-line code
//! below into the state it waits in, which
//! [`Live::deferred`](crate::combat::live::Live) holds and clones.

use smallvec::SmallVec;
use sts2_core::{CombatId, Rules, SideKinds};

use super::{Error, Fighting};
use crate::combat::board::Board;
use crate::combat::live::{CREATURES, Paused, Skips};
use crate::combat::setup::Lent;
use crate::combat::turn::{InHand, Moved, PassRest, PrePlayed, SideStarted, Stood, TurnEndPass};

/// What the turn loop waits on when it yields.
pub(super) enum Wait<V: Rules> {
    /// A question, which stands in [`Live::asking`](crate::combat::live::Live).
    Question,
    /// Every living player's pick of a monster's move, each on their queue.
    Picks,
    /// The tasks of the listeners that paused at a hook whose `WhenAll`
    /// waits for them, in the order they paused.
    Tasks(Vec<Paused<V>>),
    /// The task of a `SetupPlayerTurn` that stopped on a question, which
    /// `RunAutoPrePlayPhase` waits for. `skipping` is what skips the tick
    /// this turn, which stands once the choice the setup signalled comes up.
    Setup { task: Paused<V>, skipping: Skips<V> },
}

/// What a hook that waits for its listeners came to. It holds how the hook
/// ended the fight, and the tasks of the listeners that paused, in order.
type Waits<V> = (Option<crate::combat::Ended>, Option<Vec<Paused<V>>>);

/// The turn loop, from the player's turn end to the next turn's play phase.
/// A `first` turn is the fight's opening one, which only starts. How the
/// fight ended, if it did. It yields what it waits on, a [`Wait`].
#[diapause::coroutine(yield = Wait<V>, resume = for<'f> Lent<'f, V>)]
#[derive(Debug, Clone, PartialEq)]
pub(super) fn turn_loop<V: Rules>(
    seat: sts2_core::SeatIndex,
    ascension: V::AscensionLevel,
    first: bool,
) -> Result<Option<crate::combat::Ended>, Error> {
    let mut skipping: Skips<V> = Skips::<V>::default();
    let mut extra: bool = false;
    if !first {
        // A fight of several players runs each phase for every player, and no
        // pause holds it up yet.
        if several::<V>(resumed!()) {
            // `AfterAutoPostPlayPhaseEntered` for each player, each under its
            // own context. One that asks waits at the front of its player's
            // queue, and the turn's end waits for every one.
            if post_play_each::<V>(resumed!()) {
                yield_!(Wait::Picks);
                if let Some(ended) = resumed!().ended {
                    return Ok(Some(ended));
                }
            }
            if let Some(ended) = turn_ends_each::<V>(ascension, resumed!(), &mut skipping) {
                return Ok(Some(ended));
            }
            extra = extra_turns::<V>(resumed!());
        } else {
            // The post-play phase. An auto-play there that asks stands the turn's
            // end still behind the answer.
            let mut pass: PassRest<V> = post_play_pass::<V>(seat, resumed!());
            if post_play::<V>(seat, resumed!(), &mut pass) {
                loop {
                    yield_!(Wait::Question);
                    if let Some(ended) = resumed!().ended {
                        return Ok(Some(ended));
                    }
                    if !post_play::<V>(seat, resumed!(), &mut pass) {
                        break;
                    }
                }
            }
            post_play_over::<V>(seat, resumed!());

            // `BeforeSideTurnEnd`, whose `WhenAll` waits for every listener that
            // paused, each in the order it paused.
            skipping = std::mem::take(&mut resumed!().live.skip_tick);
            let hooks: Waits<V> = match before_turn_end::<V>(seat, ascension, resumed!()) {
                Ok(hooks) => hooks,
                Err(ended) => {
                    resumed!().live.skip_tick = skipping;
                    return Ok(Some(ended));
                }
            };
            let mut over: Option<crate::combat::Ended> = hooks.0;
            if let Some(tasks) = hooks.1 {
                yield_!(Wait::Tasks(tasks));
                over = over.or(resumed!().ended);
            }

            // `CheckWinCondition`, whatever ended the fight, then `DoTurnEnd`. A
            // card's effect that asks stands the rest still behind the answer.
            let mut done: InHand<V> =
                match turn_end::<V>(seat, ascension, over, resumed!(), &mut skipping) {
                    Ok(done) => done,
                    Err(ended) => return Ok(Some(ended)),
                };
            if let InHand::Parked { .. } = done {
                loop {
                    let held: (sts2_core::CardId<V>, crate::combat::turn::Rest<V>) =
                        match std::mem::replace(&mut done, InHand::Done(None)) {
                            InHand::Parked { card, rest: left } => (card, left),
                            InHand::Done(_) => break,
                        };
                    yield_!(Wait::Question);
                    if let Some(ended) = resumed!().ended {
                        return Ok(Some(ended));
                    }
                    done = in_hand_from::<V>(seat, ascension, resumed!(), held, &mut skipping);
                    if let InHand::Done(Some(by_hand)) = done {
                        resumed!().live.skip_tick = skipping;
                        return Ok(Some(by_hand));
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
            if let Some(tasks) = picking {
                yield_!(Wait::Tasks(tasks));
                if let Some(ended) = resumed!().ended {
                    return Ok(Some(ended));
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
            if let Some(tasks) = main.1 {
                yield_!(Wait::Tasks(tasks));
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
                if let Some(tasks) = late.1 {
                    yield_!(Wait::Tasks(tasks));
                    ended = ended.or(resumed!().ended);
                }
            }
            if let Some(by_turn_end) = ended {
                let lent = resumed!();
                lent.live.skip_tick = skipping;
                crate::combat::turn::ended_at_turn_end::<V>(lent.live, by_turn_end);
                return Ok(Some(by_turn_end));
            }
            extra = extra_turns::<V>(resumed!());
        }
        if !extra {
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
                        return Ok(Some(at_start));
                    }
                };
            let mut next: usize = 0;
            let mut moved: Moved<V> =
                enemies_act::<V>(resumed!(), &mut skipping, &acting, &mut next);
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
                    if stood.on_picks() {
                        yield_!(Wait::Picks);
                    } else {
                        yield_!(Wait::Question);
                    }
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
                return Ok(Some(by_enemies));
            }
            if let Some(at_enemy_end) =
                enemy_turn_ends::<V>(seat, ascension, resumed!(), &mut skipping)
            {
                resumed!().live.skip_tick = skipping;
                return Ok(Some(at_enemy_end));
            }
        }
    }

    // `StartTurn` for the player's side, to `OrbQueue.AfterTurnStart`.
    let started: Result<SideStarted<V>, Error> = side_starts::<V>(ascension, resumed!());
    let parked: Option<Paused<V>> = match started {
        Ok(SideStarted::Started { setup, ended: None }) => setup,
        Ok(SideStarted::Started {
            setup,
            ended: Some(at_side_start),
        }) => {
            if let Some(task) = setup {
                resumed!().live.rejoin(sts2_core::SeatIndex(0), task);
            }
            resumed!().live.skip_tick = skipping;
            return Ok(Some(at_side_start));
        }
        Ok(SideStarted::Ended(at_start)) => {
            resumed!().live.skip_tick = skipping;
            return Ok(Some(at_start));
        }
        Err(error) => return Err(error),
    };

    // `RunAutoPrePlayPhase` for each player. In a fight of several, a setup
    // that stopped on a question waits at the front of its player's queue.
    if several::<V>(resumed!()) {
        return pre_play_each::<V>(resumed!(), skipping);
    }

    // `RunAutoPrePlayPhase`, which waits for a setup that stopped on a
    // question. A choice it signalled comes up once it goes on.
    let mut pre: PrePlayed<V> = match parked {
        Some(task) => {
            yield_!(Wait::Setup { task, skipping });
            if let Some(by_setup) = resumed!().ended {
                return Ok(Some(by_setup));
            }
            pre_play::<V>(seat, resumed!())
        }
        None => {
            match come_up::<V>(seat, resumed!()) {
                Ok(None) => {}
                Ok(Some(came_up)) => {
                    resumed!().live.skip_tick = skipping;
                    return Ok(Some(came_up));
                }
                Err(error) => return Err(error),
            }
            let opening: PrePlayed<V> = pre_play::<V>(seat, resumed!());
            if let PrePlayed::Over(Some(by_pre_play)) = opening {
                resumed!().live.skip_tick = skipping;
                return Ok(Some(by_pre_play));
            }
            resumed!().live.skip_tick = skipping;
            opening
        }
    };
    if let PrePlayed::Over(Some(after_setup)) = pre {
        return Ok(Some(after_setup));
    }
    play_phase::<V>(seat, resumed!());
    // An auto-play there that asks stands the phase still behind the
    // answer.
    if let PrePlayed::Asked { .. } = pre {
        loop {
            let held: (PassRest<V>, usize) =
                match std::mem::replace(&mut pre, PrePlayed::Over(None)) {
                    PrePlayed::Asked { rest, next } => (rest, next),
                    PrePlayed::Over(_) => break,
                };
            yield_!(Wait::Question);
            if let Some(by_answer) = resumed!().ended {
                return Ok(Some(by_answer));
            }
            pre = pre_play_from::<V>(seat, resumed!(), held);
            if let PrePlayed::Over(Some(by_pre_play)) = pre {
                return Ok(Some(by_pre_play));
            }
            play_phase::<V>(seat, resumed!());
        }
    }
    Ok(None)
}

/// `StartTurn` for the player's side.
/// Out of line, which keeps the coroutine's body small.
#[inline(never)]
fn side_starts<V: Rules>(
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
) -> Result<SideStarted<V>, Error> {
    let fighting = lent.fighting();
    crate::combat::turn::side_starts::<V>(
        fighting.live,
        fighting.streams,
        fighting.belts,
        ascension,
    )
}

/// `EndPlayerTurnPhaseOne` and `EndPlayerTurnPhaseTwo` in a fight of
/// several players. Each phase runs for every player ending the turn, in
/// seat order, and the side's hooks run once. An extra turn's takers alone
/// end it. How the fight ended, if it did.
///
/// # Panics
///
/// A phase that stops on a question, or a listener that pauses, which
/// nothing here waits for yet.
#[inline(never)]
fn turn_ends_each<V: Rules>(
    ascension: V::AscensionLevel,
    lent: &mut Lent<'_, V>,
    skipping: &mut Skips<V>,
) -> Option<crate::combat::Ended> {
    // `playersEndingTurn`: an extra turn's takers, or every player.
    let seats: SmallVec<[sts2_core::SeatIndex; 4]> = lent.live.turn_seats().collect();
    // What no one player owns acts as the first.
    let host = sts2_core::SeatIndex(0);
    // Each player turns to `End` once every player's post-play phase is
    // over.
    for &seat in &seats {
        post_play_over::<V>(seat, lent);
    }
    // `BeforeSideTurnEnd`, once for the side.
    *skipping = std::mem::take(&mut lent.live.skip_tick);
    let over = match before_turn_end::<V>(host, ascension, lent) {
        Ok((over, None)) => over,
        Ok((_, Some(_))) => {
            unimplemented!(
                "a listener pausing as the side's turn ends in a fight of several players"
            )
        }
        Err(ended) => {
            lent.live.skip_tick = *skipping;
            return Some(ended);
        }
    };
    // `CheckWinCondition`, then `DoTurnEnd` for each player.
    for &seat in &seats {
        match turn_end::<V>(seat, ascension, over, lent, skipping) {
            Ok(InHand::Done(_)) => {}
            Ok(InHand::Parked { .. }) => {
                unimplemented!("seat {seat:?}'s turn-end card asking in a fight of several players")
            }
            Err(ended) => return Some(ended),
        }
    }
    // `Hook.BeforeFlush` for each player.
    for &seat in &seats {
        let (live, rngs, belts) = lent.parts();
        if !crate::combat::turn::before_flush_late::<V>(seat, live, rngs, belts).is_empty() {
            unimplemented!("seat {seat:?}'s pick before the flush in a fight of several players");
        }
    }
    // `EndPlayerTurnPhaseTwo`: each player's flush, then `AfterSideTurnEnd`
    // once. A fight that ended skips its late pass.
    // `FlushPlayerHand` does nothing for a dead player.
    for &seat in &seats {
        if lent.live.player(seat).alive {
            flush::<V>(seat, ascension, lent);
        }
    }
    let mut ended = None;
    for pass in [TurnEndPass::Main, TurnEndPass::Late] {
        if ended.is_some() {
            break;
        }
        let (by_pass, paused) = after_turn_end::<V>(host, ascension, lent, skipping, pass);
        if paused.is_some() {
            unimplemented!(
                "a listener pausing after the side's turn end in a fight of several players"
            );
        }
        ended = by_pass;
    }
    if let Some(by_turn_end) = ended {
        lent.live.skip_tick = *skipping;
        crate::combat::turn::ended_at_turn_end::<V>(lent.live, by_turn_end);
        return Some(by_turn_end);
    }
    None
}

/// `AfterAutoPostPlayPhaseEntered` for each player ending the turn, in seat
/// order. One whose auto-play asks waits at the front of its player's
/// queue, and the rest of its phase waits behind the question. The next
/// player's runs. Whether any waits.
fn post_play_each<V: Rules>(lent: &mut Lent<'_, V>) -> bool {
    let seats: SmallVec<[sts2_core::SeatIndex; 4]> = lent.live.turn_seats().collect();
    let mut waits = false;
    for seat in seats {
        let mut pass = post_play_pass::<V>(seat, lent);
        if post_play::<V>(seat, lent, &mut pass) {
            let live = &mut *lent.live;
            live.defer(crate::combat::live::Deferred::PostPlayFrom {
                seat,
                rest: Box::new(pass),
            });
            live.stop_for_answer(seat);
            waits = true;
        }
    }
    lent.live.post_plays_end();
    waits
}

/// Whether the fight has several players.
fn several<V: Rules>(lent: &mut Lent<'_, V>) -> bool {
    lent.live.players.len() > 1
}

/// `RunAutoPrePlayPhase` for each player in seat order, with no question
/// standing.
///
/// Then each setup whose choice signalled, as the play phase opens.
///
/// # Panics
///
/// A choice that signalled outside a setup, or an auto-play that asks, in
/// a fight of several players.
fn pre_play_each<V: Rules>(
    lent: &mut Lent<'_, V>,
    skipping: Skips<V>,
) -> Result<Option<crate::combat::Ended>, Error> {
    if crate::combat::signalled(lent.live) {
        unimplemented!("a choice signalled in a fight of several players");
    }
    // A dead player is ready to end the turn as it starts, and so is one
    // not taking an extra turn others take. Neither runs a pre-play phase.
    let live = &mut *lent.live;
    for seat in crate::combat::turn::seats(live) {
        if !live.player(seat).alive || !live.part_of_turn(seat) {
            live.players[seat].ready = true;
        }
    }
    for seat in crate::combat::turn::seats(lent.live) {
        // A seat whose setup waits on its question runs this once it is over.
        if !lent.live.player(seat).alive
            || !lent.live.part_of_turn(seat)
            || lent.live.queues.stopped(seat).is_some()
        {
            continue;
        }
        match pre_play::<V>(seat, lent) {
            PrePlayed::Over(Some(ended)) => {
                lent.live.skip_tick = skipping;
                return Ok(Some(ended));
            }
            PrePlayed::Over(None) => play_phase::<V>(seat, lent),
            // The question waits at the front of the player's queue, with
            // the rest of their pre-play phase behind it.
            asked @ PrePlayed::Asked { .. } => {
                crate::combat::turn::pre_played::<V>(seat, lent.live, asked);
                // A pushed selector, as Whispering Earring's, answers as the
                // question comes up, and the phase goes on before the next
                // player's. The game's selector stack is the same for every
                // player.
                while let Some(picks) = crate::combat::selector_picks(seat, lent.live) {
                    let answered = crate::combat::play::answer(seat, seat, lent.fighting(), picks)?;
                    if let Some(ended) = answered {
                        lent.live.skip_tick = skipping;
                        return Ok(Some(ended));
                    }
                }
                if lent
                    .live
                    .asking
                    .as_ref()
                    .is_some_and(|asking| asking.signalled)
                {
                    unimplemented!("seat {seat:?}'s pre-play phase signalling a choice");
                }
                lent.live.stop_for_answer(seat);
            }
        }
    }
    lent.live.skip_tick = skipping;
    // `ActionExecutor.Unpause`: a setup that signalled its choice goes on, and
    // the choice lists now, taking its cards unasked where it can. A question
    // it still asks waits at the front of its player's queue again.
    for seat in crate::combat::turn::seats(lent.live) {
        let signalled = (lent.live.queues.stopped(seat)).is_some_and(|task| task.asking.signalled);
        if !signalled {
            continue;
        }
        let (owner, task) = lent
            .live
            .queues
            .go_on(seat)
            .expect("the queue stopped on it");
        lent.live.rejoin(owner, task);
        if let Some(ended) = come_up::<V>(seat, lent)? {
            return Ok(Some(ended));
        }
        lent.live.stop_for_answer(seat);
    }
    Ok(None)
}

/// A choice signalled under a pause boundary, once the turn has run on.
fn come_up<V: Rules>(
    seat: sts2_core::SeatIndex,
    lent: &mut Lent<'_, V>,
) -> Result<Option<crate::combat::Ended>, Error> {
    crate::combat::play::come_up(seat, lent.fighting())
}

/// `RunAutoPrePlayPhase` from its start.
fn pre_play<V: Rules>(seat: sts2_core::SeatIndex, lent: &mut Lent<'_, V>) -> PrePlayed<V> {
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::pre_play_phase::<V>(seat, live, rngs, belts)
}

/// `RunAutoPrePlayPhase` from where an auto-play's question stood it still.
fn pre_play_from<V: Rules>(
    seat: sts2_core::SeatIndex,
    lent: &mut Lent<'_, V>,
    held: (PassRest<V>, usize),
) -> PrePlayed<V> {
    let (live, rngs, belts) = lent.parts();
    crate::combat::turn::pre_play_from::<V>(seat, live, rngs, belts, held.0, held.1)
}

/// The phase after the pre-play phase, which is still that phase while a
/// question it asked stands.
fn play_phase<V: Rules>(seat: sts2_core::SeatIndex, lent: &mut Lent<'_, V>) {
    let live = &mut *lent.live;
    live.players[seat].phase = crate::combat::turn::after_pre_play::<V>(live);
}

/// Whether the player takes an extra turn, with it set up where so.
fn extra_turns<V: Rules>(lent: &mut Lent<'_, V>) -> bool {
    let (live, _, belts) = lent.parts();
    crate::combat::turn::extra_turns::<V>(live, belts)
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
/// Out of line, which keeps the coroutine's body small.
#[inline(never)]
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
    let turn = live.players[seat].turn;
    PassRest::walk(
        live,
        belts,
        sts2_core::EventHook::AutoPostPlayPhase {
            player: seat.creature(),
            turn,
        },
    )
}

/// The post-play phase over, which turns the player's phase to `End`.
fn post_play_over<V: Rules>(seat: sts2_core::SeatIndex, lent: &mut Lent<'_, V>) {
    lent.live.players[seat].phase = <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::END;
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

/// Run the turn loop: from the player's turn end, or from a fight's
/// `first` turn's start, to the next play phase.
pub(super) fn begin<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    first: bool,
) -> Result<Option<crate::combat::Ended>, Error> {
    let mut looping = turn_loop::<V>(seat, ascension, first);
    let state = looping.start_with(Lent {
        live: &mut *fighting.live,
        streams: &mut *fighting.streams,
        belts: &mut *fighting.belts,
        ended: None,
        taken: None,
    });
    match state {
        diapause::CoroutineState::Yielded(wait) => park(seat, fighting, Box::new(looping), wait),
        diapause::CoroutineState::Complete(done) => done,
    }
}

/// The turn loop from where `looping` stopped, once what it waited for is
/// over. `ended` is how that ended the fight, and `taken` is the player's
/// answer where it waited on its own question.
pub(super) fn finish<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    mut looping: Box<turn_loop::State<V>>,
    ended: Option<crate::combat::Ended>,
    taken: Option<sts2_core::Taken<V>>,
) -> Result<Option<crate::combat::Ended>, Error> {
    let state = diapause::Coroutine::resume(
        &mut *looping,
        Lent {
            live: &mut *fighting.live,
            streams: &mut *fighting.streams,
            belts: &mut *fighting.belts,
            ended,
            taken,
        },
    );
    match state {
        diapause::CoroutineState::Yielded(wait) => park(seat, fighting, looping, wait),
        diapause::CoroutineState::Complete(done) => done,
    }
}

/// The turn loop waiting on `wait`. Behind tasks it is `Task.WhenAll`: the
/// tasks go on in the order they paused, then the turn loop.
fn park<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    looping: Box<turn_loop::State<V>>,
    wait: Wait<V>,
) -> Result<Option<crate::combat::Ended>, Error> {
    match wait {
        Wait::Question => {
            fighting
                .live
                .defer(crate::combat::live::Deferred::TurnLoop(looping));
            Ok(None)
        }
        // `Task.WhenAll`: it goes on once the last player has picked.
        Wait::Picks => {
            fighting.live.after_picks = Some(looping);
            Ok(None)
        }
        Wait::Tasks(tasks) => {
            let mut tasks = tasks.into_iter();
            let first = tasks.next();
            for task in tasks {
                fighting
                    .live
                    .defer(crate::combat::live::Deferred::Rejoin(Box::new(task)));
            }
            fighting
                .live
                .defer(crate::combat::live::Deferred::TurnLoop(looping));
            if let Some(first) = first {
                fighting.live.rejoin(seat, first);
            }
            Ok(None)
        }
        // The setup goes on once the turn has started around it, and a
        // choice it signalled comes up now.
        Wait::Setup { task, skipping } => {
            fighting
                .live
                .defer(crate::combat::live::Deferred::TurnLoop(looping));
            fighting.live.rejoin(seat, task);
            let came_up = crate::combat::play::come_up(
                seat,
                Fighting {
                    live: &mut *fighting.live,
                    streams: &mut *fighting.streams,
                    belts: &mut *fighting.belts,
                },
            )?;
            fighting.live.skip_tick = skipping;
            Ok(came_up)
        }
    }
}
