//! The hook surface, against the dump that measured it.
//!
//! The one dump with no generated table beside it. It describes the interface
//! between an engine and the things that react to it. A simulator implements
//! that rather than looking it up, so there is nothing to generate. This is the
//! only place the measurement is held to anything.
//!
//! What these pin is a shape that has to stay true for a hook trait to be
//! trustworthy. Every hook is dispatched. The ones worth modelling are the ones
//! some model answers. And nothing in the build is invisible to the count. The
//! `dispatchers` column got that wrong twice while it was being written, and
//! both times an anomaly here is what showed it. So the cross-checks are kept
//! rather than replaced by the counts they imply.

use super::super::{HOOKS, VERSION};
use super::{build, rows};
use std::collections::{BTreeMap, BTreeSet};

/// Every hook the build declares.
fn declared() -> BTreeSet<&'static str> {
    rows(HOOKS, "hook").map(|r| r[1]).collect()
}

/// Every hook at least one model answers.
fn answered() -> BTreeSet<&'static str> {
    rows(HOOKS, "implements").map(|r| r[2]).collect()
}

/// The anchor: a dump from another build describes another surface.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(HOOKS),
        VERSION,
        "hooks.txt was dumped from a different build than this module claims"
    );
}

/// The shape of the surface, in one place.
#[test]
fn the_surface_is_the_size_it_was_measured_at() {
    let models: BTreeSet<&str> = rows(HOOKS, "implements").map(|r| r[1]).collect();

    assert_eq!(declared().len(), 182, "hooks declared on AbstractModel");
    assert_eq!(answered().len(), 155, "hooks some model answers");
    assert_eq!(rows(HOOKS, "implements").count(), 974, "answers");
    assert_eq!(models.len(), 569, "models answering at least one hook");
}

/// Every hook is dispatched, and nothing is answered that is not declared.
///
/// Both directions, because each caught a real bug in the dump. Name-matching
/// `Hook`'s public methods reported 43 hooks as dispatched by nothing.
///
/// The replacement excluded model types from a metadata-token scan, on the
/// reasoning that a model reaching a hook token is `base.AfterX()`. That hid the
/// one hook `CardModel` fires at other models. A count catches neither.
#[test]
fn every_hook_is_dispatched_and_every_answer_is_a_hook() {
    let undispatched: Vec<&str> = rows(HOOKS, "hook")
        .filter(|r| r[4] == "-")
        .map(|r| r[1])
        .collect();
    assert_eq!(
        undispatched,
        [] as [&str; 0],
        "a hook is declared and answered but nothing fires it"
    );

    let stray: Vec<&str> = answered().difference(&declared()).copied().collect();
    assert_eq!(
        stray,
        [] as [&str; 0],
        "a model answers a hook that is not one"
    );
}

/// Twenty-six hooks are dispatched and answered by nothing.
///
/// Slots the engine fires into that no model in this build listens on. Worth a
/// number rather than a list. A trait has no methods for them, and a build that
/// starts using one is a signal to look.
#[test]
fn twenty_seven_hooks_are_slots_nothing_uses() {
    let dead: Vec<&str> = declared().difference(&answered()).copied().collect();

    assert_eq!(dead.len(), 27);
    assert!(dead.contains(&"AfterRestSiteSmith"));
    assert!(!dead.contains(&"AfterCardPlayed"));
}

/// `Hook` fires almost everything, and six other classes fire the rest.
///
/// Not a curiosity. The trait's shape follows the engine's. Dispatch being
/// nearly all in one place is what makes a single hook-listener walk the right
/// thing to port. The seven hooks with two dispatchers are the ones where a
/// helper answers the same question outside combat.
#[test]
fn dispatch_is_almost_all_in_one_place() {
    let mut by_class: BTreeMap<&str, usize> = BTreeMap::new();
    let mut shared = Vec::new();

    for row in rows(HOOKS, "hook") {
        for class in row[4].split(',') {
            *by_class.entry(class).or_default() += 1;
        }
        if row[4].contains(',') {
            shared.push(row[1]);
        }
    }

    assert_eq!(by_class[&"Hook"], 179);
    assert_eq!(
        by_class.keys().copied().collect::<Vec<_>>(),
        [
            "CardCostHelper",
            "CardModel",
            "CardPileCmd",
            "CardReward",
            "EventRoomHandler",
            "Hook",
            "NTargetManager"
        ],
        "who fires a hook has changed"
    );

    assert_eq!(shared.len(), 7, "hooks with more than one dispatcher");
}

/// Nothing in the build is invisible to the count.
///
/// Every dump in the mod enumerates models by asking the model database for an
/// id and swallowing the failure. A concrete model with no id is missing from
/// all of them and says so nowhere. Here it is a hole in a completeness claim
/// the hook trait rests on. Measured rather than assumed, and today it is
/// empty.
#[test]
fn no_model_answers_a_hook_without_an_id() {
    assert_eq!(
        rows(HOOKS, "unregistered").count(),
        0,
        "a model with no id answers a hook, so the checklist is incomplete"
    );
}

/// The object protocol is not a hook surface.
///
/// `ToString`, `Equals` and `CompareTo` are virtual, declared on
/// `AbstractModel`, and overridden by models. So all three read as hooks until
/// filtered. `ToString` was caught by being the only "hook" nothing dispatched.
/// `CompareTo` needed the interface map, being a contract with the framework's
/// sorting rather than something an engine fires. Together they were 584
/// phantom answers, all of them cards inheriting one override.
#[test]
fn the_object_protocol_is_not_in_the_surface() {
    for member in ["ToString", "Equals", "GetHashCode", "CompareTo"] {
        assert!(!declared().contains(member), "{member} is not a hook");
    }
}

/// Four classes answer hooks on behalf of more than one model.
///
/// This is what `declaredBy` is for, and the payoff is narrower than it looks.
/// Of 951 answers, all but 23 are the model's own code. The exceptions are
/// worth naming because they are the porting wins. One implementation of
/// `TemporaryStrengthPower` covers fourteen ids, so a checklist reading every
/// id as separate work overstates it.
#[test]
fn a_few_implementations_are_shared_by_several_models() {
    let mut families: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for row in rows(HOOKS, "implements") {
        families.entry(row[3]).or_default().insert(row[1]);
    }

    let shared: BTreeMap<&str, usize> = families
        .iter()
        .filter(|(_, models)| models.len() > 1)
        .map(|(class, models)| (*class, models.len()))
        .collect();

    assert_eq!(
        shared,
        BTreeMap::from([
            ("DecimillipedeSegment", 3),
            ("TemporaryDexterityPower", 4),
            ("TemporaryFocusPower", 4),
            ("TemporaryStrengthPower", 14),
        ]),
        "which implementations are shared has changed"
    );
}
