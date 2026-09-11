//! Ending a turn.
//!
//! A port of the run of `CombatManager` methods a turn end sets off, in the
//! order they run. The player's hand is flushed, the enemy side takes its
//! turn, and the player's next turn is set up.
//!
//! The whole of it happens on a copy, so a turn the simulator cannot finish
//! leaves the fight where it stood.

use sts2_core::rng::MegaRng;
use sts2_core::{
    CardTable, Combat, CombatId, Counters, PowerAmount, Rules, SideKinds, TurnPhaseKinds,
};

use super::board::{Board, draw};
use super::monster::Machine;
use super::{Error, Ongoing, legal, stream};

/// How many cards a hand is drawn to, which is `baseHandDrawCount`.
const HAND_DRAW: usize = 5;

/// End the player's turn and set up the next one.
pub(crate) fn end<V: Rules>(
    ongoing: &Ongoing<V>,
    raw_seed: u32,
    ascension: V::AscensionLevel,
    counters: &Counters<V>,
    max_energy: V::Count,
) -> Result<super::Fought<V>, Error> {
    let mut next = ongoing.combat.clone();
    let mut machines = ongoing.machines.clone();
    let mut skipping = ongoing.skipping.clone();
    let mut counters = counters.clone();
    let mut shuffle = stream::seek(raw_seed, &counters, V::shuffle_rng(), "shuffle")?;
    let mut ai = stream::seek(raw_seed, &counters, V::monster_ai_rng(), "monster_ai")?;

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
    let won = enemy_turn(&mut next, &mut shuffle, &mut machines, &mut skipping);

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
    roll_moves(&mut next, &mut machines, ascension, &mut ai)?;

    // `AfterTurnStart` on the player, then `SetupPlayerTurn`.
    clear_block::<V>(&mut next, <V::CombatSide as SideKinds>::PLAYER);
    next.energy = max_energy;
    draw(&mut next, &mut shuffle, HAND_DRAW);

    next.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY;
    next.legal = legal::hand::<V>(&next.hand, &next.creatures, next.energy, next.stars)?;

    stream::record(&mut counters, V::shuffle_rng(), "shuffle", &shuffle)?;
    stream::record(&mut counters, V::monster_ai_rng(), "monster_ai", &ai)?;
    Ok(super::Fought {
        ongoing: Ongoing {
            combat: next,
            machines,
            skipping,
        },
        counters,
        won,
    })
}

/// `DoTurnEnd`: the cards in hand that answer to a turn ending.
///
/// An ethereal card is exhausted. A card with a turn-end effect of its own
/// runs it, and that arrives with the port of the card.
fn turn_end<V: Rules>(combat: &mut Combat<V>) {
    let ethereal: Vec<usize> = combat
        .hand
        .iter()
        .enumerate()
        .filter(|(_, held)| held.card.ethereal())
        .map(|(at, _)| at)
        .collect();
    for at in ethereal.into_iter().rev() {
        combat.exhaust.push(combat.hand.remove(at));
    }
}

/// `FlushPlayerHand`: what is left in hand goes to the discard pile.
///
/// Nothing retains. No card in this build's table carries the keyword, and
/// nothing in a fight sets it yet.
fn flush_hand<V: Rules>(combat: &mut Combat<V>) {
    let flushed = std::mem::take(&mut combat.hand);
    combat.discard.extend(flushed);
}

/// `ClearBlock` on every creature of one side.
fn clear_block<V: Rules>(combat: &mut Combat<V>, side: V::CombatSide) {
    for creature in combat.creatures.iter_mut().filter(|c| c.side == side) {
        creature.block = num_traits::zero();
    }
}

/// `AfterSideTurnEnd`: the powers that count down when this side's turn ends.
///
/// Every creature in the fight, not only the ones on that side. Which side's
/// turn ending counts a power down is the power's own rule.
fn tick_powers<V: Rules>(
    combat: &mut Combat<V>,
    skipping: &mut Vec<(CombatId, V::Power)>,
    side: V::CombatSide,
) {
    let one = num_traits::one();
    for creature in &mut combat.creatures {
        let id = creature.id;
        for held in &mut creature.powers {
            let power = held.power;
            // An endless power sits out every tick. There is no number to
            // count, which is the whole of what endless means.
            let PowerAmount::Counted(amount) = held.amount else {
                continue;
            };
            if !V::ticks_down(power, side) {
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
    combat: &mut Combat<V>,
    shuffle: &mut MegaRng,
    machines: &mut [Machine<V>],
    skipping: &mut Vec<(CombatId, V::Power)>,
) -> bool {
    let mut won = false;
    let acting: Vec<CombatId> = combat
        .creatures
        .iter()
        .filter(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY)
        .map(|creature| creature.id)
        .collect();

    for id in acting {
        let Some(creature) = combat.creatures.iter().find(|c| c.id == id) else {
            continue;
        };
        let next_move = creature
            .next_move
            .clone()
            .unwrap_or_else(|| unimplemented!("an enemy with no move to make"));
        let held = machines
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

        let mut board = Board::for_move(combat, shuffle, id, damage);
        V::perform_move(monster, next_move.id, &mut board);
        won |= board.won;
        *skipping = super::play::merged::<V>(skipping, std::mem::take(&mut board.skipped));
    }
    won
}

/// `PrepareForNextTurn`: every enemy still in the fight advertises its next
/// move.
fn roll_moves<V: Rules>(
    combat: &mut Combat<V>,
    machines: &mut Vec<Machine<V>>,
    ascension: V::AscensionLevel,
    ai: &mut MegaRng,
) -> Result<(), Error> {
    machines.retain(|held| combat.creatures.iter().any(|c| c.id == held.creature));
    for held in machines.iter_mut() {
        let rolled = V::roll_monster_move(held.monster, ascension, &mut held.machine, ai)?;
        if let Some(creature) = combat.creatures.iter_mut().find(|c| c.id == held.creature) {
            creature.next_move = Some(rolled);
        }
    }
    Ok(())
}
