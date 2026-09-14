//! The event table against the dump it came from.
//!
//! Most of an event is code. `GenerateInitialOptions` builds the player's
//! choices out of run state, and all 69 override it. What is here is the shell:
//! which acts offer one, and the few facts the room reads first.
//!
//! Two of those facts share a name in the game and mean different things. The
//! tests below keep them apart on purpose, because nothing else does.

use super::super::events::*;
use super::super::{EVENTS, VERSION};
use super::{build, rows, wire};
use std::collections::{BTreeMap, BTreeSet};
use sts2_core::{EventTable, Table};

fn event(id: &str) -> Event {
    *Event::ALL
        .iter()
        .find(|e| wire(e) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The anchor: a dump from another build has another hash.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(EVENTS),
        VERSION,
        "events.txt was dumped from a different build than this module claims"
    );
}

/// The dump column each flag comes from.
const FLAG_COLUMNS: [(usize, EventFlag); 4] = [
    (2, EventFlag::Ancient),
    (3, EventFlag::SharedBetweenPlayers),
    (4, EventFlag::Deterministic),
    (7, EventFlag::Gated),
];

#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(EVENTS, "event") {
        let id = row[1];
        let d = event(id).data();

        for (column, flag) in FLAG_COLUMNS {
            let set = if d.flags.contains(flag) { "1" } else { "0" };
            assert_eq!(set, row[column], "{id}: {flag:?}");
        }

        assert_eq!(wire(&d.layout), row[5], "{id}: layout");

        let encounter = d.encounter.map_or("-".to_string(), |e| wire(&e));
        assert_eq!(encounter, row[6], "{id}: encounter");

        checked += 1;
    }

    assert_eq!(checked, 69, "the build has 69 events");
    assert_eq!(Event::ALL.len(), 69);
}

/// The acts that offer an event, and the pool that any act can roll.
///
/// The two are disjoint, and codegen refuses to generate otherwise. An event in
/// both is offered twice by the same map.
#[test]
fn an_event_comes_from_an_act_or_from_the_pool_but_not_both() {
    let mut dumped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in rows(EVENTS, "act").chain(rows(EVENTS, "ancient")) {
        dumped.entry(row[3]).or_default().push(row[1]);
    }

    let pooled: BTreeSet<&str> = rows(EVENTS, "sharedpool")
        .chain(rows(EVENTS, "sharedancient"))
        .map(|r| r[2])
        .collect();
    assert_eq!(pooled.len(), 19, "the any-act pools have 19 events");

    let mut unlisted: Vec<String> = Vec::new();
    for e in Event::ALL {
        let id = wire(&e);
        let acts: Vec<String> = e
            .acts()
            .iter()
            .map(|a| format!("{a:?}").to_uppercase())
            .collect();
        let want = dumped.get(id.as_str()).cloned().unwrap_or_default();
        assert_eq!(acts, want, "{id}: acts");

        assert_eq!(e.any_act(), pooled.contains(id.as_str()), "{id}: any_act");
        assert!(
            !e.any_act() || acts.is_empty(),
            "{id} is in the pool and in an act's list"
        );

        if acts.is_empty() && !e.any_act() {
            unlisted.push(id);
        }
    }

    assert_eq!(
        unlisted,
        [
            "DEPRECATED_ANCIENT_EVENT",
            "DEPRECATED_EVENT",
            "MOCK_EVENT_MODEL",
            "THE_ARCHITECT"
        ],
        "the events nothing offers have changed"
    );
}

/// Two events are on more than one act's list.
#[test]
fn two_events_are_offered_by_two_acts() {
    let crossover: Vec<String> = Event::ALL
        .iter()
        .filter(|e| e.acts().len() > 1)
        .map(wire)
        .collect();

    assert_eq!(crossover, ["NEOW", "SUNKEN_STATUE"]);
    assert_eq!(event("NEOW").acts().len(), 2);
}

/// `SharedBetweenPlayers` is the multiplayer sense, not the any-act pool.
///
/// The game calls both "shared" and they are unrelated. `EventSynchronizer`
/// reads `IsShared` to run an event for every player at once, while
/// `AllSharedEvents` is where an event comes from. Eight events set the flag and
/// nineteen are in the pool, and the two sets barely overlap. This is the test
/// that stops the table from conflating them.
#[test]
fn shared_between_players_is_not_the_any_act_pool() {
    let flagged: Vec<String> = Event::ALL
        .iter()
        .filter(|e| e.data().flags.contains(EventFlag::SharedBetweenPlayers))
        .map(wire)
        .collect();

    assert_eq!(
        flagged,
        [
            "BATTLEWORN_DUMMY",
            "DENSE_VEGETATION",
            "FAKE_MERCHANT",
            "JUNGLE_MAZE_ADVENTURE",
            "MORPHIC_GROVE",
            "PUNCH_OFF",
            "THE_LANTERN_KEY",
            "WAR_HISTORIAN_REPY",
        ],
        "the events shared between players have changed"
    );

    // The flag says nothing about where the event comes from: two of these are
    // in the pool, and the other six are on an act's own list.
    let pooled_and_flagged: Vec<String> = Event::ALL
        .iter()
        .filter(|e| e.any_act() && e.data().flags.contains(EventFlag::SharedBetweenPlayers))
        .map(wire)
        .collect();
    assert_eq!(pooled_and_flagged, ["FAKE_MERCHANT", "WAR_HISTORIAN_REPY"]);

    assert!(!event("BATTLEWORN_DUMMY").any_act());
    assert_eq!(event("BATTLEWORN_DUMMY").acts().len(), 1);
}

/// `Deterministic` is not the negation of `SharedBetweenPlayers`.
///
/// `EventModel.IsDeterministic` defaults to `!IsShared`, and reading that as a
/// definition is wrong. `CRYSTAL_SPHERE` overrides it and is neither.
/// `EventRoom` branches on determinism alone, so the two have to stay apart.
#[test]
fn deterministic_is_its_own_fact() {
    let mut exceptions: Vec<String> = Vec::new();
    for e in Event::ALL {
        let f = e.data().flags;
        let shared = f.contains(EventFlag::SharedBetweenPlayers);
        let deterministic = f.contains(EventFlag::Deterministic);
        if deterministic == shared {
            exceptions.push(wire(&e));
        }
    }

    assert_eq!(
        exceptions,
        ["CRYSTAL_SPHERE"],
        "the events where determinism is not the opposite of shared have changed"
    );

    let f = event("CRYSTAL_SPHERE").data().flags;
    assert!(!f.contains(EventFlag::SharedBetweenPlayers));
    assert!(!f.contains(EventFlag::Deterministic));
}

/// An ancient is a subclass, and the layout follows it exactly.
///
/// Worth checking rather than assuming. If the two ever disagree, the table has
/// a flag and a layout saying different things about the same event. Only one of
/// them drives the room.
#[test]
fn the_ancients_are_exactly_the_ancient_layout() {
    let ancient: Vec<String> = Event::ALL
        .iter()
        .filter(|e| e.data().flags.contains(EventFlag::Ancient))
        .map(wire)
        .collect();
    let laid_out: Vec<String> = Event::ALL
        .iter()
        .filter(|e| e.data().layout == EventLayoutType::Ancient)
        .map(wire)
        .collect();

    assert_eq!(ancient, laid_out, "an ancient without the ancient layout");
    assert_eq!(ancient.len(), 9);
}

/// The three events that open into a fight name the encounter they start.
#[test]
fn an_event_with_a_fight_names_it() {
    let with: Vec<(String, String)> = Event::ALL
        .iter()
        .filter_map(|e| e.data().encounter.map(|enc| (wire(e), wire(&enc))))
        .collect();

    assert_eq!(
        with,
        [
            (
                "PUNCH_OFF".to_string(),
                "PUNCH_OFF_EVENT_ENCOUNTER".to_string()
            ),
            (
                "THE_ARCHITECT".to_string(),
                "THE_ARCHITECT_EVENT_ENCOUNTER".to_string()
            ),
            (
                "THE_LANTERN_KEY".to_string(),
                "MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER".to_string()
            ),
        ],
        "the events that start a fight have changed"
    );

    // Naming an encounter and drawing the combat layout are different things:
    // three name one and three use the layout, but they are not the same three.
    let combat: Vec<String> = Event::ALL
        .iter()
        .filter(|e| e.data().layout == EventLayoutType::Combat)
        .map(wire)
        .collect();
    assert_eq!(combat.len(), 3);
}

/// The wire form round-trips, and it is the only spelling of an id.
#[test]
fn an_event_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(EVENTS, "event").map(|r| r[1]).collect();

    for (e, id) in Event::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(e).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Event>(&json).unwrap(), e);
    }

    assert!(serde_json::from_str::<Event>("\"NOT_AN_EVENT\"").is_err());
}

#[test]
fn event_id_hashes_are_generated_from_the_wire_ids() {
    for event in Event::ALL {
        assert_eq!(
            event.event_id_hash(),
            sts2_core::rng::name_hash(&wire(event)) as u32
        );
    }
}
