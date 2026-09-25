//! Ending a turn.
//!
//! A port of the run of `CombatManager` methods a turn end sets off, in the
//! order they run. The player's hand is flushed, the enemy side takes its
//! turn, and the player's next turn is set up.

use crate::rng::MegaRng;
use sts2_core::{CardTable, CombatId, Pile, PowerAmount, Rules, SideKinds, TurnPhaseKinds};

use super::board::{Board, draw};
use super::live::{Face, Landing, Live, Skips};
use super::power;
use super::relic;
use super::start::{hand_size, upgraded};
use super::{Error, Fighting};
use crate::stream::CombatRngs;

/// End the player's turn and set up the next one, on the fight where it
/// stands.
///
/// Whatever the enemy turn ended, if it ended the fight.
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
    power::before_turn_end::<V>(&mut next.creatures, <V::CombatSide as SideKinds>::PLAYER);
    flush_hand(next);
    end_of_turn_cleanup(next);
    // `AfterSideTurnEnd` for the side that just ended.
    let asked = tick_powers::<V>(next, &mut skipping, <V::CombatSide as SideKinds>::PLAYER);
    applied::<V>(next, streams.combat(), relics, ascension, asked);

    // `SwitchSides` to the enemy, then `StartTurn` for it: block clears, and
    // every enemy still in the fight performs the move it advertised.
    //
    // `StartTurn` opens with `SetPhaseForAllPlayers(None)` and puts the phase
    // back only for a turn the player is taking. So the phase a fight ending
    // here reports is the one the enemy turn ran under.
    next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::NONE;
    clear_block::<V>(next, <V::CombatSide as SideKinds>::ENEMY);
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
            relic::combat_ended::<V>(relics);
        }
        return Ok(Some(ended));
    }
    power::before_turn_end::<V>(&mut next.creatures, <V::CombatSide as SideKinds>::ENEMY);

    let asked = tick_powers::<V>(next, &mut skipping, <V::CombatSide as SideKinds>::ENEMY);
    applied::<V>(next, streams.combat(), relics, ascension, asked);

    // `SwitchSides` back. The round and the turn both count up, because this
    // is not an extra turn.
    next.round = next.round + num_traits::one();
    next.turn = next.turn + num_traits::one();

    // `PrepareForNextTurn`, before the player draws anything.
    roll_moves(next, ascension, streams.combat().ai)?;

    // `AfterTurnStart` on the player, then `SetupPlayerTurn`.
    clear_block::<V>(next, <V::CombatSide as SideKinds>::PLAYER);
    next.energy = max_energy;
    spend_on_energy_reset(next);

    // `BeforeSideTurnStart`, which is where a relic counting a turn's plays
    // moves the count on, then the draw that reads it.
    relic::turn_starting::<V>(relics, next.turn);
    power::before_turn_start::<V>(
        &mut next.creatures,
        <V::CombatSide as SideKinds>::PLAYER,
        next.turn,
    );
    let drawn = hand_size::<V>(relics, next.turn);
    draw(next, streams.combat(), relics, ascension, drawn);
    let started = relic::turn_started::<V>(relics, next.turn);
    upgraded::<V>(&mut next.cards, started);
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
fn turn_end<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut [crate::run::HeldRelic<V>],
    ascension: V::AscensionLevel,
) {
    let held: Vec<_> = live.cards.iter(Pile::Hand).collect();
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
    let ethereal: Vec<usize> = live
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
fn end_of_turn_cleanup<V: Rules>(live: &mut Live<V>) {
    live.cards
        .write_all_cards(Face::lasts_the_turn, Face::turn_ended);
}

/// `AfterSideTurnStart`, as the powers of the side taking its turn answer it.
///
/// # Panics
///
/// A power asking for something no side turn start can carry out.
fn side_turn_started<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut [crate::run::HeldRelic<V>],
    ascension: V::AscensionLevel,
    side: V::CombatSide,
) {
    let turn = live.turn;
    let asked = power::notify::<V>(live, sts2_core::EventHook::SideTurnStart { side, turn });
    for effect in asked {
        match effect {
            sts2_core::HookEffect::Forge { card, key, amount } => {
                let mut board =
                    Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
                sts2_core::Fight::forge(&mut board, card, key, amount);
            }
            effect => unimplemented!("a power asking for {effect:?} as a side turn starts"),
        }
    }
}

/// `FlushPlayerHand`: what is left in hand goes to the discard pile.
///
/// A card that retains stays where it is. The keyword is all this reads:
/// nothing in a fight sets it on a card that does not print it.
fn flush_hand<V: Rules>(live: &mut Live<V>) {
    live.cards
        .sift(Pile::Hand, Pile::Discard, |held| held.ident.card.retains());
}

/// `Hook.AfterEnergyReset`: the powers that hand the player energy and go.
///
/// After the reset and before the hand is drawn, which is the order
/// `SetupPlayerTurn` runs them in.
fn spend_on_energy_reset<V: Rules>(live: &mut Live<V>) {
    let player = live.player().id;
    let Some(at) = live.creatures.iter().position(|c| c.id == player) else {
        return;
    };
    let mut gained: V::Count = num_traits::zero();

    let mut answer = sts2_core::HookEffects::new();
    let hook = sts2_core::EventHook::EnergyReset;
    live.creatures[at].powers.retain_mut(|held| {
        if !held.hears(&hook) {
            return true;
        }
        answer.clear();
        <V as sts2_core::Hooks>::event_hook(
            held.event_listener(<V::CombatSide as SideKinds>::PLAYER),
            hook,
            &mut answer,
        );
        let amount = answer.iter().find_map(|effect| match effect {
            sts2_core::HookEffect::GainEnergyAndRemove(amount) => Some(*amount),
            _ => None,
        });
        let Some(amount) = amount else { return true };
        let amount: V::Count = num_traits::cast(amount)
            .unwrap_or_else(|| unimplemented!("a {:?} worth {amount:?} of energy", held.power));
        gained = gained + amount;
        false
    });
    live.energy = live.energy + gained;
}

/// `ClearBlock` on every creature of one side.
fn clear_block<V: Rules>(live: &mut Live<V>, side: V::CombatSide) {
    let mut answer = sts2_core::HookEffects::new();
    for creature in live.creatures.iter_mut().filter(|c| c.side == side) {
        creature.block = num_traits::zero();
        let mut gained: V::Number = num_traits::zero();
        let hook = sts2_core::EventHook::BlockCleared;
        creature.powers.retain_mut(|held| {
            if !held.hears(&hook) {
                return true;
            }
            answer.clear();
            <V as sts2_core::Hooks>::event_hook(held.event_listener(side), hook, &mut answer);
            let block = answer.iter().find_map(|effect| match effect {
                sts2_core::HookEffect::GainBlockAndRemove(amount) => Some(*amount),
                _ => None,
            });
            let Some(block) = block else { return true };
            gained = gained + block;
            false
        });
        creature.block = creature.block + gained;
    }
}

/// `AfterSideTurnEnd`: the powers that count down when this side's turn ends.
///
/// Every creature in the fight, not only the ones on that side. Which side's
/// turn ending counts a power down is the power's own rule.
fn tick_powers<V: Rules>(
    live: &mut Live<V>,
    skipping: &mut Skips<V>,
    side: V::CombatSide,
) -> Vec<(CombatId, V::Power, PowerAmount<V::Number>)> {
    let mut applying = Vec::new();
    let mut leaving: Vec<(CombatId, V::Power)> = Vec::new();
    let mut asked = sts2_core::HookEffects::new();

    for creature in &mut live.creatures {
        let id = creature.id;
        let owner = creature.side;
        for held in &mut creature.powers {
            let power = held.power;
            // An endless power sits out every tick. There is no number to
            // count, which is the whole of what endless means.
            if held.amount.count().is_none() {
                continue;
            }
            let hook = sts2_core::EventHook::SideTurnEnd { side };
            if !held.hears(&hook) {
                continue;
            }
            asked.clear();
            <V as sts2_core::Hooks>::event_hook(held.event_listener(owner), hook, &mut asked);
            for effect in &asked {
                match effect {
                    sts2_core::HookEffect::TickDownDuration | sts2_core::HookEffect::Decrement => {}
                    // `PowerCmd.Apply` on the creature carrying the power,
                    // which is what a power that grows its owner asks for.
                    sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                        applying.push((id, *power, *amount));
                    }
                    // `PowerCmd.Remove(this)`, taken after the belt is read
                    // for the same reason a power it applies is.
                    sts2_core::HookEffect::Remove => leaving.push((id, power)),
                    sts2_core::HookEffect::RemoveFromOwner(power) => {
                        leaving.push((id, *power));
                    }
                    // The number the instance counts on itself, set back to
                    // zero. A power that hands back what it took this turn
                    // asks for this after asking for the handing back.
                    other => unimplemented!("a power asking for {other:?} as a turn ends"),
                }
            }
            if asked.contains(&sts2_core::HookEffect::Decrement) {
                held.amount = held.amount.ticked();
                continue;
            }
            if !asked.contains(&sts2_core::HookEffect::TickDownDuration) {
                continue;
            }
            // `TickDownDuration` passes over an instance whose flag is set,
            // and clears the flag rather than counting down twice later.
            if let Some(at) = skipping.iter().position(|held| *held == (id, power)) {
                skipping.remove(at);
                continue;
            }
            held.amount = held.amount.ticked();
        }
        creature.powers.retain(|held| !held.amount.spent());
    }
    for (id, power) in leaving {
        let Some(creature) = live.creatures.iter_mut().find(|creature| creature.id == id) else {
            continue;
        };
        creature.powers.retain_mut(|held| held.power != power);
    }
    applying
}

/// `PowerCmd.Apply` for what the powers that just ticked asked for.
///
/// After the tick rather than inside it, because a power applying one is
/// holding the creature it sits on. The order is the belt's, which is the
/// order they were asked in.
fn applied<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut [crate::run::HeldRelic<V>],
    ascension: V::AscensionLevel,
    asked: Vec<(CombatId, V::Power, PowerAmount<V::Number>)>,
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
fn enemy_turn<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut [crate::run::HeldRelic<V>],
    skipping: &mut Skips<V>,
) -> Option<super::Ended> {
    let mut ended = None;
    let acting: Vec<CombatId> = live
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
        let asked = power::notify_creature::<V>(
            board.live,
            id,
            sts2_core::EventHook::MonsterMovePerformed {
                move_id: next_move.id,
            },
        );
        if let Some(effect) = asked.first() {
            unimplemented!("a power asking for {effect:?} after a monster move")
        }
        ended = ended.or(board.ended);
        *skipping = super::play::merged::<V>(skipping, std::mem::take(&mut board.skipped));
    }
    ended
}

/// `PrepareForNextTurn`: every enemy still in the fight advertises its next
/// move.
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
            let powers = creature
                .powers
                .iter()
                .map(|power| power.listener(creature.side));
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
            creature.next_move = Some(rolled.into());
        }
    }
    Ok(())
}
