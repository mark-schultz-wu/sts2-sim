//! Building the fight a run walks into.

use sts2_core::{
    CombatId, Creature, EventHook, HookEffect, ModelId, ModifyHook, Observation, Rules,
    RunStreamKinds, SideKinds, TurnPhaseKinds,
};

use super::live::{CardIds, CardInstance, Live};
use super::{Error, encounter, monster, pile, relic};
use crate::run::HeldRelic;
use crate::stream::Streams;

/// `CombatManager.baseHandDrawCount`, before a hook moves it.
const HAND_SIZE: usize = 5;

/// The fight this run opens on the encounter it just travelled into.
///
/// Takes the run as observed and gives back the fight and the streams it drew
/// from, without touching either. Entering a room is one transition, and a
/// fight that cannot be built must leave the run where it stood.
pub(crate) fn enter<V: Rules>(
    raw_seed: u64,
    character: V::Character,
    ascension: V::AscensionLevel,
    encounter: V::Encounter,
    observation: &Observation<V>,
    streams: &Streams<V>,
    relics: &mut [HeldRelic<V>],
) -> Result<(Live<V>, Streams<V>), Error> {
    let mut streams = streams.clone();

    let monsters = encounter::opening::<V>(raw_seed, observation.floor, encounter)?;
    let enemies = monster::initialize::<V>(ascension, encounter, &mut streams, &monsters)?;

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
    let mut card_ids = CardIds::<V>::new();
    let deck: Vec<CardInstance<V>> = observation
        .deck
        .iter()
        .map(|card| CardInstance::new(*card, card_ids.mint()))
        .collect();
    // `AfterRoomEntered` as the room opens, then `BeforeSideTurnStart` for the
    // turn the fight is about to begin on. Both run before anything is drawn.
    let turn: V::Count = num_traits::one();
    let mut stars: V::Count = num_traits::zero();
    for effect in relic::notify::<V>(relics, EventHook::CombatRoomEntered) {
        match effect {
            HookEffect::GainStars(gained) => stars = stars + count::<V>(gained),
            other => unimplemented!("a relic asking for {other:?} as a room opens"),
        }
    }
    // `BeforeCombatStart`, after the room and before the fight opens. The
    // player carries no power yet, so block gained here is what was asked for
    // and passes no modifier.
    for effect in relic::combat_starting::<V>(relics) {
        match effect {
            HookEffect::GainBlock(block) => creatures[0].block = creatures[0].block + block,
            other => unimplemented!("a relic asking for {other:?} as a fight starts"),
        }
    }
    relic::turn_starting::<V>(relics, turn);

    let mut shuffle = streams.stream(V::RunRngType::SHUFFLE);
    let drawn = hand_size::<V>(relics, turn);
    let piles = pile::shuffle(&mut shuffle, deck).deal(drawn);
    streams.record(V::RunRngType::SHUFFLE, shuffle);

    let mut hand = piles.hand().to_vec();
    upgraded::<V>(&mut hand, relic::turn_started::<V>(relics, turn));

    let energy: V::Count = num_traits::cast(V::max_energy(character))
        .expect("the starting energy fits the build's count type");

    Ok((
        Live {
            ascension,
            turn: num_traits::one(),
            phase: <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
            energy,
            stars,
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
            card_ids,
            asking: None,
        },
        streams,
    ))
}

/// How many cards the fight's first hand is drawn to.
///
/// `ModifyHandDraw` over the belt, from `baseHandDrawCount`. The relics that
/// answer it read the turn, and the first turn of a fight follows no other.
pub(super) fn hand_size<V: Rules>(relics: &mut [HeldRelic<V>], turn: V::Count) -> usize {
    let base: V::Number =
        num_traits::cast(HAND_SIZE).expect("the printed hand size fits the build's game number");
    let drawn = relic::modified::<V>(relics, ModifyHook::HandDraw { count: base, turn }, base);
    num_traits::cast(drawn).unwrap_or_else(|| unimplemented!("a hand of {drawn:?} cards"))
}

/// One number a relic gave, as the count the fight keeps it in.
fn count<V: Rules>(amount: V::Number) -> V::Count {
    num_traits::cast(amount).unwrap_or_else(|| unimplemented!("a relic worth {amount:?}"))
}

/// Apply what the relics asked for once a hand was dealt to them.
pub(super) fn upgraded<V: Rules>(hand: &mut [CardInstance<V>], effects: Vec<HookEffect<V>>) {
    for effect in effects {
        match effect {
            HookEffect::UpgradeHand => {
                for card in hand.iter_mut() {
                    card.upgrade();
                }
            }
            other => unimplemented!("a relic asking for {other:?} as a turn starts"),
        }
    }
}
