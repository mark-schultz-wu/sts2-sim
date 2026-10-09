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
use super::live::{CREATURES, Face, Landing, Live, Setup, Skips, Then};
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
pub(crate) fn end<V: Rules>(
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live: next,
        streams,
        relics,
    } = fighting;
    let mut skipping = std::mem::take(&mut next.skip_tick);

    // `EndPlayerTurnPhaseOne`: the turn-end hooks, and the win check they can
    // end the fight for, come before the orbs and the hand.
    if let Some(ended) = before_turn_end::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
    ) {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }
    // `DoTurnEnd` returns once the fight is over, and a fight an orb ended
    // ends the turn with it.
    let ended = turn_end(next, streams.combat(), relics, ascension);
    // What `DoTurnEnd` put on the player skips this turn's tick too.
    let landed = std::mem::take(&mut next.skip_tick);
    super::play::merge::<V>(&mut skipping, landed.iter().copied().collect());
    if let Some(ended) = ended {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }
    flush_hand(next, streams.combat(), relics, ascension);
    end_of_turn_cleanup(next);
    // `AfterTurnEnd` for the side that just ended.
    if let Some(ended) = after_turn_end::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        &mut skipping,
        <V::CombatSide as SideKinds>::PLAYER,
    ) {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }

    // `SwitchFromPlayerToEnemySide` asks `ShouldTakeExtraTurn` first. A yes
    // keeps the side with the player, whose turn counts up while the round
    // does not. `AfterTakingExtraTurn` follows, then the player's turn starts.
    let extra = listen::heard::<V>(next, relics, sts2_core::ModelHook::ShouldTakeExtraTurn) && {
        let plays = next.history.unprompted_plays_this_turn();
        listen::any_decides::<V>(
            listen::Walk { live: next, relics },
            sts2_core::DecisionHook::ShouldTakeExtraTurn { plays },
            true,
        )
    };
    if extra {
        next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::NONE;
        next.switch_side(<V::CombatSide as SideKinds>::PLAYER);
        next.turn = next.turn + num_traits::one();
        let taking = sts2_core::EventHook::TakingExtraTurn;
        if let Some((listener, effect)) = listen::notify::<V>(next, relics, taking).first() {
            unimplemented!("{listener:?} asking for {effect:?} as an extra turn is taken");
        }
        return start_player_turn(
            Fighting {
                live: next,
                streams,
                relics,
            },
            ascension,
            max_energy,
            skipping,
            true,
        );
    }

    // `SwitchSides` to the enemy, then `StartTurn` for it: block clears, and
    // every enemy still in the fight performs the move it advertised.
    //
    // `StartTurn` opens with `SetPhaseForAllPlayers(None)` and puts the phase
    // back only for a turn the player is taking. So the phase a fight ending
    // here reports is the one the enemy turn ran under.
    next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::NONE;
    next.switch_side(<V::CombatSide as SideKinds>::ENEMY);
    clear_block::<V>(
        next,
        streams.combat(),
        relics,
        <V::CombatSide as SideKinds>::ENEMY,
    );
    // `AfterSideTurnStart` for the enemy, which the block clear comes before
    // and the moves come after. A power counting the cards played against its
    // owner forgets the count here.
    if let Some(ended) = side_turn_started::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::ENEMY,
    ) {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }
    let acting: SmallVec<[CombatId; CREATURES]> = next
        .creatures
        .iter()
        .filter(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY)
        .map(|creature| creature.id)
        .collect();
    enemies_act(
        Fighting {
            live: next,
            streams,
            relics,
        },
        ascension,
        max_energy,
        skipping,
        &acting,
    )
}

/// Answer the choice a monster's move offered, `IChoosable.OnChosen`. What
/// the player chose acts from the player.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn resume_chosen<V: Rules>(
    fighting: Fighting<'_, V>,
    at: V::Resume,
    taken: sts2_core::Taken<V>,
) -> Option<super::Ended> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    let ascension = live.ascension;
    let mut board = Board::for_card(live, streams.combat(), relics, ascension, None, None);
    let played = V::resume_play(&mut board, at, taken);
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);
    super::play::merge::<V>(&mut live.skip_tick, skipped);
    if played != sts2_core::Played::Done {
        unimplemented!("a monster's move asking a second question");
    }
    ended
}

/// The rest of a move that stood still on a command the player had to
/// answer. It goes on from `at`, on a board built as the move's was. `None`
/// where the move stood still again, which [`Live::deferred`] now holds.
pub(super) fn resume_move<V: Rules>(
    fighting: Fighting<'_, V>,
    parked: &super::live::Deferred<V>,
) -> Option<Option<super::Ended>> {
    let super::live::Deferred::Move {
        actor,
        monster,
        move_id,
        at,
        damage,
        repeats,
        ref acting,
        max_energy,
    } = *parked
    else {
        unreachable!("only a move resumes as one")
    };
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    let Some(at) = at else {
        return Some(None);
    };
    // A monster the fight took out took its model and its move with it.
    if !live.machines.iter().any(|held| held.creature == actor) {
        return Some(None);
    }
    let ascension = live.ascension;
    let mut board = Board::for_move(
        live,
        streams.combat(),
        relics,
        ascension,
        (actor, damage, repeats),
    );
    let played = V::resume_move(monster, move_id, at, &mut board);
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);
    super::play::merge::<V>(&mut live.skip_tick, skipped);
    let again = match played {
        sts2_core::Played::Done if live.asking.is_none() => return Some(ended),
        sts2_core::Played::Done => None,
        sts2_core::Played::Waiting { at } if live.asking.is_some() => Some(at),
        played => unimplemented!("a resumed move handing back {played:?}"),
    };
    live.defer(super::live::Deferred::Move {
        actor,
        monster,
        move_id,
        at: again,
        damage,
        repeats,
        acting: acting.clone(),
        max_energy,
    });
    None
}

/// The enemy turn once the move `move_id` of `actor` is over:
/// `OnMovePerformed`, then the enemies in `acting` unless the fight ended.
pub(super) fn after_move<V: Rules>(
    fighting: Fighting<'_, V>,
    actor: CombatId,
    move_id: V::MoveId,
    acting: &[CombatId],
    max_energy: V::Count,
    ended: Option<super::Ended>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    let skipping = std::mem::take(&mut live.skip_tick);
    let ascension = live.ascension;
    move_performed::<V>(live, actor, move_id);
    if let Some(ended) = ended {
        live.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(live, relics);
        }
        return Ok(Some(ended));
    }
    enemies_act(
        Fighting {
            live,
            streams,
            relics,
        },
        ascension,
        max_energy,
        skipping,
        acting,
    )
}

/// The enemies in `acting` perform their moves in order, and the round ends.
#[tracing::instrument(level = "trace", skip_all)]
fn enemies_act<V: Rules>(
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
    mut skipping: Skips<V>,
    acting: &[CombatId],
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live: next,
        streams,
        relics,
    } = fighting;
    let ended = match enemy_turn(
        next,
        streams.combat(),
        relics,
        &mut skipping,
        acting,
        max_energy,
    ) {
        Acted::Over(ended) => ended,
        Acted::Deferred => {
            next.skip_tick = skipping;
            return Ok(None);
        }
    };
    // `EndEnemyTurn` is gated on `IsInProgress`, which both endings clear:
    // `ProcessPendingLoss` for a death and `EndCombatInternal` for a win. So
    // a fight that ended during the enemy turn ends the turn with it, and
    // nothing below here runs.
    if let Some(ended) = ended {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }
    // `EndEnemyTurnInternal`.
    let ended = match before_turn_end::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::ENEMY,
    ) {
        Some(ended) => Some(ended),
        None => after_turn_end::<V>(
            next,
            streams.combat(),
            relics,
            ascension,
            &mut skipping,
            <V::CombatSide as SideKinds>::ENEMY,
        ),
    };
    if let Some(ended) = ended {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
    }

    // `SwitchSides` back. The round and the turn both count up, because this
    // is not an extra turn. A fight that ended above switches too: its win
    // check clears `IsInProgress`, which `IsEnding` then reads as not
    // ending.
    next.switch_side(<V::CombatSide as SideKinds>::PLAYER);
    next.round = next.round + num_traits::one();
    next.turn = next.turn + num_traits::one();
    if let Some(ended) = ended {
        return Ok(Some(ended));
    }

    start_player_turn(
        Fighting {
            live: next,
            streams,
            relics,
        },
        ascension,
        max_energy,
        skipping,
        false,
    )
}

/// `StartTurn` for the player's side, from `BeforeSideTurnStart` to the play
/// phase. An `extra` turn is one `ShouldTakeExtraTurn` gave, which rolls no
/// enemy move.
#[tracing::instrument(level = "trace", skip_all)]
fn start_player_turn<V: Rules>(
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    max_energy: V::Count,
    skipping: Skips<V>,
    extra: bool,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live: next,
        streams,
        relics,
    } = fighting;
    // `StartTurn` opens on `BeforeSideTurnStart`, which is where a relic
    // counting a turn's plays moves the count on for the draw to read.
    before_side_turn_start::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
        next.turn,
    );

    next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::START;

    // `PrepareForNextTurn`, before the player draws anything. An extra turn
    // leaves the enemies' moves as they were.
    if !extra {
        roll_moves(next, relics, ascension, streams.combat().ai)?;
    }

    // `AfterTurnStart` on the player, then `SetupPlayerTurn`.
    clear_block::<V>(
        next,
        streams.combat(),
        relics,
        <V::CombatSide as SideKinds>::PLAYER,
    );
    // `ResetEnergy`, to `MaxEnergy`, which runs `Hook.ModifyMaxEnergy`. A
    // player `Hook.ShouldPlayerResetEnergy` keeps from it adds the maximum
    // to what they have instead.
    let player = next.player().id;
    let kept = listen::any_decides::<V>(
        listen::Walk { live: next, relics },
        sts2_core::DecisionHook::ShouldPlayerResetEnergy {
            player,
            turn: next.turn,
        },
        false,
    );
    let max = super::board::max_energy::<V>(listen::Walk { live: next, relics }, max_energy);
    next.energy = if kept { next.energy + max } else { max };
    // An evoke that ends the fight ends `SetupPlayerTurn` with it, as a
    // relic's damage before the hand does below.
    if let Some(ended) = energy_reset::<V>(next, streams, relics, ascension) {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }
    energy_reset_late::<V>(next, streams.combat(), relics, ascension);
    let player = next.player().id;
    let hook = sts2_core::EventHook::BeforeHandDraw {
        player,
        turn: next.turn,
    };
    let asked = listen::notify::<V>(next, relics, hook);
    // A setup that stops on a question lets the turn go on around it:
    // `StartTurn` waits for it to finish or to pause. A relic's damage can
    // end the fight, and a fight ended in `SetupPlayerTurn` starts no side
    // turn after it.
    let parked = match carry_setup(
        next,
        streams.combat(),
        relics,
        ascension,
        Setup::BeforeHandDraw(asked),
    ) {
        Carried::Done(Some(ended)) => {
            next.skip_tick = skipping;
            if ended == super::Ended::Won {
                listen::combat_ended::<V>(next, relics);
            }
            return Ok(Some(ended));
        }
        Carried::Done(None) => false,
        Carried::Parked => true,
    };
    if let Some(ended) = side_turn_started::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
    ) {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }
    // `OrbQueue.AfterTurnStart`, once the side's turn has started.
    if let Some(ended) = orb_triggers::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        OrbMoment::TurnStart,
    ) {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }

    // `RunAutoPrePlayPhase` waits for the setup, so a parked one keeps the
    // turn in its start until the answer.
    if !parked {
        if let Some(ended) = pre_play_phase::<V>(next, streams.combat(), relics) {
            next.skip_tick = skipping;
            if ended == super::Ended::Won {
                listen::combat_ended::<V>(next, relics);
            }
            return Ok(Some(ended));
        }
        next.phase = after_pre_play::<V>(next);
    }

    next.skip_tick = skipping;
    Ok(None)
}

/// `DoTurnEnd`: the orbs, then the cards in hand that answer to a turn
/// ending, and how the fight ended if an orb ended it.
///
/// Every orb triggers first. The game then looks through the hand once. An
/// ethereal card is exhausted, and a card with a turn-end effect of its own
/// runs it after every exhaust. No card is both.
#[tracing::instrument(level = "trace", skip_all)]
fn turn_end<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
) -> Option<super::Ended> {
    if let Some(ended) =
        orb_triggers::<V>(live, rngs.reborrow(), relics, ascension, OrbMoment::TurnEnd)
    {
        return Some(ended);
    }
    let held: SmallVec<[_; MAX_CARDS_IN_HAND]> = live.cards.iter(Pile::Hand).collect();
    // In hand order, each found where the exhausts before it left it.
    for ethereal in held.iter().filter(|held| held.face.ethereal()) {
        let at = live
            .cards
            .find(Pile::Hand, ethereal.id)
            .expect("an Ethereal card stays in hand until it exhausts");
        // `CardCmd.Exhaust`, which ethereal causes.
        let card = live
            .cards
            .get(Pile::Hand, at)
            .expect("an Ethereal card stands where it was found");
        live.cards.move_to(Pile::Hand, at, Pile::Exhaust);
        if let Some(ended) = exhausted::<V>(live, rngs.reborrow(), relics, &card, true, false) {
            return Some(ended);
        }
    }
    for card in held.iter().filter(|card| !card.face.ethereal()) {
        let mut board = Board::for_card(
            live,
            rngs.reborrow(),
            relics,
            ascension,
            None,
            Some(card.id),
        );
        V::on_card_turn_end(card.face.ident.card, &mut board);
        let skipped = std::mem::take(&mut board.skipped);
        let ended = board.ended();
        super::play::merge::<V>(&mut live.skip_tick, skipped);
        if ended.is_some() {
            return ended;
        }
    }
    None
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
///
/// # Panics
///
/// A listener that changes how often an orb triggers.
pub(super) fn orb_triggers<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    at: OrbMoment,
) -> Option<super::Ended> {
    let held: Vec<sts2_core::Channelled<V>> = match &live.orbs {
        Some(orbs) if !orbs.queue.is_empty() => orbs.queue.clone(),
        _ => return None,
    };
    for (place, orb) in held.into_iter().enumerate() {
        // `Hook.ModifyOrbPassiveTriggerCount` from one. Its
        // `AfterModifyingOrbPassiveTriggerCount` only flashes the one relic
        // that answers.
        let triggers = super::board::modified_by::<V>(
            super::listen::Walk { live, relics },
            num_traits::one(),
            &[sts2_core::ModifyHook::OrbPassiveTriggerCount { first: place == 0 }],
        );
        let mut orb = orb;
        for _ in 0..num_traits::ToPrimitive::to_usize(&triggers).unwrap_or(0) {
            let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
            match at {
                OrbMoment::TurnEnd => V::orb_turn_end(&mut orb, &mut board),
                OrbMoment::TurnStart => V::orb_turn_start(&mut orb, &mut board),
            }
            let ended = board.ended();
            keep_orb(live, place, orb);
            if let Some(ended) = ended {
                return Some(ended);
            }
        }
    }
    None
}

/// The numbers an orb's trigger moved, kept on the orb at `place` while that
/// orb still stands there.
fn keep_orb<V: Rules>(live: &mut Live<V>, place: usize, orb: sts2_core::Channelled<V>) {
    if let Some(held) = live
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
fn end_of_turn_cleanup<V: Rules>(live: &mut Live<V>) {
    live.cards
        .write_all_cards(Face::lasts_the_turn, Face::turn_ended);
    // `CardModel.EndOfTurnCleanup` clears `ExhaustOnNextPlay`.
    live.exhaust_next.clear();
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
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    side: V::CombatSide,
    turn: V::Count,
) {
    // `Creature.BeforeTurnStart`, which writes every power's stack as its
    // side begins.
    let starting: Vec<CombatId> = live
        .creatures
        .iter()
        .filter(|creature| creature.side == side)
        .map(|creature| creature.id)
        .collect();
    for creature in starting {
        for held in live.powers.of_mut(creature) {
            held.on_turn_start = held.amount;
        }
    }
    let hook = sts2_core::EventHook::BeforeSideTurnStart { side, turn };
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                live.gain_unpowered_block(owner, amount);
            }
            (_, sts2_core::HookEffect::AttacksFromDiscard(count)) => {
                attacks_from_discard::<V>(live, rngs.reborrow(), count)
            }
            // A relic's owner is the player, whose orbs these are.
            (Listener::Relic { .. }, sts2_core::HookEffect::Channel(orb)) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::channel(&mut board, orb);
            }
            // `PowerCmd.Apply<P>(HittableEnemies, amount, Owner)`, from the
            // player who carries the relic.
            (Listener::Relic { .. }, sts2_core::HookEffect::ApplyToOpponents(power, amount)) => {
                let player = live.player().id;
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                for target in board.hittable_opponents() {
                    board.land(target, power, amount, Some(player));
                }
            }
            (Listener::Relic { .. }, sts2_core::HookEffect::ZeroCostFromDraw(count)) => {
                use sts2_core::Fight as _;

                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
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
    }
}

/// `AggressionPower.BeforeSideTurnStart`: the discard pile's attacks, in an
/// unstable shuffle off `CombatCardSelection`, and the first `count` of them
/// into hand. Once the hand is full, each goes back onto the discard pile.
/// Each is upgraded where an upgrade can still raise it.
fn attacks_from_discard<V: Rules>(live: &mut Live<V>, rngs: CombatRngs<'_>, count: V::Number) {
    use sts2_core::rng::GameRng as _;

    let count: usize = num_traits::cast(count)
        .unwrap_or_else(|| unimplemented!("{count:?} attacks from the discard pile"));
    let mut attacks: Vec<sts2_core::CardId<V>> = live
        .cards
        .iter(Pile::Discard)
        .filter(|held| V::card_kind(held.face.ident.card) == Some(sts2_core::CardKind::Attack))
        .map(|held| held.id)
        .collect();
    rngs.selection.unstable_shuffle(&mut attacks);
    for id in attacks.into_iter().take(count) {
        // A full hand sends the card to the discard pile instead, onto its
        // end, and it is upgraded there all the same.
        let to = match live.cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
            true => Pile::Discard,
            false => Pile::Hand,
        };
        let at = live
            .cards
            .find(Pile::Discard, id)
            .expect("a shuffled attack is still in the discard pile");
        live.cards.move_to(Pile::Discard, at, to);
        let at = live.cards.len(to) - 1;
        live.cards.write(to, at, |held| {
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
fn before_turn_end<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    side: V::CombatSide,
) -> Option<super::Ended> {
    let block = live.player().block;
    let hook = sts2_core::EventHook::BeforeSideTurnEndVeryEarly { side, block };
    if let Some((listener, effect)) = listen::notify::<V>(live, relics, hook).first() {
        unimplemented!("{listener:?} asking for {effect:?} as a turn is about to end");
    }
    // Counted only where something hears the hook, as nothing else reads it.
    let plays = match listen::heard::<V>(live, relics, sts2_core::ModelHook::BeforeSideTurnEndEarly)
    {
        true => live.history.unprompted_plays_this_turn(),
        false => 0,
    };
    let hook = sts2_core::EventHook::BeforeSideTurnEndEarly { side, plays };
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                live.gain_unpowered_block(owner, amount);
            }
            // `CardCmd.Exhaust` of each card the hand holds as it starts.
            (Listener::Relic { .. }, sts2_core::HookEffect::ExhaustHand) => {
                let hand: SmallVec<[sts2_core::CardAt<V>; 10]> = (live.cards.iter(Pile::Hand))
                    .enumerate()
                    .map(|(at, card)| {
                        let index = num_traits::NumCast::from(at)
                            .unwrap_or_else(|| unimplemented!("a hand past what a fight numbers"));
                        sts2_core::CardAt::new(card.id, Pile::Hand, index)
                    })
                    .collect();
                let (_, ended) = act_on::<V>(
                    live,
                    rngs.reborrow(),
                    relics,
                    sts2_core::PileAct::Exhaust,
                    &hand,
                    false,
                );
                if ended.is_some() {
                    return ended;
                }
            }
            (_, effect) => unimplemented!("a listener asking for {effect:?} before a turn ends"),
        }
    }
    let hook = sts2_core::EventHook::BeforeSideTurnEnd {
        side,
        energy: live.energy,
        hand: live.cards.len(Pile::Hand),
    };
    let mut ended = None;
    // The instances taken off so far, which moves the place of each one
    // after them on the same creature.
    let mut removed: SmallVec<[(CombatId, u8); 2]> = SmallVec::new();
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            // `CreatureCmd.Damage` over `HittableEnemies`, unpowered, from the
            // power's owner.
            (Listener::Power { owner, .. }, sts2_core::HookEffect::DamageOpponents(amount)) => {
                if ended.is_some() {
                    continue;
                }
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                for target in board.hittable_opponents_of(owner) {
                    board.hit_unpowered(Some(owner), target, amount, false);
                }
                ended = board.ended();
            }
            // `PowerCmd.Remove(this)`, on the instance that asked.
            (Listener::Power { owner, at, power }, sts2_core::HookEffect::Remove) => {
                let before = removed
                    .iter()
                    .filter(|&&(creature, gone)| creature == owner && gone < at)
                    .count();
                let place = usize::from(at) - before;
                if let Some(effect) = live.remove_power_at_asking(owner, place).first() {
                    unimplemented!("{power:?} asking for {effect:?} as it comes off")
                }
                removed.push((owner, at));
            }
            (Listener::Card { id }, sts2_core::HookEffect::NoteOnSelf(note)) => {
                live.note(id, note);
            }
            (Listener::Power { owner, .. }, sts2_core::HookEffect::DoomKill) => {
                // `IsOverOrEnding`, and the owner dead already.
                if ended.is_some() {
                    continue;
                }
                let doomed = doomed::<V>(live, side);
                // Only the first doomed creature's own Doom kills, and it
                // kills them all.
                if doomed.first() != Some(&owner) {
                    continue;
                }
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                board.doom_kill(&doomed);
                ended = board.ended();
            }
            (
                Listener::Power { owner, .. },
                sts2_core::HookEffect::DamageOpponentsHolding { orb, amount },
            ) => {
                let holding = live
                    .orbs
                    .iter()
                    .flat_map(|orbs| &orbs.queue)
                    .any(|held| held.orb == orb);
                if !holding || ended.is_some() {
                    continue;
                }
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                for target in sts2_core::Fight::opponents(&board) {
                    let hit = sts2_core::Fight::resolve(
                        &board,
                        sts2_core::Effect::Attack {
                            dealer: owner,
                            target,
                            damage: sts2_core::Amount::Number(amount),
                            scaling: sts2_core::Scaling::Static,
                        },
                    );
                    sts2_core::Fight::apply(&mut board, hit);
                }
                ended = board.ended();
            }
            // `CreatureCmd.GainBlock`, unpowered, on the player who carries
            // the relic.
            (Listener::Relic { .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                let player = live.player().id;
                live.gain_unpowered_block(player, amount);
            }
            // `PowerCmd.Apply` on the player who carries the relic.
            (Listener::Relic { .. }, sts2_core::HookEffect::ApplyToOwner(power, amount)) => {
                let player = live.player().id;
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                board.land(player, power, amount, Some(player));
                ended = ended.or(board.ended());
            }
            // `CreatureCmd.Damage` over `HittableEnemies`, from the player who
            // carries the relic.
            (Listener::Relic { .. }, effect @ sts2_core::HookEffect::DamageOpponents(_)) => {
                if ended.is_some() {
                    continue;
                }
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                super::hook::apply(&mut board, [effect]);
                ended = board.ended();
            }
            // Written on the fight's cards.
            (_, effect @ sts2_core::HookEffect::ClearAffliction(_)) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                board.write_cards([effect]);
            }
            (_, effect) => unimplemented!("a listener asking for {effect:?} before a turn ends"),
        }
    }
    ended
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
                    let hp: super::live::Amount = num_traits::cast(creature.hp)
                        .expect("a creature's hit points fit a power's amount");
                    hp <= amount
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

/// Stop `SetupPlayerTurn` on `question`, which picks cards to `act` on, with
/// the rest of the setup from `stage` waiting behind it.
fn park_setup<V: Rules>(
    live: &mut Live<V>,
    act: sts2_core::PileAct,
    stage: Setup<V>,
    question: super::live::Asked<V>,
) -> Carried {
    live.asking = Some(super::live::Asking {
        then: Then::Setup { act },
        question,
    });
    live.defer(super::live::Deferred::Setup(stage));
    Carried::Parked
}

/// Carry `SetupPlayerTurn` on from `stage` to its end: what the hook it
/// stands in still has to carry out, then everything after that hook.
///
/// A selection that is not taken without asking parks the setup, and the
/// answer comes back to [`resume_setup`].
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn carry_setup<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    mut stage: Setup<V>,
) -> Carried {
    let player = live.player().id;
    loop {
        stage = match stage {
            // `Hook.BeforeHandDraw`, once the energy is reset and before the
            // turn's hand is drawn.
            Setup::BeforeHandDraw(mut rest) => {
                while !rest.is_empty() {
                    let (listener, effect) = rest.remove(0);
                    match effect {
                        sts2_core::HookEffect::ChooseColorlessIntoHand => {
                            let mut pool = V::combat_colorless_cards().to_vec();
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            let stream = <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION;
                            let offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] =
                                sts2_core::Fight::generate_cards(&mut board, &mut pool, stream)
                                    .unwrap_or_else(|_| {
                                        unimplemented!("a colorless pool offering fewer than three")
                                    });
                            live.asking = Some(super::live::Asking {
                                then: Then::SetupChosen { writes: &[] },
                                // `FromChooseACardScreen` hands its selector
                                // a minimum of none, whatever `canSkip` says.
                                question: super::live::Asked::Offered {
                                    offered: offered.into_iter().collect(),
                                    skippable: true,
                                },
                            });
                            live.defer(super::live::Deferred::Setup(Setup::BeforeHandDraw(rest)));
                            return Carried::Parked;
                        }
                        sts2_core::HookEffect::GenerateInHand { card, count } => {
                            let count = num_traits::cast(count).unwrap_or_else(|| {
                                unimplemented!("making {count:?} cards in hand")
                            });
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
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
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                filter,
                                count,
                                writes,
                            );
                        }
                        sts2_core::HookEffect::GenerateDistinctColorless { count } => {
                            generate_distinct_colorless::<V>(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                count,
                            );
                        }
                        sts2_core::HookEffect::ExhaustFromDrawTop { passing_innate } => {
                            let passed = passing_innate
                                .then(|| {
                                    live.cards
                                        .iter(Pile::Draw)
                                        .position(|card| !card.face.innate())
                                })
                                .flatten();
                            let top =
                                passed.or_else(|| (!live.cards.is_empty(Pile::Draw)).then_some(0));
                            if let Some(at) = top {
                                let card = live
                                    .cards
                                    .get(Pile::Draw, at)
                                    .expect("a card stands where the walk found it");
                                let index = num_traits::NumCast::from(at).unwrap_or_else(|| {
                                    unimplemented!("a pile deeper than a fight can number")
                                });
                                let mut board = Board::for_card(
                                    live,
                                    rngs.reborrow(),
                                    relics,
                                    ascension,
                                    None,
                                    None,
                                );
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
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            sts2_core::Fight::apply_to_actor(&mut board, power, amount);
                        }
                        // A shuffle's question parks the setup, and the rest
                        // of these listeners follow the answer.
                        sts2_core::HookEffect::FreeRandomFromDraw { kind } => {
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            let kinds: Vec<_> = sts2_core::Fight::cards_in(&board, Pile::Draw)
                                .into_iter()
                                .filter(|&at| {
                                    V::card_kind(sts2_core::Fight::card_ident(&board, at).card)
                                        == Some(kind)
                                })
                                .collect();
                            let stream =
                                <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_SELECTION;
                            if let Some(at) =
                                sts2_core::Fight::random_card(&mut board, &kinds, stream)
                            {
                                for pool in [sts2_core::CostPool::Energy, sts2_core::CostPool::Star]
                                {
                                    sts2_core::Fight::modify_card(
                                        &mut board,
                                        at,
                                        sts2_core::Change::SetCost(pool, num_traits::zero()),
                                        sts2_core::Lasting::ThisTurnOrUntilPlayed,
                                    );
                                }
                                sts2_core::Fight::act_on(
                                    &mut board,
                                    sts2_core::PileAct::IntoHand,
                                    &[at],
                                );
                            }
                        }
                        // `AddGeneratedCardsToCombat` into the draw pile, each
                        // at `CardPilePosition.Random`.
                        sts2_core::HookEffect::AddToDrawAtRandom { card, count } => {
                            let count: usize = num_traits::cast(count).unwrap_or_else(|| {
                                unimplemented!("adding {count:?} cards to the draw pile")
                            });
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            for _ in 0..count {
                                sts2_core::Fight::add_at_random(&mut board, card, Pile::Draw);
                            }
                        }
                        sts2_core::HookEffect::ShuffleIfNecessary => {
                            if !live.cards.is_empty(Pile::Draw) {
                                continue;
                            }
                            let Some((pile, selection, act)) = super::board::shuffle_asking::<V>(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                            ) else {
                                continue;
                            };
                            match select::<V>(live, rngs.reborrow(), relics, pile, selection, act) {
                                Err(question) => {
                                    return park_setup(
                                        live,
                                        act,
                                        Setup::BeforeHandDraw(rest),
                                        question,
                                    );
                                }
                                Ok(Some(ended)) => return Carried::Done(Some(ended)),
                                Ok(None) => {}
                            }
                        }
                        sts2_core::HookEffect::SelectFromPile {
                            pile,
                            selection,
                            act,
                        } => {
                            match select::<V>(live, rngs.reborrow(), relics, pile, selection, act) {
                                Err(question) => {
                                    return park_setup(
                                        live,
                                        act,
                                        Setup::BeforeHandDraw(rest),
                                        question,
                                    );
                                }
                                Ok(Some(ended)) => return Carried::Done(Some(ended)),
                                Ok(None) => {}
                            }
                        }
                        sts2_core::HookEffect::Remove => {
                            let Listener::Power { owner, power, .. } = listener else {
                                unimplemented!("{listener:?} removing itself before the draw")
                            };
                            live.remove_power(owner, power);
                        }
                        // Clones of the card a power keeps, one at a time, each
                        // past a full hand to the discard pile.
                        sts2_core::HookEffect::CloneKeptIntoHand { kept, count } => {
                            let count: usize = num_traits::cast(count)
                                .unwrap_or_else(|| unimplemented!("{count:?} clones"));
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
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
                                .find_map(|pile| live.cards.find(pile, id).map(|at| (pile, at)));
                            let to = match live.cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
                                true => Pile::Discard,
                                false => Pile::Hand,
                            };
                            match found {
                                Some((Pile::Hand, _)) => {}
                                Some((from, at)) if from != to => live.cards.move_to(from, at, to),
                                Some(_) => {}
                                None => unimplemented!(
                                    "a card moving to the hand from outside the piles"
                                ),
                            }
                        }
                        effect => unimplemented!(
                            "a listener asking for {effect:?} before the hand is drawn"
                        ),
                    }
                }
                let mut drawn = hand_size::<V>(listen::Walk { live, relics }, live.turn);
                super::start::after_modifying_hand_draw::<V>(live, relics);
                // On turn one a card whose enchantment says
                // `ShouldStartAtBottomOfDrawPile` sinks, then the innate cards
                // go on top and are all drawn, up to a full hand.
                if live.turn == num_traits::one() {
                    let innate = live.cards.innate_to_top(|face| {
                        face.ident.enchantment.is_some_and(|enchanted| {
                            V::should_start_at_bottom_of_draw_pile(enchanted.enchantment)
                                .is_decision_and(|sinks| sinks)
                        })
                    });
                    drawn = drawn.max(innate).min(MAX_CARDS_IN_HAND);
                }
                Setup::Drawing {
                    left: drawn,
                    resumed: false,
                }
            }
            // The hand's draw. A shuffle inside it can ask for a selection,
            // which the setup parks on and then draws the rest.
            Setup::Drawing { left, resumed } => {
                match super::board::draw_pausing::<V>(
                    live,
                    rngs.reborrow(),
                    relics,
                    ascension,
                    left,
                    true,
                    resumed,
                ) {
                    // A card auto-played as it is drawn can end the fight, and
                    // an ending fight hands the hooks after it no listeners.
                    super::board::Drawn::Done(Some(ended)) => return Carried::Done(Some(ended)),
                    super::board::Drawn::Done(None) => {
                        let hook = sts2_core::EventHook::PlayerTurnStart {
                            player,
                            turn: live.turn,
                        };
                        Setup::PlayerTurnStart(listen::notify::<V>(live, relics, hook))
                    }
                    // A card auto-played as it was drawn asked, and the rest
                    // of the draw waits for its play.
                    super::board::Drawn::Asked { left } => {
                        live.defer(super::live::Deferred::Setup(Setup::Drawing {
                            left,
                            resumed: true,
                        }));
                        return Carried::Parked;
                    }
                    super::board::Drawn::Paused {
                        left,
                        asks: (pile, selection, act),
                    } => {
                        let rest = Setup::Drawing {
                            left,
                            resumed: true,
                        };
                        match select::<V>(live, rngs.reborrow(), relics, pile, selection, act) {
                            Err(question) => {
                                return park_setup(live, act, rest, question);
                            }
                            Ok(Some(ended)) => return Carried::Done(Some(ended)),
                            Ok(None) => rest,
                        }
                    }
                }
            }
            // `Hook.AfterPlayerTurnStart`, the last of the setup. A relic's
            // damage here can end the fight.
            Setup::PlayerTurnStart(mut rest) => {
                let mut ended = None;
                let mut drawing = None;
                while !rest.is_empty() {
                    let (listener, effect) = rest.remove(0);
                    match effect {
                        // `CreatureCmd.Damage` over `HittableEnemies`, from
                        // the player.
                        sts2_core::HookEffect::DamageOpponents(amount) => {
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            for target in board.hittable_opponents() {
                                let hit = sts2_core::Fight::resolve(
                                    &board,
                                    sts2_core::Effect::Attack {
                                        dealer: player,
                                        target,
                                        damage: sts2_core::Amount::Number(amount),
                                        scaling: sts2_core::Scaling::Static,
                                    },
                                );
                                sts2_core::Fight::apply(&mut board, hit);
                            }
                            ended = ended.or(board.ended());
                        }
                        sts2_core::HookEffect::ChooseFromPoolIntoHand { count, writes } => {
                            let count: usize = num_traits::cast(count).unwrap_or_else(|| {
                                unimplemented!("offering {count:?} cards of the pool")
                            });
                            // `TakeRandom` shuffles the pool once and takes the
                            // first `count`.
                            let mut pool = V::combat_cards(live.character).to_vec();
                            sts2_core::rng::GameRng::unstable_shuffle(rngs.generation, &mut pool);
                            pool.truncate(count);
                            // An empty pool offers nothing, and the game returns.
                            if pool.is_empty() {
                                continue;
                            }
                            live.asking = Some(super::live::Asking {
                                then: Then::SetupChosen { writes },
                                // `FromSimpleGrid` with one to take, which it
                                // cannot skip.
                                question: super::live::Asked::Offered {
                                    offered: pool
                                        .into_iter()
                                        .map(sts2_core::CardIdent::new)
                                        .collect(),
                                    skippable: false,
                                },
                            });
                            live.defer(super::live::Deferred::Setup(Setup::PlayerTurnStart(rest)));
                            return Carried::Parked;
                        }
                        sts2_core::HookEffect::SelectFromPile {
                            pile,
                            selection,
                            act,
                        } => {
                            match select::<V>(live, rngs.reborrow(), relics, pile, selection, act) {
                                Err(question) => {
                                    return park_setup(
                                        live,
                                        act,
                                        Setup::PlayerTurnStart(rest),
                                        question,
                                    );
                                }
                                Ok(Some(ended)) => return Carried::Done(Some(ended)),
                                Ok(None) => {}
                            }
                        }
                        // `OstyCmd.Summon` for the listener's owner.
                        sts2_core::HookEffect::Summon(amount) => {
                            super::board::summon_osty::<V>(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                amount,
                            );
                        }
                        // `PowerCmd.Remove(this)`.
                        sts2_core::HookEffect::Remove => {
                            let Listener::Power { owner, power, .. } = listener else {
                                unimplemented!("{listener:?} removing itself as the turn starts")
                            };
                            live.remove_power(owner, power);
                        }
                        // `PowerModel.SetAmount`, which no hook hears.
                        sts2_core::HookEffect::SetAmount(amount) => {
                            let Listener::Power { owner, power, .. } = listener else {
                                unimplemented!("{listener:?} setting its amount as the turn starts")
                            };
                            for held in live.powers.of_mut(owner) {
                                if held.power() == power {
                                    held.amount = super::live::Stacked::counted(amount);
                                }
                            }
                        }
                        // `OrbCmd.Passive` on whichever orb is first each
                        // time, which returns once the fight is ending.
                        sts2_core::HookEffect::FirstOrbPassive(count) => {
                            let count: usize = num_traits::cast(count).unwrap_or_else(|| {
                                unimplemented!("{count:?} orb passives at once")
                            });
                            for _ in 0..count {
                                let Some(mut first) = live
                                    .orbs
                                    .as_ref()
                                    .and_then(|orbs| orbs.queue.first().cloned())
                                else {
                                    break;
                                };
                                if ended.is_some() {
                                    break;
                                }
                                let mut board = Board::for_card(
                                    live,
                                    rngs.reborrow(),
                                    relics,
                                    ascension,
                                    None,
                                    None,
                                );
                                V::orb_passive(&mut first, &mut board);
                                ended = ended.or(board.ended());
                                keep_orb(live, 0, first);
                            }
                        }
                        // `OrbCmd.Passive` on each orb of `OrbQueue.Orbs`, in
                        // queue order, which returns once the fight is ending.
                        sts2_core::HookEffect::EachOrbPassive => {
                            let held = live.orbs.as_ref().map_or(0, |orbs| orbs.queue.len());
                            for place in 0..held {
                                let Some(mut orb) = live
                                    .orbs
                                    .as_ref()
                                    .and_then(|orbs| orbs.queue.get(place).cloned())
                                else {
                                    break;
                                };
                                if ended.is_some() {
                                    break;
                                }
                                let mut board = Board::for_card(
                                    live,
                                    rngs.reborrow(),
                                    relics,
                                    ascension,
                                    None,
                                    None,
                                );
                                V::orb_passive(&mut orb, &mut board);
                                ended = ended.or(board.ended());
                                keep_orb(live, place, orb);
                            }
                        }
                        // `CardPileCmd.Draw` for the player, which a shuffle's
                        // question can pause. A fight that is over draws
                        // nothing, and a negative count draws nothing.
                        sts2_core::HookEffect::Draw(count) => {
                            if ended.is_none() {
                                drawing = Some(num_traits::cast(count).unwrap_or(0));
                                break;
                            }
                        }
                        // `CreatureCmd.Damage` on the listener's owner: the
                        // player for a relic, the holder for a power.
                        sts2_core::HookEffect::DamageOwnerInFight {
                            amount,
                            unblockable,
                            self_dealt,
                        } if matches!(
                            listener,
                            Listener::Relic { .. } | Listener::Power { .. }
                        ) =>
                        {
                            let owner = match listener {
                                Listener::Power { owner, .. } => owner,
                                _ => player,
                            };
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            board.hit_unpowered(
                                self_dealt.then_some(owner),
                                owner,
                                amount,
                                unblockable,
                            );
                            ended = ended.or(board.ended());
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
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            super::hook::apply::<V, _>(&mut board, [effect]);
                            ended = ended.or(board.ended());
                        }
                        sts2_core::HookEffect::GenerateFromPool {
                            filter,
                            count,
                            writes,
                        } => generate_from_pool::<V>(
                            live,
                            rngs.reborrow(),
                            relics,
                            ascension,
                            filter,
                            count,
                            writes,
                        ),
                        // `CardCmd.Upgrade` of every card in hand.
                        sts2_core::HookEffect::UpgradeHand => live.cards.write_every(
                            &[Pile::Hand],
                            |_| true,
                            |card| {
                                card.modify(
                                    sts2_core::Change::Upgrade,
                                    sts2_core::Lasting::ThisFight,
                                )
                            },
                        ),
                        effect => unimplemented!(
                            "a listener asking for {effect:?} as the player's turn starts"
                        ),
                    }
                }
                if let Some(left) = drawing {
                    Setup::TurnStartDrawing {
                        left,
                        resumed: false,
                        rest,
                    }
                } else {
                    // `AfterPlayerTurnStartLate`, once every listener above
                    // has run.
                    if ended.is_none() {
                        let player = live.player().id;
                        let hook = sts2_core::EventHook::PlayerTurnStartLate {
                            player,
                            turn: live.turn,
                        };
                        for (listener, effect) in listen::notify::<V>(live, relics, hook) {
                            match effect {
                                // `CreatureCmd.Heal` on the player who carries
                                // the relic.
                                sts2_core::HookEffect::Heal(amount) => {
                                    let mut board = Board::for_card(
                                        live,
                                        rngs.reborrow(),
                                        relics,
                                        ascension,
                                        None,
                                        None,
                                    );
                                    sts2_core::Fight::heal(&mut board, player, amount);
                                }
                                effect => unimplemented!(
                                    "{listener:?} asking for {effect:?} late as the turn starts"
                                ),
                            }
                        }
                    }
                    return Carried::Done(ended);
                }
            }
            Setup::TurnStartDrawing {
                left,
                resumed,
                rest,
            } => match super::board::draw_pausing::<V>(
                live,
                rngs.reborrow(),
                relics,
                ascension,
                left,
                false,
                resumed,
            ) {
                super::board::Drawn::Done(Some(ended)) => return Carried::Done(Some(ended)),
                super::board::Drawn::Done(None) => Setup::PlayerTurnStart(rest),
                super::board::Drawn::Asked { left } => {
                    live.defer(super::live::Deferred::Setup(Setup::TurnStartDrawing {
                        left,
                        resumed: true,
                        rest,
                    }));
                    return Carried::Parked;
                }
                super::board::Drawn::Paused {
                    left,
                    asks: (pile, selection, act),
                } => {
                    let next = Setup::TurnStartDrawing {
                        left,
                        resumed: true,
                        rest,
                    };
                    match select::<V>(live, rngs.reborrow(), relics, pile, selection, act) {
                        Err(question) => {
                            return park_setup(live, act, next, question);
                        }
                        Ok(Some(ended)) => return Carried::Done(Some(ended)),
                        Ok(None) => next,
                    }
                }
            },
        };
    }
}

/// `count` distinct colorless cards from one `GetDistinctForCombat` off
/// `CombatCardGeneration`, into the hand.
fn generate_distinct_colorless<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    count: V::Number,
) {
    let cards = V::combat_colorless_cards().to_vec();
    generate_distinct::<V>(live, rngs, relics, ascension, cards, count);
}

/// `count` distinct cards of the character's combat pool that `filter`
/// keeps, from one `GetDistinctForCombat` off `CombatCardGeneration`, into
/// the hand.
fn generate_distinct_from_pool<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    filter: sts2_core::PoolFilter,
    count: V::Number,
) {
    let cards: Vec<V::Card> = V::combat_cards(live.character)
        .iter()
        .copied()
        .filter(|&card| crate::reward::card::keeps::<V>(filter, card))
        .collect();
    generate_distinct::<V>(live, rngs, relics, ascension, cards, count);
}

/// `GetDistinctForCombat` over `cards`: `TakeRandom` shuffles them once off
/// `CombatCardGeneration` and takes the first `count`, or all of them where
/// there are fewer. Each goes into the hand.
fn generate_distinct<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    mut cards: Vec<V::Card>,
    count: V::Number,
) {
    let count: usize = num_traits::cast(count)
        .unwrap_or_else(|| unimplemented!("making {count:?} distinct cards"));
    sts2_core::rng::GameRng::unstable_shuffle(rngs.generation, &mut cards);
    let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
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
fn generate_from_pool<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    filter: sts2_core::PoolFilter,
    count: V::Number,
    writes: &[(sts2_core::Change<V>, sts2_core::Lasting)],
) {
    let count: usize = num_traits::cast(count)
        .unwrap_or_else(|| unimplemented!("making {count:?} cards from a pool"));
    let keeps = |card: V::Card| crate::reward::card::keeps::<V>(filter, card);
    for _ in 0..count {
        let mut cards: Vec<V::Card> = V::combat_cards(live.character)
            .iter()
            .copied()
            .filter(|card| keeps(*card))
            .collect();
        let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
        let Ok([card]) = sts2_core::Fight::generate_cards::<1>(
            &mut board,
            &mut cards,
            <V::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION,
        ) else {
            return;
        };
        for at in sts2_core::Fight::add_generated_cards(&mut board, card, Pile::Hand, 1) {
            for &(change, lasting) in writes {
                sts2_core::Fight::modify_card(&mut board, at, change, lasting);
            }
        }
    }
}

/// `CardSelectCmd.FromHand` or `FromCombatPile` for `count` cards of `pile`:
/// the cards taken without asking, acted on now, or the question to stop on.
///
/// A discard that took Sly cards plays them, and whatever that ended comes
/// back.
pub(super) fn select<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    pile: Pile,
    selection: sts2_core::Selection,
    act: sts2_core::PileAct,
) -> Result<Option<super::Ended>, super::live::Asked<V>> {
    match super::play::taken(live, pile, selection) {
        Some(picked) => {
            let mut rngs = rngs;
            let (sly, ended) =
                act_on::<V>(live, rngs.reborrow(), relics, act, picked.as_slice(), false);
            if ended.is_some() {
                return Ok(ended);
            }
            Ok(play_sly::<V>(live, rngs, relics, sly, false))
        }
        None => Err(super::live::Asked::Pile { pile, selection }),
    }
}

/// Carry `act` out on the cards a selection took, in the order it took them.
///
/// A discard hands back the Sly cards it took, which the caller auto-plays
/// once every card is down.
pub(super) fn act_on<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    act: sts2_core::PileAct,
    picked: &[sts2_core::CardAt<V>],
    pausable: bool,
) -> (SmallVec<[sts2_core::CardId<V>; 2]>, Option<super::Ended>) {
    let mut sly = SmallVec::new();
    let asked_before = live.asking.is_some();
    for (taking, card) in picked.iter().enumerate() {
        // A question a hook asked stands the cards after it still, which
        // nothing here holds back yet.
        if !asked_before && live.asking.is_some() && taking > 0 {
            unimplemented!("{act:?} of more cards behind a question");
        }
        let from = card.pile();
        let at = live
            .cards
            .find(from, card.id())
            .unwrap_or_else(|| unimplemented!("a picked card that left its pile"));
        let to = match act {
            // `CardCmd.Exhaust`, which raises its hook once the card is
            // down.
            sts2_core::PileAct::Exhaust => {
                let held = live
                    .cards
                    .get(from, at)
                    .expect("a card stands where it was found");
                live.cards.move_to(from, at, Pile::Exhaust);
                if let Some(ended) =
                    exhausted::<V>(live, rngs.reborrow(), relics, &held, false, pausable)
                {
                    return (sly, Some(ended));
                }
                continue;
            }
            // `CardCmd.Discard`, which plays a Sly card once every card is
            // down. `DiscardAndDraw` draws before that.
            sts2_core::PileAct::Discard | sts2_core::PileAct::DiscardAndDraw => {
                let held = live
                    .cards
                    .get(from, at)
                    .expect("a card stands where it was found");
                if held.face.sly() {
                    sly.push(held.id);
                }
                let card = held.face.ident.card;
                live.cards.move_to(from, at, Pile::Discard);
                live.history.discarded(card);
                if let Some(ended) = card_discarded::<V>(live, rngs.reborrow(), relics) {
                    return (sly, Some(ended));
                }
                continue;
            }
            // `CardCmd.TransformToRandom`, one card at a time, each drawn off
            // `CombatCardSelection` before it replaces the original where it
            // stood.
            sts2_core::PileAct::TransformToRandom => {
                let held = live
                    .cards
                    .get(from, at)
                    .expect("a card stands where it was found");
                let options = V::transform_options(held.face.ident.card, true);
                let into = *sts2_core::rng::GameRng::choose(&mut *rngs.selection, &options)
                    .unwrap_or_else(|_| unimplemented!("a transform with no option"));
                let ascension = live.ascension;
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::transform(&mut board, *card, sts2_core::CardIdent::new(into));
                if let Some(ended) = board.ended() {
                    return (sly, Some(ended));
                }
                continue;
            }
            // `CardPileCmd.Add(cards, PileType.Hand)`, which sends a card past
            // a full hand to the discard pile.
            sts2_core::PileAct::IntoHand => match live.cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
                true => Pile::Discard,
                false => Pile::Hand,
            },
            sts2_core::PileAct::IntoDraw => Pile::Draw,
            // `CardPilePosition.Top`, the front, where a draw takes from.
            sts2_core::PileAct::OntoDrawTop => {
                live.cards.move_to(from, at, Pile::Draw);
                live.cards
                    .move_to_top(Pile::Draw, live.cards.len(Pile::Draw) - 1);
                continue;
            }
        };
        live.cards.move_to(from, at, to);
    }
    // `CardCmd.DiscardAndDraw`: as many drawn as were discarded, and the Sly
    // ones only after that, which the caller plays.
    if act == sts2_core::PileAct::DiscardAndDraw && !picked.is_empty() {
        let draw: V::Number =
            num_traits::cast(picked.len()).expect("a hand's worth fits a game number");
        let ascension = live.ascension;
        let mut board = Board::for_card(live, rngs, relics, ascension, None, None);
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
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
) -> Option<super::Ended> {
    let hook = sts2_core::EventHook::CardDiscarded { side: live.side };
    let asked = listen::notify::<V>(live, relics, hook);
    if asked.is_empty() {
        return None;
    }
    let mut effects: SmallVec<[sts2_core::HookEffect<V>; 2]> = SmallVec::new();
    for (listener, effect) in asked {
        match listener {
            Listener::Relic { .. } => effects.push(effect),
            listener => unimplemented!("{listener:?} asking for {effect:?} after a discard"),
        }
    }
    let ascension = live.ascension;
    let mut board = Board::for_card(live, rngs, relics, ascension, None, None);
    super::hook::apply::<V, _>(&mut board, effects);
    board.ended()
}

/// `CardCmd.AutoPlay` with `SlyDiscard` for each Sly card a discard took, in
/// the order they went down, and whatever that ended.
pub(super) fn play_sly<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    sly: SmallVec<[sts2_core::CardId<V>; 2]>,
    pausable: bool,
) -> Option<super::Ended> {
    for id in sly {
        // `CardPileCmd.Add` to the play pile from wherever the card stands
        // by now: a draw between can shuffle it into the draw pile.
        let Some(from) = [Pile::Discard, Pile::Draw, Pile::Hand, Pile::Exhaust]
            .into_iter()
            .find(|&pile| live.cards.find(pile, id).is_some())
        else {
            unimplemented!("a Sly card that left every pile before it played");
        };
        let ended =
            super::play::auto_play::<V>(live, rngs.reborrow(), relics, id, from, None, pausable);
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
    relics: &crate::run::Belt<V>,
    hook: sts2_core::ModelHook,
) {
    if listen::heard::<V>(live, relics, hook) {
        let named = listen::listeners_to::<V>(live, relics, hook);
        unimplemented!("a listener to {}: {named:?}", hook.name());
    }
}

/// `Hook.AfterCardExhausted`, once `card` lies in the exhaust pile, and
/// whatever the answers ended.
pub(super) fn exhausted<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
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
    let asked = listen::notify::<V>(live, relics, hook);
    if asked.is_empty() {
        return None;
    }
    let ascension = live.ascension;
    // Inside an effect, a question a listener's command asks stands the
    // effect still.
    let mut board = Board::for_card(live, rngs, relics, ascension, None, None);
    if pausable {
        board = board.blocking();
    }
    let mut rest: SmallVec<[sts2_core::HookEffect<V>; 4]> = SmallVec::new();
    for (_, effect) in asked {
        match effect {
            // Clones of the card just exhausted, which the exhaust pile still
            // holds.
            sts2_core::HookEffect::CloneIntoHand {
                card,
                count,
                writes,
            } => {
                let count: usize =
                    num_traits::cast(count).unwrap_or_else(|| unimplemented!("{count:?} clones"));
                for _ in 0..count {
                    board.clone_written(card, Pile::Hand, writes);
                }
            }
            effect => rest.push(effect),
        }
    }
    super::hook::apply::<V, _>(&mut board, rest);
    let skipped = std::mem::take(&mut board.skipped);
    let ended = board.ended();
    super::play::merge::<V>(&mut live.skip_tick, skipped);
    ended
}

/// Answer the question `SetupPlayerTurn` stopped on: `act` on the cards
/// picked. The rest of the setup waits in [`Live::deferred`].
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn answer_setup<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    act: sts2_core::PileAct,
    picked: sts2_core::SelectedCards<V>,
) -> Option<super::Ended> {
    let (sly, ended) = act_on::<V>(live, rngs.reborrow(), relics, act, picked.as_slice(), false);
    // A Sly card that asks stands the setup still behind its own question,
    // whose deferrals run ahead of the setup's.
    ended.or_else(|| play_sly::<V>(live, rngs, relics, sly, true))
}

/// `SetupPlayerTurn` from `stage` to its end, once a question it stopped on
/// is answered, then `RunAutoPrePlayPhase`. `ended` is how the answer ended
/// the fight, which leaves the setup unfinished.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn finish_setup<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    stage: Setup<V>,
    ended: Option<super::Ended>,
) -> Option<super::Ended> {
    let mut rngs = rngs;
    let carried = match ended {
        Some(ended) => Carried::Done(Some(ended)),
        None => carry_setup(live, rngs.reborrow(), relics, ascension, stage),
    };
    match carried {
        Carried::Parked => None,
        Carried::Done(ended) => {
            let ended = ended.or_else(|| pre_play_phase::<V>(live, rngs, relics));
            match ended {
                Some(super::Ended::Won) => listen::combat_ended::<V>(live, relics),
                Some(super::Ended::Lost) => {}
                None => live.phase = after_pre_play::<V>(live),
            }
            ended
        }
    }
}

/// `RunAutoPrePlayPhase` once the player's turn is set up:
/// `AfterAutoPrePlayPhaseEntered`'s early pass, its own, then its late one,
/// each over every listener. How an auto-play there ended the fight.
///
/// # Panics
///
/// A listener asking for anything but an auto-play of a card.
pub(super) fn pre_play_phase<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
) -> Option<super::Ended> {
    use sts2_core::{ModelHook, Overrides};

    // Most fights hold nothing that hears any of the three passes.
    let hooks = Overrides::of(ModelHook::AfterAutoPrePlayPhaseEnteredEarly)
        .with(ModelHook::AfterAutoPrePlayPhaseEntered)
        .with(ModelHook::AfterAutoPrePlayPhaseEnteredLate);
    if !listen::heard_any::<V>(live, relics, hooks) {
        return None;
    }
    pre_play_heard::<V>(live, rngs, relics)
}

/// [`pre_play_phase`] once something hears one of its passes.
#[cold]
#[inline(never)]
#[tracing::instrument(level = "trace", skip_all)]
fn pre_play_heard<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
) -> Option<super::Ended> {
    pre_play_from::<V>(live, rngs, relics, SmallVec::new(), 0)
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
/// A card the fight no longer holds. An X-cost dupe captures the energy on
/// hand as it plays, as any auto-play does, since `AutoPlay` skips no
/// capture.
fn auto_play_dupe_of_last_play<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
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
    let Some(original) = live.cards.named(last.id) else {
        unimplemented!("a dupe of {:?}, which the fight no longer holds", last.card);
    };
    let id = live.card_ids.mint();
    let index = num_traits::NumCast::from(live.cards.len(Pile::Play))
        .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
    live.cards.enter_play(original.face.duped().named(id));
    // `CardPileCmd.Add` to the play pile from no pile raises
    // `AfterCardEnteredCombat`, and a dupe is a clone of its original.
    let ascension = live.ascension;
    let mut board =
        Board::for_card(live, rngs.reborrow(), relics, ascension, None, None).blocking();
    board.card_entered_combat(
        sts2_core::CardAt::new(id, Pile::Play, index),
        original.face.ident.card,
        true,
    );
    if let Some(ended) = board.ended() {
        return Some(ended);
    }
    super::play::auto_play_taken::<V>(live, rngs, relics, id, false, true)
}

pub(super) fn pre_play_from<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    rest: PrePlayRest<V>,
    next: usize,
) -> Option<super::Ended> {
    let turn = live.turn;
    let passes = [
        sts2_core::EventHook::AutoPrePlayPhaseEarly { turn },
        sts2_core::EventHook::AutoPrePlayPhase { turn },
        sts2_core::EventHook::AutoPrePlayPhaseLate { turn },
    ];
    let mut pass = next;
    let mut asked: PrePlayRest<V> = rest;
    loop {
        let mut effects = asked.into_iter();
        while let Some((listener, effect)) = effects.next() {
            match effect {
                // `CardCmd.AutoPlay` of the card, from wherever it stands.
                sts2_core::HookEffect::AutoPlay(card) => {
                    let Some(from) = live.cards.pile_of(card) else {
                        unimplemented!("auto-playing {card:?}, which stands in no pile");
                    };
                    let ended = super::play::auto_play::<V>(
                        live,
                        rngs.reborrow(),
                        relics,
                        card,
                        from,
                        None,
                        true,
                    );
                    if ended.is_some() {
                        return ended;
                    }
                }
                sts2_core::HookEffect::AutoPlayDupeOfLastPlay { kinds } => {
                    let ended = auto_play_dupe_of_last_play::<V>(
                        live,
                        rngs.reborrow(),
                        relics,
                        turn,
                        kinds,
                    );
                    if ended.is_some() {
                        return ended;
                    }
                    // `CardCmd.AutoPlay` is awaited, so what the pass still
                    // asked for waits on the answer.
                    if live.asking.is_some() {
                        let rest: PrePlayRest<V> = effects.collect();
                        live.defer(super::live::Deferred::PrePlay { rest, next: pass });
                        return None;
                    }
                }
                sts2_core::HookEffect::AutoPlayHand { mut left } => {
                    let start = live.turn;
                    while left > 0 && !live.ending_turn && live.turn == start {
                        let Some(first) = first_playable::<V>(live, relics) else {
                            break;
                        };
                        let captured =
                            spend_resources::<V>(live, rngs.reborrow(), relics, first.costs);
                        left -= 1;
                        let ended = super::play::auto_play_capturing::<V>(
                            live,
                            rngs.reborrow(),
                            relics,
                            first.id,
                            Pile::Hand,
                            first.target,
                            true,
                            Some(captured),
                        );
                        if ended.is_some() {
                            return ended;
                        }
                        // `CardCmd.AutoPlay` is awaited, so the plays left
                        // wait on the answer, ahead of what the pass still
                        // asked for.
                        if live.asking.is_some() {
                            let mut rest: PrePlayRest<V> = SmallVec::new();
                            rest.push((listener, sts2_core::HookEffect::AutoPlayHand { left }));
                            rest.extend(effects);
                            live.defer(super::live::Deferred::PrePlay { rest, next: pass });
                            return None;
                        }
                    }
                }
                sts2_core::HookEffect::AutoPlayFromDraw(count) => {
                    let count: usize = num_traits::cast(count)
                        .unwrap_or_else(|| unimplemented!("auto-playing {count:?} cards"));
                    let ascension = live.ascension;
                    let mut board = super::board::Board::for_card(
                        live,
                        rngs.reborrow(),
                        relics,
                        ascension,
                        None,
                        None,
                    )
                    .blocking();
                    sts2_core::Fight::auto_play_from_draw(&mut board, count, false);
                    if let Some(ended) = board.ended() {
                        return Some(ended);
                    }
                }
                effect => {
                    unimplemented!("{listener:?} asking for {effect:?} as the play phase opens")
                }
            }
            // `RunAutoPrePlayPhase` awaits each hook, so the rest waits on
            // the answer.
            if live.asking.is_some() {
                live.defer(super::live::Deferred::PrePlay {
                    rest: effects.collect(),
                    next: pass,
                });
                return None;
            }
        }
        // Past the last pass, the phase is over and the fight goes on.
        let &hook = passes.get(pass)?;
        asked = listen::notify::<V>(live, relics, hook);
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
    live: &Live<V>,
    relics: &crate::run::Belt<V>,
) -> Option<FirstPlayable<V>> {
    use sts2_core::TargetKinds;

    let walk = listen::Walk { live, relics };
    let payable = super::legal::first_playable::<V>(
        live.cards.iter(Pile::Hand),
        walk,
        live.energy,
        live.stars,
    )?;
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
                        listen::Walk { live, relics },
                        sts2_core::DecisionHook::ShouldAllowHitting {
                            creature,
                            alive: true,
                        },
                        false,
                    )
                });
            enemies.into_iter().next()
        }
        t if t == <V::TargetType as TargetKinds>::ANY_PLAYER => Some(live.player().id),
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
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    (cost, star_cost): (sts2_core::Cost, sts2_core::Cost),
) -> (V::Count, V::Count) {
    let energy = super::legal::spend(cost, live.energy)
        .unwrap_or_else(|| unimplemented!("a cost of {cost:?}"))
        .min(live.energy);
    live.energy = live.energy - energy;
    let stars: V::Count = super::legal::spend(star_cost, live.stars)
        .unwrap_or_else(|| unimplemented!("a star cost of {star_cost:?}"));
    if stars > num_traits::zero() {
        let spent: V::Number = num_traits::cast(stars).expect("a star cost fits a game number");
        live.modify_stars(-spent);
        let hook = sts2_core::EventHook::StarsSpent {
            player: live.player().id,
            amount: spent,
        };
        let asked = listen::notify::<V>(live, relics, hook);
        if !asked.is_empty() {
            let ascension = live.ascension;
            let mut board = Board::for_card(live, rngs, relics, ascension, None, None);
            super::hook::apply::<V, _>(&mut board, asked.into_iter().map(|(_, effect)| effect));
            if let Some(ended) = board.ended() {
                unimplemented!("a fight {ended:?} as its stars were spent");
            }
        }
    }
    (energy, stars)
}

/// What a pre-play pass asked for and has not yet carried out.
pub(super) type PrePlayRest<V> = SmallVec<[(Listener<V>, sts2_core::HookEffect<V>); 4]>;

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
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    side: V::CombatSide,
) -> Option<super::Ended> {
    let turn = live.turn;
    let mut ended = None;
    let hook = sts2_core::EventHook::SideTurnStart {
        side,
        turn,
        room: live.room(),
    };
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match effect {
            // `PowerCmd.Remove(this)`, from a power spent as its side begins.
            sts2_core::HookEffect::Remove => {
                let Listener::Power { owner, power, .. } = listener else {
                    unimplemented!("{listener:?} removing itself as a side turn starts")
                };
                live.remove_power(owner, power);
            }
            // `PowerCmd.Decrement(this)`.
            sts2_core::HookEffect::Decrement => {
                let Listener::Power { owner, power, .. } = listener else {
                    unimplemented!("{listener:?} decrementing itself as a side turn starts")
                };
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                board.modify_amount(owner, power, -num_traits::one::<V::Number>());
            }
            // `PowerCmd.Apply<P>(Owner, amount, Owner)`. A relic's owner is
            // the player.
            sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                let owner = match listener {
                    Listener::Power { owner, .. } => owner,
                    Listener::Relic { .. } => live.player().id,
                    listener => unimplemented!("{listener:?} applying a power to its owner"),
                };
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                board.land(owner, power, amount, Some(owner));
            }
            // `NextItem` over `HittableEnemies` off `CombatTargets`, then
            // `PowerCmd.Apply<P>` from the listener's owner.
            sts2_core::HookEffect::ApplyToRandomOpponent { power, amount } => {
                let Listener::Power { owner, .. } = listener else {
                    unimplemented!("{listener:?} applying a power to a random enemy")
                };
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                if let Some(target) = sts2_core::Fight::random_opponent(&mut board) {
                    board.land(target, power, amount, Some(owner));
                }
            }
            sts2_core::HookEffect::UpgradeHand => live.cards.write_every(
                &[sts2_core::Pile::Hand],
                |_| true,
                |card| card.modify(sts2_core::Change::Upgrade, sts2_core::Lasting::ThisFight),
            ),
            sts2_core::HookEffect::Forge { card, key, amount } => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::forge(&mut board, card, key, amount);
            }
            // `PlayerCmd.GainEnergy`.
            sts2_core::HookEffect::GainEnergy(amount) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::modify_energy(&mut board, amount);
            }
            sts2_core::HookEffect::GainEnergyForGold { energy, gold } => {
                if live.gold >= gold {
                    let mut board =
                        Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                    sts2_core::Fight::modify_energy(&mut board, energy);
                    live.gold = live.gold - gold;
                }
            }
            // `CreatureCmd.GainBlock`, unpowered, on the power's owner.
            sts2_core::HookEffect::GainBlockPerOrbKind(amount) => {
                let Listener::Power { owner, .. } = listener else {
                    unimplemented!("{listener:?} gaining block for its orbs")
                };
                let mut kinds: SmallVec<[V::Orb; 10]> = SmallVec::new();
                for held in live.orbs.iter().flat_map(|orbs| &orbs.queue) {
                    if !kinds.contains(&held.orb) {
                        kinds.push(held.orb);
                    }
                }
                let count: V::Number = num_traits::cast(kinds.len())
                    .expect("a queue's kinds of orb fit a game number");
                live.gain_unpowered_block(owner, amount * count);
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
                    let Some(PowerAmount::Counted(stack)) = live
                        .powers
                        .of(owner)
                        .find(|held| held.power() == power)
                        .map(|held| held.amount.amount())
                    else {
                        break;
                    };
                    let mut board =
                        Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                    board.hit_unpowered(None, owner, stack, true);
                    let alive = board
                        .at(owner)
                        .is_some_and(|at| board.live.creatures[at].alive);
                    if alive {
                        board.modify_amount(owner, power, -num_traits::one::<V::Number>());
                    }
                    ended = ended.or(board.ended());
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
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
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
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                for target in board.hittable_opponents() {
                    board.land(target, power, amount, Some(owner));
                }
                ended = ended.or(board.ended());
            }
            // `PlayerCmd.GainStars`.
            sts2_core::HookEffect::GainStars(amount) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::gain_stars(&mut board, amount);
            }
            // `OrbCmd.Channel` on the player, which evokes an orb to make room.
            sts2_core::HookEffect::Channel(orb) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::channel(&mut board, orb);
                ended = ended.or(board.ended());
            }
            // `OstyCmd.Summon` for the player.
            sts2_core::HookEffect::Summon(amount) => {
                super::board::summon_osty::<V>(live, rngs.reborrow(), relics, ascension, amount);
            }
            // `CreatureCmd.GainBlock`, unpowered, on a relic's owner, who is
            // the player the board acts for.
            effect @ sts2_core::HookEffect::GainBlock(_)
                if matches!(listener, Listener::Relic { .. }) =>
            {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                super::hook::apply::<V, _>(&mut board, [effect]);
                ended = ended.or(board.ended());
            }
            // `PlayerCmd.LoseEnergy` from the player's pool.
            effect @ sts2_core::HookEffect::LoseEnergy(_) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                super::hook::apply::<V, _>(&mut board, [effect]);
            }
            // `CreatureCmd.GainBlock`, unpowered, on a creature the listener
            // names.
            effect @ sts2_core::HookEffect::GainBlockOf { .. } => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                super::hook::apply::<V, _>(&mut board, [effect]);
                ended = ended.or(board.ended());
            }
            sts2_core::HookEffect::GenerateFromPool {
                filter,
                count,
                writes,
            } => generate_from_pool::<V>(
                live,
                rngs.reborrow(),
                relics,
                ascension,
                filter,
                count,
                writes,
            ),
            sts2_core::HookEffect::GenerateDistinctColorless { count } => {
                generate_distinct_colorless::<V>(live, rngs.reborrow(), relics, ascension, count)
            }
            sts2_core::HookEffect::AddOrbSlots(amount) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::add_orb_slots(&mut board, amount);
            }
            sts2_core::HookEffect::GenerateDistinctFromPool { filter, count } => {
                generate_distinct_from_pool::<V>(
                    live,
                    rngs.reborrow(),
                    relics,
                    ascension,
                    filter,
                    count,
                )
            }
            effect => unimplemented!("a listener asking for {effect:?} as a side turn starts"),
        }
    }
    if ended.is_some() {
        return ended;
    }
    // `Hook.AfterSideTurnStartLate`.
    let hook = sts2_core::EventHook::SideTurnStartLate { side };
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            // `PowerCmd.Decrement(this)`.
            (Listener::Power { owner, power, .. }, sts2_core::HookEffect::Decrement) => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                board.modify_amount(owner, power, -num_traits::one::<V::Number>());
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
fn flush_hand<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
) {
    let player = live.player().id;
    let turn = live.turn;
    let fight = listen::Walk { live, relics };
    if !listen::any_decides::<V>(
        fight,
        sts2_core::DecisionHook::ShouldFlush { player, turn },
        false,
    ) {
        live.cards.sift(Pile::Hand, Pile::Discard, Face::retains);
    }
    // `Hook.AfterFlush`, with what stayed in hand retained.
    let asked = listen::notify::<V>(live, relics, sts2_core::EventHook::Flushed);
    if asked.is_empty() {
        return;
    }
    let mut board = Board::for_card(live, rngs, relics, ascension, None, None);
    for (listener, effect) in asked {
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
    live: &mut Live<V>,
    streams: &mut crate::stream::Streams<V>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
) -> Option<super::Ended> {
    let player = live.player().id;
    let hook = sts2_core::EventHook::EnergyReset {
        player,
        turn: live.turn,
    };
    let mut ended = None;
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            // `PlayerCmd.GainEnergy`, then `PowerCmd.Remove(this)`.
            (
                Listener::Power { owner, power, .. },
                sts2_core::HookEffect::GainEnergyAndRemove(amount),
            ) => {
                let mut board =
                    Board::for_card(live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::modify_energy(&mut board, amount);
                live.remove_power(owner, power);
            }
            // `PlayerCmd.GainStars`.
            (_, sts2_core::HookEffect::GainStars(amount)) => {
                let mut board =
                    Board::for_card(live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::gain_stars(&mut board, amount);
            }
            // `PowerCmd.Remove(this)`.
            (Listener::Power { owner, power, .. }, sts2_core::HookEffect::Remove) => {
                live.remove_power(owner, power)
            }
            // `PlayerCmd.GainEnergy`.
            (_, sts2_core::HookEffect::GainEnergy(amount)) => {
                let mut board =
                    Board::for_card(live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::modify_energy(&mut board, amount);
            }
            // `PowerCmd.Decrement(this)`.
            (Listener::Power { owner, power, .. }, sts2_core::HookEffect::Decrement) => {
                let mut board =
                    Board::for_card(live, streams.combat(), relics, ascension, None, None);
                board.modify_amount(owner, power, -num_traits::one::<V::Number>());
            }
            // `OrbCmd.Channel` on the player, which evokes an orb to make room.
            // The streams are lent only here, which most turns never reach.
            (_, sts2_core::HookEffect::Channel(orb)) => {
                let mut board =
                    Board::for_card(live, streams.combat(), relics, ascension, None, None);
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
    live: &mut Live<V>,
    mut rngs: crate::stream::CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
) {
    let player = live.player().id;
    let hook = sts2_core::EventHook::EnergyResetLate {
        player,
        turn: live.turn,
    };
    for (_, effect) in listen::notify::<V>(live, relics, hook) {
        match effect {
            sts2_core::HookEffect::Summon(amount) => {
                super::board::summon_osty::<V>(live, rngs.reborrow(), relics, ascension, amount);
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
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    side: V::CombatSide,
) {
    let clearing: SmallVec<[CombatId; 8]> = live
        .creatures
        .iter()
        .filter(|c| c.side == side)
        .map(|creature| creature.id)
        .collect();
    let player = live.player().id;
    for &creature in &clearing {
        // `Hook.ShouldClearBlock`, and the model that kept the block.
        let preventer = listen::first_deciding::<V>(
            listen::Walk { live, relics },
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
                for effect in listen::ask_relic(relics, relic, hook) {
                    match effect {
                        sts2_core::HookEffect::LoseSomeBlock(amount) => {
                            let ascension = live.ascension;
                            let mut board = Board::for_card(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                None,
                                None,
                            );
                            sts2_core::Fight::lose_block(&mut board, creature, amount);
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
        for (listener, effect) in listen::notify::<V>(live, relics, hook) {
            match (listener, effect) {
                // `CreatureCmd.GainBlock`, unpowered, on the player who
                // carries the relic.
                (Listener::Relic { .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                    live.gain_unpowered_block(player, amount);
                }
                // `PowerCmd.Apply` on the player who carries the relic, from
                // the player.
                (Listener::Relic { .. }, sts2_core::HookEffect::ApplyToOwner(power, amount)) => {
                    let ascension = live.ascension;
                    let mut board =
                        Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                    board.land(player, power, amount, Some(player));
                }
                // `CreatureCmd.GainBlock`, unpowered, then
                // `PowerCmd.Remove(this)`.
                (
                    Listener::Power { owner, power, .. },
                    sts2_core::HookEffect::GainBlockAndRemove(amount),
                ) => {
                    live.gain_unpowered_block(owner, amount);
                    live.remove_power(owner, power);
                }
                // `CreatureCmd.GainBlock` of a power's unpowered block on its
                // owner, which runs `Hook.ModifyBlock`.
                (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                    use sts2_core::Fight as _;

                    let ascension = live.ascension;
                    let mut board =
                        Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
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

/// `hook`, `AfterSideTurnEnd` or its late pass, raised on every listener once a
/// side's turn ends.
///
/// Which side's turn ending counts a power down is the power's own rule. What
/// the powers ask to apply comes back, in the order they asked.
///
/// # Panics
///
/// A listener asking for something no turn end carries out.
#[tracing::instrument(level = "trace", skip_all)]
fn tick_powers<V: Rules>(
    live: &mut Live<V>,
    relics: &mut crate::run::Belt<V>,
    skipping: &mut Skips<V>,
    hook: sts2_core::EventHook<V>,
) -> Applying<V> {
    let mut applying = Applying::<V>::new();
    let mut ticking: SmallVec<[(CombatId, V::Power); 8]> = SmallVec::new();
    let mut leaving: SmallVec<[(CombatId, V::Power); 4]> = SmallVec::new();
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        // What a relic asks for, on the player who carries it.
        if let Listener::Relic { .. } = listener {
            applying.push(TurnEndAsk::Relic(effect));
            continue;
        }
        let Listener::Power { owner, power, .. } = listener else {
            unimplemented!("{listener:?} asking for {effect:?} as a turn ends")
        };
        match effect {
            sts2_core::HookEffect::TickDownDuration => ticking.push((owner, power)),
            // `PowerCmd.Apply` on the creature carrying the power, which is
            // what a power that grows its owner asks for.
            sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                applying.push(TurnEndAsk::Apply(owner, power, amount));
            }
            sts2_core::HookEffect::DamageOwnerInFight {
                amount,
                unblockable,
                self_dealt,
            } => applying.push(TurnEndAsk::Damage {
                owner,
                amount,
                unblockable,
                dealer: self_dealt.then_some(owner),
            }),
            // `PowerCmd.Remove(this)`, taken after the ticks for the same
            // reason a power it applies is.
            sts2_core::HookEffect::Remove => leaving.push((owner, power)),
            sts2_core::HookEffect::RemoveFromOwner(power) => leaving.push((owner, power)),
            // `CardPileCmd.Draw` for the player, once the hand is flushed.
            sts2_core::HookEffect::Draw(amount) => applying.push(TurnEndAsk::Draw(amount)),
            // `CreatureCmd.Heal` on the creature carrying the power.
            sts2_core::HookEffect::Heal(amount) => applying.push(TurnEndAsk::Heal(owner, amount)),
            // `PowerCmd.Decrement(this)`.
            sts2_core::HookEffect::Decrement => applying.push(TurnEndAsk::Decrement(owner, power)),
            sts2_core::HookEffect::Escape => applying.push(TurnEndAsk::Escape(owner)),
            sts2_core::HookEffect::EvokeLast(count) => applying.push(TurnEndAsk::EvokeLast(count)),
            other => unimplemented!("a power asking for {other:?} as a turn ends"),
        }
    }

    for (owner, power) in ticking {
        // `TickDownDuration` passes over an instance whose flag is set, and
        // clears the flag rather than counting down twice later.
        if let Some(at) = skipping.iter().position(|held| *held == (owner, power)) {
            skipping.remove(at);
            continue;
        }
        // `PowerCmd.Decrement`, which is `ModifyAmount` by one down.
        let fight = listen::Walk { live, relics };
        let (_, modifiers) =
            super::board::received_by::<V>(fight, owner, power, -num_traits::one::<V::Number>());
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
        if let Some((listener, effect)) = listen::notify::<V>(live, relics, hook).into_iter().next()
        {
            unimplemented!("{listener:?} asking for {effect:?} as {power:?} ticks down");
        }
        // `ShouldRemoveDueToAmount`, and `PowerCmd.Remove`.
        if live
            .powers
            .of(owner)
            .any(|held| held.power() == power && held.amount.spent())
        {
            live.remove_power(owner, power);
        }
    }
    for (owner, power) in leaving {
        live.remove_power(owner, power);
    }
    applying
}

/// `Hook.AfterTurnEnd`: every `AfterSideTurnEnd`, then every
/// `AfterSideTurnEndLate`, and how the fight ended if either ended it.
#[tracing::instrument(level = "trace", skip_all)]
fn after_turn_end<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    skipping: &mut Skips<V>,
    side: V::CombatSide,
) -> Option<super::Ended> {
    let hooks = [
        sts2_core::EventHook::SideTurnEnd {
            side,
            block: live.player().block,
        },
        sts2_core::EventHook::SideTurnEndLate { side },
    ];
    for hook in hooks {
        let asked = tick_powers::<V>(live, relics, skipping, hook);
        if let Some(ended) = applied::<V>(live, rngs.reborrow(), relics, ascension, asked) {
            return Some(ended);
        }
    }
    None
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
    /// `PowerCmd.Decrement`.
    Decrement(CombatId, V::Power),
    /// `CreatureCmd.Escape`.
    Escape(CombatId),
    /// `OrbCmd.EvokeLast`, this many times.
    EvokeLast(V::Number),
    /// What a relic asked for, carried out for the player.
    Relic(sts2_core::HookEffect<V>),
}

/// What the powers that just ticked asked for, and whatever that ended.
///
/// After the tick rather than inside it, because a power applying one is
/// holding the creature it sits on. The order is the belt's, which is the
/// order they were asked in. A fight that ends runs no more of them.
#[tracing::instrument(level = "trace", skip_all)]
fn applied<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    asked: Applying<V>,
) -> Option<super::Ended> {
    for ask in asked {
        let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
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
            TurnEndAsk::Draw(amount) => sts2_core::Fight::draw(&mut board, amount),
            TurnEndAsk::Relic(effect) => super::hook::apply::<V, _>(&mut board, [effect]),
            TurnEndAsk::Heal(at, amount) => sts2_core::Fight::heal(&mut board, at, amount),
            TurnEndAsk::Decrement(at, power) => {
                board.modify_amount(at, power, -num_traits::one::<V::Number>())
            }
            TurnEndAsk::Escape(at) => board.escape_creature(at),
            TurnEndAsk::EvokeLast(count) => {
                for _ in 0..num_traits::cast::<_, usize>(count).unwrap_or(0) {
                    board.evoke_last();
                }
            }
        }
        if let Some(ended) = board.ended() {
            return Some(ended);
        }
    }
    None
}

/// How far an enemy turn got.
enum Acted {
    /// Every enemy acted, or one ended the fight.
    Over(Option<super::Ended>),
    /// A move asked, or a command inside it did, and the rest of the turn
    /// waits in [`Live::deferred`].
    Deferred,
}

/// `ExecuteEnemyTurn`: each enemy in `acting` performs what it advertised.
///
/// Each is held against the fight again before it acts. An earlier move can
/// take a later one out of the fight.
#[tracing::instrument(level = "trace", skip_all)]
fn enemy_turn<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    skipping: &mut Skips<V>,
    acting: &[CombatId],
    max_energy: V::Count,
) -> Acted {
    let mut ended = None;
    for (at, &id) in acting.iter().enumerate() {
        let Some(creature) = live.creatures.iter().find(|c| c.id == id) else {
            continue;
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
            continue;
        };
        let monster = held.monster;
        // `OnMovePerformed`, which is what lets the graph leave this move. It
        // happens beside `PerformMove` rather than after what the move does.
        held.machine.performed();

        let attack = next_move.attack().map(Landing::attack::<V>);
        let damage = attack.map_or_else(num_traits::zero, |attack| attack.damage);
        let repeats = attack.map_or_else(num_traits::zero, |attack| attack.repeats);

        let ascension = live.ascension;
        let mut board = Board::for_move(
            live,
            rngs.reborrow(),
            relics,
            ascension,
            (id, damage, repeats),
        );
        let played = V::perform_move(monster, next_move.id, &mut board);
        ended = ended.or(board.ended());
        super::play::merge::<V>(skipping, std::mem::take(&mut board.skipped));
        match played {
            sts2_core::Played::Done => {}
            sts2_core::Played::Choosing {
                offered,
                skippable,
                at: resume,
            } => {
                if ended.is_some() {
                    unimplemented!("a move asking in a fight it ended");
                }
                // The choice is the move's last command, so the move is over
                // once it is answered.
                live.asking = Some(super::live::Asking {
                    then: Then::Chosen { at: resume },
                    question: super::live::Asked::Offered { offered, skippable },
                });
                live.defer(super::live::Deferred::Move {
                    actor: id,
                    monster,
                    move_id: next_move.id,
                    at: None,
                    damage,
                    repeats,
                    acting: acting[at + 1..].iter().copied().collect(),
                    max_energy,
                });
                return Acted::Deferred;
            }
            sts2_core::Played::Asking { .. } | sts2_core::Played::AskingAmong { .. } => {
                unimplemented!("a monster's move selecting from a pile")
            }
            // Its attack asked, and the move stands still on it with the
            // rest of the turn behind it.
            sts2_core::Played::Waiting { at: resume } => {
                if live.asking.is_none() {
                    unimplemented!("a move waiting on nothing asked");
                }
                live.defer(super::live::Deferred::Move {
                    actor: id,
                    monster,
                    move_id: next_move.id,
                    at: Some(resume),
                    damage,
                    repeats,
                    acting: acting[at + 1..].iter().copied().collect(),
                    max_energy,
                });
                return Acted::Deferred;
            }
        }
        // Its last command, an attack, asked: the move is over, and the turn
        // waits. Only an attack can ask, and the board stops any command that
        // runs on past one that did.
        if live.asking.is_some() {
            live.defer(super::live::Deferred::Move {
                actor: id,
                monster,
                move_id: next_move.id,
                at: None,
                damage,
                repeats,
                acting: acting[at + 1..].iter().copied().collect(),
                max_energy,
            });
            return Acted::Deferred;
        }
        move_performed::<V>(live, id, next_move.id);
        // `CheckWinCondition` after each enemy's turn: an ended fight gives
        // the enemies after it no turn.
        if ended.is_some() {
            break;
        }
    }
    Acted::Over(ended)
}

/// `MonsterMoveState.OnPerform`, which the move state gives the powers on
/// the creature that performed it.
///
/// # Panics
///
/// A power asking for something once the move is performed, which nothing
/// here carries out.
#[tracing::instrument(level = "trace", skip_all)]
fn move_performed<V: Rules>(live: &mut Live<V>, creature: CombatId, move_id: V::MoveId) {
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
    relics: &crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    ai: &mut MegaRng,
) -> Result<(), Error> {
    live.machines
        .retain(|held| live.creatures.iter().any(|c| c.id == held.creature));
    roll_moves_of::<V>(live, relics, ascension, ai, |_| true)
}

/// `MonsterModel.RollMove` for each enemy `which` picks, in fight order.
pub(super) fn roll_moves_of<V: Rules>(
    live: &mut Live<V>,
    relics: &crate::run::Belt<V>,
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
                relics: Some(relics),
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
                    ..held.spawn
                },
                powers,
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
