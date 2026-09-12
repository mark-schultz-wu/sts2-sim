//! Building the fight a run walks into.

use sts2_core::{
    CombatId, Counters, Creature, ModelId, Observation, Rules, SideKinds, TurnPhaseKinds,
};

use super::live::{CardInstance, Live};
use super::{Error, encounter, monster, pile, stream};

/// `CombatManager.baseHandDrawCount`, before a hook moves it.
const HAND_SIZE: usize = 5;

/// Divine Right's three stars.
const DIVINE_RIGHT_STARS: u8 = 3;

/// The fight this run opens on the encounter it just travelled into.
///
/// Takes the run as observed and gives back the fight and the draw counts it
/// consumed, without touching either. Entering a room is one transition, and a
/// fight that cannot be built must leave the run where it stood.
pub(crate) fn enter<V: Rules>(
    raw_seed: u32,
    character: V::Character,
    ascension: V::AscensionLevel,
    encounter: V::Encounter,
    observation: &Observation<V>,
) -> Result<(Live<V>, Counters<V>), Error> {
    let mut counters = observation.rng.clone();

    let monsters = encounter::opening::<V>(raw_seed, observation.floor, encounter)?;
    let enemies =
        monster::initialize::<V>(raw_seed, ascension, encounter, &mut counters, &monsters)?;

    // Id zero, and the enemies from one. `CombatId` counts from the player.
    let mut creatures = Vec::with_capacity(enemies.creatures.len() + 1);
    creatures.push(Creature {
        id: CombatId::new(nonmax::NonMaxU8::ZERO),
        model: Some(ModelId::Character(character)),
        side: <V::CombatSide as SideKinds>::PLAYER,
        hp: observation.hp,
        max_hp: observation.max_hp,
        block: num_traits::zero(),
        alive: observation.alive,
        powers: Vec::new(),
        next_move: None,
    });
    creatures.extend(enemies.creatures);

    // The deck's own cards, each becoming a copy the fight can write on.
    let deck: Vec<CardInstance<V>> = observation
        .deck
        .iter()
        .map(|card| CardInstance::new(*card))
        .collect();
    let mut shuffle = stream::seek(raw_seed, &counters, V::shuffle_rng(), "shuffle")?;
    let piles = pile::shuffle(&mut shuffle, deck).deal(HAND_SIZE);
    stream::record(&mut counters, V::shuffle_rng(), "shuffle", &shuffle)?;

    let energy: V::Count = num_traits::cast(V::max_energy(character))
        .expect("the starting energy fits the build's count type");

    Ok((
        Live {
            ascension,
            turn: num_traits::one(),
            phase: <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
            energy,
            stars: stars_on_entry::<V>(observation),
            round: num_traits::one(),
            encounter,
            hand: piles.hand().to_vec(),
            draw: piles.draw().to_vec(),
            discard: piles.discard().to_vec(),
            exhaust: piles.exhaust().to_vec(),
            creatures,
            // Absent rather than an empty queue: the subsystem is inert until
            // something gives the player a slot.
            orbs: None,
            machines: enemies.machines,
            // Nothing has been applied to anyone yet.
            skip_tick: Vec::new(),
        },
        counters,
    ))
}

/// The stars a fight begins with.
///
/// One relic gives any: Divine Right, the Regent's starter, whose text is "At
/// the start of each combat, gain three stars". Every other source is a card
/// or a power inside the fight.
fn stars_on_entry<V: Rules>(observation: &Observation<V>) -> V::Count {
    let held = observation
        .relics
        .iter()
        .filter(|carried| carried.relic == V::divine_right())
        .count();
    let held: V::Count =
        num_traits::cast(held).expect("the relics held fit the build's count type");
    let each: V::Count =
        num_traits::cast(DIVINE_RIGHT_STARS).expect("a relic's stars fit the build's count type");
    held * each
}
