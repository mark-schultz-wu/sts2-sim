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
    /// The player's potions, in slot order.
    pub(crate) potions: &'a [V::Potion],
    pub(crate) alive: bool,
    pub(crate) deck: &'a [sts2_core::DeckCard<V>],
    /// Whether the run reached this room off the map's lines.
    pub(crate) strayed: bool,
    /// The type of the map point the fight stands on. None off the map.
    pub(crate) point: Option<V::MapPointType>,
    /// Where on the map the fight stands. None off the map.
    pub(crate) coord: Option<sts2_core::Coord>,
    /// The monsters' health, where an event rolled it as it opened. Empty
    /// for a fight that rolls its own.
    pub(crate) rolled: &'a [V::Health],
    /// The monsters an encounter whose roster is configured opens on, and
    /// nothing for any other.
    pub(crate) roster: &'a [V::Monster],
    /// `RunState.BaseRoom` is the fight's room: no event opened it.
    pub(crate) base: bool,
}

/// A fight as it opens, with the streams it drew from.
pub(crate) struct Entered<V: Rules> {
    pub(crate) live: Box<Live<V>>,
    pub(crate) streams: Streams<V>,
    /// How the fight ended before the player took a turn in it, if it did.
    pub(crate) ended: Option<super::Ended>,
    /// What a listener asked of the run as the fight opened, which the run
    /// carries out once it stands in the fight.
    pub(crate) arriving: Vec<HookEffect<V>>,
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
        potions: super::live::PotionBelt::holding(entering.potions),
        round: num_traits::one(),
        side: <V::CombatSide as SideKinds>::PLAYER,
        history: super::live::CombatHistory::new(),
        kept: Vec::new(),
        encounter,
        cards: Cards::with_cards(0),
        removed_from_deck: Vec::new(),
        ending_turn: false,
        point: entering.coord,
        deck_versions: super::live::DeckVersions::none(),
        taken_from_deck: Vec::new(),
        extra_rewards: Vec::new(),
        exhaust_next: smallvec::SmallVec::new(),
        leaving: Vec::new(),
        for_run: Vec::new(),
        after_pick: Vec::new(),
        hook_draws: smallvec::SmallVec::new(),
        drawn_late: None,
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
        kept_faces: Vec::new(),
        attacks: 0,
        attack_results: Vec::new(),
        last_creature: CombatId::new(nonmax::NonMaxU8::ZERO),
        won: false,
        notes: smallvec::SmallVec::new(),
        asking: None,
        deferred: super::live::Deferrals::new(),
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
    let monsters = encounter::opening::<V>(raw_seed, entering.floor, encounter, entering.roster)?;
    let Live {
        creatures,
        machines,
        ..
    } = &mut *live;
    monster::initialize::<V>(
        ascension,
        encounter,
        &mut streams,
        monster::Roster {
            monsters: &monsters,
            rolled: entering.rolled,
        },
        creatures,
        machines,
    )?;
    live.last_creature = live
        .creatures
        .iter()
        .map(|creature| creature.id)
        .max_by_key(|id| id.get())
        .unwrap_or(live.last_creature);

    // The deck's own cards, each becoming a copy the fight can write on, in
    // the order `PopulateCombatState` shuffles them into the draw pile as the
    // fight is set up. They are dealt once the first energy is in.
    // `DeckVersion`, set on each of them.
    if !entering.deck.is_empty() {
        let ids = entering.deck.iter().map(|card| card.id).collect();
        live.deck_versions = super::live::DeckVersions::new(live.card_ids.peek(), ids);
    }
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
    let mut order = pile::shuffle(streams.combat().shuffle, deck).into_cards();
    // `AfterRoomEntered` as the room opens, then `BeforeSideTurnStart` for the
    // turn the fight is about to begin on. Both run before anything is drawn.
    let turn = live.turn;
    let entered = EventHook::RoomEntered {
        room: live.room(),
        strayed: entering.strayed,
        point: entering.point,
        deck: entering.deck.len(),
        player: sts2_core::PlayerHealth {
            hp: live.player().hp,
            max_hp: live.player().max_hp,
        },
        base: entering.base,
    };
    for (_, effect) in super::listen::notify::<V>(&mut live, relics, entered) {
        match effect {
            // `PlayerCmd.GainStars`.
            HookEffect::GainStars(gained) => {
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::gain_stars(&mut board, gained);
            }
            // `PowerCmd.Apply` on the relic's owner, which is the player.
            HookEffect::ApplyToOwner(power, amount) => {
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::apply_to_actor(&mut board, power, amount);
            }
            HookEffect::UpgradeRandomDrawCards(count) => {
                use sts2_core::CardTable as _;
                use sts2_core::rng::GameRng as _;

                let mut upgradable: Vec<usize> = order
                    .iter()
                    .enumerate()
                    .filter(|(_, held)| {
                        held.face.ident.upgrade < held.face.ident.card.max_upgrade()
                    })
                    .map(|(at, _)| at)
                    .collect();
                // `CardModel.CompareTo`: its `ModelId`, then its upgrade.
                streams
                    .combat()
                    .selection
                    .stable_shuffle_by_key(&mut upgradable, |&at| {
                        (
                            order[at].face.ident.card.order(),
                            order[at].face.ident.upgrade,
                        )
                    });
                for at in upgradable.into_iter().take(count) {
                    order[at].face.upgrade();
                }
            }
            // `PlayerCmd.GainGold` for the player who carries the relic.
            HookEffect::GainGold(amount) => {
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::gain_gold(&mut board, amount);
            }
            // `CreatureCmd.Heal` on the player who carries the relic.
            HookEffect::Heal(amount) => {
                let player = live.player().id;
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::heal(&mut board, player, amount);
            }
            // `CardCmd.ApplyKeyword` over `AllCards`, which is the deck the
            // piles hold by now.
            HookEffect::AddKeyword {
                to: sts2_core::Writing::EveryCopyOf(card),
                keyword,
            } => {
                for held in order.iter_mut().filter(|held| held.face.ident.card == card) {
                    held.face.modify(
                        sts2_core::Change::AddKeyword(keyword),
                        sts2_core::Lasting::ThisFight,
                    );
                }
            }
            // `PowerCmd.Apply<P>` on every living opponent, from nobody.
            HookEffect::ApplyToLivingOpponents(power, amount) => {
                let enemies: Vec<CombatId> = (live.creatures.iter())
                    .filter(|creature| {
                        creature.side == <V::CombatSide as SideKinds>::ENEMY && creature.alive
                    })
                    .map(|creature| creature.id)
                    .collect();
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                for enemy in enemies {
                    board.land(enemy, power, amount, None);
                }
            }
            other => unimplemented!("a listener asking for {other:?} as a room opens"),
        }
    }
    // `StartCombatInternal`'s `AfterCreatureAdded`, which `SetUpCombat`
    // queues behind `AfterRoomEntered`.
    let Live {
        creatures,
        powers,
        machines,
        ..
    } = &mut *live;
    monster::added_to_room::<V>(ascension, creatures, powers, machines);
    // `BeforeCombatStart`, after the room and before the fight opens. The
    // player carries no power yet, so block gained here is what was asked for
    // and passes no modifier.
    //
    // The game's piles hold the deck by now, and the ones here are dealt
    // below. So an affliction asked for here lands once they are, before
    // anything draws.
    let mut afflicting: Vec<HookEffect<V>> = Vec::new();
    let mut shuffled_in: Vec<V::Card> = Vec::new();
    let mut arriving: Vec<HookEffect<V>> = Vec::new();
    let starting = EventHook::CombatStarting {
        room: live.room(),
        holding_potions: !live.potions.is_empty(),
        point: live.point,
    };
    for (listener, effect) in super::listen::notify::<V>(&mut live, relics, starting) {
        match (listener, effect) {
            // On the creature carrying the listener, which for a relic is the
            // player.
            (listener, HookEffect::GainBlock(block)) => {
                let owner = match listener {
                    super::listen::Listener::Power { owner, .. } => owner,
                    _ => live.player().id,
                };
                super::board::gain_unpowered_block::<V>(
                    &mut live,
                    streams.combat(),
                    relics,
                    ascension,
                    owner,
                    block,
                );
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
            // `CardPileCmd.AddToCombatAndPreview` into the draw pile at random,
            // which the deck is in by now. It lands once the piles are dealt.
            (
                super::listen::Listener::Relic { .. },
                HookEffect::AddToDrawAtRandom { card, count },
            ) => {
                let count: usize = num_traits::cast(count)
                    .unwrap_or_else(|| unimplemented!("{count:?} cards into the draw pile"));
                shuffled_in.extend(std::iter::repeat_n(card, count));
            }
            // The belt is the run's.
            (super::listen::Listener::Relic { .. }, fill @ HookEffect::FillPotionSlots { .. }) => {
                arriving.push(fill)
            }
            // `CreatureCmd.Heal` on the player who carries the relic.
            (super::listen::Listener::Relic { .. }, HookEffect::Heal(amount)) => {
                let player = live.player().id;
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::heal(&mut board, player, amount);
            }
            // `PowerCmd.Apply` on the relic's owner, which is the player.
            (super::listen::Listener::Relic { .. }, HookEffect::ApplyToOwner(power, amount)) => {
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                sts2_core::Fight::apply_to_actor(&mut board, power, amount);
            }
            // `CreatureCmd.SetCurrentHp` on the enemies a relic names.
            (super::listen::Listener::Relic { .. }, HookEffect::SetEnemiesHp { creature, hp }) => {
                let mut board =
                    Board::for_card(&mut live, streams.combat(), relics, ascension, None, None);
                board.set_enemies_hp(creature, hp);
            }
            (_, other) => unimplemented!("a listener asking for {other:?} as a fight starts"),
        }
    }
    // `BeforeCombatStartLate`, once every listener has heard the first.
    let late = EventHook::CombatStartingLate { room: live.room() };
    for (listener, effect) in super::listen::notify::<V>(&mut live, relics, late) {
        match (listener, effect) {
            // The belt is the run's.
            (super::listen::Listener::Relic { .. }, potion @ HookEffect::ProcurePotion(_)) => {
                arriving.push(potion)
            }
            (_, other) => {
                unimplemented!("a listener asking for {other:?} as a fight is about to start")
            }
        }
    }
    // The whole deck into the draw pile in the order the shuffle left it.
    live.cards = Cards::dealt(&[], &order);
    if !afflicting.is_empty() {
        super::board::Board::for_card(&mut live, streams.combat(), relics, ascension, None, None)
            .write_cards(afflicting);
    }
    if !shuffled_in.is_empty() {
        let mut board = super::board::Board::for_card(
            &mut live,
            streams.combat(),
            relics,
            ascension,
            None,
            None,
        );
        for card in shuffled_in {
            board.add_to_draw_at_random(card);
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
    // `RandomizeOrderInternal` raises `ModifyShuffleOrder` before the innate
    // cards move. No listener answers it.
    let hook = sts2_core::EventHook::ShuffleOrder { initial: true };
    if let Some((listener, effect)) = super::listen::notify::<V>(&mut live, relics, hook).first() {
        unimplemented!("{listener:?} asking for {effect:?} over the opening shuffle")
    }
    // `BeforeHandDraw`, then the hand, then `AfterPlayerTurnStart`: the first
    // turn's `SetupPlayerTurn` runs as every other turn's does, with the
    // innate cards put on top first.
    let player = live.player().id;
    let hook = sts2_core::EventHook::BeforeHandDraw {
        player,
        turn: live.turn,
    };
    let asked = super::listen::notify::<V>(&mut live, relics, hook);
    let ended = match super::turn::carry_setup(
        &mut live,
        streams.combat(),
        relics,
        ascension,
        super::live::Setup::BeforeHandDraw(asked),
    ) {
        super::turn::Carried::Done(ended) => ended,
        // A setup that stops on a question lets the fight open around it, as
        // any turn's does. `RunAutoPrePlayPhase` waits for the answer, so the
        // turn stays in its start until then.
        super::turn::Carried::Parked => {
            live.phase = <V::PlayerTurnPhase as TurnPhaseKinds>::START;
            None
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
            arriving,
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
            arriving,
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
    // `RunAutoPrePlayPhase`, which a setup parked on a question waits out.
    let ended = match ended {
        None if live.phase == <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY => {
            let ended = super::turn::pre_play_phase::<V>(&mut live, streams.combat(), relics);
            if ended.is_none() {
                live.phase = super::turn::after_pre_play::<V>(&live);
            }
            ended
        }
        ended => ended,
    };
    if ended == Some(super::Ended::Won) {
        super::listen::combat_ended::<V>(&mut live, relics);
    }

    Ok(Entered {
        live,
        streams,
        ended,
        arriving,
    })
}

/// `Hook.AfterModifyingHandDraw`, once the turn's hand size is worked out.
/// A listener's own hook reads whether it changed the size.
pub(super) fn after_modifying_hand_draw<V: Rules>(
    live: &mut super::live::Live<V>,
    relics: &mut Belt<V>,
) {
    let hook = sts2_core::EventHook::HandDrawModified;
    if let Some((listener, effect)) = super::listen::notify::<V>(live, relics, hook).first() {
        unimplemented!("{listener:?} asking for {effect:?} after the hand size is worked out")
    }
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

/// The health a `Combat`-layout event's monsters roll as it opens, off the
/// run's `Niche` stream.
pub(crate) fn roll_layout<V: Rules>(
    raw_seed: u64,
    floor: V::Count,
    ascension: V::AscensionLevel,
    encounter: V::Encounter,
    streams: &mut Streams<V>,
) -> Result<smallvec::SmallVec<[V::Health; 2]>, Error> {
    let monsters = encounter::opening::<V>(raw_seed, floor, encounter, &[])?;
    Ok(monster::roll_layout::<V>(
        &monsters,
        ascension,
        streams.combat().niche,
    ))
}
