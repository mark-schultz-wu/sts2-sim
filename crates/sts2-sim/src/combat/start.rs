//! Building the fight a run walks into.

use sts2_core::{
    CardIdent, CombatId, EventHook, HookEffect, ModelId, ModifyHook, Rules, SideKinds,
    TurnPhaseKinds,
};

use super::board::Board;
use super::cards::Cards;
use super::listen::Walk;
use super::live::{CardIds, CardInstance, Fighter, Live};
use super::powers::{PowerStore, Powers as _};
use super::{Error, encounter, monster, pile};
use crate::bounded::Bounded;
use crate::run::Belt;
use crate::stream::Streams;

/// `CombatManager.baseHandDrawCount`, before a hook moves it.
const HAND_SIZE: usize = 5;

/// The run as it stands in the door of a fight.
///
/// What a fight is built from, and no more. The observation carried this here
/// once, which cost a whole report for five of its fields.
pub(crate) struct Entering<'a, V: Rules> {
    /// The floor the fight is fought on, which the run has already stepped up
    /// to. The encounter is rolled against it.
    pub(crate) floor: V::Count,
    pub(crate) hp: V::Health,
    pub(crate) max_hp: V::Health,
    pub(crate) alive: bool,
    pub(crate) deck: &'a [CardIdent<V>],
    /// Whether the run reached this room off the map's lines.
    pub(crate) strayed: bool,
}

/// A fight as it opens, with the streams it drew from.
pub(crate) struct Entered<V: Rules> {
    pub(crate) live: Box<Live<V>>,
    pub(crate) streams: Streams<V>,
    /// How the fight ended before the player took a turn in it, if it did.
    pub(crate) ended: Option<super::Ended>,
}

/// The fight this run opens on the encounter it just travelled into.
///
/// Takes the run as it stands and gives back the fight and the streams it drew
/// from, without touching either. Entering a room is one transition, and a
/// fight that cannot be built must leave the run where it stood.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn enter<V: Rules>(
    raw_seed: u64,
    character: V::Character,
    ascension: V::AscensionLevel,
    encounter: V::Encounter,
    entering: &Entering<'_, V>,
    streams: &Streams<V>,
    relics: &mut Belt<V>,
) -> Result<Entered<V>, Error> {
    let mut streams = *streams;
    let energy: V::Count = num_traits::cast(V::max_energy(character))
        .expect("the starting energy fits the build's count type");

    // Built where it will live, and filled in place.
    let mut live = Box::new(Live {
        character,
        ascension,
        turn: num_traits::one(),
        phase: <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
        energy,
        stars: num_traits::zero(),
        round: num_traits::one(),
        side: <V::CombatSide as SideKinds>::PLAYER,
        plays: super::live::PlayHistory::new(),
        encounter,
        cards: Cards::with_cards(0),
        removed_from_deck: Vec::new(),
        returned: Vec::new(),
        leaving: Vec::new(),
        creatures: super::live::Creatures::new(),
        powers: PowerStore::new(),
        // Absent rather than an empty queue: the subsystem is inert until
        // something gives the player a slot.
        // `PlayerCombatState` opens on `BaseOrbSlotCount` empty slots. Absent
        // rather than empty for a character with none: the subsystem is inert
        // until something gives the player a slot.
        orbs: match V::orb_slots(character) {
            0 => None,
            slots => Some(sts2_core::Orbs {
                capacity: num_traits::cast(slots).expect("a slot count fits the build's count"),
                queue: Vec::new(),
            }),
        },
        machines: Bounded::new(),
        // Nothing has been applied to anyone yet.
        skip_tick: Bounded::new(),
        card_ids: CardIds::new(),
        attacks: 0,
        asking: None,
    });

    // Id zero, and the enemies from one. `CombatId` counts from the player.
    live.creatures.push(Fighter {
        id: CombatId::new(nonmax::NonMaxU8::ZERO),
        model: Some(ModelId::Character(character)),
        side: <V::CombatSide as SideKinds>::PLAYER,
        hp: entering.hp,
        max_hp: entering.max_hp,
        block: num_traits::zero(),
        alive: entering.alive,
        next_move: None,
        owner: None,
    });
    let monsters = encounter::opening::<V>(raw_seed, entering.floor, encounter)?;
    let Live {
        creatures,
        powers,
        machines,
        ..
    } = &mut *live;
    monster::initialize::<V>(
        ascension,
        encounter,
        &mut streams,
        &monsters,
        creatures,
        powers,
        machines,
    )?;

    // The deck's own cards, each becoming a copy the fight can write on.
    let deck: Vec<CardInstance<V>> = entering
        .deck
        .iter()
        .map(|card| CardInstance::new(*card, live.card_ids.mint()))
        .collect();
    // `AfterRoomEntered` as the room opens, then `BeforeSideTurnStart` for the
    // turn the fight is about to begin on. Both run before anything is drawn.
    let turn = live.turn;
    let entered = EventHook::RoomEntered {
        room: live.room(),
        strayed: entering.strayed,
    };
    for (_, effect) in super::listen::notify::<V>(&mut live, relics, entered) {
        match effect {
            HookEffect::GainStars(gained) => live.stars = live.stars + count::<V>(gained),
            // `PowerCmd.Apply` on the relic's owner, which is the player.
            HookEffect::ApplyToOwner(power, amount) => {
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::apply_to_actor(&mut board, power, amount);
            }
            other => unimplemented!("a listener asking for {other:?} as a room opens"),
        }
    }
    // `BeforeCombatStart`, after the room and before the fight opens. The
    // player carries no power yet, so block gained here is what was asked for
    // and passes no modifier.
    for (listener, effect) in
        super::listen::notify::<V>(&mut live, relics, EventHook::CombatStarting)
    {
        match (listener, effect) {
            // On the creature carrying the listener, which for a relic is the
            // player.
            (listener, HookEffect::GainBlock(block)) => {
                let owner = match listener {
                    super::listen::Listener::Power { owner, .. } => owner,
                    _ => live.player().id,
                };
                if let Some(creature) = live.creatures.iter_mut().find(|c| c.id == owner) {
                    creature.block = creature.block + block;
                }
            }
            (_, HookEffect::Summon(amount)) => {
                super::board::summon_osty::<V>(
                    &mut live,
                    streams.combat(),
                    relics,
                    ascension,
                    amount,
                );
            }
            (_, other) => unimplemented!("a listener asking for {other:?} as a fight starts"),
        }
    }
    // `BeforeSideTurnStart`, which is where a monster that opens the fight
    // behind block gains it.
    super::turn::before_side_turn_start::<V>(
        &mut live,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
        turn,
    );
    // `SetupPlayerTurn` resets the energy, which the fight opened on already,
    // and raises the hooks that follow.
    super::turn::energy_reset::<V>(&mut live, relics);
    super::turn::energy_reset_late::<V>(&mut live, streams.combat(), relics, ascension);
    // `BeforeHandDraw`. The opening hand is dealt below rather than drawn onto
    // what is already there, so nothing may be made in hand here.
    let player = live.player().id;
    let hook = sts2_core::EventHook::BeforeHandDraw { player };
    if let Some((listener, effect)) = super::listen::notify::<V>(&mut live, relics, hook).first() {
        unimplemented!("{listener:?} asking for {effect:?} before the opening hand")
    }

    let drawn = hand_size::<V>(
        Walk {
            live: &live,
            relics,
        },
        turn,
    );
    let piles = pile::shuffle(streams.combat().shuffle, deck).deal_opening(drawn);

    live.cards = Cards::dealt(piles.hand(), piles.draw());
    // `SetupPlayerTurn` ends on `AfterPlayerTurnStart`, which can end the
    // fight before the player takes a turn in it.
    let hook = sts2_core::EventHook::PlayerTurnStart { player };
    let asked = super::listen::notify::<V>(&mut live, relics, hook);
    let ended = match super::turn::carry_setup(
        &mut live,
        streams.combat(),
        relics,
        ascension,
        super::live::Setup::PlayerTurnStart(asked),
    ) {
        super::turn::Carried::Done(ended) => ended,
        super::turn::Carried::Parked(_) => {
            unimplemented!("a choice as the fight opens")
        }
    };
    if let Some(ended) = ended {
        if ended == super::Ended::Won {
            super::listen::combat_ended::<V>(&mut live, relics);
        }
        return Ok(Entered {
            live,
            streams,
            ended: Some(ended),
        });
    }
    super::turn::side_turn_started::<V>(
        &mut live,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
    );
    // `OrbQueue.AfterTurnStart`, which a fight's opening turn runs too.
    let ended = super::turn::orb_triggers::<V>(
        &mut live,
        streams.combat(),
        relics,
        ascension,
        super::turn::OrbMoment::TurnStart,
    );
    if ended == Some(super::Ended::Won) {
        super::listen::combat_ended::<V>(&mut live, relics);
    }

    Ok(Entered {
        live,
        streams,
        ended,
    })
}

/// How many cards a turn's hand is drawn to.
///
/// `Hook.ModifyHandDraw` from `baseHandDrawCount`, then its late phase. The
/// listeners that answer it read the turn, and the first turn of a fight
/// follows no other.
pub(super) fn hand_size<V: Rules>(fight: Walk<'_, V>, turn: V::Count) -> usize {
    let base: V::Number =
        num_traits::cast(HAND_SIZE).expect("the printed hand size fits the build's game number");
    let player = fight.live.player().id;
    let room = fight.live.room();
    let phases = [
        ModifyHook::HandDraw {
            player,
            count: base,
            turn,
            room,
        },
        ModifyHook::HandDrawLate {
            player,
            count: base,
            turn,
            room,
        },
    ];
    let drawn = super::board::modified_by::<V>(fight, base, &phases);
    num_traits::cast(drawn).unwrap_or_else(|| unimplemented!("a hand of {drawn:?} cards"))
}

/// One number a relic gave, as the count the fight keeps it in.
fn count<V: Rules>(amount: V::Number) -> V::Count {
    num_traits::cast(amount).unwrap_or_else(|| unimplemented!("a relic worth {amount:?}"))
}
