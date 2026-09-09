//! Starting a simulator and resolving its first choice against a recorded run.

use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::ascension::AscensionLevel;
use sts2_content::v0_107_1::characters::Character;
use sts2_core::{Answer, Decision, Play, Seed, Start};
use sts2_sim::Sim;

type V = V0_107_1;

#[test]
fn the_simulator_opens_where_the_game_opened() {
    let start = Start {
        seed: Seed::new("0RACLE1").expect("the fixture's seed is a seed"),
        character: Character::Regent,
        ascension: AscensionLevel::None,
        modifiers: Vec::new(),
    };
    let mut sim = Sim::<V>::begin((), &start).expect("the simulator starts this run");

    let opened = sim
        .observe()
        .expect("the simulator reports its run")
        .expect("the run has an observation");
    assert_eq!(
        format!("{}", opened.observation.rules_digest()),
        "a94e804fd91b096a"
    );

    let chosen = sim
        .play(&Decision::ChooseOption { index: 0 })
        .expect("the simulator implements this option")
        .expect("the game accepts this option");
    assert_eq!(
        format!("{}", chosen.observation.rules_digest()),
        "36dad63b8bb88605"
    );

    let answered = sim
        .play(&Decision::Answer(Answer::Cards(vec![1])))
        .expect("the simulator implements this answer")
        .expect("the game accepts this answer");
    assert_eq!(
        format!("{}", answered.observation.rules_digest()),
        "31052d0ba1079207"
    );
}
