//! Playing a card, and the plays the fight refuses.
//!
//! The recording only ever holds legal moves, so the ratchet never asks what
//! a refusal does. These do. A refused play leaves the fight exactly as it
//! stood, which is the property the whole design rests on.

use sts2_content::v0_107_1::ascension::AscensionLevel;
use sts2_content::v0_107_1::{
    V0_107_1, cards::Cards, cards::RegentCard, characters::Character, runstate::UnplayableReason,
};
use sts2_core::fixture::{at, hand};
use sts2_core::{Answer, Coord, Decision, Play, Reason, Seed, SideKinds, Start};
use sts2_sim::Sim;

type V = V0_107_1;

/// A run standing in its first fight, which is where a card can be played.
fn fighting() -> Sim<V> {
    let mut sim = Sim::begin(
        (),
        &Start {
            seed: Seed::new("0RACLE1").expect("a canonical seed"),
            character: Character::Regent,
            ascension: AscensionLevel::None,
            modifiers: Vec::new(),
            wins: 0,
        },
    )
    .expect("the run starts");
    sim.play(&Decision::ChooseOption { index: 0 })
        .expect("the choice is implemented")
        .expect("the choice is legal");
    sim.play(&Decision::Answer(Answer::Cards([1].into())))
        .expect("the answer is implemented")
        .expect("the answer is legal");
    sim.play(&Decision::Travel {
        to: Coord::new(5, 1),
    })
    .expect("travel is implemented")
    .expect("the destination is legal");
    sim
}

#[test]
fn a_strike_spends_energy_and_lands_on_what_it_was_pointed_at() {
    let mut sim = fighting();
    let before = sim.observe().expect("the run reports").observation;
    let fight = before.fight().expect("the run is in a fight");
    let strike = fight
        .legal
        .iter()
        .position(|play| play.card.card == Cards::Regent(RegentCard::Strike))
        .expect("the opening hand holds a Strike");
    let hp = fight.creatures[1].hp;

    sim.play(&Decision::PlayCard {
        index: hand(strike as u8),
        target: Some(at(1)),
    })
    .expect("the card is ported")
    .expect("the play is legal");

    let after = sim.observe().expect("the run reports").observation;
    let fight = after.fight().expect("the fight goes on");
    assert_eq!(fight.energy, 2, "a Strike costs one energy");
    assert!(fight.creatures[1].hp < hp, "the target took the damage");
    assert_eq!(fight.hand.len(), 4, "the card left the hand");
    assert_eq!(fight.discard.len(), 1, "and went to the discard pile");
}

#[test]
fn a_defend_gains_block_without_naming_a_creature() {
    let mut sim = fighting();
    let observation = sim.observe().expect("the run reports").observation;
    let fight = observation.fight().expect("the run is in a fight");
    let defend = fight
        .legal
        .iter()
        .position(|play| play.card.card == Cards::Regent(RegentCard::Defend))
        .expect("the opening hand holds a Defend");

    sim.play(&Decision::PlayCard {
        index: hand(defend as u8),
        target: None,
    })
    .expect("the card is ported")
    .expect("the play is legal");

    let after = sim.observe().expect("the run reports").observation;
    let fight = after.fight().expect("the fight goes on");
    let player = fight
        .creatures
        .iter()
        .find(|creature| creature.side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER)
        .expect("the player is in the fight");
    assert!(player.block > 0, "the block landed on the player");
}

#[test]
fn a_refused_play_leaves_the_fight_where_it_stood() {
    use sts2_content::v0_107_1::cards::TargetType;
    use sts2_core::CardIdent;

    let mut sim = fighting();
    let before = sim.observe().expect("the run reports");
    let fight = before.observation.fight().expect("the run is in a fight");
    let find = |card| {
        let at = fight
            .legal
            .iter()
            .position(|play| play.card.card == Cards::Regent(card))
            .expect("the opening hand holds the card");
        hand(u8::try_from(at).expect("a hand index fits a byte"))
    };
    let (strike, defend) = (find(RegentCard::Strike), find(RegentCard::Defend));

    // An index past the hand, a creature that is not in the fight, an attack
    // pointed at nobody, and a Defend pointed at a creature. Each is refused
    // for its own reason: `IsValidTarget` wants a creature for a card that
    // names one, and nothing for a card that names none.
    let refusals = [
        (
            Decision::PlayCard {
                index: hand(9),
                target: Some(at(1)),
            },
            Reason::OutOfRange { index: 9, count: 5 },
        ),
        (
            Decision::PlayCard {
                index: hand(0),
                target: Some(at(9)),
            },
            Reason::NoSuchCreature { id: at(9) },
        ),
        (
            Decision::PlayCard {
                index: strike,
                target: None,
            },
            Reason::WrongTarget {
                card: CardIdent::new(Cards::Regent(RegentCard::Strike)),
                target: None,
                takes: TargetType::AnyEnemy,
            },
        ),
        (
            Decision::PlayCard {
                index: defend,
                target: Some(at(1)),
            },
            Reason::WrongTarget {
                card: CardIdent::new(Cards::Regent(RegentCard::Defend)),
                target: Some(at(1)),
                takes: TargetType::SelfTarget,
            },
        ),
    ];

    for (decision, want) in refusals {
        let refusal = sim
            .play(&decision)
            .expect("the fight answered")
            .expect_err("the play is refused");
        assert_eq!(refusal.what, "play_card");
        assert_eq!(refusal.reason, want);
        assert_eq!(
            sim.observe().expect("the run still reports"),
            before,
            "a refusal moved the fight"
        );
    }
}

#[test]
fn a_card_the_pools_cannot_pay_for_is_unplayable_before_it_is_refused() {
    let mut sim = fighting();

    // Spend the turn's energy on whatever the hand can pay for. The cards
    // still in hand then say why they cannot be played, rather than the fight
    // refusing them late.
    loop {
        let observation = sim.observe().expect("the run reports").observation;
        let fight = observation.fight().expect("the run is in a fight");
        let Some(playable) = fight.legal.iter().position(|play| play.reason.is_none()) else {
            break;
        };
        let target = fight.legal[playable].targets.first().copied();
        sim.play(&Decision::PlayCard {
            index: hand(playable as u8),
            target,
        })
        .expect("the card is ported")
        .expect("the play is legal");
    }

    let observation = sim.observe().expect("the run reports").observation;
    let fight = observation.fight().expect("the fight goes on");
    assert_eq!(fight.energy, 0, "the turn's energy is spent");
    assert!(!fight.hand.is_empty(), "cards are left to refuse");
    for play in &fight.legal {
        let why = play.reason.expect("nothing in hand can be paid for");
        assert!(
            why.contains(UnplayableReason::EnergyCostTooHigh),
            "{:?}: {why:?}",
            play.card
        );
    }
}

#[test]
fn a_selection_refuses_the_clicks_its_screen_would() {
    use sts2_content::v0_107_1::cards::IroncladCard;
    use sts2_core::{Construct as _, Gift, Setup};

    // Burning Pact asks for exactly one card from the hand.
    let mut sim = fighting();
    sim.setup(&Setup::Grant(Gift::Card(Cards::Ironclad(
        IroncladCard::BurningPact,
    ))))
    .expect("the grant is implemented")
    .expect("the grant is accepted");
    let seen = sim.observe().expect("the run reports");
    let fight = seen.observation.fight().expect("the run is in a fight");
    let pact = fight
        .legal
        .iter()
        .position(|play| play.card.card == Cards::Ironclad(IroncladCard::BurningPact))
        .expect("the granted card is in hand");
    sim.play(&Decision::PlayCard {
        index: hand(u8::try_from(pact).expect("a hand index fits a byte")),
        target: None,
    })
    .expect("the play is implemented")
    .expect("the play is legal");
    let asked = sim.observe().expect("the run reports");
    let Some(sts2_core::Question::SelectCards { options, .. }) = &asked.asking else {
        panic!("Burning Pact asks for a card, not {:?}", asked.asking);
    };
    let offered = options.len();

    let refuse = |sim: &mut Sim<V>, answer: Answer, want: Reason<V>| {
        let before = sim.observe().expect("the run reports");
        let refusal = sim
            .play(&Decision::Answer(answer))
            .expect("the selection answered")
            .expect_err("the click is refused");
        assert_eq!(refusal.reason, want, "{answer:?}");
        assert_eq!(
            sim.observe().expect("the run still reports"),
            before,
            "a refused {answer:?} moved the selection"
        );
    };
    let click = |sim: &mut Sim<V>, answer: Answer| {
        sim.play(&Decision::Answer(answer))
            .expect("the selection answered")
            .expect("the click is accepted");
    };
    let wrong = |chosen| Reason::WrongCount {
        chosen,
        min: 1,
        max: 1,
    };

    refuse(&mut sim, Answer::Undo, Reason::NothingPicked);
    refuse(&mut sim, Answer::Done, wrong(0));
    refuse(
        &mut sim,
        Answer::Pick(99),
        Reason::OutOfRange {
            index: 99,
            count: offered,
        },
    );
    click(&mut sim, Answer::Pick(0));
    refuse(&mut sim, Answer::Pick(0), Reason::Duplicate { index: 0 });
    refuse(&mut sim, Answer::Pick(1), wrong(2));
    click(&mut sim, Answer::Undo);
    click(&mut sim, Answer::Pick(1));
    let picked = sim.observe().expect("the run reports");
    let Some(sts2_core::Question::SelectCards { picked, .. }) = picked.asking else {
        panic!("a pick leaves the selection parked");
    };
    assert_eq!(picked, [1].into());
    click(&mut sim, Answer::Done);
    assert_eq!(
        sim.observe().expect("the run reports").asking,
        None,
        "done hands the pick over"
    );
}
