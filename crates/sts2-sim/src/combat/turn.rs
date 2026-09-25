//! Ending a turn.
//!
//! A port of the run of `CombatManager` methods a turn end sets off, in the
//! order they run. The player's hand is flushed, the enemy side takes its
//! turn, and the player's next turn is set up.

use crate::rng::MegaRng;
use smallvec::SmallVec;
use sts2_core::{CardTable, CombatId, Pile, PowerAmount, Rules, SideKinds, TurnPhaseKinds};

use super::board::{Board, draw};
use super::listen::{self, Listener};
use super::live::{CREATURES, Face, Landing, Live, Skips};
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

    turn_end(next, streams.combat(), relics, ascension);
    before_turn_end::<V>(next, relics, <V::CombatSide as SideKinds>::PLAYER);
    flush_hand(next);
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
    clear_block::<V>(next, relics, <V::CombatSide as SideKinds>::ENEMY);
    // `AfterSideTurnStart` for the enemy, which the block clear comes before
    // and the moves come after. A power counting the cards played against its
    // owner forgets the count here.
    side_turn_started::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::ENEMY,
    );
    let ended = enemy_turn(next, streams.combat(), relics, &mut skipping);
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
    before_turn_end::<V>(next, relics, <V::CombatSide as SideKinds>::ENEMY);

    let asked = tick_powers::<V>(
        next,
        relics,
        &mut skipping,
        <V::CombatSide as SideKinds>::ENEMY,
    );
    applied::<V>(next, streams.combat(), relics, ascension, asked);

    // `SwitchSides` back. The round and the turn both count up, because this
    // is not an extra turn.
    next.round = next.round + num_traits::one();
    next.turn = next.turn + num_traits::one();

    // `PrepareForNextTurn`, before the player draws anything.
    roll_moves(next, ascension, streams.combat().ai)?;

    // `AfterTurnStart` on the player, then `SetupPlayerTurn`.
    clear_block::<V>(next, relics, <V::CombatSide as SideKinds>::PLAYER);
    next.energy = max_energy;
    energy_reset::<V>(next, relics);

    // `BeforeSideTurnStart`, which is where a relic counting a turn's plays
    // moves the count on, then the draw that reads it.
    before_side_turn_start::<V>(
        next,
        relics,
        <V::CombatSide as SideKinds>::PLAYER,
        next.turn,
    );
    let drawn = hand_size::<V>(listen::Walk { live: next, relics }, next.turn);
    draw(next, streams.combat(), relics, ascension, drawn);
    side_turn_started::<V>(
        next,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
    );

    next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY;

    next.skip_tick = skipping;
    Ok(None)
}

/// `DoTurnEnd`: the cards in hand that answer to a turn ending.
///
/// An ethereal card is exhausted. A card with a turn-end effect of its own
/// runs it, and that arrives with the port of the card.
#[tracing::instrument(level = "trace", skip_all)]
fn turn_end<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    ascension: V::AscensionLevel,
) {
    let held: SmallVec<[_; MAX_CARDS_IN_HAND]> = live.cards.iter(Pile::Hand).collect();
    for card in &held {
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
    let ethereal: SmallVec<[usize; MAX_CARDS_IN_HAND]> = live
        .cards
        .idents(Pile::Hand)
        .enumerate()
        .filter(|(_, held)| held.card.ethereal())
        .map(|(at, _)| at)
        .collect();
    for at in ethereal.into_iter().rev() {
        live.cards.move_to(Pile::Hand, at, Pile::Exhaust);
    }
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
    relics: &mut crate::run::Belt<V>,
    side: V::CombatSide,
    turn: V::Count,
) {
    let hook = sts2_core::EventHook::BeforeSideTurnStart { side, turn };
    for (listener, effect) in listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            (Listener::Power { owner, .. }, sts2_core::HookEffect::GainBlock(amount)) => {
                if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == owner) {
                    creature.block = creature.block + amount;
                }
            }
            (_, effect) => unimplemented!("a listener asking for {effect:?} before a turn starts"),
        }
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
    relics: &mut crate::run::Belt<V>,
    side: V::CombatSide,
) {
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
) {
    let turn = live.turn;
    let hook = sts2_core::EventHook::SideTurnStart { side, turn };
    for (_, effect) in listen::notify::<V>(live, relics, hook) {
        match effect {
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
            effect => unimplemented!("a listener asking for {effect:?} as a side turn starts"),
        }
    }
}

/// `FlushPlayerHand`: what is left in hand goes to the discard pile.
///
/// A card that retains stays where it is. The keyword is all this reads:
/// nothing in a fight sets it on a card that does not print it.
#[tracing::instrument(level = "trace", skip_all)]
fn flush_hand<V: Rules>(live: &mut Live<V>) {
    live.cards
        .sift(Pile::Hand, Pile::Discard, |held| held.ident.card.retains());
}

/// `Hook.AfterEnergyReset`, raised on every listener.
///
/// After the reset and before the hand is drawn, which is the order
/// `SetupPlayerTurn` runs them in.
///
/// # Panics
///
/// A listener asking for something no energy reset carries out.
#[tracing::instrument(level = "trace", skip_all)]
fn energy_reset<V: Rules>(live: &mut Live<V>, relics: &mut crate::run::Belt<V>) {
    let player = live.player().id;
    let hook = sts2_core::EventHook::EnergyReset { player };
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
            (_, effect) => unimplemented!("a listener asking for {effect:?} as energy resets"),
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
    let Some(carrier) = live
        .creatures
        .iter()
        .find(|c| c.id == creature)
        .map(|performer| performer.carrier())
    else {
        return;
    };
    for held in live.powers.of_mut(creature) {
        <V as sts2_core::Hooks>::power_event_hook(held.event_power(carrier), hook, &mut asked);
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
    ascension: V::AscensionLevel,
    ai: &mut MegaRng,
) -> Result<(), Error> {
    live.machines
        .retain(|held| live.creatures.iter().any(|c| c.id == held.creature));
    for held in live.machines.iter_mut() {
        let rolled = {
            let creature = live
                .creatures
                .iter()
                .find(|creature| creature.id == held.creature)
                .expect("a retained move machine has its creature");
            let powers = live
                .powers
                .of(creature.id)
                .map(|power| power.listener(creature.carrier()));
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
