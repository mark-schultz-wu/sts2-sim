//! Tests that need a real build to spell.
//!
//! An integration test rather than a `#[cfg(test)]` module, because naming a
//! build here means depending on `sts2-content`, which depends back on this
//! crate. Cargo allows that cycle, but a *unit* test rebuilds this crate under
//! `cfg(test)` and links a second copy. `V0_107_1` then implements the other
//! copy's `ContentVersion`, and nothing lines up. An integration test links the
//! same copy `sts2-content` did.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::Seed;
use sts2_core::{ContentVersion, Decision, Recording, Setup, Start, Step};

type V = V0_107_1;
type Character = <V as ContentVersion>::Character;
type AscensionLevel = <V as ContentVersion>::AscensionLevel;
type Encounter = <V as ContentVersion>::Encounter;

#[test]
fn a_recording_takes_either_half_of_a_step() {
    let mut r: Recording<V> = Recording::new(Start {
        seed: Seed::new("SP1RE1").expect("a seed"),
        character: Character::Ironclad,
        ascension: AscensionLevel::None,
        modifiers: vec![],
    });
    assert!(r.is_empty(), "the start is not a step");

    r.push(Setup::Fight {
        encounter: Encounter::ShrinkerBeetleWeak,
    });
    r.push(Decision::EndTurn);

    assert_eq!(r.len(), 2);
    assert_eq!(r.start().seed.as_str(), "SP1RE1");
    assert!(
        matches!(r.steps()[0], Step::Setup(_)),
        "a constructed run records too"
    );
    assert!(matches!(r.steps()[1], Step::Decision(_)));
}
