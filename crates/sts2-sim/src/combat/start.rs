//! Building the fight a run walks into.

use sts2_core::{
    CombatId, EventHook, HookEffect, ModelId, ModifyHook, Rules, SideKinds, TurnPhaseKinds,
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
    pub(crate) gold: V::Number,
    pub(crate) alive: bool,
    pub(crate) deck: &'a [sts2_core::DeckCard<V>],
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
        gold: entering.gold,
        round: num_traits::one(),
        side: <V::CombatSide as SideKinds>::PLAYER,
        history: super::live::CombatHistory::new(),
        encounter,
        cards: Cards::with_cards(0),
        removed_from_deck: Vec::new(),
        returned: Vec::new(),
        exhaust_next: smallvec::SmallVec::new(),
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
        .map(|card| {
            let mut copy = CardInstance::new(card.ident, live.card_ids.mint());
            if let Some(change) = sts2_core::CardModel::written(&card.state) {
                copy.face.modify(change, sts2_core::Lasting::ThisFight);
            }
            copy
        })
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
            HookEffect::GainStars(gained) => live.modify_stars(gained),
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
    //
    // The game's piles hold the deck by now, and the ones here are dealt
    // below. So an affliction asked for here lands once they are, before
    // anything draws.
    let mut afflicting: Vec<HookEffect<V>> = Vec::new();
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
                live.gain_unpowered_block(owner, block);
            }
            // `PlayerCmd.AddPet`.
            (_, HookEffect::AddPet(monster)) => {
                let mut board = super::board::Board::for_card(
                    &mut live,
                    streams.combat(),
                    relics,
                    ascension,
                    None,
                    None,
                );
                board.add_pet(monster);
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
            (_, afflict @ HookEffect::Afflict { .. }) => afflicting.push(afflict),
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
    // `SetupPlayerTurn` resets the energy to `MaxEnergy`, which runs
    // `Hook.ModifyMaxEnergy`, and raises the hooks that follow.
    live.energy = super::board::max_energy::<V>(
        Walk {
            live: &live,
            relics,
        },
        energy,
    );
    if let Some(ended) = super::turn::energy_reset::<V>(&mut live, &mut streams, relics, ascension)
    {
        unimplemented!("a fight that ends {ended:?} as its first energy resets");
    }
    super::turn::energy_reset_late::<V>(&mut live, streams.combat(), relics, ascension);
    // `BeforeHandDraw`. The opening hand is dealt below rather than drawn onto
    // what is already there, so nothing may be made in hand here.
    let player = live.player().id;
    let hook = sts2_core::EventHook::BeforeHandDraw {
        player,
        turn: live.turn,
    };
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

    // The whole deck into the draw pile in the order the deal left it, then
    // the hand drawn off its top, which is `CardPileCmd.Draw` raising each
    // card's hooks and history in turn.
    let order: Vec<_> = piles.hand().iter().chain(piles.draw()).copied().collect();
    live.cards = Cards::dealt(&[], &order);
    if !afflicting.is_empty() {
        super::board::Board::for_card(&mut live, streams.combat(), relics, ascension, None, None)
            .write_cards(afflicting);
    }
    // `RandomizeOrderInternal` raises `ModifyShuffleOrder` before the innate
    // cards move. The same cards are in the draw pile here, and no listener
    // answers it.
    let hook = sts2_core::EventHook::ShuffleOrder { initial: true };
    if let Some((listener, effect)) = super::listen::notify::<V>(&mut live, relics, hook).first() {
        unimplemented!("{listener:?} asking for {effect:?} over the opening shuffle")
    }
    if let Some(ended) = super::board::draw(
        &mut live,
        streams.combat(),
        relics,
        ascension,
        piles.hand().len(),
        true,
    ) {
        unimplemented!("a fight that ends {ended:?} as its opening hand is drawn");
    }
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
    if let Some(ended) = super::turn::side_turn_started::<V>(
        &mut live,
        streams.combat(),
        relics,
        ascension,
        <V::CombatSide as SideKinds>::PLAYER,
    ) {
        if ended == super::Ended::Won {
            super::listen::combat_ended::<V>(&mut live, relics);
        }
        return Ok(Entered {
            live,
            streams,
            ended: Some(ended),
        });
    }
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
