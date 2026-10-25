//! Travel legality before room entry begins.

use sts2_content::v0_107_1::{
    V0_107_1, ascension::AscensionLevel, cards::TargetType, characters::Character,
    encounters::Encounter, monsters::Monster,
};
use sts2_core::fixture::at;
use sts2_core::{Answer, Coord, Decision, ModelId, Play, Reason, Seed, Start};
use sts2_sim::Sim;

type V = V0_107_1;

fn opened() -> Sim<V> {
    let mut sim = Sim::begin(
        (),
        &Start {
            seed: Seed::new("0RACLE1").expect("a canonical seed"),
            players: vec![sts2_core::Player {
                character: Character::Regent,
                wins: 0,
            }],
            ascension: AscensionLevel::None,
            modifiers: Vec::new(),
        },
    )
    .expect("the run starts");
    sim.play(&Decision::ChooseOption {
        index: 0,
        target: None,
    })
    .expect("the choice is implemented")
    .expect("the choice is legal");
    sim.play(&Decision::Answer(Answer::Cards([1].into())))
        .expect("the answer is implemented")
        .expect("the answer is legal");
    sim
}

#[test]
fn an_unreachable_destination_is_refused_without_moving_the_run() {
    let mut sim = opened();
    let before = sim.observe().expect("the run can be observed");
    let nowhere = Coord::new(0, 0);

    let refused = sim
        .play(&Decision::Travel { to: nowhere })
        .expect("travel was answered")
        .expect_err("the node is not reachable");

    assert_eq!(refused.what, "travel");
    assert_eq!(refused.reason, Reason::NotTravelable { at: nowhere });
    assert_eq!(
        sim.observe().expect("the run can still be observed"),
        before
    );
}

#[test]
fn travelling_into_a_monster_room_opens_the_fight() {
    let mut sim = opened();
    sim.play(&Decision::Travel {
        to: Coord::new(5, 1),
    })
    .expect("travel is implemented")
    .expect("the destination is legal");
    let observation = sim.observe().expect("the run can be observed").observation;

    assert_eq!(observation.floor, 2);
    assert_eq!(observation.map.at, Some(Coord::new(5, 1)));
    assert!(observation.in_combat);
    assert_eq!(observation.room, None);

    let combat = observation.fight().expect("the run is in a fight");
    assert_eq!(combat.turn, 1);
    assert_eq!(combat.round, 1);
    assert_eq!(combat.encounter, Some(Encounter::ToadpolesWeak));
    // Three energy from the Regent, three stars from Divine Right.
    assert_eq!((combat.energy, combat.stars), (3, 3));
    assert_eq!(combat.hand.len(), 5);
    assert_eq!(
        combat.hand.len() + combat.draw.len(),
        observation.deck.len()
    );
    assert!(combat.discard.is_empty() && combat.exhaust.is_empty());

    // The player first, then the encounter's monsters in the order it fields
    // them. Every card in the opening hand is playable.
    assert_eq!(
        combat
            .creatures
            .iter()
            .map(|creature| (creature.id, creature.model, creature.max_hp))
            .collect::<Vec<_>>(),
        [
            (at(0), Some(ModelId::Character(Character::Regent)), 75),
            (at(1), Some(ModelId::Monster(Monster::Toadpole)), 22),
            (at(2), Some(ModelId::Monster(Monster::Toadpole)), 25),
        ]
    );
    assert!(combat.legal.iter().all(|play| play.reason.is_none()));
}

/// An attack names each living enemy, and a skill on its owner names nobody.
#[test]
fn a_cards_targets_are_the_creatures_it_can_be_pointed_at() {
    let mut sim = opened();
    sim.play(&Decision::Travel {
        to: Coord::new(5, 1),
    })
    .unwrap()
    .unwrap();
    let observation = sim.observe().unwrap().observation;
    let combat = observation.fight().expect("the run is in a fight");

    for play in &combat.legal {
        match play.target {
            TargetType::AnyEnemy => {
                assert_eq!(play.targets.as_slice(), [at(1), at(2)], "{:?}", play.card)
            }
            TargetType::SelfTarget => assert!(play.targets.is_empty(), "{:?}", play.card),
            other => panic!("the opening hand has no {other:?} card"),
        }
        // One preview, with no target, because nothing moves the numbers yet.
        assert_eq!(play.effect.len(), 1, "{:?}", play.card);
        assert_eq!(play.effect[0].target, None, "{:?}", play.card);
        assert_eq!(play.effect[0].vars, play.effect[0].base, "{:?}", play.card);
    }
}
