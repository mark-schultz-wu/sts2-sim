//! The digest, against a real build's ids.
//!
//! An integration test for the reason `vocabulary.rs` is one. Naming a build
//! means depending on `sts2-content`, which depends back on this crate, and a
//! unit test links a second copy of it.
//!
//! # What these hold
//!
//! One thing, in two directions. Every field of an [`Observation`] reaches the
//! digest, so a divergence in any of them is a divergence the fast path sees.
//! Nothing else does, so two runs that agree digest the same however their maps
//! happened to be laid out in memory.
//!
//! The first is the one that rots. A field added to `Observation` and not to
//! the walk is invisible to every comparison, and a simulator can get it wrong
//! forever and pass. The walk destructures exhaustively so that cannot
//! compile, and this is what says the destructured field was then *used*.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use sts2_core::fixture::at;
use sts2_core::{PowerAmount, Section};

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{
    Attack, Blocked, CardIdent, Carried, Channelled, ChestOption, Combat, ContentVersion, Coord,
    Cost, Creature, Digest, Entry, EventOption, EventOptionKey, FlagSet, Held, Intent, Map,
    ModelId, NextMove, Node, Observation, Orbs, Playable, Position, Positions, Preview, Reachable,
    RestOption, Room, ShopCard, Stack, Stock, Table, Take, Undrinkable,
};

type V = V0_107_1;

type Card = <V as ContentVersion>::Card;
type CardVar = <V as ContentVersion>::CardVar;
type Relic = <V as ContentVersion>::Relic;
type Potion = <V as ContentVersion>::Potion;
type Power = <V as ContentVersion>::Power;
type Orb = <V as ContentVersion>::Orb;
type Monster = <V as ContentVersion>::Monster;
type Encounter = <V as ContentVersion>::Encounter;
type Event = <V as ContentVersion>::Event;
type EventOptionId = <V as ContentVersion>::EventOptionId;
type Affliction = <V as ContentVersion>::Affliction;
type Enchantment = <V as ContentVersion>::Enchantment;
type MapPointType = <V as ContentVersion>::MapPointType;
type CombatSide = <V as ContentVersion>::CombatSide;
type PlayerTurnPhase = <V as ContentVersion>::PlayerTurnPhase;
type UnplayableReason = <V as ContentVersion>::UnplayableReason;
type UnplayableReasons = <V as ContentVersion>::UnplayableReasons;
type RunRngType = <V as ContentVersion>::RunRngType;
type PlayerRngType = <V as ContentVersion>::PlayerRngType;
type IntentKind = <V as ContentVersion>::IntentKind;
type MoveId = <V as ContentVersion>::MoveId;
type RestSiteOption = <V as ContentVersion>::RestSiteOption;
type TargetType = <V as ContentVersion>::TargetType;

/// The nth of a kind, so a test can hold two distinct ids without naming a
/// variant of a table that has hundreds.
fn nth<T: Table + Copy>(n: usize) -> T {
    <T as Table>::ALL[n]
}

fn carried(n: usize) -> Carried<V> {
    Carried {
        relic: nth::<Relic>(n),
        amount: n as i32,
        title: Some(format!("Relic {n}")),
        text: Some(format!("It does thing {n}.")),
    }
}

fn held(n: usize) -> Held<V> {
    Held {
        potion: nth::<Potion>(n),
        title: Some(format!("Potion {n}")),
        text: Some(format!("It does thing {n}.")),
        reason: None,
        targets: vec![at(1), at(2)],
    }
}

fn card(n: usize) -> CardIdent<V> {
    CardIdent::new(nth::<Card>(n))
}

/// An observation with something in every field.
///
/// Not a position the game produces. A run is not both in a shop and in a
/// fight. The question here is coverage, and a field only reaches the digest if
/// there is something there to reach it. The values are real ids, and only the
/// combination is fabricated.
fn full() -> Observation<V> {
    Observation {
        act: 1,
        floor: 3,
        hp: 60,
        max_hp: 80,
        alive: true,
        run_over: false,
        victory: Some(false),
        gold: 99,
        relics: vec![carried(0), carried(1)],
        potions: vec![held(0)],
        potion_slots: 2,
        deck: vec![card(0), card(1)],
        in_combat: true,
        map: Map {
            at: Some(Coord::new(3, 11)),
            visited: vec![Coord::new(0, 0), Coord::new(3, 11)],
            travelable: vec![Reachable {
                at: Coord::new(4, 12),
                kind: MapPointType::Monster,
            }],
            points: [Node {
                at: Coord::new(3, 11),
                kind: MapPointType::RestSite,
                to: vec![Coord::new(4, 12)],
            }]
            .into(),
            proceed: false,
        },
        rng: Positions {
            run: HashMap::from([
                (RunRngType::UpFront, Position::counted(4)),
                (RunRngType::Shuffle, Position::counted(17)),
                (RunRngType::MonsterAi, Position::counted(2)),
            ]),
            player: HashMap::from([
                (PlayerRngType::Rewards, Position::counted(1)),
                (PlayerRngType::Shops, Position::counted(0)),
            ]),
        },
        room: Some(rooms()[0].clone()),
        combat: Some(Combat {
            turn: 2,
            phase: PlayerTurnPhase::Play,
            energy: 3,
            stars: 0,
            round: 2,
            encounter: Some(nth::<Encounter>(0)),
            hand: vec![card(2), card(3)],
            draw: vec![card(4)],
            discard: vec![card(5)],
            exhaust: vec![card(6)],
            creatures: vec![Creature {
                id: at(1),
                model: Some(ModelId::Monster(nth::<Monster>(0))),
                side: CombatSide::Enemy,
                hp: 30,
                max_hp: 40,
                block: 5,
                alive: true,
                powers: vec![Stack {
                    power: nth::<Power>(0),
                    amount: PowerAmount::Counted(3),
                }],
                next_move: Some(NextMove {
                    id: MoveId::AboutToBlowMove,
                    intents: vec![Intent {
                        kind: IntentKind::Attack,
                        attack: Some(Attack {
                            damage: 12,
                            repeats: 2,
                        }),
                    }],
                }),
            }],
            legal: vec![Playable {
                card: card(2),
                cost: Cost::fixed(1),
                star_cost: Cost::None,
                target: TargetType::AnyEnemy,
                reason: Some(UnplayableReasons::from_members(&[
                    UnplayableReason::EnergyCostTooHigh,
                ])),
                preventer: Some(ModelId::Power(nth::<Power>(1))),
                targets: vec![at(1), at(2)],
                effect: vec![Preview {
                    target: Some(at(1)),
                    text: Some("Deal {Damage} damage.".into()),
                    vars: BTreeMap::from([(CardVar::Damage, 9)]),
                    base: BTreeMap::from([(CardVar::Damage, 6)]),
                }],
            }],
            orbs: Some(Orbs {
                capacity: 3,
                queue: vec![Channelled {
                    orb: Orb::DarkOrb,
                    passive: 6,
                    evoke: 6,
                }],
            }),
        }),
    }
}

/// Every shape a room takes, each differing from its neighbour in one place.
///
/// A room is a sum, so the fields of one cannot be nudged in an observation
/// holding another. This is the same coverage written as a list of whole rooms.
fn rooms() -> Vec<Room<V>> {
    let option = |index, id, enabled| RestOption { index, id, enabled };
    let chest = |index, take, chosen| ChestOption {
        index,
        take,
        chosen,
    };
    let event_option = || EventOption {
        index: 0,
        key: EventOptionKey {
            event: sts2_content::v0_107_1::events::Event::TeaMaster,
            option: EventOptionId::TeaMaster(sts2_content::v0_107_1::TeaMasterOption::BoneTea),
        },
        title: Some("Eat".into()),
        text: Some("Swallow it whole.".into()),
        locked: false,
        proceed: false,
        chosen: false,
        relic: Some(nth::<Relic>(0)),
    };
    let on_sale = |n| ShopCard {
        card: card(n),
        cost: Cost::fixed(1),
        star_cost: Cost::None,
    };
    let entry = |index, stock, cost| Entry {
        index,
        stock,
        cost,
        title: None,
        text: None,
        stocked: true,
        affordable: true,
        blocked: None,
    };

    vec![
        Room::Shop {
            entries: vec![entry(0, Stock::Card(Some(on_sale(0))), 50)],
        },
        Room::Shop {
            entries: vec![entry(1, Stock::Card(Some(on_sale(0))), 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::Card(Some(on_sale(1))), 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::Card(None), 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::Relic(Some(nth::<Relic>(0))), 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::Relic(None), 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::Potion(Some(nth::<Potion>(0))), 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::Potion(None), 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::CardRemoval, 50)],
        },
        Room::Shop {
            entries: vec![entry(0, Stock::CardRemoval, 75)],
        },
        Room::Shop {
            entries: vec![Entry {
                stocked: false,
                ..entry(0, Stock::CardRemoval, 50)
            }],
        },
        Room::Shop {
            entries: vec![Entry {
                affordable: false,
                ..entry(0, Stock::CardRemoval, 50)
            }],
        },
        Room::Shop {
            entries: vec![Entry {
                blocked: Some(Blocked::NoPotionSlot),
                ..entry(0, Stock::CardRemoval, 50)
            }],
        },
        Room::Shop { entries: vec![] },
        Room::RestSite {
            options: vec![option(0, RestSiteOption::Heal, true)],
        },
        Room::RestSite {
            options: vec![option(1, RestSiteOption::Heal, true)],
        },
        Room::RestSite {
            options: vec![option(0, RestSiteOption::Smith, true)],
        },
        Room::RestSite {
            options: vec![option(0, RestSiteOption::Heal, false)],
        },
        Room::RestSite { options: vec![] },
        Room::Treasure {
            finished: false,
            options: vec![chest(0, Take::Relic(nth::<Relic>(0)), false)],
        },
        Room::Treasure {
            finished: true,
            options: vec![chest(0, Take::Relic(nth::<Relic>(0)), false)],
        },
        Room::Treasure {
            finished: false,
            options: vec![chest(1, Take::Relic(nth::<Relic>(0)), false)],
        },
        Room::Treasure {
            finished: false,
            options: vec![chest(0, Take::Relic(nth::<Relic>(1)), false)],
        },
        Room::Treasure {
            finished: false,
            options: vec![chest(0, Take::Skip, false)],
        },
        Room::Treasure {
            finished: false,
            options: vec![chest(0, Take::Skip, true)],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![event_option()],
        },
        Room::Event {
            event: nth::<Event>(1),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![event_option()],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: true,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![event_option()],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: None,
            text: Some("Something stirs.".into()),
            options: vec![event_option()],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: None,
            options: vec![event_option()],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                index: 1,
                ..event_option()
            }],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                key: EventOptionKey {
                    event: sts2_content::v0_107_1::events::Event::TeaMaster,
                    option: EventOptionId::TeaMaster(
                        sts2_content::v0_107_1::TeaMasterOption::EmberTea,
                    ),
                },
                ..event_option()
            }],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                title: None,
                ..event_option()
            }],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                text: None,
                ..event_option()
            }],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                locked: true,
                ..event_option()
            }],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                proceed: true,
                ..event_option()
            }],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                chosen: true,
                ..event_option()
            }],
        },
        Room::Event {
            event: nth::<Event>(0),
            finished: false,
            title: Some("A Nest".into()),
            text: Some("Something stirs.".into()),
            options: vec![EventOption {
                relic: None,
                ..event_option()
            }],
        },
    ]
}

/// One nudge to an observation, and the field it moves.
type Nudge = (&'static str, fn(&mut Observation<V>));

/// One nudge each, naming the field it moves.
/// Change the act's first node in place.
///
/// `Map::points` is shared, so a nudge rebuilds the slice. Every field of a
/// node still has to reach the digest, and the table below says so one field
/// at a time.
fn edit_first_node(observation: &mut Observation<V>, edit: impl FnOnce(&mut Node<V>)) {
    let mut points = observation.map.points.to_vec();
    edit(points.first_mut().expect("the fixture map has a node"));
    observation.map.points = points.into();
}

fn mutations() -> Vec<Nudge> {
    fn combat(o: &mut Observation<V>) -> &mut Combat<V> {
        o.combat.as_mut().expect("the fixture is in a fight")
    }
    fn creature(o: &mut Observation<V>) -> &mut Creature<V> {
        &mut combat(o).creatures[0]
    }
    fn playable(o: &mut Observation<V>) -> &mut Playable<V> {
        &mut combat(o).legal[0]
    }

    vec![
        ("act", |o| o.act += 1),
        ("floor", |o| o.floor += 1),
        ("hp", |o| o.hp += 1),
        ("max_hp", |o| o.max_hp += 1),
        ("alive", |o| o.alive = false),
        ("run_over", |o| o.run_over = true),
        ("victory", |o| o.victory = Some(true)),
        ("victory absent", |o| o.victory = None),
        ("gold", |o| o.gold += 1),
        ("relics: another", |o| o.relics.push(carried(2))),
        ("relics: a different one", |o| o.relics[0] = carried(3)),
        ("relics: reordered", |o| o.relics.swap(0, 1)),
        ("relics: none", |o| o.relics.clear()),
        ("relics: renamed", |o| {
            o.relics[0].title = Some("Akabeko".into())
        }),
        ("relics: a nameless one", |o| o.relics[0].title = None),
        // The count a relic prints is in its prose and nowhere else, so a
        // divergence in a counter is a divergence in this string.
        ("relics: a different count", |o| {
            o.relics[0].text = Some("Its counter reads 4.".into())
        }),
        ("potions", |o| o.potions[0] = held(1)),
        ("potions: renamed", |o| {
            o.potions[0].title = Some("Ashwater".into())
        }),
        ("potions: reworded", |o| {
            o.potions[0].text = Some("Deal 9 damage.".into())
        }),
        ("potions: no longer drinkable", |o| {
            o.potions[0].reason = Some(Undrinkable::OutOfCombat)
        }),
        ("potions: a different reason", |o| {
            o.potions[0].reason = Some(Undrinkable::NotYourTurn)
        }),
        ("potions: another target", |o| {
            o.potions[0].targets.push(at(7))
        }),
        ("potions: a different target", |o| {
            o.potions[0].targets[0] = at(9)
        }),
        ("potions: no targets", |o| o.potions[0].targets.clear()),
        ("potion_slots", |o| o.potion_slots += 1),
        ("deck: a different card", |o| o.deck[0] = card(9)),
        ("deck: reordered", |o| o.deck.swap(0, 1)),
        ("deck: upgraded", |o| o.deck[0].upgrade += 1),
        ("deck: afflicted", |o| {
            o.deck[0].affliction = Some(Affliction::Bound);
        }),
        ("deck: enchanted", |o| {
            o.deck[0].enchantment = Some(Enchantment::Adroit);
        }),
        ("in_combat", |o| o.in_combat = false),
        ("map.at", |o| o.map.at = Some(Coord::new(4, 11))),
        ("map.at absent", |o| o.map.at = None),
        ("map.at transposed", |o| o.map.at = Some(Coord::new(11, 3))),
        ("map.visited", |o| o.map.visited.swap(0, 1)),
        ("map.travelable.at", |o| {
            o.map.travelable[0].at = Coord::new(5, 12);
        }),
        ("map.travelable.kind", |o| {
            o.map.travelable[0].kind = MapPointType::Elite;
        }),
        ("map.points.at", |o| {
            edit_first_node(o, |node| node.at = Coord::new(9, 9));
        }),
        ("map.points.kind", |o| {
            edit_first_node(o, |node| node.kind = MapPointType::Shop);
        }),
        ("map.points.to", |o| {
            edit_first_node(o, |node| node.to.push(Coord::new(5, 12)));
        }),
        ("map.proceed", |o| o.map.proceed = true),
        ("rng.run: a draw", |o| {
            o.rng
                .run
                .get_mut(&RunRngType::Shuffle)
                .expect("a stream")
                .counter += 1;
        }),
        ("rng.run: another stream", |o| {
            o.rng.run.insert(RunRngType::Niche, Position::counted(1));
        }),
        ("rng.player: a draw", |o| {
            o.rng
                .player
                .get_mut(&PlayerRngType::Rewards)
                .expect("a stream")
                .counter += 1;
        }),
        ("room absent", |o| o.room = None),
        ("combat absent", |o| o.combat = None),
        ("combat.turn", |o| combat(o).turn += 1),
        ("combat.phase", |o| {
            combat(o).phase = PlayerTurnPhase::End;
        }),
        ("combat.energy", |o| combat(o).energy += 1),
        ("combat.round", |o| combat(o).round += 1),
        ("combat.encounter", |o| {
            combat(o).encounter = Some(nth::<Encounter>(1));
        }),
        ("combat.encounter absent", |o| combat(o).encounter = None),
        ("combat.hand", |o| combat(o).hand.swap(0, 1)),
        ("combat.draw", |o| combat(o).draw.push(card(7))),
        ("combat.discard", |o| combat(o).discard.clear()),
        ("combat.exhaust", |o| combat(o).exhaust[0] = card(8)),
        ("creature.id", |o| creature(o).id = at(9)),
        ("creature.model", |o| {
            creature(o).model = Some(ModelId::Monster(nth::<Monster>(1)));
        }),
        ("creature.model: another category", |o| {
            creature(o).model = Some(ModelId::Relic(nth::<Relic>(0)));
        }),
        ("creature.model absent", |o| creature(o).model = None),
        ("creature.side", |o| {
            creature(o).side = CombatSide::Player;
        }),
        ("creature.hp", |o| creature(o).hp += 1),
        ("creature.max_hp", |o| creature(o).max_hp += 1),
        ("creature.block", |o| creature(o).block += 1),
        ("creature.alive", |o| creature(o).alive = false),
        ("creature.powers: which", |o| {
            creature(o).powers[0].power = nth::<Power>(2);
        }),
        ("creature.powers: how many", |o| {
            creature(o).powers[0].amount = PowerAmount::Counted(4);
        }),
        ("creature.next_move absent", |o| {
            creature(o).next_move = None;
        }),
        ("next_move.id", |o| {
            creature(o).next_move.as_mut().expect("a move").id = MoveId::AcidGoop;
        }),
        ("next_move.intents: kind", |o| {
            creature(o).next_move.as_mut().expect("a move").intents[0].kind = IntentKind::Defend;
        }),
        ("next_move.intents: damage", |o| {
            creature(o).next_move.as_mut().expect("a move").intents[0]
                .attack
                .as_mut()
                .expect("an attack")
                .damage += 1;
        }),
        ("next_move.intents: repeats", |o| {
            creature(o).next_move.as_mut().expect("a move").intents[0]
                .attack
                .as_mut()
                .expect("an attack")
                .repeats += 1;
        }),
        ("next_move.intents: no numbers", |o| {
            creature(o).next_move.as_mut().expect("a move").intents[0].attack = None;
        }),
        ("next_move.intents: another", |o| {
            creature(o)
                .next_move
                .as_mut()
                .expect("a move")
                .intents
                .push(Intent {
                    kind: IntentKind::Buff,
                    attack: None,
                });
        }),
        ("playable.card", |o| playable(o).card = card(10)),
        ("playable.cost", |o| playable(o).cost = Cost::fixed(2)),
        // A card that costs nothing and one that is not priced at all are
        // different states of the game, so they must digest differently.
        ("playable.cost: none vs zero", |o| {
            playable(o).cost = Cost::None;
        }),
        ("playable.cost: X", |o| playable(o).cost = Cost::X),
        ("playable.star_cost", |o| {
            playable(o).star_cost = Cost::fixed(0);
        }),
        ("playable.target", |o| {
            playable(o).target = TargetType::AllEnemies;
        }),
        ("playable.reason: which", |o| {
            playable(o).reason = Some(UnplayableReasons::from_members(&[
                UnplayableReason::NoLivingAllies,
            ]));
        }),
        ("playable.reason: how many", |o| {
            playable(o).reason = Some(UnplayableReasons::from_members(&[
                UnplayableReason::EnergyCostTooHigh,
                UnplayableReason::NoLivingAllies,
            ]));
        }),
        ("playable.reason: playable", |o| playable(o).reason = None),
        ("playable.preventer", |o| {
            playable(o).preventer = Some(ModelId::Power(nth::<Power>(2)));
        }),
        ("playable.preventer absent", |o| {
            playable(o).preventer = None;
        }),
        ("playable.targets", |o| playable(o).targets.swap(0, 1)),
        ("preview.target", |o| {
            playable(o).effect[0].target = Some(at(2));
        }),
        ("preview.target absent", |o| {
            playable(o).effect[0].target = None;
        }),
        ("preview.text", |o| {
            playable(o).effect[0].text = Some("Deal {Damage} damage twice.".into());
        }),
        ("preview.vars", |o| {
            playable(o).effect[0].vars.insert(CardVar::Damage, 10);
        }),
        ("preview.base", |o| {
            playable(o).effect[0].base.insert(CardVar::Damage, 7);
        }),
        ("preview.vars: another", |o| {
            playable(o).effect[0].vars.insert(CardVar::Block, 5);
        }),
        ("combat.orbs absent", |o| combat(o).orbs = None),
        ("orbs.capacity", |o| {
            combat(o).orbs.as_mut().expect("orbs").capacity += 1;
        }),
        ("orbs.queue: which", |o| {
            combat(o).orbs.as_mut().expect("orbs").queue[0].orb = Orb::FrostOrb;
        }),
        ("orbs.queue: passive", |o| {
            combat(o).orbs.as_mut().expect("orbs").queue[0].passive += 1;
        }),
        ("orbs.queue: evoke", |o| {
            combat(o).orbs.as_mut().expect("orbs").queue[0].evoke += 1;
        }),
        ("orbs.queue: another", |o| {
            combat(o)
                .orbs
                .as_mut()
                .expect("orbs")
                .queue
                .push(Channelled {
                    orb: Orb::FrostOrb,
                    passive: 2,
                    evoke: 5,
                });
        }),
    ]
}

/// The simulator is measured on the rules, not on the wording.
///
/// Two things must hold. Prose alone must not move the rules digest, and
/// anything a rule sees must move it. The second is the one easy to lose.
#[test]
fn the_rules_digest_ignores_prose_and_nothing_else() {
    let plain = full();

    // Every prose field the observation has, reworded.
    let mut reworded = full();
    for relic in &mut reworded.relics {
        relic.title = Some("Something Else".into());
        relic.text = Some("It does something else.".into());
    }
    for potion in &mut reworded.potions {
        potion.title = Some("Another Flask".into());
        potion.text = Some("It does another thing.".into());
    }

    assert_ne!(
        plain.digest(),
        reworded.digest(),
        "the full digest is what two copies of the game are compared on, and \
         a description that changed is a difference"
    );
    assert_eq!(
        plain.rules_digest(),
        reworded.rules_digest(),
        "the same run, said differently, is the same run"
    );

    // And the exclusion is narrow: the ids beside the prose still count.
    let mut other_relic = full();
    other_relic.relics[0].relic = other_relic.relics[1].relic;
    assert_ne!(
        plain.rules_digest(),
        other_relic.rules_digest(),
        "a different relic is a different run, however it is described"
    );

    let mut poorer = full();
    poorer.gold -= 1;
    assert_ne!(
        plain.rules_digest(),
        poorer.rules_digest(),
        "dropping prose must not drop anything else"
    );
}

/// Every field a scenario can see reaches a section, and every section moves.
///
/// The sweep below proves each field reaches the whole digest. This proves the
/// split into sections loses none of it. A field in no section is one a
/// scenario is blind to.
///
/// Not "exactly one section". Taking the whole fight away moves every section
/// it owns, and that is right rather than double-counting.
#[test]
fn every_field_reaches_a_section_and_every_section_is_reached() {
    let base = full();
    let mut ever_moved: BTreeSet<Section> = BTreeSet::new();

    for (what, nudge) in mutations() {
        let mut moved = base.clone();
        nudge(&mut moved);
        if moved.rules_digest() == base.rules_digest() {
            continue;
        }

        let differing: Vec<Section> = base
            .sections()
            .into_iter()
            .zip(moved.sections())
            .filter(|((_, was), (_, now))| was != now)
            .map(|((section, _), _)| section)
            .collect();

        assert!(
            !differing.is_empty(),
            "{what} changed the digest and no section with it"
        );
        ever_moved.extend(differing);
    }

    let never: Vec<Section> = Section::ALL
        .into_iter()
        .filter(|section| !ever_moved.contains(section))
        .collect();
    assert!(never.is_empty(), "no nudge moves {never:?}");
}

/// Every field of an observation reaches the digest.
///
/// Each nudge is checked twice. First, that it changed the observation at all,
/// which catches a mutation that pointed at the wrong place. Second, that its
/// digest is one nothing else in the sweep produced.
#[test]
fn every_field_reaches_the_digest() {
    let base = full();
    let mut seen: BTreeMap<Digest, &'static str> = BTreeMap::new();
    seen.insert(base.digest(), "the fixture itself");

    for (what, nudge) in mutations() {
        let mut moved = base.clone();
        nudge(&mut moved);
        assert_ne!(moved, base, "{what} did not change the observation");

        if let Some(other) = seen.insert(moved.digest(), what) {
            panic!("{what} digests the same as {other}");
        }
    }
}

/// The nudges that move only wording, and so are invisible to the rules.
///
/// Checked both ways below: each must vanish from the rules digest, and
/// everything else must survive it.
const PROSE_ONLY: &[&str] = &[
    "relics: renamed",
    "relics: a nameless one",
    // A relic's counter is shown only in its description, so the rules digest
    // cannot see one change. The simulator's own counters are therefore not
    // compared by this route.
    "relics: a different count",
    "potions: renamed",
    "potions: reworded",
    "preview.text",
];

/// Everything the full digest sees, the rules digest sees too, bar the wording.
#[test]
fn every_field_but_prose_reaches_the_rules_digest() {
    let base = full();
    let mut seen: BTreeMap<Digest, &'static str> = BTreeMap::new();
    seen.insert(base.rules_digest(), "the fixture itself");

    for (what, nudge) in mutations() {
        let mut moved = base.clone();
        nudge(&mut moved);

        if PROSE_ONLY.contains(&what) {
            assert_eq!(
                moved.rules_digest(),
                base.rules_digest(),
                "{what} is listed as wording, but the rules digest saw it"
            );
            continue;
        }

        if let Some(other) = seen.insert(moved.rules_digest(), what) {
            panic!(
                "{what} digests the same as {other} once prose is dropped. \
                 Either it is wording, and belongs in PROSE_ONLY, or the rules \
                 digest is dropping something a rule can see."
            );
        }
    }
}

/// Every shape a room takes digests as itself.
#[test]
fn every_room_reaches_the_digest() {
    let base = full();
    let mut seen: BTreeMap<Digest, String> = BTreeMap::new();

    let mut absent = base.clone();
    absent.room = None;
    seen.insert(absent.digest(), "no room".to_string());

    for room in rooms() {
        let mut observed = base.clone();
        observed.room = Some(room.clone());

        let what = format!("{room:?}");
        if let Some(other) = seen.insert(observed.digest(), what.clone()) {
            panic!("{what} digests the same as {other}");
        }
    }
}

/// The same observation digests the same, and equality and the digest agree.
#[test]
fn the_same_observation_digests_the_same() {
    let base = full();
    assert_eq!(base.digest(), full().digest());
    assert_eq!(base.digest(), base.clone().digest());
}

/// A counter map's storage order is not part of the position.
///
/// Two `HashMap`s built by inserting the same pairs in different orders are the
/// same map and iterate differently. The run they describe is the same run.
#[test]
fn how_the_counters_were_stored_is_not_in_the_digest() {
    let mut forwards = full();
    let mut backwards = full();

    let pairs = [
        (RunRngType::UpFront, Position::counted(4)),
        (RunRngType::Shuffle, Position::counted(17)),
        (RunRngType::MonsterAi, Position::counted(2)),
        (RunRngType::Niche, Position::counted(8)),
        (RunRngType::CombatOrbs, Position::counted(3)),
    ];

    forwards.rng.run = pairs.iter().copied().collect();
    backwards.rng.run = pairs.iter().rev().copied().collect();

    assert_eq!(forwards.rng.run, backwards.rng.run);
    assert_eq!(forwards.digest(), backwards.digest());
}

/// Every id this build names spells itself as a name.
///
/// Not a digest property. `Sink::id` hashes a discriminant and never a name.
/// This is the wire property, checked here because this is where a real build's
/// ids are already in hand. An id that serde spells as anything but a string
/// reaches the protocol as a shape no name can be read out of.
#[test]
fn every_id_this_build_names_is_a_name() {
    fn is_a_name<T: serde::Serialize + std::fmt::Debug>(value: T) {
        let json = serde_json::to_value(&value).expect("an id serializes");
        assert!(
            json.is_string(),
            "{value:?} serializes as {json}, which the protocol cannot read a name out of"
        );
    }

    is_a_name(nth::<Card>(0));
    is_a_name(nth::<Relic>(0));
    is_a_name(nth::<Potion>(0));
    is_a_name(nth::<Power>(0));
    is_a_name(nth::<Monster>(0));
    is_a_name(nth::<Encounter>(0));
    is_a_name(nth::<Event>(0));
    is_a_name(<V as ContentVersion>::Character::Ironclad);
    is_a_name(Affliction::Bound);
    is_a_name(Enchantment::Adroit);
    is_a_name(Orb::DarkOrb);
    is_a_name(MapPointType::RestSite);
    is_a_name(CombatSide::Enemy);
    is_a_name(PlayerTurnPhase::Play);
    is_a_name(UnplayableReason::EnergyCostTooHigh);
    is_a_name(RunRngType::MonsterAi);
    is_a_name(PlayerRngType::Rewards);
    is_a_name(IntentKind::Attack);
    is_a_name(MoveId::AboutToBlowMove);
    is_a_name(RestSiteOption::Smith);
    is_a_name(TargetType::AnyEnemy);

    // `Cards` is the one that is not a plain enum: it is `#[serde(untagged)]`
    // over one enum per pool, so a card's name comes from the inner one. That
    // is a delegation and not a shape the digest has to know about, which this
    // is what says.
    is_a_name(nth::<Card>(120));
}
