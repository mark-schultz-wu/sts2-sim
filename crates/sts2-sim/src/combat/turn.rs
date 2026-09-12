//! Ending a turn.
//!
//! A port of the run of `CombatManager` methods a turn end sets off, in the
//! order they run. The player's hand is flushed, the enemy side takes its
//! turn, and the player's next turn is set up.
//!
//! The whole of it happens on a copy, so a turn the simulator cannot finish
//! leaves the fight where it stood.

use crate::rng::MegaRng;
use sts2_core::{CardTable, CombatId, Counters, PowerAmount, Rules, SideKinds, TurnPhaseKinds};

use super::board::{Board, draw};
use super::live::Live;
use super::{Error, stream};

/// How many cards a hand is drawn to, which is `baseHandDrawCount`.
const HAND_DRAW: usize = 5;

/// End the player's turn and set up the next one.
pub(crate) fn end<V: Rules>(
    live: &Live<V>,
    raw_seed: u32,
    ascension: V::AscensionLevel,
    counters: &Counters<V>,
    max_energy: V::Count,
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let mut skipping = std::mem::take(&mut next.skip_tick);
    let mut counters = counters.clone();
    let mut shuffle = stream::seek(raw_seed, &counters, V::shuffle_rng(), "shuffle")?;
    let mut ai = stream::seek(raw_seed, &counters, V::monster_ai_rng(), "monster_ai")?;
    let mut niche = stream::seek(raw_seed, &counters, V::niche_rng(), "niche")?;

    turn_end(&mut next);
    flush_hand(&mut next);
    // `AfterSideTurnEnd` for the side that just ended.
    tick_powers::<V>(
        &mut next,
        &mut skipping,
        <V::CombatSide as SideKinds>::PLAYER,
    );

    // `SwitchSides` to the enemy, then `StartTurn` for it: block clears, and
    // every enemy still in the fight performs the move it advertised.
    clear_block::<V>(&mut next, <V::CombatSide as SideKinds>::ENEMY);
    let won = enemy_turn(&mut next, &mut shuffle, &mut niche, &mut skipping);

    tick_powers::<V>(
        &mut next,
        &mut skipping,
        <V::CombatSide as SideKinds>::ENEMY,
    );

    // `SwitchSides` back. The round and the turn both count up, because this
    // is not an extra turn.
    next.round = next.round + num_traits::one();
    next.turn = next.turn + num_traits::one();

    // `PrepareForNextTurn`, before the player draws anything.
    roll_moves(&mut next, ascension, &mut ai)?;

    // `AfterTurnStart` on the player, then `SetupPlayerTurn`.
    clear_block::<V>(&mut next, <V::CombatSide as SideKinds>::PLAYER);
    next.energy = max_energy;
    spend_on_energy_reset(&mut next);
    draw(&mut next, &mut shuffle, HAND_DRAW);

    next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY;

    stream::record(&mut counters, V::shuffle_rng(), "shuffle", &shuffle)?;
    stream::record(&mut counters, V::monster_ai_rng(), "monster_ai", &ai)?;
    stream::record(&mut counters, V::niche_rng(), "niche", &niche)?;
    next.skip_tick = skipping;
    Ok(super::Fought {
        live: next,
        counters,
        won,
    })
}

/// `DoTurnEnd`: the cards in hand that answer to a turn ending.
///
/// An ethereal card is exhausted. A card with a turn-end effect of its own
/// runs it, and that arrives with the port of the card.
fn turn_end<V: Rules>(live: &mut Live<V>) {
    let ethereal: Vec<usize> = live
        .hand
        .iter()
        .enumerate()
        .filter(|(_, held)| held.ident.card.ethereal())
        .map(|(at, _)| at)
        .collect();
    for at in ethereal.into_iter().rev() {
        live.exhaust.push(live.hand.remove(at));
    }
}

/// `FlushPlayerHand`: what is left in hand goes to the discard pile.
///
/// A card that retains stays where it is. The keyword is all this reads:
/// nothing in a fight sets it on a card that does not print it.
fn flush_hand<V: Rules>(live: &mut Live<V>) {
    let mut kept = Vec::new();
    for held in std::mem::take(&mut live.hand) {
        match held.ident.card.retains() {
            true => kept.push(held),
            false => live.discard.push(held),
        }
    }
    live.hand = kept;
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
    live.creatures[at].powers.retain(|held| {
        if !V::spent_on_energy_reset(held.power) {
            return true;
        }
        // An endless instance has no amount to hand over. No power both
        // grants energy and comes without a count.
        let PowerAmount::Counted(amount) = held.amount else {
            unimplemented!("an endless {:?} handing over energy", held.power);
        };
        let amount: V::Count = num_traits::cast(amount)
            .unwrap_or_else(|| unimplemented!("a {:?} worth {amount:?} of energy", held.power));
        gained = gained + amount;
        false
    });
    live.energy = live.energy + gained;
}

/// `ClearBlock` on every creature of one side.
fn clear_block<V: Rules>(live: &mut Live<V>, side: V::CombatSide) {
    for creature in live.creatures.iter_mut().filter(|c| c.side == side) {
        creature.block = num_traits::zero();
    }
}

/// `AfterSideTurnEnd`: the powers that count down when this side's turn ends.
///
/// Every creature in the fight, not only the ones on that side. Which side's
/// turn ending counts a power down is the power's own rule.
fn tick_powers<V: Rules>(
    live: &mut Live<V>,
    skipping: &mut Vec<(CombatId, V::Power)>,
    side: V::CombatSide,
) {
    let one = num_traits::one();
    for creature in &mut live.creatures {
        let id = creature.id;
        for held in &mut creature.powers {
            let power = held.power;
            // An endless power sits out every tick. There is no number to
            // count, which is the whole of what endless means.
            let PowerAmount::Counted(amount) = held.amount else {
                continue;
            };
            if V::side_turn_end(power, side) != sts2_core::SideTurnEnd::TicksDownDuration {
                continue;
            }
            // `TickDownDuration` passes over an instance whose flag is set,
            // and clears the flag rather than counting down twice later.
            if let Some(at) = skipping.iter().position(|held| *held == (id, power)) {
                skipping.remove(at);
                continue;
            }
            held.amount = PowerAmount::Counted(amount - one);
        }
        creature
            .powers
            .retain(|held| held.amount != PowerAmount::Counted(num_traits::zero()));
    }
}

/// `ExecuteEnemyTurn`: each enemy performs what it advertised.
///
/// The list is taken first, and each is held against the fight again before it
/// acts. An earlier move can take a later one out of the fight.
fn enemy_turn<V: Rules>(
    live: &mut Live<V>,
    shuffle: &mut MegaRng,
    niche: &mut MegaRng,
    skipping: &mut Vec<(CombatId, V::Power)>,
) -> bool {
    let mut won = false;
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
        let reviving = live
            .machines
            .iter()
            .find(|held| held.creature == id)
            .is_some_and(|held| held.resume.is_some());
        if reviving && !creature.alive {
            let creature = live
                .creatures
                .iter_mut()
                .find(|creature| creature.id == id)
                .expect("a reviving monster remains in the fight");
            creature.hp = creature.max_hp;
            creature.alive = true;
            continue;
        }
        let next_move = creature
            .next_move
            .clone()
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

        let damage = next_move
            .intents
            .iter()
            .find_map(|intent| intent.attack.map(|attack| attack.damage))
            .unwrap_or_else(num_traits::zero);

        let ascension = live.ascension;
        let mut board = Board::for_move(live, shuffle, niche, ascension, id, damage);
        V::perform_move(monster, next_move.id, &mut board);
        won |= board.won;
        *skipping = super::play::merged::<V>(skipping, std::mem::take(&mut board.skipped));
    }
    won
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
        if let Some(resume) = held.resume.take() {
            if let Some(creature) = live
                .creatures
                .iter_mut()
                .find(|creature| creature.id == held.creature)
            {
                creature.next_move = Some(resume);
            }
            continue;
        }
        let rolled =
            V::roll_monster_move(held.monster, ascension, held.spawn, &mut held.machine, ai)?;
        if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == held.creature) {
            creature.next_move = Some(rolled);
        }
    }
    Ok(())
}
