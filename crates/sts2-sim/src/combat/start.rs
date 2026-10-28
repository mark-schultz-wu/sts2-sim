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
use crate::run::Belts;
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
    /// The act the fight is fought in, counted from zero, which scales the
    /// monsters in a fight of several players.
    pub(crate) act: usize,
    /// The modifiers the run was started with.
    pub(crate) modifiers: &'a [sts2_core::RunModifier<V>],
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

/// One player as they stand in the door of a fight, in seat order.
pub(crate) struct EnteringPlayer<'a, V: Rules> {
    pub(crate) character: V::Character,
    pub(crate) hp: V::Health,
    pub(crate) max_hp: V::Health,
    pub(crate) gold: V::Number,
    /// The player's potions, in slot order.
    pub(crate) potions: &'a [V::Potion],
    pub(crate) alive: bool,
    pub(crate) deck: &'a [sts2_core::DeckCard<V>],
}

/// A fight as it opens, with the streams it drew from.
pub(crate) struct Entered<V: Rules> {
    pub(crate) live: Box<Live<V>>,
    pub(crate) streams: Streams<V>,
    /// How the fight ended before the player took a turn in it, if it did.
    pub(crate) ended: Option<super::Ended>,
    /// What a listener asked of the run as the fight opened, which the run
    /// carries out for its owner once it stands in the fight.
    pub(crate) arriving: Vec<(sts2_core::SeatIndex, HookEffect<V>)>,
}

/// The fight this run opens on the encounter it just travelled into.
///
/// Takes the run as it stands and gives back the fight and the streams it drew
/// from, without touching either. Entering a room is one transition, and a
/// fight that cannot be built must leave the run where it stood.
#[tracing::instrument(level = "trace", skip_all)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn enter<V: Rules>(
    raw_seed: u64,
    ascension: V::AscensionLevel,
    encounter: V::Encounter,
    entering: &Entering<'_, V>,
    players: &[EnteringPlayer<'_, V>],
    streams: &Streams<V>,
    belts: &mut Belts<V>,
) -> Result<Entered<V>, Error> {
    let mut streams = *streams;
    let modifiers = super::live::RunModifiers::holding(entering.modifiers);
    let [first, others @ ..] = players else {
        unreachable!("a fight has a player");
    };
    let combat_of = |player: &EnteringPlayer<'_, V>| player_combat::<V>(player, &modifiers);
    let host_combat = combat_of(first);
    let others_combat = others.iter().map(combat_of).collect();
    // What no one player owns acts as the first.
    let host = sts2_core::SeatIndex(0);
    let mut live = Box::new(Live {
        players: crate::seats::BySeat::new(host_combat, others_combat),
        ascension,
        turn: num_traits::one(),
        modifiers,
        round: num_traits::one(),
        side: <V::CombatSide as SideKinds>::PLAYER,
        history: super::live::CombatHistory::new(),
        kept: Vec::new(),
        encounter,
        encounter_state: sts2_core::EncounterModel::new(encounter),
        scaling: monster::Scaling::of::<V>(players.len(), encounter, entering.act),
        in_progress: false,
        point: entering.coord,
        extra_rewards: Vec::new(),
        selector_answers: false,
        pause_boundary: false,
        leaving: Vec::new(),
        for_run: Vec::new(),
        after_pick: None,
        detached: Vec::new(),
        created: Vec::new(),
        hook_draws: smallvec::SmallVec::new(),
        drawn_late: None,
        drew: None,
        creatures: super::live::Creatures::new(),
        powers: PowerStore::new(),
        machines: Bounded::new(),
        // Nothing has been applied to anyone yet.
        skip_tick: Bounded::new(),
        card_ids: CardIds::new(),
        kept_faces: Vec::new(),
        attacks: 0,
        attack_results: Vec::new(),
        landing: Vec::new(),
        last_creature: CombatId::new(nonmax::NonMaxU8::ZERO),
        won: false,
        lost: false,
        notes: smallvec::SmallVec::new(),
        asking: None,
        deferred: super::live::Deferrals::new(),
        queues: super::queues::Queues::new(),
        waited_attack: None,
    });

    // `AddPlayer` for each player in run order, then the monsters.
    // `CombatId` counts from the first player.
    for (at, player) in players.iter().enumerate() {
        let id = u8::try_from(at)
            .ok()
            .and_then(nonmax::NonMaxU8::new)
            .ok_or(Error::CreatureIdOverflow)?;
        live.creatures.push(Fighter {
            id: CombatId::new(id),
            model: Some(ModelId::Character(player.character)),
            side: <V::CombatSide as SideKinds>::PLAYER,
            hp: player.hp,
            max_hp: player.max_hp,
            block: num_traits::zero(),
            alive: player.alive,
            next_move: None,
            owner: None,
        });
    }
    let monsters = encounter::opening::<V>(raw_seed, entering.floor, encounter, entering.roster)?;
    let Live {
        creatures,
        machines,
        scaling,
        ..
    } = &mut *live;
    monster::initialize::<V>(
        ascension,
        *scaling,
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

    // Each player's deck, each card becoming a copy the fight can write on,
    // in the order `PopulateCombatState` shuffles it into the draw pile as
    // the fight is set up. One player after another, off the run's one
    // shuffle stream. They are dealt once the first energy is in.
    // `DeckVersion`, set on each of them.
    let mut orders: smallvec::SmallVec<[Vec<CardInstance<V>>; 1]> = smallvec::SmallVec::new();
    for (at, player) in players.iter().enumerate() {
        let seat = sts2_core::SeatIndex(u8::try_from(at).expect("a fight's players fit a u8"));
        if !player.deck.is_empty() {
            let ids = player.deck.iter().map(|card| card.id).collect();
            live.players[seat].deck_versions =
                super::live::DeckVersions::new(live.card_ids.peek(), ids);
        }
        let deck: Vec<CardInstance<V>> = player
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
        orders.push(pile::shuffle(streams.combat().shuffle, deck).into_cards());
    }
    // `AfterRoomEntered` as the room opens, before anything is drawn. A
    // listener reads its own player's numbers, which the walk fills in.
    let host_deck = first.deck.len();
    let entered = EventHook::RoomEntered {
        room: live.room(),
        strayed: entering.strayed,
        point: entering.point,
        deck: host_deck,
        player: sts2_core::PlayerHealth {
            hp: live.player(host).hp,
            max_hp: live.player(host).max_hp,
        },
        base: entering.base,
    };
    let mut walking = super::listen::Walking::begin(&live, belts, entered);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(host, &mut live, belts))
    {
        let seat = super::listen::owner::<V>(&live, listener).unwrap_or(host);
        let order = &mut orders[seat.at()];
        match effect {
            // `PlayerCmd.GainStars`.
            HookEffect::GainStars(gained) => {
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                sts2_core::Fight::gain_stars(&mut board, gained);
            }
            // `PowerCmd.Apply` on the relic's owner, which is the player.
            HookEffect::ApplyToOwner(power, amount) => {
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
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
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                sts2_core::Fight::gain_gold(&mut board, amount);
            }
            // `CreatureCmd.Heal` on the player who carries the relic.
            HookEffect::Heal(amount) => {
                let player = live.player(seat).id;
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
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
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                for enemy in enemies {
                    board.land(enemy, power, amount, None);
                }
            }
            // `PowerCmd.Apply<P>` on every creature in the fight, from nobody.
            HookEffect::ApplyToEveryCreature(power, amount) => {
                let everyone: Vec<CombatId> =
                    live.creatures.iter().map(|creature| creature.id).collect();
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                for creature in everyone {
                    board.land(creature, power, amount, None);
                }
            }
            // `PowerCmd.Apply<P>` on the player, from nobody.
            HookEffect::ApplyToPlayers(power, amount) => {
                let player = live.player(seat).id;
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                board.land(player, power, amount, None);
            }
            // `CreatureCmd.LoseMaxHp` on the player.
            HookEffect::LoseMaxHp(amount) => {
                let player = live.player(seat).id;
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                if sts2_core::Fight::lose_max_hp(&mut board, player, amount).is_some() {
                    unimplemented!("a question inside a maximum health loss as a room opens");
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
        scaling,
        ..
    } = &mut *live;
    monster::added_to_room::<V>(ascension, *scaling, creatures, powers, machines);
    // `BeforeCombatStart`, after the room and before the fight opens. The
    // player carries no power yet, so block gained here is what was asked for
    // and passes no modifier.
    //
    // The game's piles hold the deck by now, and the ones here are dealt
    // below. So an affliction asked for here lands once they are, before
    // anything draws.
    let mut afflicting: smallvec::SmallVec<[Vec<HookEffect<V>>; 1]> =
        smallvec::smallvec![Vec::new(); players.len()];
    let mut shuffled_in: smallvec::SmallVec<[Vec<V::Card>; 1]> =
        smallvec::smallvec![Vec::new(); players.len()];
    let mut arriving: Vec<(sts2_core::SeatIndex, HookEffect<V>)> = Vec::new();
    live.in_progress = true;
    let starting = EventHook::CombatStarting {
        room: live.room(),
        holding_potions: !live.players[host].potions.is_empty(),
        point: live.point,
    };
    let mut walking = super::listen::Walking::begin(&live, belts, starting);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(host, &mut live, belts))
    {
        let seat = super::listen::owner::<V>(&live, listener).unwrap_or(host);
        match (listener, effect) {
            // On the creature carrying the listener, which for a relic is the
            // player.
            (listener, HookEffect::GainBlock(block)) => {
                let owner = match listener {
                    super::listen::Listener::Power { owner, .. } => owner,
                    _ => live.player(seat).id,
                };
                super::board::gain_unpowered_block::<V>(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    owner,
                    block,
                );
            }
            // `PlayerCmd.AddPet`.
            (_, HookEffect::AddPet(monster)) => {
                let mut board = super::board::Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                board.add_pet(monster);
            }
            (_, HookEffect::Summon(amount)) => {
                super::board::summon_osty::<V>(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    amount,
                );
            }
            (_, afflict @ HookEffect::Afflict { .. }) => afflicting[seat.at()].push(afflict),
            // `CardPileCmd.AddToCombatAndPreview` into the draw pile at random,
            // which the deck is in by now. It lands once the piles are dealt.
            (
                super::listen::Listener::Relic { .. },
                HookEffect::AddToDrawAtRandom { card, count, .. },
            ) => {
                let count: usize = num_traits::cast(count)
                    .unwrap_or_else(|| unimplemented!("{count:?} cards into the draw pile"));
                shuffled_in[seat.at()].extend(std::iter::repeat_n(card, count));
            }
            // The belt is the run's.
            (super::listen::Listener::Relic { .. }, fill @ HookEffect::FillPotionSlots { .. }) => {
                arriving.push((seat, fill))
            }
            // `CreatureCmd.Heal` on the player who carries the relic.
            (super::listen::Listener::Relic { .. }, HookEffect::Heal(amount)) => {
                let player = live.player(seat).id;
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                sts2_core::Fight::heal(&mut board, player, amount);
            }
            // `PowerCmd.Apply` on the relic's owner, which is the player.
            (super::listen::Listener::Relic { .. }, HookEffect::ApplyToOwner(power, amount)) => {
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                sts2_core::Fight::apply_to_actor(&mut board, power, amount);
            }
            // `CreatureCmd.SetCurrentHp` on the enemies a relic names.
            (super::listen::Listener::Relic { .. }, HookEffect::SetEnemiesHp { creature, hp }) => {
                let mut board = Board::for_card(
                    seat,
                    &mut live,
                    streams.combat(),
                    belts,
                    ascension,
                    None,
                    None,
                );
                board.set_enemies_hp(creature, hp);
            }
            (_, other) => unimplemented!("a listener asking for {other:?} as a fight starts"),
        }
    }
    // `BeforeCombatStartLate`, once every listener has heard the first.
    let late = EventHook::CombatStartingLate { room: live.room() };
    let mut walking = super::listen::Walking::begin(&live, belts, late);
    while let Some((listener, effect)) = walking
        .as_mut()
        .and_then(|walking| walking.next(host, &mut live, belts))
    {
        let seat = super::listen::owner::<V>(&live, listener).unwrap_or(host);
        match (listener, effect) {
            // The belt is the run's.
            (super::listen::Listener::Relic { .. }, potion @ HookEffect::ProcurePotion(_)) => {
                arriving.push((seat, potion))
            }
            (_, other) => {
                unimplemented!("a listener asking for {other:?} as a fight is about to start")
            }
        }
    }
    // Each whole deck into its draw pile in the order the shuffle left it.
    for (at, order) in orders.iter().enumerate() {
        let seat = sts2_core::SeatIndex(u8::try_from(at).expect("a fight's players fit a u8"));
        live.players[seat].cards = Cards::dealt(&[], order);
        let afflicting = std::mem::take(&mut afflicting[at]);
        if !afflicting.is_empty() {
            super::board::Board::for_card(
                seat,
                &mut live,
                streams.combat(),
                belts,
                ascension,
                None,
                None,
            )
            .write_cards(afflicting);
        }
        let shuffled_in = std::mem::take(&mut shuffled_in[at]);
        if !shuffled_in.is_empty() {
            let mut board = super::board::Board::for_card(
                seat,
                &mut live,
                streams.combat(),
                belts,
                ascension,
                None,
                None,
            );
            // A relic's, whose owner is the player.
            let player = board.live.player(seat).id;
            for card in shuffled_in {
                board.add_to_draw_at_random(card, Some(player));
            }
        }
    }
    // `RandomizeOrderInternal` raises `ModifyShuffleOrder` before the innate
    // cards move. No listener answers it.
    let hook = sts2_core::EventHook::ShuffleOrder { initial: true };
    if let Some((listener, effect)) = super::listen::first_answer::<V>(host, &mut live, belts, hook)
    {
        unimplemented!("{listener:?} asking for {effect:?} over the opening shuffle")
    }
    // `StartTurn`, as for every turn. `SetupPlayerTurn` puts the innate cards
    // on top on the first.
    let ended = super::turn_loop::begin(
        host,
        super::Fighting {
            live: &mut live,
            streams: &mut streams,
            belts: &mut *belts,
        },
        ascension,
        true,
    )?;
    Ok(Entered {
        live,
        streams,
        ended,
        arriving,
    })
}

/// One player's side of the fight as it opens, before anything is dealt.
fn player_combat<V: Rules>(
    player: &EnteringPlayer<'_, V>,
    modifiers: &super::live::RunModifiers<V>,
) -> super::live::PlayerCombat<V> {
    let character = player.character;
    super::live::PlayerCombat {
        character,
        phase: <V::PlayerTurnPhase as TurnPhaseKinds>::PLAY,
        energy: num_traits::cast(V::max_energy(character))
            .expect("the starting energy fits the build's count type"),
        stars: num_traits::zero(),
        gold: player.gold,
        potions: super::live::PotionBelt::holding(player.potions, modifiers.hooks),
        cards: Cards::with_cards(0),
        removed_from_deck: Vec::new(),
        ending_turn: false,
        ready: false,
        deaf: false,
        effect_depth: 0,
        deck_versions: super::live::DeckVersions::none(),
        taken_from_deck: Vec::new(),
        exhaust_next: smallvec::SmallVec::new(),
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
    }
}

/// `Hook.AfterModifyingHandDraw`, once the turn's hand size is worked out.
/// A listener's own hook reads whether it changed the size.
pub(super) fn after_modifying_hand_draw<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut super::live::Live<V>,
    belts: &mut Belts<V>,
) {
    let hook = sts2_core::EventHook::HandDrawModified;
    if let Some((listener, effect)) = super::listen::first_answer::<V>(seat, live, belts, hook) {
        unimplemented!("{listener:?} asking for {effect:?} after the hand size is worked out")
    }
}

/// How many cards a turn's hand is drawn to.
///
/// `Hook.ModifyHandDraw` from `baseHandDrawCount`, then its late phase. The
/// listeners that answer it read the turn, and the first turn of a fight
/// follows no other.
pub(super) fn hand_size<V: Rules>(
    seat: sts2_core::SeatIndex,
    fight: Walk<'_, V>,
    turn: V::Count,
) -> usize {
    let base: V::Number =
        num_traits::cast(HAND_SIZE).expect("the printed hand size fits the build's game number");
    let player = fight.live.player(seat).id;
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
