//! Ending a turn.
//!
//! A port of the run of `CombatManager` methods a turn end sets off, in the
//! order they run. The player's hand is flushed, the enemy side takes its
//! turn, and the player's next turn is set up.

use crate::rng::MegaRng;
use smallvec::SmallVec;
use sts2_core::{CombatId, Pile, PowerAmount, Rules, SideKinds, TurnPhaseKinds};

use super::board::{Board, draw};
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
    if let Some(ended) = turn_end(next, streams.combat(), relics, ascension) {
        next.skip_tick = skipping;
        if ended == super::Ended::Won {
            listen::combat_ended::<V>(next, relics);
        }
        return Ok(Some(ended));
    }
    flush_hand(next, relics);
    end_of_turn_cleanup(next);
    // `AfterSideTurnEnd` for the side that just ended.
    let asked = tick_powers::<V>(
        next,
        relics,
        &mut skipping,
        <V::CombatSide as SideKinds>::PLAYER,
    );
    applied::<V>(next, streams.combat(), relics, ascension, asked);

    // `SwitchSides` to the enemy, then `StartTurn` for it: block clears, and
    // every enemy still in the fight performs the move it advertised.
    //
    // `StartTurn` opens with `SetPhaseForAllPlayers(None)` and puts the phase
    // back only for a turn the player is taking. So the phase a fight ending
    // here reports is the one the enemy turn ran under.
    next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::NONE;
    next.switch_side(<V::CombatSide as SideKinds>::ENEMY);
    clear_block::<V>(next, relics, <V::CombatSide as SideKinds>::ENEMY);
    // `AfterSideTurnStart` for the enemy, which the block clear comes before
    // and the moves come after. A power counting the cards played against its
    // owner forgets the count here.
    let ended = side_turn_started::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::ENEMY,
    )
    .or_else(|| enemy_turn(next, streams.combat(), relics, &mut skipping));
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
    if let Some(ended) = before_turn_end::<V>(
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

    let asked = tick_powers::<V>(
        next,
        relics,
        &mut skipping,
        <V::CombatSide as SideKinds>::ENEMY,
    );
    applied::<V>(next, streams.combat(), relics, ascension, asked);

    // `SwitchSides` back. The round and the turn both count up, because this
    // is not an extra turn.
    next.switch_side(<V::CombatSide as SideKinds>::PLAYER);
    next.round = next.round + num_traits::one();
    next.turn = next.turn + num_traits::one();

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

    // `PrepareForNextTurn`, before the player draws anything.
    roll_moves(next, relics, ascension, streams.combat().ai)?;

    // `AfterTurnStart` on the player, then `SetupPlayerTurn`.
    clear_block::<V>(next, relics, <V::CombatSide as SideKinds>::PLAYER);
    // `ResetEnergy`, to `MaxEnergy`, which runs `Hook.ModifyMaxEnergy`.
    next.energy = super::board::max_energy::<V>(listen::Walk { live: next, relics }, max_energy);
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
    let hook = sts2_core::EventHook::BeforeHandDraw { player };
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
        Carried::Done(None) => None,
        Carried::Parked(asking) => Some(asking),
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
    match parked {
        Some(asking) => next.asking = Some(asking),
        None => next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
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
    let ethereal: SmallVec<[usize; MAX_CARDS_IN_HAND]> = held
        .iter()
        .enumerate()
        .filter(|(_, held)| held.face.ethereal())
        .map(|(at, _)| at)
        .collect();
    for at in ethereal.into_iter().rev() {
        // `CardCmd.Exhaust`, which ethereal causes.
        unheard::<V>(live, relics, sts2_core::ModelHook::AfterCardExhausted);
        live.cards.move_to(Pile::Hand, at, Pile::Exhaust);
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
    if listen::heard::<V>(
        live,
        relics,
        sts2_core::ModelHook::AfterModifyingOrbPassiveTriggerCount,
    ) {
        unimplemented!("a listener changing how often an orb triggers");
    }
    for orb in &held {
        let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
        match at {
            OrbMoment::TurnEnd => V::orb_turn_end(orb, &mut board),
            OrbMoment::TurnStart => V::orb_turn_start(orb, &mut board),
        }
        if let Some(ended) = board.ended {
            return Some(ended);
        }
    }
    None
}

/// `PlayerCombatState.EndOfTurnCleanup`: what a card carried only for the turn.
#[tracing::instrument(level = "trace", skip_all)]
fn end_of_turn_cleanup<V: Rules>(live: &mut Live<V>) {
    live.cards
        .write_all_cards(Face::lasts_the_turn, Face::turn_ended);
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
                if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == owner) {
                    creature.block = creature.block + amount;
                }
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
            (_, effect) => unimplemented!("a listener asking for {effect:?} before a turn starts"),
        }
    }
}

/// `AggressionPower.BeforeSideTurnStart`: the discard pile's attacks, in an
/// unstable shuffle off `CombatCardSelection`, and the first `count` of them
/// into hand. Each is upgraded where an upgrade can still raise it.
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
        if live.cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
            unimplemented!("an attack from the discard pile into a full hand");
        }
        let at = live
            .cards
            .find(Pile::Discard, id)
            .expect("a shuffled attack is still in the discard pile");
        live.cards.move_to(Pile::Discard, at, Pile::Hand);
        let at = live.cards.len(Pile::Hand) - 1;
        live.cards.write(Pile::Hand, at, |held| {
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
    unheard::<V>(
        live,
        relics,
        sts2_core::ModelHook::BeforeSideTurnEndVeryEarly,
    );
    let hook = sts2_core::EventHook::BeforeSideTurnEndEarly { side };
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == owner) {
                    creature.block = creature.block + amount;
                }
            }
            (_, effect) => unimplemented!("a listener asking for {effect:?} before a turn ends"),
        }
    }
    let hook = sts2_core::EventHook::BeforeSideTurnEnd { side };
    let mut ended = None;
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
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
                ended = board.ended;
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
                ended = board.ended;
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
/// ended if it did, or a question it stopped on.
pub(super) enum Carried<V: Rules> {
    Done(Option<super::Ended>),
    Parked(super::live::Asking<V>),
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
) -> Carried<V> {
    let player = live.player().id;
    loop {
        stage = match stage {
            // `Hook.BeforeHandDraw`, once the energy is reset and before the
            // turn's hand is drawn.
            Setup::BeforeHandDraw(mut rest) => {
                while !rest.is_empty() {
                    let (listener, effect) = rest.remove(0);
                    match effect {
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
                            keyword,
                        } => {
                            generate_from_pool::<V>(
                                live,
                                rngs.reborrow(),
                                relics,
                                ascension,
                                filter,
                                count,
                                keyword,
                            );
                        }
                        sts2_core::HookEffect::ShuffleIfNecessary => {
                            if live.cards.is_empty(Pile::Draw) {
                                super::board::shuffle::<V>(live, rngs.shuffle, relics);
                            }
                        }
                        sts2_core::HookEffect::SelectFromPile { pile, count, act } => {
                            if let Some(question) = select::<V>(live, relics, pile, count, act) {
                                return Carried::Parked(super::live::Asking {
                                    then: Then::Setup {
                                        act,
                                        stage: Setup::BeforeHandDraw(rest),
                                    },
                                    question,
                                });
                            }
                        }
                        sts2_core::HookEffect::Remove => {
                            let Listener::Power { owner, power, .. } = listener else {
                                unimplemented!("{listener:?} removing itself before the draw")
                            };
                            live.remove_power(owner, power);
                        }
                        effect => unimplemented!(
                            "a listener asking for {effect:?} before the hand is drawn"
                        ),
                    }
                }
                let drawn = hand_size::<V>(listen::Walk { live, relics }, live.turn);
                // A card auto-played as it is drawn can end the fight, and an
                // ending fight hands the hooks after it no listeners.
                if let Some(ended) = draw(live, rngs.reborrow(), relics, ascension, drawn, true) {
                    return Carried::Done(Some(ended));
                }
                let hook = sts2_core::EventHook::PlayerTurnStart { player };
                Setup::PlayerTurnStart(listen::notify::<V>(live, relics, hook))
            }
            // `Hook.AfterPlayerTurnStart`, the last of the setup. A relic's
            // damage here can end the fight.
            Setup::PlayerTurnStart(mut rest) => {
                let mut ended = None;
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
                            for target in sts2_core::Fight::opponents(&board) {
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
                            ended = ended.or(board.ended);
                        }
                        sts2_core::HookEffect::SelectFromPile { pile, count, act } => {
                            if let Some(question) = select::<V>(live, relics, pile, count, act) {
                                return Carried::Parked(super::live::Asking {
                                    then: Then::Setup {
                                        act,
                                        stage: Setup::PlayerTurnStart(rest),
                                    },
                                    question,
                                });
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
                        // `OrbCmd.Passive` on whichever orb is first each
                        // time, which returns once the fight is ending.
                        sts2_core::HookEffect::FirstOrbPassive(count) => {
                            let count: usize = num_traits::cast(count).unwrap_or_else(|| {
                                unimplemented!("{count:?} orb passives at once")
                            });
                            for _ in 0..count {
                                let Some(first) = live
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
                                V::orb_passive(&first, &mut board);
                                ended = ended.or(board.ended);
                            }
                        }
                        effect => unimplemented!(
                            "a listener asking for {effect:?} as the player's turn starts"
                        ),
                    }
                }
                return Carried::Done(ended);
            }
        };
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
    keyword: Option<V::CardKeyword>,
) {
    use sts2_core::CardTable as _;

    let count: usize = num_traits::cast(count)
        .unwrap_or_else(|| unimplemented!("making {count:?} cards from a pool"));
    let keeps = |card: V::Card| match filter {
        sts2_core::PoolFilter::OfKind(kind) => V::card_kind(card) == Some(kind),
        sts2_core::PoolFilter::NotBasicOrAncient => {
            let rarity = card.rarity();
            rarity != <<V::Card as sts2_core::CardTable>::Rarity as sts2_core::CardRarityKinds>::BASIC
                && rarity
                    != <<V::Card as sts2_core::CardTable>::Rarity as sts2_core::CardRarityKinds>::ANCIENT
        }
    };
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
            if let Some(keyword) = keyword {
                sts2_core::Fight::modify_card(
                    &mut board,
                    at,
                    sts2_core::Change::AddKeyword(keyword),
                    sts2_core::Lasting::ThisFight,
                );
            }
        }
    }
}

/// `CardSelectCmd.FromHand` or `FromCombatPile` for `count` cards of `pile`:
/// the cards taken without asking, acted on now, or the question to stop on.
fn select<V: Rules>(
    live: &mut Live<V>,
    relics: &crate::run::Belt<V>,
    pile: Pile,
    count: V::Number,
    act: sts2_core::PileAct,
) -> Option<super::live::Asked<V>> {
    let count = num_traits::cast(count)
        .and_then(nonmax::NonMaxUsize::new)
        .unwrap_or_else(|| unimplemented!("a selection of {count:?} cards"));
    let selection = sts2_core::Selection::Exactly(count);
    match super::play::taken(live, pile, selection) {
        Some(picked) => {
            act_on::<V>(live, relics, act, picked.as_slice());
            None
        }
        None => Some(super::live::Asked::Pile { pile, selection }),
    }
}

/// Carry `act` out on the cards a selection took, in the order it took them.
pub(super) fn act_on<V: Rules>(
    live: &mut Live<V>,
    relics: &crate::run::Belt<V>,
    act: sts2_core::PileAct,
    picked: &[sts2_core::CardAt<V>],
) {
    for card in picked {
        let from = card.pile();
        let at = live
            .cards
            .find(from, card.id())
            .unwrap_or_else(|| unimplemented!("a picked card that left its pile"));
        let to = match act {
            // `CardCmd.Exhaust`.
            sts2_core::PileAct::Exhaust => {
                unheard::<V>(live, relics, sts2_core::ModelHook::AfterCardExhausted);
                Pile::Exhaust
            }
            // `CardCmd.Discard`, which plays a Sly card once every card is
            // down.
            sts2_core::PileAct::Discard => {
                let held = live
                    .cards
                    .get(from, at)
                    .expect("a card stands where it was found");
                if held.face.sly() {
                    unimplemented!("a Sly card discarded, which then plays itself");
                }
                unheard::<V>(live, relics, sts2_core::ModelHook::AfterCardDiscarded);
                Pile::Discard
            }
            // `CardPileCmd.Add(cards, PileType.Hand)`, which sends a card past
            // a full hand to the discard pile.
            sts2_core::PileAct::IntoHand => match live.cards.len(Pile::Hand) >= MAX_CARDS_IN_HAND {
                true => Pile::Discard,
                false => Pile::Hand,
            },
        };
        live.cards.move_to(from, at, to);
    }
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

/// Answer the question `SetupPlayerTurn` stopped on: `act` on the cards
/// picked, then the rest of the setup from where it stopped.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn resume_setup<V: Rules>(
    fighting: Fighting<'_, V>,
    act: sts2_core::PileAct,
    picked: sts2_core::SelectedCards<V>,
    stage: Setup<V>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    act_on::<V>(live, relics, act, picked.as_slice());
    let ascension = live.ascension;
    match carry_setup(live, streams.combat(), relics, ascension, stage) {
        Carried::Parked(asking) => {
            live.asking = Some(asking);
            Ok(None)
        }
        Carried::Done(ended) => {
            match ended {
                Some(super::Ended::Won) => listen::combat_ended::<V>(live, relics),
                Some(super::Ended::Lost) => {}
                // `RunAutoPrePlayPhase`, once the setup is over.
                None => live.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
            }
            Ok(ended)
        }
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
            // `PowerCmd.Apply<P>(Owner, amount, Owner)`.
            sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                let Listener::Power { owner, .. } = listener else {
                    unimplemented!("{listener:?} applying a power to its owner")
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
                let gained: V::Count = num_traits::cast(amount)
                    .unwrap_or_else(|| unimplemented!("{amount:?} energy at once"));
                live.energy = live.energy + gained;
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
                if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == owner) {
                    creature.block = creature.block + amount * count;
                }
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
                    let damage = super::board::modified::<V>(
                        board.walk(),
                        None,
                        Some(owner),
                        None,
                        stack,
                        false,
                    );
                    board.hit(None, owner, damage, false, true);
                    let alive = board
                        .at(owner)
                        .is_some_and(|at| board.live.creatures[at].alive);
                    if alive {
                        board.modify_amount(owner, power, -num_traits::one::<V::Number>());
                    }
                    ended = ended.or(board.ended);
                }
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
                ended = ended.or(board.ended);
            }
            effect => unimplemented!("a listener asking for {effect:?} as a side turn starts"),
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
fn flush_hand<V: Rules>(live: &mut Live<V>, relics: &crate::run::Belt<V>) {
    let player = live.player().id;
    let fight = listen::Walk { live, relics };
    if listen::any_decides::<V>(
        fight,
        sts2_core::DecisionHook::ShouldFlush { player },
        false,
    ) {
        return;
    }
    live.cards.sift(Pile::Hand, Pile::Discard, Face::retains);
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
    let hook = sts2_core::EventHook::EnergyReset { player };
    let mut ended = None;
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            // `PlayerCmd.GainEnergy`, then `PowerCmd.Remove(this)`.
            (
                Listener::Power { owner, power, .. },
                sts2_core::HookEffect::GainEnergyAndRemove(amount),
            ) => {
                let gained: V::Count = num_traits::cast(amount)
                    .unwrap_or_else(|| unimplemented!("a {power:?} worth {amount:?} of energy"));
                live.energy = live.energy + gained;
                live.remove_power(owner, power);
            }
            // `PlayerCmd.GainStars`.
            (_, sts2_core::HookEffect::GainStars(amount)) => {
                let gained: V::Count = num_traits::cast(amount)
                    .unwrap_or_else(|| unimplemented!("gaining {amount:?} stars"));
                live.stars = live.stars + gained;
            }
            // `PowerCmd.Remove(this)`.
            (Listener::Power { owner, power, .. }, sts2_core::HookEffect::Remove) => {
                live.remove_power(owner, power)
            }
            // `OrbCmd.Channel` on the player, which evokes an orb to make room.
            // The streams are lent only here, which most turns never reach.
            (_, sts2_core::HookEffect::Channel(orb)) => {
                let mut board =
                    Board::for_card(live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::channel(&mut board, orb);
                ended = ended.or(board.ended);
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
    relics: &mut crate::run::Belt<V>,
    side: V::CombatSide,
) {
    let clearing: SmallVec<[CombatId; 8]> = live
        .creatures
        .iter()
        .filter(|c| c.side == side)
        .map(|creature| creature.id)
        .collect();
    for &creature in &clearing {
        // `Hook.ShouldClearBlock`.
        let kept = listen::any_decides::<V>(
            listen::Walk { live, relics },
            sts2_core::DecisionHook::ShouldClearBlock { creature },
            false,
        );
        if !kept && let Some(held) = live.creatures.iter_mut().find(|c| c.id == creature) {
            held.block = num_traits::zero();
        }
    }
    for creature in clearing {
        let hook = sts2_core::EventHook::BlockCleared { creature };
        for (listener, effect) in listen::notify::<V>(live, relics, hook) {
            match (listener, effect) {
                // `CreatureCmd.GainBlock`, unpowered, then
                // `PowerCmd.Remove(this)`.
                (
                    Listener::Power { owner, power, .. },
                    sts2_core::HookEffect::GainBlockAndRemove(amount),
                ) => {
                    if let Some(holder) = live.creatures.iter_mut().find(|c| c.id == owner) {
                        holder.block = holder.block + amount;
                    }
                    live.remove_power(owner, power);
                }
                (_, effect) => {
                    unimplemented!("a listener asking for {effect:?} as block clears")
                }
            }
        }
    }
}

/// `AfterSideTurnEnd`, raised on every listener once this side's turn ends.
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
    side: V::CombatSide,
) -> Applying<V> {
    let mut applying = Applying::<V>::new();
    let mut ticking: SmallVec<[(CombatId, V::Power); 8]> = SmallVec::new();
    let mut leaving: SmallVec<[(CombatId, V::Power); 4]> = SmallVec::new();
    let hook = sts2_core::EventHook::SideTurnEnd { side };
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        let Listener::Power { owner, power, .. } = listener else {
            unimplemented!("{listener:?} asking for {effect:?} as a turn ends")
        };
        match effect {
            sts2_core::HookEffect::TickDownDuration => ticking.push((owner, power)),
            // `PowerCmd.Apply` on the creature carrying the power, which is
            // what a power that grows its owner asks for.
            sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                applying.push((owner, power, amount));
            }
            // `PowerCmd.Remove(this)`, taken after the ticks for the same
            // reason a power it applies is.
            sts2_core::HookEffect::Remove => leaving.push((owner, power)),
            sts2_core::HookEffect::RemoveFromOwner(power) => leaving.push((owner, power)),
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
        for held in live
            .powers
            .of_mut(owner)
            .filter(|held| held.power() == power)
        {
            held.amount = held.amount.ticked();
        }
        live.powers.retain(owner, |held| !held.amount.spent());
    }
    for (owner, power) in leaving {
        live.remove_power(owner, power);
    }
    applying
}

/// The powers a turn end's listeners asked to apply, each beside the creature
/// that asked, inline to one for each creature a fight holds.
type Applying<V> = SmallVec<
    [(
        CombatId,
        <V as sts2_core::ContentVersion>::Power,
        PowerAmount<<V as sts2_core::ContentVersion>::Number>,
    ); CREATURES],
>;

/// `PowerCmd.Apply` for what the powers that just ticked asked for.
///
/// After the tick rather than inside it, because a power applying one is
/// holding the creature it sits on. The order is the belt's, which is the
/// order they were asked in.
#[tracing::instrument(level = "trace", skip_all)]
fn applied<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
    asked: Applying<V>,
) {
    for (at, power, amount) in asked {
        let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
        sts2_core::Fight::apply_power(&mut board, at, power, amount);
    }
}

/// `ExecuteEnemyTurn`: each enemy performs what it advertised.
///
/// The list is taken first, and each is held against the fight again before it
/// acts. An earlier move can take a later one out of the fight.
#[tracing::instrument(level = "trace", skip_all)]
fn enemy_turn<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    skipping: &mut Skips<V>,
) -> Option<super::Ended> {
    let mut ended = None;
    let acting: SmallVec<[CombatId; CREATURES]> = live
        .creatures
        .iter()
        .filter(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY)
        .map(|creature| creature.id)
        .collect();

    for id in acting {
        let Some(creature) = live.creatures.iter().find(|c| c.id == id) else {
            continue;
        };
        let next_move = creature
            .next_move
            .unwrap_or_else(|| unimplemented!("an enemy with no move to make"));
        let held = live
            .machines
            .iter_mut()
            .find(|held| held.creature == id)
            .unwrap_or_else(|| unimplemented!("an enemy with no move graph"));
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
        V::perform_move(monster, next_move.id, &mut board);
        move_performed::<V>(board.live, id, next_move.id);
        ended = ended.or(board.ended);
        super::play::merge::<V>(skipping, std::mem::take(&mut board.skipped));
    }
    ended
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
    for held in live.machines.iter_mut() {
        let rolled = {
            // Field by field, because the machines are borrowed.
            let fight = super::live::Onlookers {
                creatures: &live.creatures,
                powers: &live.powers,
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
                held.spawn,
                powers,
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
