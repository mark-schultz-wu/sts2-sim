//! Building the fight a run walks into.

use sts2_core::{
    CombatId, Counters, Creature, EventHook, HookEffect, ModelId, Observation, Rules, SideKinds,
    TurnPhaseKinds,
};

use super::live::{CardInstance, Live};
use super::{Error, encounter, monster, pile, stream};
use crate::run::HeldRelic;

/// `CombatManager.baseHandDrawCount`, before a hook moves it.
const HAND_SIZE: usize = 5;

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
    relics: &mut [HeldRelic<V>],
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

    let mut hand = piles.hand().to_vec();
    let opened = opening_hooks::<V>(relics);
    if opened.upgrade_hand {
        for card in &mut hand {
            card.upgrade();
        }
    }

    let energy: V::Count = num_traits::cast(V::max_energy(character))
        .expect("the starting energy fits the build's count type");

    Ok((
        Live {
            ascension,
            turn: num_traits::one(),
            phase: <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
            energy,
            stars: opened.stars,
            round: num_traits::one(),
            encounter,
            hand,
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

/// What the relics a run carries do as it walks into a fight.
///
/// Two hooks, in the order the game runs them: `AfterRoomEntered` as the room
/// opens, then `AfterSideTurnStart` for the player's first turn. The fight is
/// not built yet, so that turn is the one [`Live`] opens on.
///
/// A relic that acts writes its own counter back. That is why this takes the
/// run's relics and not the observation's.
fn opening_hooks<V: Rules>(relics: &mut [HeldRelic<V>]) -> Opened<V> {
    let mut opened = Opened {
        stars: num_traits::zero(),
        upgrade_hand: false,
    };
    for held in relics {
        let hooks = [
            EventHook::CombatRoomEntered,
            EventHook::SideTurnStart {
                side: <V::CombatSide as SideKinds>::PLAYER,
                turn: num_traits::one(),
                counter: held.counter,
            },
        ];
        for effect in hooks
            .into_iter()
            .flat_map(|hook| <V as sts2_core::Hooks>::event_hook(ModelId::Relic(held.relic), hook))
        {
            match effect {
                HookEffect::GainStars(stars) => {
                    let stars: V::Count = num_traits::cast(stars).unwrap_or_else(|| {
                        unimplemented!("a {:?} worth {stars:?} stars", held.relic)
                    });
                    opened.stars = opened.stars + stars;
                }
                HookEffect::UpgradeHand => opened.upgrade_hand = true,
                HookEffect::SetCounter(counter) => held.counter = Some(counter),
                other => unimplemented!("a relic asking for {other:?} as a fight opens"),
            }
        }
    }
    opened
}

/// What the opening hooks did, as the fight being built reads it.
struct Opened<V: Rules> {
    /// `PlayerCmd.GainStars`, summed over the relics that gave any.
    stars: V::Count,
    /// Whether the opening hand is upgraded before the player sees it.
    upgrade_hand: bool,
}
