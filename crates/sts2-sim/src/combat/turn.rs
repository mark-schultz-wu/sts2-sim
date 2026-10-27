//! Ending a turn.
//!
//! A port of the run of `CombatManager` methods a turn end sets off, in the
//! order they run. The player's hand is flushed, the enemy side takes its
//! turn, and the player's next turn is set up.

use crate::rng::MegaRng;
use smallvec::SmallVec;
use sts2_core::{CombatId, Pile, PowerAmount, Rules, SideKinds, TurnPhaseKinds};

use super::board::Board;
use super::listen::{self, Listener};
use super::live::{CREATURES, Face, Landing, Live, Skips, Then};
use super::pile::MAX_CARDS_IN_HAND;
use super::powers::Powers as _;
use super::start::hand_size;
use super::{Error, Fighting};
use crate::stream::CombatRngs;

/// End the player's turn and set up the next one, on the fight where it
/// stands.
///
/// Whatever the enemy turn ended, if it ended the fight.
#[tracing::instrument(level = "trace", skip_all)]
///
/// # Panics
///
/// A fight of several players, whose turn ends once every player is ready.
pub(crate) fn end<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
) -> Result<Option<super::Ended>, Error> {
    if fighting.live.players.len() > 1 {
        unimplemented!(
            "ending a turn in a fight of {} players",
            fighting.live.players.len()
        );
    }
    super::turn_loop::begin(seat, fighting, ascension, false)
}

/// The cards still to run `DoTurnEnd` behind one whose effect asked.
pub(super) type Rest<V> = SmallVec<[sts2_core::CardId<V>; MAX_CARDS_IN_HAND]>;

/// `DoTurnEnd` from the end of `card`'s `OnTurnEndInHandWrapper`, once the
/// question its effect asked is answered. The cards in `rest` run theirs
/// after it.
pub(super) fn in_hand_from<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    card: sts2_core::CardId<V>,
    rest: &[sts2_core::CardId<V>],
) -> InHand<V> {
    match wrapper_end(seat, live, rngs.reborrow(), belts, card) {
        Some(ended) => InHand::Done(Some(ended)),
        None => cards_turn_end(seat, live, rngs, belts, ascension, rest),
    }
}

/// A fight that `AfterTurnEnd` ended. A loss is only pending here:
/// `SwitchFromPlayerToEnemySide` and the enemy's `StartTurn` run up to its
/// win check. Every hook walk yields nothing while the fight is ending. So
/// what is left is the switch, the phase `StartTurn` opens on, and
/// `Creature.AfterTurnStart` clearing each enemy's block.
pub(super) fn ended_at_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    ended: super::Ended,
) {
    match ended {
        super::Ended::Won => {}
        super::Ended::Lost => {
            live.switch_side(<V::CombatSide as SideKinds>::ENEMY);
            live.players[seat].phase = <V::PlayerTurnPhase as TurnPhaseKinds>::NONE;
            for creature in live.creatures.iter_mut() {
                if creature.side == <V::CombatSide as SideKinds>::ENEMY {
                    creature.block = num_traits::zero();
                }
            }
        }
    }
}

/// `SwitchFromPlayerToEnemySide` asks `ShouldTakeExtraTurn` first. A yes
/// keeps the side with the player, whose turn counts up while the round
/// does not, and `AfterTakingExtraTurn` follows. True where it said yes.
pub(super) fn takes_extra_turn<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    belts: &mut crate::run::Belts<V>,
) -> bool {
    let extra = listen::heard::<V>(live, belts, sts2_core::ModelHook::ShouldTakeExtraTurn) && {
        let plays = live.history.unprompted_plays_this_turn();
        listen::any_decides::<V>(
            listen::Walk { live, belts },
            sts2_core::DecisionHook::ShouldTakeExtraTurn {
                plays,
                turn: live.turn,
            },
            true,
        )
    };
    if !extra {
        return false;
    }
    live.players[seat].phase = <V::PlayerTurnPhase as TurnPhaseKinds>::NONE;
    live.switch_side(<V::CombatSide as SideKinds>::PLAYER);
    live.turn = live.turn + num_traits::one();
    // `PowerCmd.Decrement` of a power asking runs inside the walk.
    let taking = sts2_core::EventHook::TakingExtraTurn;
    let mut walking = listen::Walking::begin(live, belts, taking);
    if let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        unimplemented!("{listener:?} asking for {effect:?} as an extra turn is taken");
    }
    true
}

/// `SwitchSides` to the enemy, then `StartTurn` for it, up to the moves:
/// block clears, and `AfterSideTurnStart` runs. The enemies that perform a
/// move this turn, in order, or how the fight ended.
///
/// `StartTurn` opens with `SetPhaseForAllPlayers(None)` and puts the phase
/// back only for a turn the player is taking. So the phase a fight ending
/// here reports is the one the enemy turn ran under.
pub(super) fn enemy_side_starts<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
) -> Result<SmallVec<[CombatId; CREATURES]>, super::Ended> {
    live.players[seat].phase = <V::PlayerTurnPhase as TurnPhaseKinds>::NONE;
    live.switch_side(<V::CombatSide as SideKinds>::ENEMY);
    // `Creature.BeforeTurnStart` for each enemy, then `BeforeSideTurnStart`
    // for their side. Hardened Shell's cap starts again here.
    let turn = live.turn;
    before_side_turn_start::<V>(
        seat,
        live,
        rngs.reborrow(),
        belts,
        ascension,
        <V::CombatSide as SideKinds>::ENEMY,
        turn,
    );
    clear_block::<V>(
        seat,
        live,
        rngs.reborrow(),
        belts,
        <V::CombatSide as SideKinds>::ENEMY,
    );
    // `AfterSideTurnStart` for the enemy, which the block clear comes before
    // and the moves come after. A power counting the cards played against its
    // owner forgets the count here.
    if let Some(ended) = side_turn_started::<V>(
        seat,
        live,
        rngs,
        belts,
        ascension,
        <V::CombatSide as SideKinds>::ENEMY,
        false,
    ) {
        return Err(ended);
    }
    // `Creature.TakeTurn` performs nothing for a monster `SpawnedThisTurn`,
    // which came in as this side's turn started.
    let fresh = |id: CombatId| {
        live.machines
            .iter()
            .any(|held| held.creature == id && held.spawned_this_turn)
    };
    Ok(live
        .creatures
        .iter()
        .filter(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY)
        .map(|creature| creature.id)
        .filter(|&id| !fresh(id))
        .collect())
}

/// Where a monster's move that stood still goes on from, once what it stood
/// still on is over.
#[allow(clippy::too_many_arguments)]
pub(super) fn move_goes_on<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    skipping: &mut Skips<V>,
    stood: Stood<V>,
    answer: Option<sts2_core::Taken<V>>,
    ended: Option<super::Ended>,
) -> Moved<V> {
    let Stood { moving, on } = stood;
    let ended = match (on, ended) {
        // `IChoosable.OnChosen`: what the player chose acts from the
        // player, and the move is over.
        (StoodOn::Choice(at), None) => {
            let taken = answer.unwrap_or_else(|| unreachable!("a move's choice answered"));
            let ascension = live.ascension;
            let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
            let played = V::resume_play::<super::board::BoardKind<V>>(&mut board, at, taken);
            let ended = board.ended();
            super::play::merge::<V>(skipping, std::mem::take(&mut board.skipped));
            if played != sts2_core::Played::Done {
                unimplemented!("a monster's move asking a second question");
            }
            ended
        }
        (StoodOn::Question(Some(at)), None) => {
            // A monster the fight took out took its model and its move with
            // it.
            if !live
                .machines
                .iter()
                .any(|held| held.creature == moving.actor)
            {
                None
            } else {
                let ascension = live.ascension;
                let mut board = Board::for_move(
                    live,
                    rngs,
                    belts,
                    ascension,
                    (moving.actor, moving.damage, moving.repeats),
                );
                let played = V::resume_move::<super::board::BoardKind<V>>(
                    moving.monster,
                    moving.move_id,
                    at,
                    &mut board,
                );
                let ended = board.ended();
                super::play::merge::<V>(skipping, std::mem::take(&mut board.skipped));
                match played {
                    sts2_core::Played::Done if live.asking.is_none() => ended,
                    sts2_core::Played::Done => {
                        let on = StoodOn::Question(None);
                        return Moved::Stood(Stood { moving, on });
                    }
                    sts2_core::Played::Waiting { at } if live.asking.is_some() => {
                        let on = StoodOn::Question(Some(at));
                        return Moved::Stood(Stood { moving, on });
                    }
                    played => unimplemented!("a resumed move handing back {played:?}"),
                }
            }
        }
        (StoodOn::Question(None), None) => None,
        (_, ended) => ended,
    };
    move_performed::<V>(live, moving.actor, moving.move_id);
    Moved::Over(ended)
}

/// `EndEnemyTurnInternal`, then `SwitchSides` back to the player. How the
/// fight ended, if the enemies' turn-end hooks ended it.
pub(super) fn enemy_turn_ends<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    skipping: &mut Skips<V>,
) -> Option<super::Ended> {
    let (ended, listening) = before_turn_end::<V>(
        seat,
        live,
        rngs.reborrow(),
        belts,
        ascension,
        <V::CombatSide as SideKinds>::ENEMY,
    );
    listening.none_waits("the enemies' BeforeSideTurnEnd");
    let ended = match ended {
        Some(ended) => Some(ended),
        None => after_turn_end::<V>(
            seat,
            live,
            rngs,
            belts,
            ascension,
            skipping,
            <V::CombatSide as SideKinds>::ENEMY,
        ),
    };
    // The round and the turn both count up, because this is not an extra
    // turn. A fight that ended above switches too: its win check clears
    // `IsInProgress`, which `IsEnding` then reads as not ending.
    live.switch_side(<V::CombatSide as SideKinds>::PLAYER);
    live.round = live.round + num_traits::one();
    live.turn = live.turn + num_traits::one();
    ended
}

/// `StartTurn` for the player's side, from `BeforeSideTurnStart` to
/// `OrbQueue.AfterTurnStart`. An `extra` turn is one `ShouldTakeExtraTurn`
/// gave, which rolls no enemy move.
///
/// `SetupPlayerTurn` runs for each player in seat order, under a pause
/// boundary. One that stops on a question lets the turn go on around it:
/// `StartTurn` waits for each setup only to finish or to pause. Each paused
/// setup's task comes back, set aside, for `RunAutoPrePlayPhase` to wait for.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn side_starts<V: Rules>(
    live: &mut Live<V>,
    streams: &mut crate::stream::Streams<V>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    extra: bool,
) -> Result<SideStarted<V>, Error> {
    // What no one player owns acts as the first.
    let host = sts2_core::SeatIndex(0);
    // `StartTurn` opens on `BeforeSideTurnStart`, which is where a relic
    // counting a turn's plays moves the count on for the draw to read.
    let turn = live.turn;
    before_side_turn_start::<V>(
        host,
        live,
        streams.combat(),
        belts,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
        turn,
    );

    for player in live.players.iter_mut() {
        player.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::START;
    }

    // `PrepareForNextTurn`, before the player draws anything. An extra turn
    // leaves the enemies' moves as they were.
    if !extra {
        roll_moves(live, belts, ascension, streams.combat().ai)?;
    }

    // `AfterTurnStart` on each player, then `SetupPlayerTurn`.
    clear_block::<V>(
        host,
        live,
        streams.combat(),
        belts,
        <V::CombatSide as SideKinds>::PLAYER,
    );
    // A block the clear gives back can end the fight, through a hit it sets
    // off. `ResetEnergy` still runs, but no listener hears a hook from then
    // on, so the energy is the plain maximum and nothing is drawn.
    if live.won && super::board::ending::<V>(listen::Walk { live, belts }) {
        for seat in seats(live) {
            live.players[seat].energy = base_energy::<V>(live, seat);
        }
        return Ok(SideStarted::Ended(super::Ended::Won));
    }
    let mut setup = None;
    for seat in seats(live) {
        match player_setup::<V>(seat, live, streams, belts, ascension) {
            PlayerSetUp::Ended(ended) => {
                if let Some(task) = setup {
                    live.rejoin(host, task);
                }
                return Ok(SideStarted::Ended(ended));
            }
            PlayerSetUp::Parked(task) if seat == host => setup = Some(task),
            PlayerSetUp::Parked(_) => {
                unimplemented!("seat {seat:?}'s setup stopping on a question")
            }
            PlayerSetUp::Done => {}
        }
    }
    let ended = side_turn_started::<V>(
        host,
        live,
        streams.combat(),
        belts,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
        extra,
    )
    // `OrbQueue.AfterTurnStart` for each player, once the side's turn has
    // started.
    .or_else(|| {
        seats(live).find_map(|seat| {
            orbs_at_turn_start::<V>(seat, live, streams.combat(), belts, ascension)
        })
    });
    Ok(SideStarted::Started { setup, ended })
}

/// Every seat in the fight, in order.
pub(super) fn seats<V: Rules>(
    live: &Live<V>,
) -> impl Iterator<Item = sts2_core::SeatIndex> + use<V> {
    (0..live.players.len())
        .map(|at| sts2_core::SeatIndex(u8::try_from(at).expect("a fight's players fit a u8")))
}

/// `MaxEnergy` before `Hook.ModifyMaxEnergy`: the character's own.
fn base_energy<V: Rules>(live: &Live<V>, seat: sts2_core::SeatIndex) -> V::Count {
    num_traits::cast(V::max_energy(live.players[seat].character))
        .expect("the turn's energy fits the build's count type")
}

/// How one player's `SetupPlayerTurn` came out.
enum PlayerSetUp<V: Rules> {
    /// It ended the fight.
    Ended(super::Ended),
    /// It stopped on a question, and its task was set aside.
    Parked(super::live::Paused<V>),
    /// It ran to its end.
    Done,
}

/// `SetupPlayerTurn` for `seat`: the energy reset and its hooks, then the
/// setup as far as its end or the first question, under a pause boundary.
fn player_setup<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    streams: &mut crate::stream::Streams<V>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
) -> PlayerSetUp<V> {
    // `ResetEnergy`, to `MaxEnergy`, which runs `Hook.ModifyMaxEnergy`. A
    // player `Hook.ShouldPlayerResetEnergy` keeps from it adds the maximum
    // to what they have instead.
    let player = live.player(seat).id;
    let kept = listen::any_decides::<V>(
        listen::Walk { live, belts },
        sts2_core::DecisionHook::ShouldPlayerResetEnergy {
            player,
            turn: live.turn,
        },
        false,
    );
    let base = base_energy::<V>(live, seat);
    let max = super::board::max_energy::<V>(listen::Walk { live, belts }, base);
    live.players[seat].energy = if kept {
        live.players[seat].energy + max
    } else {
        max
    };
    // An evoke that ends the fight ends `SetupPlayerTurn` with it, as a
    // relic's damage before the hand does below.
    if let Some(ended) = energy_reset::<V>(seat, live, streams.combat(), belts, ascension) {
        return PlayerSetUp::Ended(ended);
    }
    energy_reset_late::<V>(seat, live, streams.combat(), belts, ascension);
    let hook = sts2_core::EventHook::BeforeHandDraw {
        player,
        turn: live.turn,
    };
    let asked = listen::Walking::begin(live, belts, hook).map(Box::new);
    // A relic's damage can end the fight, and a fight ended in
    // `SetupPlayerTurn` starts no side turn after it.
    let fresh = live.deferred.open_fresh();
    live.pause_boundary = true;
    let carried = begin_setup(seat, live, streams, belts, ascension, asked);
    live.pause_boundary = false;
    match carried {
        Carried::Done(Some(ended)) => PlayerSetUp::Ended(ended),
        Carried::Done(None) => {
            live.deferred.keep_fresh(fresh);
            PlayerSetUp::Done
        }
        Carried::Parked => PlayerSetUp::Parked(live.set_aside(seat, fresh)),
    }
}

/// How the player's side turn start came out.
pub(super) enum SideStarted<V: Rules> {
    /// The fight ended before the side's turn started.
    Ended(super::Ended),
    /// The side's turn started. `setup` is the task of the first player's
    /// setup, if it stopped on a question, and `ended` is how the side's
    /// start ended the fight.
    Started {
        setup: Option<super::live::Paused<V>>,
        ended: Option<super::Ended>,
    },
}

/// The post-play phase from `rest`, what its listeners still ask for. It
/// runs until an auto-play asks, which is true. Otherwise it runs to its
/// end, which turns the phase to `End`.
///
/// # Panics
///
/// A listener asking for anything but an auto-play.
pub(super) fn post_play_until_asked<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    rest: &mut PassRest<V>,
) -> bool {
    while let Some((listener, effect)) = rest.next(seat, live, belts) {
        // A Stampede part-way through puts what it has left back first.
        let mut left_over = None;
        match effect {
            // `CardCmd.AutoPlay` of the card, from wherever it stands. It
            // returns at once where the fight is over or ending, which a
            // card before this one can make it, or its owner is dead.
            sts2_core::HookEffect::AutoPlay(card) => {
                if live.lost
                    || !live.player(seat).alive
                    || super::board::ending::<V>(listen::Walk { live, belts })
                {
                    continue;
                }
                let Some(from) = live.players[seat].cards.pile_of(card) else {
                    unimplemented!("auto-playing {card:?}, which stands in no pile");
                };
                super::play::auto_play::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    card,
                    from,
                    None,
                    true,
                );
            }
            sts2_core::HookEffect::AutoPlayFromDraw(count) => {
                let count: usize = num_traits::cast(count)
                    .unwrap_or_else(|| unimplemented!("auto-playing {count:?} cards"));
                auto_play_from_draw::<V>(seat, live, rngs.reborrow(), belts, count);
            }
            sts2_core::HookEffect::AutoPlayFromDrawIfTop(card) => {
                if live.players[seat]
                    .cards
                    .get(Pile::Draw, 0)
                    .map(|held| held.id)
                    == Some(card)
                {
                    auto_play_from_draw::<V>(seat, live, rngs.reborrow(), belts, 1);
                }
            }
            sts2_core::HookEffect::AutoPlayRandomFromHand { kind, count } => {
                let mut left = count;
                while left > num_traits::zero() && live.asking.is_none() {
                    left = left - num_traits::one();
                    let standing: Vec<sts2_core::CardId<V>> = live.players[seat]
                        .cards
                        .iter(Pile::Hand)
                        .filter(|held| {
                            let card = held.face.ident.card;
                            V::card_kind(card) == Some(kind)
                                && !sts2_core::CardTable::unplayable(card)
                        })
                        .map(|held| held.id)
                        .collect();
                    let Ok(&card) = sts2_core::rng::GameRng::choose(rngs.shuffle, &standing) else {
                        continue;
                    };
                    // `CardCmd.AutoPlay` returns at once where the fight is
                    // over or ending, which a card before this one can make
                    // it. The draw is taken all the same.
                    if live.lost || super::board::ending::<V>(listen::Walk { live, belts }) {
                        continue;
                    }
                    super::play::auto_play::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        card,
                        Pile::Hand,
                        None,
                        true,
                    );
                }
                if left > num_traits::zero() {
                    left_over =
                        Some(sts2_core::HookEffect::AutoPlayRandomFromHand { kind, count: left });
                }
            }
            effect => {
                unimplemented!("{listener:?} asking for {effect:?} as the post-play phase opens")
            }
        }
        // `CardCmd.AutoPlay` is awaited, so what is left waits on the answer.
        if live.asking.is_some() {
            rest.first = left_over.map(|effect| (listener, effect));
            return true;
        }
    }
    live.players[seat].phase = <V::PlayerTurnPhase as TurnPhaseKinds>::END;
    false
}

/// `BeforeFlushLate`, the second pass of `Hook.BeforeFlush`. It hands back
/// the tasks of the listeners that paused on a pick, and the turn's end
/// waits for them before the flush. A listener that paused lets the later
/// ones run first.
///
/// # Panics
///
/// A listener asking for anything but a pick of cards to retain.
pub(super) fn before_flush_late<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
) -> Vec<super::live::Paused<V>> {
    if !listen::heard::<V>(live, belts, sts2_core::ModelHook::BeforeFlushLate) {
        return Vec::new();
    }
    let player = live.player(seat).id;
    let turn = live.turn;
    let mut walking = listen::Walking::begin(live, belts, sts2_core::EventHook::FlushingLate);
    // Each listener runs under a pause boundary of its own, and the hook
    // waits for every one at its end.
    let mut listening = super::live::Listening::new();
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        let boundary = listening.enter(live, listener);
        match effect {
            // `CardSelectCmd.FromHand` of up to `count` cards that do not
            // retain already, then `GiveSingleTurnRetain` on each. Nothing
            // where the hand is not flushed, or once the fight is ending.
            sts2_core::HookEffect::PickToRetain(count) => {
                let flushes = !listen::any_decides::<V>(
                    listen::Walk { live, belts },
                    sts2_core::DecisionHook::ShouldFlush { player, turn },
                    false,
                );
                if flushes && !super::board::ending::<V>(listen::Walk { live, belts }) {
                    pick_to_retain::<V>(seat, live, rngs.reborrow(), belts, count);
                }
            }
            effect => unimplemented!("{listener:?} asking for {effect:?} before the hand flushes"),
        }
        listening.leave(seat, live, boundary);
    }
    // The turn's end from the flush on waits for every paused listener.
    listening.into_tasks()
}

/// `CardSelectCmd.FromHand` of up to `count` cards that do not retain
/// already, then `GiveSingleTurnRetain` on each. A build that signals before
/// it lists pauses the listener even where the pick takes its cards unasked.
fn pick_to_retain<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    count: V::Number,
) {
    let count = num_traits::cast(count)
        .and_then(nonmax::NonMaxUsize::new)
        .unwrap_or_else(|| unimplemented!("retaining {count:?} cards"));
    let selection = sts2_core::Selection::UpTo(count);
    let cards: SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]> = live.players[seat]
        .cards
        .iter(Pile::Hand)
        .enumerate()
        .filter(|(_, held)| !held.face.retains())
        .map(|(at, held)| {
            let at = num_traits::NumCast::from(at).expect("a hand's place fits a hand index");
            sts2_core::CardAt::new(held.id, Pile::Hand, at)
        })
        .collect();
    let signalled = pauses::<V>(live, belts);
    let automatic = match signalled {
        true => None,
        false => selection.validate_automatic(cards.iter().copied().collect()),
    };
    match automatic {
        Some(picked) => {
            act_on::<V>(
                seat,
                live,
                rngs,
                belts,
                sts2_core::PileAct::RetainThisTurn,
                picked.as_slice(),
                false,
            );
        }
        None => {
            live.asking = Some(super::live::Asking {
                then: Then::Pick {
                    act: sts2_core::PileAct::RetainThisTurn,
                },
                question: super::live::Asked::Among { cards, selection },
                signalled,
            });
        }
    }
}

/// `AutoPlayFromDrawPile(count, Top)` from outside any effect.
fn auto_play_from_draw<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    count: usize,
) {
    let ascension = live.ascension;
    let mut board =
        super::board::Board::for_card(seat, live, rngs, belts, ascension, None, None).blocking();
    sts2_core::Fight::auto_play_from_draw(&mut board, count, false);
}

/// `DoTurnEnd`: the orbs, then the cards in hand that answer to a turn
/// ending, and how the fight ended if an orb ended it.
///
/// Every orb triggers first. The game then looks through the hand once. A
/// card with `HasTurnEndInHandEffect` runs `OnTurnEndInHandWrapper`, after
/// every other ethereal card is exhausted. An ethereal card with the effect
/// is exhausted by the wrapper, once the effect is over.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
) -> InHand<V> {
    if let Some(ended) = orb_triggers::<V>(
        seat,
        live,
        rngs.reborrow(),
        belts,
        ascension,
        OrbMoment::TurnEnd,
    ) {
        return InHand::Done(Some(ended));
    }
    let held: SmallVec<[_; MAX_CARDS_IN_HAND]> =
        live.players[seat].cards.iter(Pile::Hand).collect();
    let acts = |held: &&super::live::CardInstance<V>| {
        sts2_core::CardTable::turn_end_in_hand(held.face.ident.card)
    };
    // In hand order, each found where the exhausts before it left it.
    for ethereal in (held.iter()).filter(|held| held.face.ethereal() && !acts(held)) {
        let at = live.players[seat]
            .cards
            .find(Pile::Hand, ethereal.id)
            .expect("an Ethereal card stays in hand until it exhausts");
        // `CardCmd.Exhaust`, which ethereal causes.
        let card = live.players[seat]
            .cards
            .get(Pile::Hand, at)
            .expect("an Ethereal card stands where it was found");
        live.players[seat]
            .cards
            .move_to(Pile::Hand, at, Pile::Exhaust);
        if let Some(ended) = exhausted::<V>(seat, live, rngs.reborrow(), belts, &card, true, false)
        {
            return InHand::Done(Some(ended));
        }
    }
    let acting: SmallVec<[_; MAX_CARDS_IN_HAND]> =
        held.iter().filter(acts).map(|held| held.id).collect();
    cards_turn_end(seat, live, rngs, belts, ascension, &acting)
}

/// What `DoTurnEnd` came to.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum InHand<V: Rules> {
    /// Done, and how the fight ended if it did.
    Done(Option<super::Ended>),
    /// `card`'s effect stood still on a question, with `rest` still to run.
    Parked {
        card: sts2_core::CardId<V>,
        rest: SmallVec<[sts2_core::CardId<V>; MAX_CARDS_IN_HAND]>,
    },
}

/// `OnTurnEndInHandWrapper` for each of `cards` still in hand. The card goes
/// to the play pile and its effect runs. Then it goes to the exhaust pile if
/// ethereal, or the discard pile if not. A question stands the rest still
/// behind the answer.
///
/// Every effect runs, ending fight or not. `CardPileCmd.Add` refuses a
/// combat pile once the fight is ending, so the cards stay put from then on.
pub(super) fn cards_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    cards: &[sts2_core::CardId<V>],
) -> InHand<V> {
    let mut ended = None;
    for (at_card, &id) in cards.iter().enumerate() {
        let Some(at) = live.players[seat].cards.find(Pile::Hand, id) else {
            continue;
        };
        let card = live.players[seat]
            .cards
            .get(Pile::Hand, at)
            .expect("a card stands where it was found");
        if ended.is_none() {
            live.players[seat].cards.move_to(Pile::Hand, at, Pile::Play);
        }
        let mut board = Board::for_card(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            None,
            Some(id),
        )
        .blocking();
        V::on_card_turn_end(card.face.ident.card, &mut board);
        let skipped = std::mem::take(&mut board.skipped);
        let this_ended = board.ended();
        super::play::merge::<V>(&mut live.skip_tick, skipped);
        if ended.is_some() || this_ended.is_some() {
            ended = ended.or(this_ended);
            if live.asking.is_some() {
                unimplemented!("a turn-end effect asking once the fight is ending");
            }
            continue;
        }
        if live.asking.is_some() {
            return InHand::Parked {
                card: id,
                rest: cards[at_card + 1..].iter().copied().collect(),
            };
        }
        ended = wrapper_end(seat, live, rngs.reborrow(), belts, id);
    }
    InHand::Done(ended)
}

/// The end of `OnTurnEndInHandWrapper` for `id`, once its effect is over:
/// to the exhaust pile for an ethereal card, or the discard pile for any
/// other.
pub(super) fn wrapper_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    id: sts2_core::CardId<V>,
) -> Option<super::Ended> {
    let at = live.players[seat].cards.find(Pile::Play, id)?;
    let card = live.players[seat]
        .cards
        .get(Pile::Play, at)
        .expect("a card stands where it was found");
    if !card.face.ethereal() {
        live.players[seat]
            .cards
            .move_to(Pile::Play, at, Pile::Discard);
        return None;
    }
    // `CardCmd.Exhaust`, which ethereal causes.
    live.players[seat]
        .cards
        .move_to(Pile::Play, at, Pile::Exhaust);
    exhausted::<V>(seat, live, rngs, belts, &card, true, false)
}

/// When an orb's own trigger runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OrbMoment {
    /// `OrbQueue.BeforeTurnEnd`.
    TurnEnd,
    /// `OrbQueue.AfterTurnStart`.
    TurnStart,
}

/// Each orb's trigger for one moment, in queue order, and how the fight ended
/// if a trigger ended it.
///
/// The queue is read once, before the first trigger, as `Orbs.ToList()` is.
/// A trigger that ends the fight stops none after it. The game stops only
/// once the player leaves the combat state. So a Glass orb behind the one
/// that killed the last enemy still loses its one.
///
/// # Panics
///
/// A listener that changes how often an orb triggers.
pub(super) fn orb_triggers<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    at: OrbMoment,
) -> Option<super::Ended> {
    let held: Vec<sts2_core::Channelled<V>> = match &live.players[seat].orbs {
        Some(orbs) if !orbs.queue.is_empty() => orbs.queue.clone(),
        _ => return None,
    };
    let mut first_ended = None;
    for (place, orb) in held.into_iter().enumerate() {
        // `Hook.ModifyOrbPassiveTriggerCount` from one. Its
        // `AfterModifyingOrbPassiveTriggerCount` only flashes the one relic
        // that answers.
        let triggers = super::board::modified_by::<V>(
            super::listen::Walk { live, belts },
            num_traits::one(),
            &[sts2_core::ModifyHook::OrbPassiveTriggerCount { first: place == 0 }],
        );
        let mut orb = orb;
        for _ in 0..num_traits::ToPrimitive::to_usize(&triggers).unwrap_or(0) {
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            match at {
                OrbMoment::TurnEnd => V::orb_turn_end(&mut orb, &mut board),
                OrbMoment::TurnStart => V::orb_turn_start(&mut orb, &mut board),
            }
            let ended = board.ended();
            keep_orb(seat, live, place, orb);
            first_ended = first_ended.or(ended);
        }
    }
    first_ended
}

/// `OrbQueue.AfterTurnStart`, which runs under a pause boundary of the
/// player's with nothing waiting for it. A task that paused goes on as a
/// queued action.
pub(super) fn orbs_at_turn_start<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
) -> Option<super::Ended> {
    if live.players[seat]
        .orbs
        .as_ref()
        .is_none_or(|orbs| orbs.queue.is_empty())
    {
        return None;
    }
    let mut listening = super::live::Listening::new();
    let boundary = listening.enter_task(live);
    let ended = orb_triggers::<V>(seat, live, rngs, belts, ascension, OrbMoment::TurnStart);
    listening.leave(seat, live, boundary);
    listening.queue(seat, live);
    ended
}

/// The numbers an orb's trigger moved, kept on the orb at `place` while that
/// orb still stands there.
fn keep_orb<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    place: usize,
    orb: sts2_core::Channelled<V>,
) {
    if let Some(held) = live.players[seat]
        .orbs
        .as_mut()
        .and_then(|orbs| orbs.queue.get_mut(place))
        && held.orb == orb.orb
    {
        *held = orb;
    }
}

/// `PlayerCombatState.EndOfTurnCleanup`: what a card carried only for the turn.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn end_of_turn_cleanup<V: Rules>(seat: sts2_core::SeatIndex, live: &mut Live<V>) {
    live.players[seat]
        .cards
        .write_all_cards(Face::lasts_the_turn, Face::turn_ended);
    // `CardModel.EndOfTurnCleanup` clears `ExhaustOnNextPlay`.
    live.players[seat].exhaust_next.clear();
}

/// `BeforeSideTurnStart`, raised on every listener before the side draws.
///
/// Which side's turn a listener answers is its own rule. Block a power asks
/// for lands on the creature carrying it, and is `ValueProp.Unpowered`, so
/// nothing that creature carries moves it.
///
/// # Panics
///
/// A listener asking for something no side turn start can carry out.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn before_side_turn_start<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    side: V::CombatSide,
    turn: V::Count,
) {
    // `Creature.BeforeTurnStart`, which writes every power's stack as its
    // side begins.
    for creature in live
        .creatures
        .iter()
        .filter(|creature| creature.side == side)
    {
        for held in live.powers.of_mut(creature.id) {
            held.on_turn_start = held.amount;
        }
    }
    let round = live.round;
    let hook = sts2_core::EventHook::BeforeSideTurnStart { side, turn, round };
    // Each listener runs under a pause boundary of its own, and nothing
    // waits for one that paused: it goes on as a queued action.
    let mut listening = super::live::Listening::new();
    let mut walking = listen::Walking::begin(live, belts, hook);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        let boundary = listening.enter(live, listener);
        match (listener, effect) {
            (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                super::board::gain_unpowered_block::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    live.ascension,
                    owner,
                    amount,
                );
            }
            (_, sts2_core::HookEffect::AttacksFromDiscard(count)) => {
                attacks_from_discard::<V>(seat, live, rngs.reborrow(), count)
            }
            // A relic's owner is the player, whose orbs these are.
            (Listener::Relic { .. }, sts2_core::HookEffect::Channel(orb)) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::channel(&mut board, orb);
            }
            // `PowerCmd.Apply<P>(HittableEnemies, amount, Owner)`, from the
            // player who carries the relic.
            (Listener::Relic { .. }, sts2_core::HookEffect::ApplyToOpponents(power, amount)) => {
                let player = live.player(seat).id;
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                for target in board.hittable_opponents() {
                    board.land(target, power, amount, Some(player));
                }
            }
            (Listener::Relic { .. }, sts2_core::HookEffect::ZeroCostFromDraw(count)) => {
                use sts2_core::Fight as _;

                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                let mut free: Vec<_> = board
                    .cards_in(Pile::Draw)
                    .into_iter()
                    .filter(|&at| board.card_costs(at, false).0 == sts2_core::Cost::fixed(0))
                    .collect();
                board.stable_shuffle_cards(
                    &mut free,
                    <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_SELECTION,
                );
                free.truncate(count);
                board.act_on(sts2_core::PileAct::IntoHand, &free);
            }
            (_, effect) => unimplemented!("a listener asking for {effect:?} before a turn starts"),
        }
        listening.leave(seat, live, boundary);
    }
    listening.queue(seat, live);
}

/// `AggressionPower.BeforeSideTurnStart`: the discard pile's attacks, in an
/// unstable shuffle off `CombatCardSelection`, and the first `count` of them
/// into hand. Once the hand is full, each goes back onto the discard pile.
/// Each is upgraded where an upgrade can still raise it.
fn attacks_from_discard<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    count: V::Number,
) {
    use sts2_core::rng::GameRng as _;

    let count: usize = num_traits::cast(count)
        .unwrap_or_else(|| unimplemented!("{count:?} attacks from the discard pile"));
    let mut attacks: Vec<sts2_core::CardId<V>> = live.players[seat]
        .cards
        .iter(Pile::Discard)
        .filter(|held| V::card_kind(held.face.ident.card) == Some(sts2_core::CardKind::Attack))
        .map(|held| held.id)
        .collect();
    rngs.selection.unstable_shuffle(&mut attacks);
    for id in attacks.into_iter().take(count) {
        // A full hand sends the card to the discard pile instead, onto its
        // end, and it is upgraded there all the same.
        let to = match live.players[seat].cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
            true => Pile::Discard,
            false => Pile::Hand,
        };
        let at = live.players[seat]
            .cards
            .find(Pile::Discard, id)
            .expect("a shuffled attack is still in the discard pile");
        live.players[seat].cards.move_to(Pile::Discard, at, to);
        let at = live.players[seat].cards.len(to) - 1;
        live.players[seat].cards.write(to, at, |held| {
            held.modify(sts2_core::Change::Upgrade, sts2_core::Lasting::ThisFight);
            true
        });
    }
}

/// `BeforeSideTurnEndEarly`, raised on every listener before the side's turn
/// ends.
///
/// Block a power asks for lands on the creature carrying it. Plating re-plates
/// here, ahead of every later turn-end effect.
///
/// # Panics
///
/// A listener asking for something no turn end carries out.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn before_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    side: V::CombatSide,
) -> (Option<super::Ended>, super::live::Listening<V>) {
    // Each listener of every pass runs under a pause boundary of its own,
    // and the hook waits for every one at its end.
    let mut listening = super::live::Listening::new();
    let block = live.player(seat).block;
    let hook = sts2_core::EventHook::BeforeSideTurnEndVeryEarly { side, block };
    let mut walking = listen::Walking::begin(live, belts, hook);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        let boundary = listening.enter(live, listener);
        match (listener, effect) {
            // `PowerCmd.Remove` of another power on the listener's owner.
            (Listener::Power { owner, .. }, sts2_core::HookEffect::RemoveFromOwner(power)) => {
                live.remove_power(seat, owner, power);
            }
            (listener, effect) => {
                unimplemented!("{listener:?} asking for {effect:?} as a turn is about to end")
            }
        }
        listening.leave(seat, live, boundary);
    }
    // Counted only where something hears the hook, as nothing else reads it.
    let plays = match listen::heard::<V>(live, belts, sts2_core::ModelHook::BeforeSideTurnEndEarly)
    {
        true => live.history.unprompted_plays_this_turn(),
        false => 0,
    };
    let hook = sts2_core::EventHook::BeforeSideTurnEndEarly {
        side,
        plays,
        turn: live.turn,
    };
    let mut walking = listen::Walking::begin(live, belts, hook);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        let boundary = listening.enter(live, listener);
        let mut ended = None;
        match (listener, effect) {
            // `CreatureCmd.Heal` on the creature carrying the power.
            (Listener::Power { owner, .. }, sts2_core::HookEffect::Heal(amount)) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::heal(&mut board, owner, amount);
            }
            (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                super::board::gain_unpowered_block::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    live.ascension,
                    owner,
                    amount,
                );
            }
            // `CardCmd.Exhaust` of each card the hand holds as it starts.
            (Listener::Relic { .. }, sts2_core::HookEffect::ExhaustHand) => {
                let hand: SmallVec<[sts2_core::CardAt<V>; 10]> = (live.players[seat]
                    .cards
                    .iter(Pile::Hand))
                .enumerate()
                .map(|(at, card)| {
                    let index = num_traits::NumCast::from(at)
                        .unwrap_or_else(|| unimplemented!("a hand past what a fight numbers"));
                    sts2_core::CardAt::new(card.id, Pile::Hand, index)
                })
                .collect();
                ended = act_on::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    sts2_core::PileAct::Exhaust,
                    &hand,
                    true,
                )
                .1;
            }
            (_, effect) => unimplemented!("a listener asking for {effect:?} before a turn ends"),
        }
        listening.leave(seat, live, boundary);
        if ended.is_some() {
            return (ended, listening);
        }
    }
    // What a listener reads is the hand and energy as its turn comes. One
    // before it can draw, as Regret counts its pile then.
    let now = |live: &Live<V>| sts2_core::EventHook::BeforeSideTurnEnd {
        side,
        energy: live.players[seat].energy,
        hand: live.players[seat].cards.len(Pile::Hand),
    };
    let mut ended = None;
    let mut walking = listen::Walking::begin(live, belts, now(live));
    while let Some((listener, effect)) = walking.as_mut().and_then(|walking| {
        walking.rehook(now(live));
        walking.next(seat, live, belts)
    }) {
        let boundary = listening.enter(live, listener);
        'effect: {
            match (listener, effect) {
                // `CreatureCmd.Damage` over `HittableEnemies`, unpowered, from the
                // power's owner.
                (Listener::Power { owner, .. }, sts2_core::HookEffect::DamageOpponents(amount)) => {
                    if ended.is_some() {
                        break 'effect;
                    }
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    let targets = board.hittable_opponents_of(owner);
                    board.damage_command(|board| {
                        for target in targets {
                            board.hit_unpowered(Some(owner), target, amount, false);
                        }
                    });
                    ended = board.ended();
                }
                // `PowerCmd.Remove(this)`, on the instance that asked.
                // An effect before this one can take it off already, as The Bomb's
                // blast does when it kills a Possess Strength's caster.
                (
                    Listener::Power {
                        owner,
                        instance,
                        power,
                    },
                    sts2_core::HookEffect::Remove,
                ) => {
                    if let Some(place) = live.powers.place_of(owner, instance)
                        && let Some(effect) =
                            live.remove_power_at_asking(seat, owner, place).first()
                    {
                        unimplemented!("{power:?} asking for {effect:?} as it comes off")
                    }
                }
                (Listener::Card { id }, sts2_core::HookEffect::NoteOnSelf(note)) => {
                    live.note(id, note);
                }
                (Listener::Power { owner, .. }, sts2_core::HookEffect::DoomKill) => {
                    // `IsOverOrEnding`, and the owner dead already.
                    if ended.is_some() {
                        break 'effect;
                    }
                    let doomed = doomed::<V>(live, side);
                    // Only the first doomed creature's own Doom kills, and it
                    // kills them all.
                    if doomed.first() != Some(&owner) {
                        break 'effect;
                    }
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    board.doom_kill(&doomed);
                    ended = board.ended();
                }
                (
                    Listener::Power { owner, .. },
                    sts2_core::HookEffect::DamageOpponentsHolding { orb, amount },
                ) => {
                    let holding = live.players[seat]
                        .orbs
                        .iter()
                        .flat_map(|orbs| &orbs.queue)
                        .any(|held| held.orb == orb);
                    if !holding || ended.is_some() {
                        break 'effect;
                    }
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    let targets = sts2_core::Fight::opponents(&board);
                    board.damage_command(|board| {
                        for target in targets {
                            let hit = sts2_core::Fight::resolve(
                                board,
                                sts2_core::Effect::Attack {
                                    dealer: owner,
                                    target,
                                    damage: sts2_core::Amount::Number(amount),
                                    scaling: sts2_core::Scaling::Static,
                                },
                            );
                            sts2_core::Fight::apply(board, hit);
                        }
                    });
                    ended = board.ended();
                }
                // `CreatureCmd.GainBlock`, unpowered, on the player who carries
                // the relic.
                (Listener::Relic { .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                    let player = live.player(seat).id;
                    super::board::gain_unpowered_block::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        live.ascension,
                        player,
                        amount,
                    );
                }
                // `PowerCmd.Apply` on the player who carries the relic.
                (Listener::Relic { .. }, sts2_core::HookEffect::ApplyToOwner(power, amount)) => {
                    let player = live.player(seat).id;
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    board.land(player, power, amount, Some(player));
                    ended = ended.or(board.ended());
                }
                // `CreatureCmd.Damage` over `HittableEnemies`, from the player who
                // carries the relic.
                (Listener::Relic { .. }, effect @ sts2_core::HookEffect::DamageOpponents(_)) => {
                    if ended.is_some() {
                        break 'effect;
                    }
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    super::hook::apply(&mut board, [effect]);
                    ended = board.ended();
                }
                // Written on the fight's cards.
                (_, effect @ sts2_core::HookEffect::ClearAffliction(_)) => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    board.write_cards([effect]);
                }
                (_, effect) => {
                    unimplemented!("a listener asking for {effect:?} before a turn ends")
                }
            }
        }
        listening.leave(seat, live, boundary);
    }
    (ended, listening)
}

/// `DoomPower.GetDoomedCreatures` over the creatures standing on `side`: each
/// whose hit points are no more than its Doom, in fight order.
fn doomed<V: Rules>(live: &Live<V>, side: V::CombatSide) -> SmallVec<[CombatId; CREATURES]> {
    let doom = <V::Power as sts2_core::PowerKinds>::DOOM;
    live.creatures
        .iter()
        .filter(|creature| creature.side == side && creature.alive)
        .filter(|creature| {
            live.powers
                .of(creature.id)
                .find(|held| held.power() == doom)
                .and_then(|held| held.amount.count())
                .is_some_and(|amount| {
                    // A long, since About to Blow's hit points are past any
                    // stack a power carries.
                    let hp: i64 =
                        num_traits::cast(creature.hp).expect("a creature's hit points fit a long");
                    hp <= i64::from(amount)
                })
        })
        .map(|creature| creature.id)
        .collect()
}

/// What part of `SetupPlayerTurn` came to: the end of it, and how the fight
/// ended if it did, or a question it stopped on. The rest of a stopped
/// setup waits in [`Live::deferred`].
pub(super) enum Carried {
    Done(Option<super::Ended>),
    Parked,
}

/// What one listener's effect in `SetupPlayerTurn` came to.
pub(super) enum Listened {
    /// The walk has no listener left.
    Over,
    /// The effect ran, and the walk goes on.
    Done,
    /// The effect ended the fight.
    Ended(super::Ended),
    /// The effect asked, and the setup waits for the answer.
    Parked,
    /// `CardPileCmd.Draw` of this many cards, which the walk waits for.
    Draw(usize),
}

/// Stop `SetupPlayerTurn` on `question`, which picks cards to `act` on.
pub(super) fn ask_setup<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    act: sts2_core::PileAct,
    question: super::live::Asked<V>,
    signalled: bool,
) -> Listened {
    // `CardSelectCmd.FromHand` lists the hand as it asks. The setup's task
    // pauses there, and `AfterSideTurnStart` runs on while it waits, so a
    // card a listener adds then is not among the choices. A build that
    // signals before it lists keeps the question on the live hand.
    let question = match question {
        super::live::Asked::Pile {
            pile: Pile::Hand,
            selection,
        } if !V::signals_choice_before_listing() => super::live::Asked::Among {
            cards: live.players[seat]
                .cards
                .iter(Pile::Hand)
                .enumerate()
                .map(|(at, held)| {
                    let at = num_traits::NumCast::from(at)
                        .unwrap_or_else(|| unimplemented!("a hand deeper than a fight can number"));
                    sts2_core::CardAt::new(held.id, Pile::Hand, at)
                })
                .collect(),
            selection,
        },
        question => question,
    };
    live.asking = Some(super::live::Asking {
        then: Then::Setup { act },
        question,
        signalled,
    });
    Listened::Parked
}

/// Whether a pick pauses the setup before it lists the cards. A build's
/// `CardSelectCmd` that signals first does under the pause boundary, unless
/// the fight is ending, where it returns before signalling.
fn pauses<V: Rules>(live: &Live<V>, belts: &crate::run::Belts<V>) -> bool {
    live.pause_boundary
        && V::signals_choice_before_listing()
        && !super::board::ending::<V>(listen::Walk { live, belts })
}

/// One listener's `effect` in `Hook.BeforeHandDraw`, once the energy is
/// reset and before the turn's hand is drawn.
#[allow(clippy::too_many_arguments)]
pub(super) fn before_hand_draw<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    listener: Listener<V>,
    effect: sts2_core::HookEffect<V>,
) -> Listened {
    match effect {
        sts2_core::HookEffect::ChooseColorlessIntoHand => {
            let mut pool = V::combat_colorless_cards(live.players.len() > 1).to_vec();
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            let stream = <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION;
            let offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] =
                sts2_core::Fight::generate_cards(&mut board, &mut pool, stream).unwrap_or_else(
                    |_| unimplemented!("a colorless pool offering fewer than three"),
                );
            live.asking = Some(super::live::Asking {
                then: Then::SetupChosen { writes: &[] },
                // `FromChooseACardScreen` without `canSkip`,
                // so its screen has no skip button.
                question: super::live::Asked::Offered {
                    offered: offered.into_iter().collect(),
                    skippable: false,
                },
                signalled: false,
            });
            return Listened::Parked;
        }
        sts2_core::HookEffect::GenerateInHand { card, count } => {
            let count = num_traits::cast(count)
                .unwrap_or_else(|| unimplemented!("making {count:?} cards in hand"));
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            sts2_core::Fight::add_generated_cards(
                &mut board,
                sts2_core::CardIdent::new(card),
                Pile::Hand,
                count,
            );
        }
        sts2_core::HookEffect::GenerateFromPool {
            filter,
            count,
            writes,
        } => {
            generate_from_pool::<V>(
                seat,
                live,
                rngs.reborrow(),
                belts,
                ascension,
                filter,
                count,
                writes,
            );
        }
        sts2_core::HookEffect::GenerateDistinctColorless { count } => {
            generate_distinct_colorless::<V>(seat, live, rngs.reborrow(), belts, ascension, count);
        }
        sts2_core::HookEffect::GenerateDistinctFromPool { filter, count } => {
            generate_distinct_from_pool::<V>(
                seat,
                live,
                rngs.reborrow(),
                belts,
                ascension,
                filter,
                count,
            );
        }
        sts2_core::HookEffect::ExhaustFromDrawTop { passing_innate } => {
            let passed = passing_innate
                .then(|| {
                    live.players[seat]
                        .cards
                        .iter(Pile::Draw)
                        .position(|card| !card.face.innate())
                })
                .flatten();
            let top =
                passed.or_else(|| (!live.players[seat].cards.is_empty(Pile::Draw)).then_some(0));
            if let Some(at) = top {
                let card = live.players[seat]
                    .cards
                    .get(Pile::Draw, at)
                    .expect("a card stands where the walk found it");
                let index = num_traits::NumCast::from(at)
                    .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::act_on(
                    &mut board,
                    sts2_core::PileAct::Exhaust,
                    &[sts2_core::CardAt::new(card.id, Pile::Draw, index)],
                );
            }
        }
        // `PowerCmd.Apply` on the player, from a relic.
        sts2_core::HookEffect::ApplyToOwner(power, amount)
            if matches!(listener, Listener::Relic { .. }) =>
        {
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            sts2_core::Fight::apply_to_actor(&mut board, power, amount);
        }
        // A shuffle's question parks the setup, and the rest
        // of these listeners follow the answer.
        sts2_core::HookEffect::FreeRandomFromDraw {
            kind,
            preferring_without,
        } => {
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            let kinds: Vec<_> = sts2_core::Fight::cards_in(&board, Pile::Draw)
                .into_iter()
                .filter(|&at| {
                    V::card_kind(sts2_core::Fight::card_ident(&board, at).card) == Some(kind)
                })
                .collect();
            let kinds = match preferring_without {
                Some(keyword) => {
                    let without: Vec<_> = kinds
                        .iter()
                        .copied()
                        .filter(|&at| !sts2_core::Fight::card_keyword(&board, at, keyword))
                        .collect();
                    match without.is_empty() {
                        true => kinds,
                        false => without,
                    }
                }
                None => kinds,
            };
            let stream = <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_SELECTION;
            if let Some(at) = sts2_core::Fight::random_card(&mut board, &kinds, stream) {
                for pool in [sts2_core::CostPool::Energy, sts2_core::CostPool::Star] {
                    sts2_core::Fight::modify_card(
                        &mut board,
                        at,
                        sts2_core::Change::SetCost(pool, num_traits::zero()),
                        sts2_core::Lasting::ThisTurnOrUntilPlayed,
                    );
                }
                sts2_core::Fight::act_on(&mut board, sts2_core::PileAct::IntoHand, &[at]);
            }
        }
        // `AddGeneratedCardsToCombat` into the draw pile, each
        // at `CardPilePosition.Random`.
        // A relic's, whose owner is the player.
        sts2_core::HookEffect::AddToDrawAtRandom { card, count, .. } => {
            let count: usize = num_traits::cast(count)
                .unwrap_or_else(|| unimplemented!("adding {count:?} cards to the draw pile"));
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            for _ in 0..count {
                sts2_core::Fight::add_at_random(&mut board, card, Pile::Draw);
            }
        }
        sts2_core::HookEffect::ShuffleIfNecessary => {
            if !live.players[seat].cards.is_empty(Pile::Draw) {
                return Listened::Done;
            }
            let (pile, selection, act) = match super::board::shuffle_asking::<V>(
                seat,
                live,
                rngs.reborrow(),
                belts,
                ascension,
            ) {
                Ok(Some(asks)) => asks,
                Ok(None) => return Listened::Done,
                Err(ended) => return Listened::Ended(ended),
            };
            if pauses::<V>(live, belts) {
                let question = super::live::Asked::Pile { pile, selection };
                return ask_setup::<V>(seat, live, act, question, true);
            }
            match select::<V>(seat, live, rngs.reborrow(), belts, pile, selection, act) {
                Err(question) => {
                    return ask_setup::<V>(seat, live, act, question, false);
                }
                Ok(Some(ended)) => return Listened::Ended(ended),
                Ok(None) => {}
            }
        }
        sts2_core::HookEffect::SelectFromPile {
            pile,
            selection,
            act,
        } => {
            if pauses::<V>(live, belts) {
                let question = super::live::Asked::Pile { pile, selection };
                return ask_setup::<V>(seat, live, act, question, true);
            }
            match select::<V>(seat, live, rngs.reborrow(), belts, pile, selection, act) {
                Err(question) => {
                    return ask_setup::<V>(seat, live, act, question, false);
                }
                Ok(Some(ended)) => return Listened::Ended(ended),
                Ok(None) => {}
            }
        }
        sts2_core::HookEffect::Remove => {
            let Listener::Power { owner, power, .. } = listener else {
                unimplemented!("{listener:?} removing itself before the draw")
            };
            live.remove_power(seat, owner, power);
        }
        // Clones of the card a power keeps, one at a time, each
        // past a full hand to the discard pile.
        sts2_core::HookEffect::CloneKeptIntoHand { kept, count } => {
            let count: usize =
                num_traits::cast(count).unwrap_or_else(|| unimplemented!("{count:?} clones"));
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            for _ in 0..count {
                board.clone_kept(kept, Pile::Hand);
            }
        }
        // `CardPileCmd.Add(this, PileType.Hand)`, which sends a
        // card past a full hand to the discard pile.
        sts2_core::HookEffect::ToHand => {
            let Listener::Card { id } = listener else {
                unimplemented!("{listener:?} moving itself to the hand")
            };
            let found = [Pile::Hand, Pile::Draw, Pile::Discard, Pile::Exhaust]
                .into_iter()
                .find_map(|pile| live.players[seat].cards.find(pile, id).map(|at| (pile, at)));
            let to = match live.players[seat].cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
                true => Pile::Discard,
                false => Pile::Hand,
            };
            // A full hand sends it to the bottom of the
            // discard pile, out of the discard pile too.
            match found {
                Some((Pile::Hand, _)) => {}
                Some((from, at)) => live.players[seat].cards.move_to(from, at, to),
                None => unimplemented!("a card moving to the hand from outside the piles"),
            }
        }
        effect => unimplemented!("a listener asking for {effect:?} before the hand is drawn"),
    }
    Listened::Done
}

/// How many cards the turn's hand draws, once `BeforeHandDraw` is over, or
/// how a listener there ended the fight.
pub(super) fn hand_draw<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    belts: &mut crate::run::Belts<V>,
) -> Result<usize, super::Ended> {
    // `IterateCombatHookListeners` hands an ending fight no listeners, and
    // `IsOverOrEnding` draws nothing. So a fight a listener in
    // `BeforeHandDraw` ended, as Forgotten Soul's damage on the card Toasty
    // Mittens exhausts can, hears nothing more of the setup.
    if live.lost {
        return Err(super::Ended::Lost);
    }
    if live.won {
        return Err(super::Ended::Won);
    }
    let mut drawn = hand_size::<V>(seat, listen::Walk { live, belts }, live.turn);
    super::start::after_modifying_hand_draw::<V>(seat, live, belts);
    // On turn one a card whose enchantment says
    // `ShouldStartAtBottomOfDrawPile` sinks, then the innate cards
    // go on top and are all drawn, up to a full hand.
    if live.turn == num_traits::one() {
        let innate = live.players[seat].cards.innate_to_top(|face| {
            face.ident.enchantment.is_some_and(|enchanted| {
                V::should_start_at_bottom_of_draw_pile(enchanted.enchantment)
                    .is_decision_and(|sinks| sinks)
            })
        });
        drawn = drawn.max(innate).min(MAX_CARDS_IN_HAND);
    }
    Ok(drawn)
}

/// What a draw inside `SetupPlayerTurn` came to.
pub(super) enum Drew {
    /// The draw is over, and how it ended the fight if it did.
    Done(Option<super::Ended>),
    /// A question stopped it, with `left` cards still to draw.
    Parked { left: usize },
}

/// Draw `left` cards for `SetupPlayerTurn`: the turn's `hand`, or one a
/// listener asked for. `resumed` for a draw that a question paused.
///
/// A card auto-played as it is drawn can end the fight, and an ending fight
/// hands the hooks after it no listeners.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_in_setup<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    mut left: usize,
    hand: bool,
    mut resumed: bool,
) -> Drew {
    loop {
        match draw_step::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            left,
            hand,
            resumed,
        ) {
            Ok(drew) => return drew,
            // A shuffle took its pick unasked, and the draw goes on.
            Err(more) => {
                left = more;
                resumed = true;
            }
        }
    }
}

/// One step of [`draw_in_setup`]: how the draw ended, or how many cards are
/// still to draw where a shuffle inside it took its pick unasked.
#[allow(clippy::too_many_arguments)]
fn draw_step<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    left: usize,
    hand: bool,
    resumed: bool,
) -> Result<Drew, usize> {
    Ok(
        match super::board::draw_pausing::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            left,
            hand,
            resumed,
            None,
        ) {
            super::board::Drawn::Done(ended) => Drew::Done(ended),
            // A card auto-played as it was drawn asked, and the rest of the draw
            // waits for its play.
            super::board::Drawn::Asked { left } => Drew::Parked { left },
            super::board::Drawn::Paused {
                left,
                asks: (pile, selection, act),
            } => {
                if pauses::<V>(live, belts) {
                    let question = super::live::Asked::Pile { pile, selection };
                    ask_setup::<V>(seat, live, act, question, true);
                    return Ok(Drew::Parked { left });
                }
                match select::<V>(seat, live, rngs, belts, pile, selection, act) {
                    Err(question) => {
                        ask_setup::<V>(seat, live, act, question, false);
                        Drew::Parked { left }
                    }
                    Ok(Some(ended)) => Drew::Done(Some(ended)),
                    Ok(None) => return Err(left),
                }
            }
        },
    )
}

/// One listener's `effect` in `Hook.AfterPlayerTurnStart`, the last of the
/// setup. A relic's damage here can end the fight, which `ended` keeps.
#[allow(clippy::too_many_arguments)]
pub(super) fn after_player_turn_start<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    player: CombatId,
    ended: &mut Option<super::Ended>,
    listener: Listener<V>,
    effect: sts2_core::HookEffect<V>,
) -> Listened {
    match effect {
        // `CreatureCmd.Damage` over `HittableEnemies`, from
        // the player.
        sts2_core::HookEffect::DamageOpponents(amount) => {
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            let targets = board.hittable_opponents();
            board.damage_command(|board| {
                for target in targets {
                    let hit = sts2_core::Fight::resolve(
                        board,
                        sts2_core::Effect::Attack {
                            dealer: player,
                            target,
                            damage: sts2_core::Amount::Number(amount),
                            scaling: sts2_core::Scaling::Static,
                        },
                    );
                    sts2_core::Fight::apply(board, hit);
                }
            });
            *ended = ended.or(board.ended());
        }
        sts2_core::HookEffect::ChooseFromPoolIntoHand { count, writes } => {
            let count: usize = num_traits::cast(count)
                .unwrap_or_else(|| unimplemented!("offering {count:?} cards of the pool"));
            // `TakeRandom` shuffles the pool once and takes the
            // first `count`.
            let mut pool =
                V::combat_cards(live.players[seat].character, live.players.len() > 1).to_vec();
            sts2_core::rng::GameRng::unstable_shuffle(rngs.generation, &mut pool);
            pool.truncate(count);
            // An empty pool offers nothing, and the game returns.
            // `FromSimpleGrid` takes nothing once the fight is
            // ending, after the shuffle.
            if pool.is_empty() || super::board::ending::<V>(listen::Walk { live, belts }) {
                return Listened::Done;
            }
            live.asking = Some(super::live::Asking {
                then: Then::SetupChosen { writes },
                // `FromSimpleGrid` with one to take, which it
                // cannot skip.
                question: super::live::Asked::Offered {
                    offered: pool.into_iter().map(sts2_core::CardIdent::new).collect(),
                    skippable: false,
                },
                signalled: false,
            });
            return Listened::Parked;
        }
        sts2_core::HookEffect::SelectFromPile {
            pile,
            selection,
            act,
        } => {
            if pauses::<V>(live, belts) {
                let question = super::live::Asked::Pile { pile, selection };
                return ask_setup::<V>(seat, live, act, question, true);
            }
            match select::<V>(seat, live, rngs.reborrow(), belts, pile, selection, act) {
                Err(question) => {
                    return ask_setup::<V>(seat, live, act, question, false);
                }
                Ok(Some(ended)) => return Listened::Ended(ended),
                Ok(None) => {}
            }
        }
        // `OstyCmd.Summon` for the listener's owner.
        sts2_core::HookEffect::Summon(amount) => {
            super::board::summon_osty::<V>(seat, live, rngs.reborrow(), belts, ascension, amount);
        }
        // `PowerCmd.Remove(this)`.
        sts2_core::HookEffect::Remove => {
            let Listener::Power { owner, power, .. } = listener else {
                unimplemented!("{listener:?} removing itself as the turn starts")
            };
            live.remove_power(seat, owner, power);
        }
        // `PowerModel.SetAmount` on the listener's own instance,
        // which no hook hears.
        sts2_core::HookEffect::SetAmount(amount) => {
            let Listener::Power {
                owner,
                instance,
                power,
            } = listener
            else {
                unimplemented!("{listener:?} setting its amount as the turn starts")
            };
            let held = live
                .powers
                .of_mut(owner)
                .find(|held| held.instance == instance)
                .unwrap_or_else(|| unimplemented!("{power:?} left before it set its amount"));
            held.amount = super::live::Stacked::counted(amount);
        }
        // `OrbCmd.Passive` on whichever orb is first each
        // time, which returns once the fight is ending.
        sts2_core::HookEffect::FirstOrbPassive(count) => {
            let count: usize = num_traits::cast(count)
                .unwrap_or_else(|| unimplemented!("{count:?} orb passives at once"));
            for _ in 0..count {
                let Some(mut first) = live.players[seat]
                    .orbs
                    .as_ref()
                    .and_then(|orbs| orbs.queue.first().cloned())
                else {
                    break;
                };
                if ended.is_some() {
                    break;
                }
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                V::orb_passive(&mut first, &mut board);
                *ended = ended.or(board.ended());
                keep_orb(seat, live, 0, first);
            }
        }
        // `OrbCmd.Passive` on each orb of `OrbQueue.Orbs`, in
        // queue order, which returns once the fight is ending.
        sts2_core::HookEffect::EachOrbPassive => {
            let held = live.players[seat]
                .orbs
                .as_ref()
                .map_or(0, |orbs| orbs.queue.len());
            for place in 0..held {
                let Some(mut orb) = live.players[seat]
                    .orbs
                    .as_ref()
                    .and_then(|orbs| orbs.queue.get(place).cloned())
                else {
                    break;
                };
                if ended.is_some() {
                    break;
                }
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                V::orb_passive(&mut orb, &mut board);
                *ended = ended.or(board.ended());
                keep_orb(seat, live, place, orb);
            }
        }
        // `CardPileCmd.Draw` for the player, which a shuffle's
        // question can pause. A fight that is over draws
        // nothing, and a negative count draws nothing.
        sts2_core::HookEffect::Draw(count) => {
            if ended.is_none() {
                return Listened::Draw(num_traits::cast(count).unwrap_or(0));
            }
        }
        // `CreatureCmd.Damage` on the listener's owner: the
        // player for a relic, the holder for a power.
        sts2_core::HookEffect::DamageOwnerInFight {
            amount,
            unblockable,
            self_dealt,
        } if matches!(listener, Listener::Relic { .. } | Listener::Power { .. }) => {
            let owner = match listener {
                Listener::Power { owner, .. } => owner,
                _ => player,
            };
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            board.hit_unpowered(self_dealt.then_some(owner), owner, amount, unblockable);
            *ended = ended.or(board.ended());
        }
        // `CreatureCmd.GainBlock`, unpowered, on the player,
        // whose power or relic asked.
        effect @ sts2_core::HookEffect::GainBlock(_)
            if match listener {
                Listener::Power { owner, .. } => owner == player,
                Listener::Relic { .. } => true,
                _ => false,
            } =>
        {
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            super::hook::apply::<V, _>(&mut board, [effect]);
            *ended = ended.or(board.ended());
        }
        sts2_core::HookEffect::GenerateFromPool {
            filter,
            count,
            writes,
        } => generate_from_pool::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            filter,
            count,
            writes,
        ),
        // `CardCmd.Upgrade` of every card in hand.
        sts2_core::HookEffect::UpgradeHand => live.players[seat].cards.write_every(
            &[Pile::Hand],
            |_| true,
            |card| card.modify(sts2_core::Change::Upgrade, sts2_core::Lasting::ThisFight),
        ),
        // `PowerCmd.Apply` on the player, from a relic.
        sts2_core::HookEffect::ApplyToOwner(power, amount)
            if matches!(listener, Listener::Relic { .. }) =>
        {
            let mut board =
                Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
            sts2_core::Fight::apply_to_actor(&mut board, power, amount);
        }
        effect => unimplemented!("a listener asking for {effect:?} as the player's turn starts"),
    }
    Listened::Done
}

/// `AfterPlayerTurnStartLate`, once every listener of
/// `AfterPlayerTurnStart` has run.
pub(super) fn after_player_turn_start_late<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    player: CombatId,
) {
    let hook = sts2_core::EventHook::PlayerTurnStartLate {
        player,
        turn: live.turn,
    };
    let mut walking = listen::Walking::begin(live, belts, hook);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        match effect {
            // `CreatureCmd.Heal` on the player who carries
            // the relic.
            sts2_core::HookEffect::Heal(amount) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::heal(&mut board, player, amount);
            }
            effect => unimplemented!("{listener:?} asking for {effect:?} late as the turn starts"),
        }
    }
}

/// The rest of `Hook.BeforeHandDraw`'s walk, up to its end or the first
/// listener whose effect ended the fight or asked.
pub(super) fn walk_before_hand_draw<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    walk: &mut Option<Box<listen::Walking<V>>>,
) -> Listened {
    while let Some((listener, effect)) = walk
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        match before_hand_draw::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            listener,
            effect,
        ) {
            Listened::Done => {}
            heard => return heard,
        }
    }
    Listened::Over
}

/// The rest of `Hook.AfterPlayerTurnStart`'s walk, up to its end or the
/// first listener whose effect ended the fight, asked or drew.
#[allow(clippy::too_many_arguments)]
pub(super) fn walk_after_player_turn_start<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    player: CombatId,
    ended: &mut Option<super::Ended>,
    walk: &mut Option<Box<listen::Walking<V>>>,
) -> Listened {
    while let Some((listener, effect)) = walk
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        match after_player_turn_start::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            player,
            ended,
            listener,
            effect,
        ) {
            Listened::Done => {}
            heard => return heard,
        }
    }
    Listened::Over
}

/// `count` distinct colorless cards from one `GetDistinctForCombat` off
/// `CombatCardGeneration`, into the hand.
fn generate_distinct_colorless<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    count: V::Number,
) {
    let cards = V::combat_colorless_cards(live.players.len() > 1).to_vec();
    generate_distinct::<V>(seat, live, rngs, belts, ascension, cards, count);
}

/// `count` distinct cards of the character's combat pool that `filter`
/// keeps, from one `GetDistinctForCombat` off `CombatCardGeneration`, into
/// the hand.
fn generate_distinct_from_pool<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    filter: sts2_core::PoolFilter,
    count: V::Number,
) {
    let cards: Vec<V::Card> = V::combat_cards(live.players[seat].character, live.players.len() > 1)
        .iter()
        .copied()
        .filter(|&card| crate::reward::card::keeps::<V>(filter, card))
        .collect();
    generate_distinct::<V>(seat, live, rngs, belts, ascension, cards, count);
}

/// `GetDistinctForCombat` over `cards`: `TakeRandom` shuffles them once off
/// `CombatCardGeneration` and takes the first `count`, or all of them where
/// there are fewer. Each goes into the hand.
fn generate_distinct<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    mut cards: Vec<V::Card>,
    count: V::Number,
) {
    let count: usize = num_traits::cast(count)
        .unwrap_or_else(|| unimplemented!("making {count:?} distinct cards"));
    sts2_core::rng::GameRng::unstable_shuffle(rngs.generation, &mut cards);
    let mut board = Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
    for card in cards.into_iter().take(count) {
        sts2_core::Fight::add_generated_cards(
            &mut board,
            sts2_core::CardIdent::new(card),
            Pile::Hand,
            1,
        );
    }
}

/// `count` cards of the character's combat pool that `filter` keeps, each
/// drawn by `CardFactory.GetDistinctForCombat` off `CombatCardGeneration`
/// from the pool afresh, in the pool's own order. An empty pool gives
/// nothing. Each takes `keyword` where one is named, and goes into hand.
#[allow(clippy::too_many_arguments)]
fn generate_from_pool<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    filter: sts2_core::PoolFilter,
    count: V::Number,
    writes: &[(sts2_core::Change<V>, sts2_core::Lasting)],
) {
    let count: usize = num_traits::cast(count)
        .unwrap_or_else(|| unimplemented!("making {count:?} cards from a pool"));
    let keeps = |card: V::Card| crate::reward::card::keeps::<V>(filter, card);
    for _ in 0..count {
        let mut cards: Vec<V::Card> =
            V::combat_cards(live.players[seat].character, live.players.len() > 1)
                .iter()
                .copied()
                .filter(|card| keeps(*card))
                .collect();
        let mut board = Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
        let Ok([card]) = sts2_core::Fight::generate_cards::<1>(
            &mut board,
            &mut cards,
            <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION,
        ) else {
            return;
        };
        board.generate_written(card, Pile::Hand, 1, writes);
    }
}

/// `CardSelectCmd.FromHand` or `FromCombatPile` for `count` cards of `pile`:
/// the cards taken without asking, acted on now, or the question to stop on.
///
/// A discard that took Sly cards plays them, and whatever that ended comes
/// back.
///
/// Both commands take nothing once the fight is ending, and so ask nothing.
/// `FromHandForDiscard` and `FromHandForUpgrade` go through `FromHand`'s
/// check or one of their own.
pub(super) fn select<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    pile: Pile,
    selection: sts2_core::Selection,
    act: sts2_core::PileAct,
) -> Result<Option<super::Ended>, super::live::Asked<V>> {
    if super::board::ending::<V>(listen::Walk { live, belts }) {
        return Ok(None);
    }
    match super::play::taken(seat, live, pile, selection) {
        Some(picked) => {
            let mut rngs = rngs;
            let (sly, ended) = act_on::<V>(
                seat,
                live,
                rngs.reborrow(),
                belts,
                act,
                picked.as_slice(),
                false,
            );
            if ended.is_some() {
                return Ok(ended);
            }
            Ok(play_sly::<V>(seat, live, rngs, belts, sly, false))
        }
        None => Err(super::live::Asked::Pile { pile, selection }),
    }
}

/// Carry `act` out on the cards a selection took, in the order it took them.
///
/// A discard hands back the Sly cards it took, which the caller auto-plays
/// once every card is down.
pub(super) fn act_on<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    act: sts2_core::PileAct,
    picked: &[sts2_core::CardAt<V>],
    pausable: bool,
) -> (SmallVec<[sts2_core::CardId<V>; 2]>, Option<super::Ended>) {
    let mut sly = SmallVec::new();
    let asked_before = live.asking.is_some();
    for (taking, card) in picked.iter().enumerate() {
        // A question a hook asked stands the cards after it still. They go
        // on once what the question stood still is over.
        if !asked_before && live.asking.is_some() && taking > 0 {
            if !pausable {
                unimplemented!("{act:?} of more cards behind a question that cannot pause");
            }
            if !sly.is_empty() {
                unimplemented!("{act:?} of more cards behind a question, after a Sly card");
            }
            live.defer(super::live::Deferred::ActOn {
                act,
                cards: picked[taking..].iter().copied().collect(),
            });
            return (sly, None);
        }
        // `CardCmd` acts on the card wherever it stands now, which an
        // earlier card's hook can have moved.
        let from = match live.players[seat].cards.find(card.pile(), card.id()) {
            Some(_) => card.pile(),
            None => live.players[seat]
                .cards
                .find_any(card.id())
                .unwrap_or_else(|| unimplemented!("a picked card that left the fight"))
                .pile(),
        };
        let at = live.players[seat]
            .cards
            .find(from, card.id())
            .expect("a card stands in the pile it was just found in");
        let to = match act {
            // `CardCmd.Exhaust`, which raises its hook once the card is
            // down, and does nothing once the fight is over or ending.
            sts2_core::PileAct::Exhaust => {
                if live.won
                    || live.lost
                    || super::board::ending::<V>(super::listen::Walk { live, belts })
                {
                    continue;
                }
                let held = live.players[seat]
                    .cards
                    .get(from, at)
                    .expect("a card stands where it was found");
                live.players[seat].cards.move_to(from, at, Pile::Exhaust);
                if let Some(ended) =
                    exhausted::<V>(seat, live, rngs.reborrow(), belts, &held, false, pausable)
                {
                    return (sly, Some(ended));
                }
                continue;
            }
            // `CardCmd.Discard`, which plays a Sly card once every card is
            // down. `DiscardAndDraw` draws before that.
            sts2_core::PileAct::Discard | sts2_core::PileAct::DiscardAndDraw => {
                let held = live.players[seat]
                    .cards
                    .get(from, at)
                    .expect("a card stands where it was found");
                if held.face.sly() {
                    sly.push(held.id);
                }
                let card = held.face.ident.card;
                live.players[seat].cards.move_to(from, at, Pile::Discard);
                live.history.discarded(card);
                if let Some(ended) = card_discarded::<V>(seat, live, rngs.reborrow(), belts) {
                    return (sly, Some(ended));
                }
                continue;
            }
            // `CardCmd.TransformToRandom`, one card at a time, each drawn off
            // `CombatCardSelection` before it replaces the original where it
            // stood.
            sts2_core::PileAct::TransformToRandom => {
                let held = live.players[seat]
                    .cards
                    .get(from, at)
                    .expect("a card stands where it was found");
                let options =
                    V::transform_options(held.face.ident.card, true, live.players.len() > 1);
                let into = *sts2_core::rng::GameRng::choose(&mut *rngs.selection, &options)
                    .unwrap_or_else(|_| unimplemented!("a transform with no option"));
                let ascension = live.ascension;
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::transform(&mut board, *card, sts2_core::CardIdent::new(into));
                if let Some(ended) = board.ended() {
                    return (sly, Some(ended));
                }
                continue;
            }
            // `CardPileCmd.Add(cards, PileType.Hand)`, which sends a card past
            // a full hand to the discard pile.
            sts2_core::PileAct::IntoHand => {
                match live.players[seat].cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
                    true => Pile::Discard,
                    false => Pile::Hand,
                }
            }
            sts2_core::PileAct::IntoDraw => Pile::Draw,
            // `GiveSingleTurnRetain`, which `EndOfTurnCleanup` clears.
            sts2_core::PileAct::RetainThisTurn => {
                live.players[seat].cards.write(from, at, |face| {
                    face.modify(
                        sts2_core::Change::AddKeyword(
                            <V::CardKeyword as sts2_core::KeywordKinds<V>>::RETAIN,
                        ),
                        sts2_core::Lasting::ThisTurn,
                    );
                    true
                });
                continue;
            }
            // `CardPilePosition.Top`, the front, where a draw takes from.
            sts2_core::PileAct::OntoDrawTop => {
                live.players[seat].cards.move_to(from, at, Pile::Draw);
                let cards = &mut live.players[seat].cards;
                cards.move_to_top(Pile::Draw, cards.len(Pile::Draw) - 1);
                continue;
            }
        };
        live.players[seat].cards.move_to(from, at, to);
    }
    // `CardCmd.DiscardAndDraw`: as many drawn as were discarded, and the Sly
    // ones only after that, which the caller plays.
    if act == sts2_core::PileAct::DiscardAndDraw && !picked.is_empty() {
        let draw: V::Number =
            num_traits::cast(picked.len()).expect("a hand's worth fits a game number");
        let ascension = live.ascension;
        let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
        sts2_core::Fight::draw(&mut board, draw);
        return (sly, board.ended());
    }
    (sly, None)
}

/// `Hook.AfterCardDiscarded`, once a card of the player's is down. A relic
/// acts for the player, and how the fight ended comes back where it did.
///
/// # Panics
///
/// A listener other than a relic answering.
pub(super) fn card_discarded<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
) -> Option<super::Ended> {
    let hook = sts2_core::EventHook::CardDiscarded { side: live.side };
    let mut walking = listen::Walking::begin(live, belts, hook)?;
    let ascension = live.ascension;
    let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
    while let Some((listener, effect)) = board.next_of(&mut walking) {
        match listener {
            Listener::Relic { seat, slot } => board.relic_asked(seat, slot, effect),
            listener => unimplemented!("{listener:?} asking for {effect:?} after a discard"),
        }
    }
    board.ended()
}

/// `CardCmd.AutoPlay` with `SlyDiscard` for each Sly card a discard took, in
/// the order they went down, and whatever that ended.
pub(super) fn play_sly<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    sly: SmallVec<[sts2_core::CardId<V>; 2]>,
    pausable: bool,
) -> Option<super::Ended> {
    for id in sly {
        // `CardPileCmd.Add` to the play pile from wherever the card stands
        // by now: a draw between can shuffle it into the draw pile.
        let Some(from) = [Pile::Discard, Pile::Draw, Pile::Hand, Pile::Exhaust]
            .into_iter()
            .find(|&pile| live.players[seat].cards.find(pile, id).is_some())
        else {
            unimplemented!("a Sly card that left every pile before it played");
        };
        let ended = super::play::auto_play::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            id,
            from,
            None,
            pausable,
        );
        if ended.is_some() {
            return ended;
        }
    }
    None
}

/// A hook this fight raises and nothing ported answers.
///
/// # Panics
///
/// Anything in the fight listening to `hook`.
pub(super) fn unheard<V: Rules>(
    live: &Live<V>,
    belts: &crate::run::Belts<V>,
    hook: sts2_core::ModelHook,
) {
    if listen::heard::<V>(live, belts, hook) {
        let named = listen::listeners_to::<V>(live, belts, hook);
        unimplemented!("a listener to {}: {named:?}", hook.name());
    }
}

/// `Hook.AfterCardExhausted`, once `card` lies in the exhaust pile, and
/// whatever the answers ended.
pub(super) fn exhausted<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    card: &super::live::CardInstance<V>,
    ethereal: bool,
    pausable: bool,
) -> Option<super::Ended> {
    // `History.CardExhausted`, before the hook.
    live.history.exhausted(card.face.ident.card);
    let hook = sts2_core::EventHook::CardExhausted {
        card: card.face.ident.card,
        id: card.id,
        ethereal,
    };
    let mut walking = listen::Walking::begin(live, belts, hook)?;
    let ascension = live.ascension;
    // Inside an effect, a question a listener's command asks stands the
    // effect still.
    let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
    if pausable {
        board = board.blocking();
    }
    while let Some((listener, effect)) = board.next_of(&mut walking) {
        match (listener, effect) {
            // Clones of the card just exhausted, which the exhaust pile still
            // holds.
            (
                _,
                sts2_core::HookEffect::CloneIntoHand {
                    card,
                    count,
                    writes,
                },
            ) => {
                let count: usize =
                    num_traits::cast(count).unwrap_or_else(|| unimplemented!("{count:?} clones"));
                for _ in 0..count {
                    board.clone_written(card, Pile::Hand, writes);
                }
            }
            (Listener::Relic { seat, slot }, effect) => board.relic_asked(seat, slot, effect),
            (_, effect) => super::hook::apply::<V, _>(&mut board, [effect]),
        }
    }
    let skipped = std::mem::take(&mut board.skipped);
    let ended = board.ended();
    super::play::merge::<V>(&mut live.skip_tick, skipped);
    ended
}

/// Answer the question `SetupPlayerTurn` stopped on: `act` on the cards
/// picked. The rest of the setup waits in [`Live::deferred`].
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn answer_setup<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    act: sts2_core::PileAct,
    picked: sts2_core::SelectedCards<V>,
) -> Option<super::Ended> {
    let (sly, ended) = act_on::<V>(
        seat,
        live,
        rngs.reborrow(),
        belts,
        act,
        picked.as_slice(),
        false,
    );
    // A Sly card that asks stands the setup still behind its own question,
    // whose deferrals run ahead of the setup's.
    ended.or_else(|| play_sly::<V>(seat, live, rngs, belts, sly, true))
}

/// `SetupPlayerTurn` from where `setup` stopped, once the question it
/// stopped on is answered. `ended` is how the answer ended the fight, which
/// leaves the setup unfinished. How the setup ended the fight, where it ran
/// to its end.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn finish_setup<V: Rules>(
    live: &mut Live<V>,
    streams: &mut crate::stream::Streams<V>,
    belts: &mut crate::run::Belts<V>,
    mut setup: Box<super::setup::setup::State<V>>,
    ended: Option<super::Ended>,
) -> Option<super::Ended> {
    let lent = super::setup::Lent {
        live: &mut *live,
        streams,
        belts,
        ended,
        taken: None,
    };
    match diapause::Coroutine::resume(&mut *setup, lent) {
        diapause::CoroutineState::Yielded(()) => {
            live.defer(super::live::Deferred::Setup(setup));
            None
        }
        diapause::CoroutineState::Complete(ended) => ended,
    }
}

/// `SetupPlayerTurn` from inside `Hook.BeforeHandDraw`, whose walk `before`
/// is: as far as its end, or the first question it stops on, whose rest
/// waits in [`Live::deferred`].
pub(super) fn begin_setup<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    streams: &mut crate::stream::Streams<V>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    before: Option<Box<listen::Walking<V>>>,
) -> Carried {
    let lent = super::setup::Lent {
        live: &mut *live,
        streams,
        belts,
        ended: None,
        taken: None,
    };
    let mut setup = super::setup::setup::<V>(seat, ascension, before);
    let state = setup.start_with(lent);
    match state {
        diapause::CoroutineState::Yielded(()) => {
            live.defer(super::live::Deferred::Setup(Box::new(setup)));
            Carried::Parked
        }
        diapause::CoroutineState::Complete(ended) => Carried::Done(ended),
    }
}

/// The rest of the walk a setup stood still in, once the answer ended the
/// fight. The game's `foreach` goes on over the list it took, so every
/// listener left in it is asked. Nothing they ask for is carried out, as
/// an ending fight runs no commands, though a relic's code after its
/// `CardPileCmd.Draw` still runs.
pub(super) fn ask_out_walk<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    belts: &mut crate::run::Belts<V>,
    walking: Option<Box<listen::Walking<V>>>,
) {
    let Some(mut walking) = walking else {
        return;
    };
    while let Some((listener, effect)) = walking.next(seat, live, belts) {
        if let (Listener::Relic { seat, slot }, sts2_core::HookEffect::Draw(_)) = (listener, effect)
        {
            let relic = belts[seat][usize::from(slot)].relic;
            super::relic::own_command_over::<V>(&mut belts[seat], slot, relic);
        }
    }
}

/// How far `RunAutoPrePlayPhase` got.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PrePlayed<V: Rules> {
    /// Over, with how an auto-play ended the fight.
    Over(Option<super::Ended>),
    /// An auto-play asked. Once the question is over, what the pass still
    /// asked for goes on from `rest`, then the passes from `next`.
    Asked { rest: PassRest<V>, next: usize },
}

/// `RunAutoPrePlayPhase` once the player's turn is set up:
/// `AfterAutoPrePlayPhaseEntered`'s early pass, its own, then its late one,
/// each over every listener. How an auto-play there ended the fight.
///
/// # Panics
///
/// A listener asking for anything but an auto-play of a card.
pub(super) fn pre_play_phase<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
) -> PrePlayed<V> {
    use sts2_core::{ModelHook, Overrides};

    // `Phase = PlayerTurnPhase.AutoPrePlay`, which what the passes play reads.
    live.players[seat].phase = <V::PlayerTurnPhase as TurnPhaseKinds>::AUTO_PRE_PLAY;
    if V::checks_empty_hand_entering_pre_play()
        && let Some(ended) = empty_hand_entering_pre_play::<V>(seat, live, rngs.reborrow(), belts)
    {
        return PrePlayed::Over(Some(ended));
    }
    // Most fights hold nothing that hears any of the three passes.
    let hooks = Overrides::of(ModelHook::AfterAutoPrePlayPhaseEnteredEarly)
        .with(ModelHook::AfterAutoPrePlayPhaseEntered)
        .with(ModelHook::AfterAutoPrePlayPhaseEnteredLate);
    if !listen::heard_any::<V>(live, belts, hooks) {
        return PrePlayed::Over(None);
    }
    pre_play_heard::<V>(seat, live, rngs, belts)
}

/// `CheckForEmptyHand` as the phase turns to `AutoPrePlay`: an empty hand
/// raises `AfterHandEmptied`, where the player's turn is in progress. How
/// what it asked for ended the fight.
fn empty_hand_entering_pre_play<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
) -> Option<super::Ended> {
    if live.players[seat].cards.len(Pile::Hand) != 0 || live.won || live.lost {
        return None;
    }
    let hook = sts2_core::EventHook::HandEmptied {
        player: live.player(seat).id,
        phase: <V::PlayerTurnPhase as TurnPhaseKinds>::AUTO_PRE_PLAY,
    };
    let walking = listen::Walking::begin(live, belts, hook);
    let ascension = live.ascension;
    let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
    board.carry_out_walk(walking);
    board.ended()
}

/// [`pre_play_phase`] once something hears one of its passes.
#[cold]
#[inline(never)]
#[tracing::instrument(level = "trace", skip_all)]
fn pre_play_heard<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
) -> PrePlayed<V> {
    pre_play_from::<V>(seat, live, rngs, belts, PassRest::none(), 0)
}

/// The pre-play phase from `rest`, what a pass still asked for, then from the
/// pass at `next`. A question an auto-play asks stands the phase still, with
/// what is left of it deferred behind the answer.
///
/// # Panics
///
/// A listener asking for anything but an auto-play of a card.
/// `CardCmd.AutoPlay` of `CreateDupe` on the last card of `kinds` its owner
/// finished playing in its last turn that was no dupe itself. Nothing when
/// there is none. The dupe enters the fight in the play pile.
///
/// # Panics
///
/// A card the fight never held. An X-cost dupe captures the energy on
/// hand as it plays, as any auto-play does, since `AutoPlay` skips no
/// capture.
fn auto_play_dupe_of_last_play<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    turn: V::Count,
    kinds: &[sts2_core::CardKind],
) -> Option<super::Ended> {
    let last = live
        .history()
        .finished_last_player_turn(turn)
        .filter(|finished| {
            !finished.dupe && V::card_kind(finished.card).is_some_and(|kind| kinds.contains(&kind))
        })
        .last()?;
    // `CreateDupe` clones the card's model, which outlives its leaving the
    // fight, as a transform's original does.
    let Some(original) = live.players[seat].cards.last_state(last.id) else {
        unimplemented!("a dupe of {:?}, which the fight never held", last.card);
    };
    let id = live.card_ids.mint();
    let index = num_traits::NumCast::from(live.players[seat].cards.len(Pile::Play))
        .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
    live.players[seat]
        .cards
        .enter_play(original.face.duped().named(id));
    // `CardPileCmd.Add` to the play pile from no pile raises
    // `AfterCardEnteredCombat`, and a dupe is a clone of its original.
    let ascension = live.ascension;
    let mut board =
        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None).blocking();
    board.card_entered_combat(
        sts2_core::CardAt::new(id, Pile::Play, index),
        original.face.ident.card,
        true,
    );
    if let Some(ended) = board.ended() {
        return Some(ended);
    }
    super::play::auto_play_taken::<V>(seat, live, rngs, belts, id, false, true)
}

pub(super) fn pre_play_from<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    rest: PassRest<V>,
    next: usize,
) -> PrePlayed<V> {
    let turn = live.turn;
    let passes = [
        sts2_core::EventHook::AutoPrePlayPhaseEarly { turn },
        sts2_core::EventHook::AutoPrePlayPhase { turn },
        sts2_core::EventHook::AutoPrePlayPhaseLate { turn },
    ];
    let mut pass = next;
    let mut asked = rest;
    loop {
        while let Some((listener, effect)) = asked.next(seat, live, belts) {
            match effect {
                // `CardCmd.AutoPlay` of the card, from wherever it stands.
                sts2_core::HookEffect::AutoPlay(card) => {
                    let Some(from) = live.players[seat].cards.pile_of(card) else {
                        unimplemented!("auto-playing {card:?}, which stands in no pile");
                    };
                    let ended = super::play::auto_play::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        card,
                        from,
                        None,
                        true,
                    );
                    if ended.is_some() {
                        return PrePlayed::Over(ended);
                    }
                }
                sts2_core::HookEffect::AutoPlayDupeOfLastPlay { kinds } => {
                    let ended = auto_play_dupe_of_last_play::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        turn,
                        kinds,
                    );
                    if ended.is_some() {
                        return PrePlayed::Over(ended);
                    }
                    // `CardCmd.AutoPlay` is awaited, so what the pass still
                    // asked for waits on the answer.
                    if live.asking.is_some() {
                        return PrePlayed::Asked {
                            rest: asked,
                            next: pass,
                        };
                    }
                }
                // `CardSelectCmd.PushSelector(new VakuuCardSelector())` around
                // the plays, which answers every question they ask.
                sts2_core::HookEffect::AutoPlayHand { mut left } => {
                    live.selector_answers = true;
                    let start = live.turn;
                    while left > 0 && !live.players[seat].ending_turn && live.turn == start {
                        let Some(first) = first_playable::<V>(seat, live, belts) else {
                            break;
                        };
                        let captured =
                            spend_resources::<V>(seat, live, rngs.reborrow(), belts, first.costs);
                        left -= 1;
                        let ended = super::play::auto_play_capturing::<V>(
                            seat,
                            live,
                            rngs.reborrow(),
                            belts,
                            first.id,
                            Pile::Hand,
                            first.target,
                            true,
                            Some(captured),
                        );
                        if ended.is_some() {
                            return PrePlayed::Over(ended);
                        }
                        // `CardCmd.AutoPlay` is awaited, so the plays left
                        // wait on the answer, ahead of what the pass still
                        // asked for.
                        if live.asking.is_some() {
                            asked.first =
                                Some((listener, sts2_core::HookEffect::AutoPlayHand { left }));
                            return PrePlayed::Asked {
                                rest: asked,
                                next: pass,
                            };
                        }
                    }
                    live.selector_answers = false;
                }
                sts2_core::HookEffect::AutoPlayFromDraw(count) => {
                    let count: usize = num_traits::cast(count)
                        .unwrap_or_else(|| unimplemented!("auto-playing {count:?} cards"));
                    let ascension = live.ascension;
                    let mut board = super::board::Board::for_card(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        ascension,
                        None,
                        None,
                    )
                    .blocking();
                    sts2_core::Fight::auto_play_from_draw(&mut board, count, false);
                    if let Some(ended) = board.ended() {
                        return PrePlayed::Over(Some(ended));
                    }
                }
                effect => {
                    unimplemented!("{listener:?} asking for {effect:?} as the play phase opens")
                }
            }
            // `RunAutoPrePlayPhase` awaits each hook, so the rest waits on
            // the answer.
            if live.asking.is_some() {
                return PrePlayed::Asked {
                    rest: asked,
                    next: pass,
                };
            }
        }
        // Past the last pass, the phase is over and the fight goes on.
        let Some(&hook) = passes.get(pass) else {
            return PrePlayed::Over(None);
        };
        asked = PassRest::walk(live, belts, hook);
        pass += 1;
    }
}

/// The hand's first card that `CanPlay`, as `WhisperingEarring` plays it.
struct FirstPlayable<V: Rules> {
    id: sts2_core::CardId<V>,
    /// The target the relic names for it.
    target: Option<CombatId>,
    /// What it costs now, in energy and in stars.
    costs: (sts2_core::Cost, sts2_core::Cost),
}

/// The hand's first card that `CanPlay`.
///
/// # Panics
///
/// A card that targets an ally, which the game draws off `CombatTargets`.
fn first_playable<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &Live<V>,
    belts: &crate::run::Belts<V>,
) -> Option<FirstPlayable<V>> {
    use sts2_core::TargetKinds;

    let walk = listen::Walk { live, belts };
    let payable =
        super::legal::first_playable::<V>(seat, live.players[seat].cards.iter(Pile::Hand), walk)?;
    let target = match payable.target {
        // `HittableEnemies.FirstOrDefault()`.
        t if t == <V::TargetType as TargetKinds>::ANY_ENEMY => {
            let enemies = live
                .creatures
                .iter()
                .filter(|creature| {
                    creature.side == <V::CombatSide as SideKinds>::ENEMY && creature.alive
                })
                .map(|creature| creature.id)
                .filter(|&creature| {
                    !listen::any_decides::<V>(
                        listen::Walk { live, belts },
                        sts2_core::DecisionHook::ShouldAllowHitting {
                            creature,
                            alive: true,
                        },
                        false,
                    )
                });
            enemies.into_iter().next()
        }
        t if t == <V::TargetType as TargetKinds>::ANY_PLAYER => Some(live.player(seat).id),
        t if t == <V::TargetType as TargetKinds>::ANY_ALLY => {
            unimplemented!("Whispering Earring aiming a card at an ally")
        }
        _ => None,
    };
    Some(FirstPlayable {
        id: payable.held.id,
        target,
        costs: payable.costs,
    })
}

/// `CardModel.SpendResources`: the energy and the stars a card costs now,
/// taken from the player's pools, with `AfterStarsSpent` for the stars. An
/// X takes the whole pool. The energy and the stars spent, which are what
/// `SpendEnergy` captures as an X and `SpendStars` keeps as
/// `LastStarsSpent`.
fn spend_resources<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    (cost, star_cost): (sts2_core::Cost, sts2_core::Cost),
) -> (V::Count, V::Count) {
    let energy = super::legal::spend(cost, live.players[seat].energy)
        .unwrap_or_else(|| unimplemented!("a cost of {cost:?}"))
        .min(live.players[seat].energy);
    live.history.energy_spent(energy);
    live.players[seat].energy = live.players[seat].energy - energy;
    let spent: V::Number = num_traits::cast(energy).expect("an energy cost fits a game number");
    if let Some(ended) = super::board::energy_spent::<V>(seat, live, rngs.reborrow(), belts, spent)
    {
        unimplemented!("a fight {ended:?} as its energy was spent");
    }
    let stars: V::Count = super::legal::spend(star_cost, live.players[seat].stars)
        .unwrap_or_else(|| unimplemented!("a star cost of {star_cost:?}"));
    if stars > num_traits::zero() {
        let spent: V::Number = num_traits::cast(stars).expect("a star cost fits a game number");
        live.modify_stars(seat, -spent);
        let hook = sts2_core::EventHook::StarsSpent {
            player: live.player(seat).id,
            amount: spent,
        };
        if let Some(mut walking) = listen::Walking::begin(live, belts, hook) {
            let ascension = live.ascension;
            let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
            while let Some((_, effect)) = board.next_of(&mut walking) {
                super::hook::apply::<V, _>(&mut board, [effect]);
            }
            if let Some(ended) = board.ended() {
                unimplemented!("a fight {ended:?} as its stars were spent");
            }
        }
    }
    (energy, stars)
}

/// What a pre-play or post-play pass still has to carry out. That is an
/// answer an auto-play's question stopped part-way through, then the rest
/// of the pass's walk.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PassRest<V: Rules> {
    first: Option<(Listener<V>, sts2_core::HookEffect<V>)>,
    walking: Option<Box<listen::Walking<V>>>,
}

impl<V: Rules> PassRest<V> {
    /// The walk over `hook`'s listeners, where any hears it.
    pub(super) fn walk(
        live: &Live<V>,
        belts: &crate::run::Belts<V>,
        hook: sts2_core::EventHook<V>,
    ) -> Self {
        PassRest {
            first: None,
            walking: listen::Walking::begin(live, belts, hook).map(Box::new),
        }
    }

    /// Nothing left.
    fn none() -> Self {
        PassRest {
            first: None,
            walking: None,
        }
    }

    /// The next answer: the one stopped part-way through, then the walk's.
    fn next(
        &mut self,
        seat: sts2_core::SeatIndex,
        live: &mut Live<V>,
        belts: &mut crate::run::Belts<V>,
    ) -> Option<(Listener<V>, sts2_core::HookEffect<V>)> {
        self.first
            .take()
            .or_else(|| self.walking.as_mut()?.next(seat, live, belts))
    }
}

/// The phase a player's turn stands in once its pre-play phase has run. That
/// is the play phase, or still the pre-play one while its question waits.
pub(super) fn after_pre_play<V: Rules>(live: &Live<V>) -> V::PlayerTurnPhase {
    match live.asking.is_some() {
        true => <V::PlayerTurnPhase as TurnPhaseKinds>::AUTO_PRE_PLAY,
        false => <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
    }
}

/// `AfterSideTurnStart`, raised on every listener once the side has drawn.
///
/// # Panics
///
/// A listener asking for something no side turn start can carry out.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn side_turn_started<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    side: V::CombatSide,
    extra: bool,
) -> Option<super::Ended> {
    let turn = live.turn;
    let mut ended = None;
    let hook = sts2_core::EventHook::SideTurnStart {
        side,
        turn,
        round: live.round,
        room: live.room(),
        extra,
    };
    // Each listener acts as the walk reaches it.
    let mut walk = listen::Walker::begin(live, belts, hook);
    let mut asked = sts2_core::HookEffects::new();
    while let Some(listener) = walk
        .as_mut()
        .and_then(|walk| walk.next(seat, live, belts, &mut asked))
    {
        for effect in asked.drain(..) {
            match effect {
                // `PowerCmd.Remove(this)`, from a power spent as its side begins.
                sts2_core::HookEffect::Remove => {
                    let Listener::Power { owner, power, .. } = listener else {
                        unimplemented!("{listener:?} removing itself as a side turn starts")
                    };
                    live.remove_power(seat, owner, power);
                }
                // `PowerCmd.Decrement(this)`.
                sts2_core::HookEffect::Decrement => {
                    let Listener::Power { owner, power, .. } = listener else {
                        unimplemented!("{listener:?} decrementing itself as a side turn starts")
                    };
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    board.modify_amount(owner, power, -num_traits::one::<V::Number>());
                }
                // `PowerCmd.Apply<P>(Owner, amount, Owner)`. A relic's owner is
                // the player.
                sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                    let owner = match listener {
                        Listener::Power { owner, .. } => owner,
                        Listener::Relic { .. } => live.player(seat).id,
                        listener => unimplemented!("{listener:?} applying a power to its owner"),
                    };
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    board.land(owner, power, amount, Some(owner));
                }
                // `NextItem` over `HittableEnemies` off `CombatTargets`, then
                // `PowerCmd.Apply<P>` from the listener's owner.
                sts2_core::HookEffect::ApplyToRandomOpponent { power, amount } => {
                    let Listener::Power { owner, .. } = listener else {
                        unimplemented!("{listener:?} applying a power to a random enemy")
                    };
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    if let Some(target) = sts2_core::Fight::random_opponent(&mut board) {
                        board.land(target, power, amount, Some(owner));
                    }
                }
                sts2_core::HookEffect::UpgradeHand => live.players[seat].cards.write_every(
                    &[sts2_core::Pile::Hand],
                    |_| true,
                    |card| card.modify(sts2_core::Change::Upgrade, sts2_core::Lasting::ThisFight),
                ),
                sts2_core::HookEffect::Forge { card, amount } => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    sts2_core::Fight::forge(&mut board, card, amount);
                }
                // `PlayerCmd.GainEnergy`.
                sts2_core::HookEffect::GainEnergy(amount) => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    sts2_core::Fight::modify_energy(&mut board, amount);
                }
                sts2_core::HookEffect::GainEnergyForGold { energy, gold } => {
                    if live.players[seat].gold >= gold {
                        let mut board = Board::for_card(
                            seat,
                            live,
                            rngs.reborrow(),
                            belts,
                            ascension,
                            None,
                            None,
                        );
                        sts2_core::Fight::modify_energy(&mut board, energy);
                        live.players[seat].gold = live.players[seat].gold - gold;
                    }
                }
                // `CreatureCmd.GainBlock`, unpowered, on the power's owner.
                sts2_core::HookEffect::GainBlockPerOrbKind(amount) => {
                    let Listener::Power { owner, .. } = listener else {
                        unimplemented!("{listener:?} gaining block for its orbs")
                    };
                    let mut kinds: SmallVec<[V::Orb; 10]> = SmallVec::new();
                    for held in live.players[seat].orbs.iter().flat_map(|orbs| &orbs.queue) {
                        if !kinds.contains(&held.orb) {
                            kinds.push(held.orb);
                        }
                    }
                    let count: V::Number = num_traits::cast(kinds.len())
                        .expect("a queue's kinds of orb fit a game number");
                    super::board::gain_unpowered_block::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        live.ascension,
                        owner,
                        amount * count,
                    );
                }
                // `PoisonPower.AfterSideTurnStart`: each round hits the owner for
                // the stack it holds then, and takes one off while it lives. An
                // ending fight runs no more commands.
                sts2_core::HookEffect::DamageSelfByStack { times } => {
                    let Listener::Power { owner, power, .. } = listener else {
                        unimplemented!("{listener:?} damaging itself by its stack")
                    };
                    let mut round = num_traits::zero::<V::Number>();
                    while round < times && ended.is_none() {
                        round = round + num_traits::one();
                        // The instance keeps its `Amount` once a death took
                        // it off, and the loop goes on reading it, as where
                        // the Waterfall Giant comes back.
                        let on = live
                            .powers
                            .of(owner)
                            .find(|held| held.power() == power)
                            .map(|held| held.amount.amount());
                        let detached = || {
                            live.detached
                                .iter()
                                .rev()
                                .find(|(holder, held)| *holder == owner && held.power() == power)
                                .map(|(_, held)| held.amount.amount())
                        };
                        let Some(PowerAmount::Counted(stack)) = on.or_else(detached) else {
                            break;
                        };
                        let mut board = Board::for_card(
                            seat,
                            live,
                            rngs.reborrow(),
                            belts,
                            ascension,
                            None,
                            None,
                        );
                        board.hit_unpowered(None, owner, stack, true);
                        let alive = board
                            .at(owner)
                            .is_some_and(|at| board.live.creatures[at].alive);
                        ended = ended.or(board.ended());
                        // A death inside the hit can take it off, and its
                        // owner can come back inside the same hit.
                        let removed = !live.powers.of(owner).any(|held| held.power() == power);
                        match (alive, removed) {
                            (true, false) => {
                                let mut board = Board::for_card(
                                    seat,
                                    live,
                                    rngs.reborrow(),
                                    belts,
                                    ascension,
                                    None,
                                    None,
                                );
                                board.modify_amount(owner, power, -num_traits::one::<V::Number>());
                                ended = ended.or(board.ended());
                            }
                            (true, true) => {
                                let mut board = Board::for_card(
                                    seat,
                                    live,
                                    rngs.reborrow(),
                                    belts,
                                    ascension,
                                    None,
                                    None,
                                );
                                board.modify_detached(
                                    owner,
                                    power,
                                    -num_traits::one::<V::Number>(),
                                );
                                ended = ended.or(board.ended());
                            }
                            (false, _) => {}
                        }
                    }
                }
                // `PowerCmd.Apply<P>` over `GetOpponentsOf(Owner)` that are
                // alive, from nobody. A relic's owner is the player.
                sts2_core::HookEffect::ApplyToLivingOpponents(power, amount)
                    if matches!(listener, Listener::Relic { .. }) =>
                {
                    let targets: SmallVec<[CombatId; CREATURES]> = (live.creatures.iter())
                        .filter(|creature| {
                            creature.side == <V::CombatSide as SideKinds>::ENEMY && creature.alive
                        })
                        .map(|creature| creature.id)
                        .collect();
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    for target in targets {
                        board.land(target, power, amount, None);
                    }
                    ended = ended.or(board.ended());
                }
                // `PowerCmd.Apply<P>(HittableEnemies, amount, Owner)`.
                sts2_core::HookEffect::ApplyToOpponents(power, amount) => {
                    let Listener::Power { owner, .. } = listener else {
                        unimplemented!("{listener:?} applying a power to its opponents")
                    };
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    for target in board.hittable_opponents() {
                        board.land(target, power, amount, Some(owner));
                    }
                    ended = ended.or(board.ended());
                }
                // `PlayerCmd.GainStars`.
                sts2_core::HookEffect::GainStars(amount) => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    sts2_core::Fight::gain_stars(&mut board, amount);
                }
                // `OrbCmd.Channel` on the player, which evokes an orb to make room.
                sts2_core::HookEffect::Channel(orb) => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    sts2_core::Fight::channel(&mut board, orb);
                    ended = ended.or(board.ended());
                }
                // `OstyCmd.Summon` for the player.
                sts2_core::HookEffect::Summon(amount) => {
                    super::board::summon_osty::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        ascension,
                        amount,
                    );
                }
                // `CreatureCmd.GainBlock`, unpowered, on a relic's owner, who is
                // the player the board acts for.
                effect @ sts2_core::HookEffect::GainBlock(_)
                    if matches!(listener, Listener::Relic { .. }) =>
                {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    super::hook::apply::<V, _>(&mut board, [effect]);
                    ended = ended.or(board.ended());
                }
                // `PlayerCmd.LoseEnergy` from the player's pool.
                effect @ sts2_core::HookEffect::LoseEnergy(_) => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    super::hook::apply::<V, _>(&mut board, [effect]);
                }
                // `CreatureCmd.GainBlock`, unpowered, on a creature the listener
                // names.
                effect @ sts2_core::HookEffect::GainBlockOf { .. } => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    super::hook::apply::<V, _>(&mut board, [effect]);
                    ended = ended.or(board.ended());
                }
                sts2_core::HookEffect::GenerateFromPool {
                    filter,
                    count,
                    writes,
                } => generate_from_pool::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    ascension,
                    filter,
                    count,
                    writes,
                ),
                sts2_core::HookEffect::GenerateDistinctColorless { count } => {
                    generate_distinct_colorless::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        ascension,
                        count,
                    )
                }
                sts2_core::HookEffect::AddOrbSlots(amount) => {
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    sts2_core::Fight::add_orb_slots(&mut board, amount);
                }
                sts2_core::HookEffect::GenerateDistinctFromPool { filter, count } => {
                    generate_distinct_from_pool::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        ascension,
                        filter,
                        count,
                    )
                }
                effect => unimplemented!("a listener asking for {effect:?} as a side turn starts"),
            }
        }
    }
    if ended.is_some() {
        return ended;
    }
    // `Hook.AfterSideTurnStartLate`. A power's `PowerCmd.Decrement(this)`
    // has ticked by the time this hears it.
    let hook = sts2_core::EventHook::SideTurnStartLate { side };
    let mut walking = listen::Walking::begin(live, belts, hook);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        match (listener, effect) {
            // `CreatureCmd.Kill`, from a power the decrement took off.
            (Listener::Power { .. }, sts2_core::HookEffect::KillCreature(creature)) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                board.kill_outright(creature, true);
                ended = ended.or(board.ended());
            }
            (listener, effect) => {
                unimplemented!("{listener:?} asking for {effect:?} late as a side turn starts")
            }
        }
    }
    ended
}

/// `FlushPlayerHand`: what is left in hand goes to the discard pile, unless
/// a listener answers `ShouldFlush` no, which keeps the whole hand.
///
/// A card that retains stays where it is, whether it prints the keyword or
/// something in the fight wrote it on.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn flush_hand<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
) {
    let player = live.player(seat).id;
    let turn = live.turn;
    let fight = listen::Walk { live, belts };
    if !listen::any_decides::<V>(
        fight,
        sts2_core::DecisionHook::ShouldFlush { player, turn },
        false,
    ) {
        live.players[seat]
            .cards
            .sift(Pile::Hand, Pile::Discard, Face::retains);
    }
    // `Hook.AfterFlush`, with what stayed in hand retained.
    let hook = sts2_core::EventHook::Flushed;
    let Some(mut walking) = listen::Walking::begin(live, belts, hook) else {
        return;
    };
    let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
    while let Some((listener, effect)) = board.next_of(&mut walking) {
        match effect {
            sts2_core::HookEffect::LowerRetainedCost => {
                let costly: Vec<_> = sts2_core::Fight::cards_in(&board, Pile::Hand)
                    .into_iter()
                    .filter(|&at| {
                        matches!(
                            sts2_core::Fight::card_costs(&board, at, false).0,
                            sts2_core::Cost::Fixed(cost) if cost.get() > 0
                        )
                    })
                    .collect();
                let stream = <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_SELECTION;
                if let Some(at) = sts2_core::Fight::random_card(&mut board, &costly, stream) {
                    sts2_core::Fight::modify_card(
                        &mut board,
                        at,
                        sts2_core::Change::ChangeCost(
                            sts2_core::CostPool::Energy,
                            -num_traits::one::<V::Number>(),
                        ),
                        sts2_core::Lasting::UntilPlayed,
                    );
                }
            }
            effect => unimplemented!("{listener:?} asking for {effect:?} after a flush"),
        }
    }
}

/// `Hook.AfterEnergyReset`, raised on every listener, and how the fight
/// ended if an orb evoked to make room ended it.
///
/// After the reset and before the hand is drawn, which is the order
/// `SetupPlayerTurn` runs them in.
///
/// # Panics
///
/// A listener asking for something no energy reset carries out.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn energy_reset<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
) -> Option<super::Ended> {
    let player = live.player(seat).id;
    let hook = sts2_core::EventHook::EnergyReset {
        player,
        turn: live.turn,
    };
    let mut ended = None;
    let mut walking = listen::Walking::begin(live, belts, hook);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        match (listener, effect) {
            // `PlayerCmd.GainEnergy`, then `PowerCmd.Remove(this)`.
            (
                Listener::Power { owner, power, .. },
                sts2_core::HookEffect::GainEnergyAndRemove(amount),
            ) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::modify_energy(&mut board, amount);
                live.remove_power(seat, owner, power);
            }
            // `PlayerCmd.GainStars`, whose `AfterStarsGained` can end the
            // fight, as Black Hole's damage does.
            (_, sts2_core::HookEffect::GainStars(amount)) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::gain_stars(&mut board, amount);
                ended = ended.or(board.ended());
            }
            // `PowerCmd.Remove(this)`.
            (Listener::Power { owner, power, .. }, sts2_core::HookEffect::Remove) => {
                live.remove_power(seat, owner, power)
            }
            // `PlayerCmd.GainEnergy`.
            (_, sts2_core::HookEffect::GainEnergy(amount)) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::modify_energy(&mut board, amount);
            }
            // `PowerCmd.Decrement(this)`.
            (Listener::Power { owner, power, .. }, sts2_core::HookEffect::Decrement) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                board.modify_amount(owner, power, -num_traits::one::<V::Number>());
            }
            // `OrbCmd.Channel` on the player, which evokes an orb to make room.
            (_, sts2_core::HookEffect::Channel(orb)) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                sts2_core::Fight::channel(&mut board, orb);
                ended = ended.or(board.ended());
            }
            (_, effect) => unimplemented!("a listener asking for {effect:?} as energy resets"),
        }
    }
    ended
}

/// `AfterEnergyResetLate`, which `Hook.AfterEnergyReset` raises once every
/// listener has heard `AfterEnergyReset`.
///
/// # Panics
///
/// A listener asking for something no energy reset carries out.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn energy_reset_late<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: crate::stream::CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
) {
    let player = live.player(seat).id;
    let hook = sts2_core::EventHook::EnergyResetLate {
        player,
        turn: live.turn,
    };
    let mut walking = listen::Walking::begin(live, belts, hook);
    while let Some((_, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        match effect {
            sts2_core::HookEffect::Summon(amount) => {
                super::board::summon_osty::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    ascension,
                    amount,
                );
            }
            effect => unimplemented!("a listener asking for {effect:?} as energy resets late"),
        }
    }
}

/// `AfterTurnStart` on every creature of one side, which clears its block
/// unless a listener says otherwise, then `Hook.AfterBlockCleared` for each
/// of them in turn.
///
/// # Panics
///
/// A listener asking for something no block clear carries out.
#[tracing::instrument(level = "trace", skip_all)]
fn clear_block<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    side: V::CombatSide,
) {
    let clearing: SmallVec<[CombatId; 8]> = live
        .creatures
        .iter()
        .filter(|c| c.side == side)
        .map(|creature| creature.id)
        .collect();
    let player = live.player(seat).id;
    for &creature in &clearing {
        // `Creature.AfterTurnStart` keeps the player's block on its first
        // turn, before it asks anything. `AfterBlockCleared` still runs.
        if creature == player && live.turn == num_traits::one() {
            continue;
        }
        // `Hook.ShouldClearBlock`, and the model that kept the block.
        let preventer = listen::first_deciding::<V>(
            listen::Walk { live, belts },
            sts2_core::DecisionHook::ShouldClearBlock { creature, player },
            false,
        );
        match preventer {
            None => {
                if let Some(held) = live.creatures.iter_mut().find(|c| c.id == creature) {
                    held.block = num_traits::zero();
                }
            }
            // `Hook.AfterPreventingBlockClear`, asked of that model alone. A
            // power's only flashes.
            Some(sts2_core::ModelId::Relic(relic)) => {
                let block = live
                    .creatures
                    .iter()
                    .find(|c| c.id == creature)
                    .map_or_else(num_traits::zero, |held| held.block);
                let hook = sts2_core::EventHook::BlockClearPrevented { block };
                for effect in listen::ask_relic(seat, belts, relic, hook) {
                    match effect {
                        sts2_core::HookEffect::LoseSomeBlock(amount) => {
                            let ascension = live.ascension;
                            let mut board = Board::for_card(
                                seat,
                                live,
                                rngs.reborrow(),
                                belts,
                                ascension,
                                None,
                                None,
                            );
                            sts2_core::Fight::lose_block(&mut board, creature, amount, None);
                        }
                        effect => {
                            unimplemented!("{relic:?} asking for {effect:?} as it keeps block")
                        }
                    }
                }
            }
            Some(_) => {}
        }
    }
    for creature in clearing {
        let hook = sts2_core::EventHook::BlockCleared {
            creature,
            player,
            turn: live.turn,
        };
        let mut walking = listen::Walking::begin(live, belts, hook);
        while let Some((listener, effect)) = walking
            .as_mut()
            .and_then(|walking| walking.next(seat, live, belts))
        {
            match (listener, effect) {
                // `CreatureCmd.GainBlock`, unpowered, on the player who
                // carries the relic.
                (Listener::Relic { .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                    super::board::gain_unpowered_block::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        live.ascension,
                        player,
                        amount,
                    );
                }
                // `PowerCmd.Apply` on the player who carries the relic, from
                // the player.
                (Listener::Relic { .. }, sts2_core::HookEffect::ApplyToOwner(power, amount)) => {
                    let ascension = live.ascension;
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    board.land(player, power, amount, Some(player));
                }
                // `CreatureCmd.GainBlock`, unpowered, then
                // `PowerCmd.Remove(this)`.
                (
                    Listener::Power { owner, power, .. },
                    sts2_core::HookEffect::GainBlockAndRemove(amount),
                ) => {
                    super::board::gain_unpowered_block::<V>(
                        seat,
                        live,
                        rngs.reborrow(),
                        belts,
                        live.ascension,
                        owner,
                        amount,
                    );
                    live.remove_power(seat, owner, power);
                }
                // `CreatureCmd.GainBlock` of a power's unpowered block on its
                // owner, which runs `Hook.ModifyBlock`.
                (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                    use sts2_core::Fight as _;

                    let ascension = live.ascension;
                    let mut board =
                        Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                    let block = board.resolve(sts2_core::Effect::GainBlock {
                        recipient: owner,
                        amount: sts2_core::Amount::Number(amount),
                        scaling: sts2_core::Scaling::Static,
                    });
                    board.apply(block);
                }
                (_, effect) => {
                    unimplemented!("a listener asking for {effect:?} as block clears")
                }
            }
        }
    }
}

/// One of `AfterTurnEnd`'s passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnEndPass {
    /// `AfterSideTurnEnd`.
    Main,
    /// `AfterSideTurnEndLate`.
    Late,
}

/// `hook`, `AfterSideTurnEnd` or its late pass, raised on every listener once a
/// side's turn ends. Each listener runs under a pause boundary of its own,
/// and acts as the walk reaches it. A fight that ends comes back ended, and
/// the walk goes on past that, carrying out nothing more. With `pausing`, a
/// question a draw raises pauses its listener, and the rest go on first.
///
/// # Panics
///
/// A listener asking for something no turn end carries out, or asking for
/// more behind a draw that paused it.
#[tracing::instrument(level = "trace", skip_all)]
#[allow(clippy::too_many_arguments)]
fn walk_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    skipping: &mut Skips<V>,
    hook: sts2_core::EventHook<V>,
    pausing: bool,
) -> (Option<super::Ended>, super::live::Listening<V>) {
    let mut ended = None;
    let mut listening = super::live::Listening::new();
    let mut walk = listen::Walker::begin(live, belts, hook);
    let mut asked = sts2_core::HookEffects::new();
    while let Some(listener) = walk
        .as_mut()
        .and_then(|walk| walk.next(seat, live, belts, &mut asked))
    {
        let boundary = listening.enter(live, listener);
        let mut at = 0;
        while at < asked.len() {
            let effect = asked[at];
            at += 1;
            // A fight that is ending runs no more commands, though the walk
            // still asks every listener. `CardPileCmd.Draw` returns at once
            // there, and a relic's code after it still runs. `PowerCmd.Remove`
            // checks nothing, so a power still takes itself off.
            if ended.is_some() {
                match (listener, effect) {
                    (Listener::Relic { seat, slot }, sts2_core::HookEffect::Draw(_)) => {
                        let relic = belts[seat][usize::from(slot)].relic;
                        super::relic::own_command_over::<V>(&mut belts[seat], slot, relic);
                    }
                    (
                        Listener::Power {
                            owner,
                            instance,
                            power,
                        },
                        sts2_core::HookEffect::Remove,
                    ) => {
                        if let Some(at) = live.powers.place_of(owner, instance)
                            && let Some(effect) =
                                live.remove_power_at_asking(seat, owner, at).first()
                        {
                            unimplemented!("{power:?} asking for {effect:?} as it comes off")
                        }
                    }
                    _ => {}
                }
                continue;
            }
            let ask = match listener {
                // What a relic asks for, on the player who carries it.
                Listener::Relic { seat, slot } => TurnEndAsk::Relic(seat, slot, effect),
                Listener::Power {
                    owner,
                    instance,
                    power,
                } => match effect {
                    sts2_core::HookEffect::TickDownDuration => {
                        tick_down::<V>(seat, live, belts, skipping, owner, power);
                        continue;
                    }
                    // `PowerCmd.Apply` on the creature carrying the power,
                    // which is what a power that grows its owner asks for.
                    sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                        TurnEndAsk::Apply(owner, power, amount)
                    }
                    sts2_core::HookEffect::DamageOwnerInFight {
                        amount,
                        unblockable,
                        self_dealt,
                    } => TurnEndAsk::Damage {
                        owner,
                        amount,
                        unblockable,
                        dealer: self_dealt.then_some(owner),
                    },
                    // `PowerCmd.Remove(this)`.
                    sts2_core::HookEffect::Remove => {
                        if let Some(at) = live.powers.place_of(owner, instance)
                            && let Some(effect) =
                                live.remove_power_at_asking(seat, owner, at).first()
                        {
                            unimplemented!("{power:?} asking for {effect:?} as it comes off")
                        }
                        continue;
                    }
                    sts2_core::HookEffect::RemoveFromOwner(power) => {
                        live.remove_power(seat, owner, power);
                        continue;
                    }
                    // `CardPileCmd.Draw` for the player, once the hand is
                    // flushed.
                    sts2_core::HookEffect::Draw(amount) => TurnEndAsk::Draw(amount),
                    // `CreatureCmd.Heal` on the creature carrying the power.
                    sts2_core::HookEffect::Heal(amount) => TurnEndAsk::Heal(owner, amount),
                    sts2_core::HookEffect::Escape => TurnEndAsk::Escape(owner),
                    sts2_core::HookEffect::EvokeLast(count) => TurnEndAsk::EvokeLast(count),
                    sts2_core::HookEffect::ClearAffliction(to) => TurnEndAsk::ClearAffliction(to),
                    // `DoomPower.DoomKill`, from the first doomed creature on
                    // the owner's side alone.
                    sts2_core::HookEffect::DoomKill => {
                        let sts2_core::EventHook::SideTurnEnd { side, .. } = hook else {
                            unimplemented!("{power:?} killing the doomed at {hook:?}")
                        };
                        let doomed = doomed::<V>(live, side);
                        if doomed.first() != Some(&owner) {
                            continue;
                        }
                        TurnEndAsk::DoomKill(doomed)
                    }
                    other => unimplemented!("a power asking for {other:?} as a turn ends"),
                },
                listener => unimplemented!("{listener:?} asking for {effect:?} as a turn ends"),
            };
            ended = applied::<V>(
                seat,
                live,
                rngs.reborrow(),
                belts,
                ascension,
                [ask].into_iter().collect(),
                pausing,
            );
            // A draw a pausing walk asked for pauses the listener.
            if pausing && live.asking.is_some() {
                if let Some(effect) = asked.get(at) {
                    unimplemented!(
                        "{listener:?} asking for {effect:?} behind a draw that paused it"
                    );
                }
                break;
            }
        }
        listening.leave(seat, live, boundary);
    }
    (ended, listening)
}

/// `PowerModel.TickDownDuration` of `power` on `owner`: one off it, unless its
/// skip flag is set, which clears instead. At nothing, it comes off.
///
/// # Panics
///
/// A listener changing the tick, or one asking for anything after it.
fn tick_down<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    belts: &mut crate::run::Belts<V>,
    skipping: &mut Skips<V>,
    owner: CombatId,
    power: V::Power,
) {
    // `TickDownDuration` passes over an instance whose flag is set, and
    // clears the flag rather than counting down twice later.
    if let Some(at) = skipping.iter().position(|held| *held == (owner, power)) {
        skipping.remove(at);
        return;
    }
    // `PowerCmd.Decrement`, which is `ModifyAmount` by one down.
    let fight = listen::Walk { live, belts };
    let (_, modifiers) =
        super::board::received_by::<V>(seat, fight, owner, power, -num_traits::one::<V::Number>());
    if !modifiers.is_empty() {
        unimplemented!("a listener changing a {power:?} tick");
    }
    for held in live
        .powers
        .of_mut(owner)
        .filter(|held| held.power() == power)
    {
        held.amount = held.amount.ticked();
    }
    // `AfterPowerAmountChanged` for the one it took.
    let hook = sts2_core::EventHook::PowerAmountChanged {
        creature: owner,
        power,
        delta: -num_traits::one::<V::Number>(),
        applier: None,
    };
    if let Some((listener, effect)) = listen::first_answer::<V>(seat, live, belts, hook) {
        unimplemented!("{listener:?} asking for {effect:?} as {power:?} ticks down");
    }
    // `ShouldRemoveDueToAmount`, and `PowerCmd.Remove`.
    if live
        .powers
        .of(owner)
        .any(|held| held.power() == power && held.amount.spent())
    {
        live.remove_power(seat, owner, power);
    }
}

/// `Hook.AfterTurnEnd`: every `AfterSideTurnEnd`, then every
/// `AfterSideTurnEndLate`, and how the fight ended if either ended it.
///
/// # Panics
///
/// A listener that paused, which nothing waits for on this side yet.
#[tracing::instrument(level = "trace", skip_all)]
fn after_turn_end<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    skipping: &mut Skips<V>,
    side: V::CombatSide,
) -> Option<super::Ended> {
    for pass in [TurnEndPass::Main, TurnEndPass::Late] {
        let (ended, listening) = after_turn_end_pass::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            skipping,
            side,
            pass,
            false,
        );
        listening.none_waits("the enemies' AfterSideTurnEnd");
        if ended.is_some() {
            return ended;
        }
    }
    None
}

/// The pass `pass` of `Hook.AfterTurnEnd` once `side`'s turn ends. With
/// `pausing`, a question a draw raises pauses its listener, and the
/// listeners that paused come back for the caller to wait for.
#[allow(clippy::too_many_arguments)]
pub(super) fn after_turn_end_pass<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    skipping: &mut Skips<V>,
    side: V::CombatSide,
    pass: TurnEndPass,
    pausing: bool,
) -> (Option<super::Ended>, super::live::Listening<V>) {
    let hook = match pass {
        TurnEndPass::Main => sts2_core::EventHook::SideTurnEnd {
            side,
            block: live.player(seat).block,
        },
        TurnEndPass::Late => sts2_core::EventHook::SideTurnEndLate { side },
    };
    walk_turn_end::<V>(seat, live, rngs, belts, ascension, skipping, hook, pausing)
}

/// What a turn end's listeners asked for on the creature carrying them,
/// inline to one for each creature a fight holds.
type Applying<V> = SmallVec<[TurnEndAsk<V>; CREATURES]>;

/// One thing a power asked for as a turn ended, on the creature carrying it.
enum TurnEndAsk<V: Rules> {
    /// `PowerCmd.Apply`.
    Apply(CombatId, V::Power, PowerAmount<V::Number>),
    /// `CreatureCmd.Damage`, unpowered, from `dealer` or from nobody.
    Damage {
        owner: CombatId,
        amount: V::Number,
        unblockable: bool,
        dealer: Option<CombatId>,
    },
    /// `CardPileCmd.Draw`.
    Draw(V::Number),
    /// `CreatureCmd.Heal`.
    Heal(CombatId, V::Number),
    /// `CreatureCmd.Escape`.
    Escape(CombatId),
    /// `OrbCmd.EvokeLast`, this many times.
    EvokeLast(V::Number),
    /// `CardCmd.ClearAffliction` on the fight's cards.
    ClearAffliction(sts2_core::Writing<V>),
    /// `DoomPower.DoomKill` of these creatures.
    DoomKill(SmallVec<[CombatId; CREATURES]>),
    /// What the relic in a slot asked for, carried out for the player.
    Relic(sts2_core::SeatIndex, u8, sts2_core::HookEffect<V>),
}

/// What a turn end's listener asked for, carried out in order, and whatever
/// that ended. A fight that ends runs no more of them. With `pausing`, a
/// question a draw raises stands it still, with the rest deferred.
#[tracing::instrument(level = "trace", skip_all)]
fn applied<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    asked: Applying<V>,
    pausing: bool,
) -> Option<super::Ended> {
    for ask in asked {
        let board = Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
        // A draw, a power's or a relic's, can stop on a shuffle's pick. So
        // can a hit, where a relic answering it draws.
        let mut board = match (pausing, &ask) {
            (
                true,
                TurnEndAsk::Draw(_)
                | TurnEndAsk::Relic(_, _, sts2_core::HookEffect::Draw(_))
                | TurnEndAsk::Damage { .. },
            ) => board.blocking(),
            _ => board,
        };
        match ask {
            TurnEndAsk::Apply(at, power, amount) => {
                sts2_core::Fight::apply_power(&mut board, at, power, amount);
            }
            TurnEndAsk::Damage {
                owner,
                amount,
                unblockable,
                dealer,
            } => board.hit_unpowered(dealer, owner, amount, unblockable),
            TurnEndAsk::Draw(amount) => {
                sts2_core::Fight::draw(&mut board, amount);
            }
            TurnEndAsk::Relic(seat, slot, effect) => board.relic_asked(seat, slot, effect),
            TurnEndAsk::Heal(at, amount) => sts2_core::Fight::heal(&mut board, at, amount),
            TurnEndAsk::Escape(at) => board.escape_creature(at),
            TurnEndAsk::EvokeLast(count) => {
                for _ in 0..num_traits::cast::<_, usize>(count).unwrap_or(0) {
                    board.evoke_last();
                }
            }
            TurnEndAsk::ClearAffliction(to) => {
                board.write_cards([sts2_core::HookEffect::ClearAffliction(to)]);
            }
            TurnEndAsk::DoomKill(doomed) => board.doom_kill(&doomed),
        }
        if let Some(ended) = board.ended() {
            return Some(ended);
        }
    }
    None
}

/// A monster's move, as what goes on from where it stood still reads it.
/// `damage` and `repeats` are what the move advertised, which its attacks
/// read.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Moving<V: Rules> {
    actor: CombatId,
    monster: V::Monster,
    move_id: V::MoveId,
    damage: V::Number,
    repeats: V::Count,
}

/// A monster's move that stood still on what the player has to answer.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Stood<V: Rules> {
    moving: Moving<V>,
    on: StoodOn<V>,
}

/// What a monster's move stood still on.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum StoodOn<V: Rules> {
    /// The choice it offered, its last command, which goes on from `.0`
    /// with the player's answer.
    Choice(super::board::Resume<V>),
    /// A question one of its commands asked. It goes on from `.0` once
    /// that is over, and is over where there is none.
    Question(Option<super::board::Resume<V>>),
}

/// How a monster's move came out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Moved<V: Rules> {
    /// Over, with `OnMovePerformed` run, and how it ended the fight.
    Over(Option<super::Ended>),
    /// Standing still, with the question in [`Live::asking`].
    Stood(Stood<V>),
}

/// `ExecuteEnemyTurn` from the enemy at `next` in `acting`, which moves on
/// past each that acts. It runs until a move stands still, or until every
/// enemy has acted or one ended the fight.
pub(super) fn enemies_act<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    skipping: &mut Skips<V>,
    acting: &[CombatId],
    next: &mut usize,
) -> Moved<V> {
    while let Some(&id) = acting.get(*next) {
        *next += 1;
        match perform_move::<V>(live, rngs.reborrow(), belts, skipping, id) {
            Moved::Over(None) => {}
            moved => return moved,
        }
    }
    Moved::Over(None)
}

/// `Creature.TakeTurn` for the enemy `id`: it performs the move it
/// advertised. One the fight took out, or one that advertised nothing,
/// does nothing.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn perform_move<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut crate::run::Belts<V>,
    skipping: &mut Skips<V>,
    id: CombatId,
) -> Moved<V> {
    let Some(creature) = live.creatures.iter().find(|c| c.id == id) else {
        return Moved::Over(None);
    };
    let next_move = creature.next_move;
    let held = live
        .machines
        .iter_mut()
        .find(|held| held.creature == id)
        .unwrap_or_else(|| unimplemented!("an enemy with no move graph"));
    // `new MoveState()`, which a monster that joined on its own side's
    // turn still advertises. It does nothing, and the graph still stands
    // on the state it opened on.
    let Some(next_move) = next_move else {
        return Moved::Over(None);
    };
    let monster = held.monster;
    // `OnMovePerformed`, which is what lets the graph leave this move. It
    // happens beside `PerformMove` rather than after what the move does.
    held.machine.performed();

    let attack = next_move.attack().map(Landing::attack::<V>);
    let damage = attack.map_or_else(num_traits::zero, |attack| attack.damage);
    let repeats = attack.map_or_else(num_traits::zero, |attack| attack.repeats);

    let ascension = live.ascension;
    let mut board = Board::for_move(live, rngs, belts, ascension, (id, damage, repeats));
    let played = V::perform_move::<super::board::BoardKind<V>>(monster, next_move.id, &mut board);
    let ended = board.ended();
    super::play::merge::<V>(skipping, std::mem::take(&mut board.skipped));
    // An intent that works its hit count or damage out from the model
    // shows what the move left, until the next move is drawn. Only a
    // fight that ends here shows it before then.
    if ended.is_some()
        && let Some(held) = live.machines.iter().find(|held| held.creature == id)
    {
        let repeats = V::intent_repeats_now(monster, next_move.id, &held.state);
        let damage = V::intent_damage_now(monster, next_move.id, ascension, &held.state);
        if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == id)
            && let Some(telegraphed) = creature.next_move.as_mut()
            && telegraphed.id == next_move.id
        {
            if let Some(repeats) = repeats {
                telegraphed.reread_repeats(repeats);
            }
            if let Some(damage) = damage {
                telegraphed.reread_damage(damage);
            }
        }
    }
    let moving = Moving {
        actor: id,
        monster,
        move_id: next_move.id,
        damage,
        repeats,
    };
    let on = match played {
        sts2_core::Played::Done if live.asking.is_none() => {
            move_performed::<V>(live, id, next_move.id);
            return Moved::Over(ended);
        }
        // Its last command, an attack, asked: the move is over once the
        // question is. Only an attack can ask, and the board stops any
        // command that runs on past one that did.
        sts2_core::Played::Done => StoodOn::Question(None),
        sts2_core::Played::Choosing {
            offered,
            skippable,
            at,
        } => {
            if ended.is_some() {
                unimplemented!("a move asking in a fight it ended");
            }
            // The choice is the move's last command, so the move is over
            // once it is answered.
            live.asking = Some(super::live::Asking {
                then: Then::Turn,
                question: super::live::Asked::Offered { offered, skippable },
                signalled: false,
            });
            StoodOn::Choice(at)
        }
        sts2_core::Played::Asking { .. } | sts2_core::Played::AskingAmong { .. } => {
            unimplemented!("a monster's move selecting from a pile")
        }
        // Its attack asked, and the move stands still on it with the rest of
        // the turn behind it.
        sts2_core::Played::Waiting { at } => {
            if live.asking.is_none() {
                unimplemented!("a move waiting on nothing asked");
            }
            StoodOn::Question(Some(at))
        }
    };
    Moved::Stood(Stood { moving, on })
}

/// `MonsterMoveState.OnPerform`, which the move state gives the powers on
/// the creature that performed it.
///
/// # Panics
///
/// A power asking for something once the move is performed, which nothing
/// here carries out.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn move_performed<V: Rules>(live: &mut Live<V>, creature: CombatId, move_id: V::MoveId) {
    let hook = sts2_core::EventHook::MonsterMovePerformed { move_id };
    let mut asked = sts2_core::HookEffects::new();
    // No belt is lent here: a monster's move reads no relic.
    for at in 0..live.powers.of(creature).count() {
        live.ask_power(None, creature, at, hook, &mut asked);
    }
    if let Some(effect) = asked.first() {
        unimplemented!("a power asking for {effect:?} after a monster move")
    }
}

/// `PrepareForNextTurn`: every enemy still in the fight advertises its next
/// move.
#[tracing::instrument(level = "trace", skip_all)]
fn roll_moves<V: Rules>(
    live: &mut Live<V>,
    belts: &crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    ai: &mut MegaRng,
) -> Result<(), Error> {
    live.machines
        .retain(|held| live.creatures.iter().any(|c| c.id == held.creature));
    roll_moves_of::<V>(live, belts, ascension, ai, |_| true)
}

/// `Encounter.GetNextSlot` names a slot: one of `Encounter.Slots` no enemy in
/// the fight holds.
pub(super) fn next_slot<V: Rules>(live: &Live<V>) -> bool {
    use sts2_core::EncounterTable as _;

    (0..live.encounter.slots()).any(|slot| {
        !live.machines.iter().any(|held| {
            held.slot == Some(slot) && live.creatures.iter().any(|c| c.id == held.creature)
        })
    })
}

/// `MonsterModel.RollMove` for each enemy `which` picks, in fight order.
pub(super) fn roll_moves_of<V: Rules>(
    live: &mut Live<V>,
    belts: &crate::run::Belts<V>,
    ascension: V::AscensionLevel,
    ai: &mut MegaRng,
    which: impl Fn(CombatId) -> bool,
) -> Result<(), Error> {
    // `GetTeammatesOf(Creature).Count(IsAlive)`. Every creature with a move
    // graph is an enemy.
    let standing: u8 = live
        .creatures
        .iter()
        .filter(|c| c.side == <V::CombatSide as SideKinds>::ENEMY && c.alive)
        .count()
        .try_into()
        .expect("a side's creatures fit a byte");
    let next_slot = next_slot(live);
    for held in live.machines.iter_mut() {
        if !which(held.creature) {
            continue;
        }
        let rolled = {
            // Field by field, because the machines are borrowed.
            let fight = super::live::Onlookers {
                creatures: &live.creatures,
                powers: &live.powers,
                kept: &live.kept,
                belts: Some(belts),
                attack: &live.attack_results,
            };
            let creature = live
                .creatures
                .iter()
                .find(|creature| creature.id == held.creature)
                .expect("a retained move machine has its creature");
            let powers = live.powers.of(creature.id).map(|power| {
                // Through the field, because the machine is borrowed.
                power.listener(creature.carrier(), live.history.view(), &fight)
            });
            V::roll_monster_move(
                held.monster,
                ascension,
                sts2_core::MonsterSpawn {
                    standing,
                    hp: num_traits::cast(creature.hp).expect("a monster's health fits a u32"),
                    max_hp: num_traits::cast(creature.max_hp)
                        .expect("a monster's health fits a u32"),
                    next_slot,
                    ..held.spawn
                },
                powers,
                // `GetTeammatesOf(Creature)` less itself. Every creature with
                // a move graph is an enemy.
                |wanted| {
                    live.creatures.iter().any(|other| {
                        other.id != creature.id
                            && other.side == <V::CombatSide as SideKinds>::ENEMY
                            && other
                                .next_move
                                .as_ref()
                                .is_some_and(|next| next.id == wanted)
                    })
                },
                &held.state,
                &mut held.machine,
                ai,
            )?
        };
        // `None` is a monster still standing on a state something built for
        // it, which goes on advertising what that caller set.
        let Some(rolled) = rolled else { continue };
        if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == held.creature) {
            creature.next_move = Some((&rolled).into());
        }
    }
    Ok(())
}
