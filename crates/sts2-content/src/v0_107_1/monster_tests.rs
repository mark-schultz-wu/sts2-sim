//! The monster table against the dump it came from.
//!
//! Structured like the card tests and for the same reason: a generated table is
//! only trustworthy while something replays the dump against it. What is
//! different here is that a monster's table is a graph, so most of this checks
//! that the graph closes. The codegen turned state *names* into *indices*, and
//! a mistranslation there would be invisible right up until a monster took a
//! turn and walked to the wrong move.

use super::monsters::*;
use super::{MONSTERS, VERSION};
use crate::{Field, Scaled};

fn rows(kind: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    MONSTERS
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(move |f| f.first() == Some(&kind))
}

fn monster(id: &str) -> Monster {
    *Monster::ALL
        .iter()
        .find(|m| m.id() == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// A monster's state names in index order.
///
/// The dump emits one row per state, sorted by id, and the codegen made a
/// state's position in that sequence its index. So walking the rows in order
/// reproduces the mapping — which is what lets every assertion below compare a
/// generated index against the name the dump used.
fn state_names(id: &str) -> Vec<&'static str> {
    MONSTERS
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|f| matches!(f.first(), Some(&"move" | &"rand" | &"cond")) && f[1] == id)
        .map(|f| f[2])
        .collect()
}

/// The dump's `repeatType`, `maxTimes` and `cooldown` columns as the [`Repeat`]
/// they should have generated, spelled out here independently of the codegen
/// that produced the table — otherwise this would only prove codegen agrees
/// with itself.
///
/// Three columns become one enum and the mapping is not one to one, which is
/// the whole reason to restate it rather than compare strings. `CannotRepeat`
/// is `InARow(One)` because `GetStateWeight` computes
/// `num2 = CannotRepeat ? 1 : maxTimes` and then runs the same loop for both;
/// `maxTimes` is only read for `CanRepeatXTimes`, so a nonzero one anywhere
/// else means the mapping has stopped being true; and a cooldown beside a
/// repeat type absorbs it, or is absorbed by `UseOnlyOnce`, because
/// `GetStateWeight` ORs the two tests and one of them always implies the
/// other. The argument is in `Repeat`'s own documentation.
fn repeat(kind: &str, max_times: &str, cooldown: &str) -> Repeat {
    let max_times: u8 = max_times.parse().expect("a maxTimes");
    let cooldown: u8 = cooldown.parse().expect("a cooldown");

    let count = |n: u8| match n {
        1 => RepeatCount::One,
        2 => RepeatCount::Two,
        3 => RepeatCount::Three,
        n => panic!("no RepeatCount spelling for {n}"),
    };

    if kind != "CanRepeatXTimes" {
        assert_eq!(max_times, 0, "{kind} carries a maxTimes");
    }

    match (kind, cooldown) {
        ("UseOnlyOnce", _) => Repeat::Once,
        ("CanRepeatForever", 0) => Repeat::Forever,
        ("CannotRepeat", 0) => Repeat::InARow(RepeatCount::One),
        ("CanRepeatXTimes", 0) => Repeat::InARow(count(max_times)),
        ("CanRepeatForever" | "CannotRepeat" | "CanRepeatXTimes", 1) => {
            Repeat::InARow(RepeatCount::One)
        }
        ("CanRepeatForever" | "CannotRepeat" | "CanRepeatXTimes", c) => Repeat::Cooldown(count(c)),
        (other, _) => panic!("unknown MoveRepeatType {other}"),
    }
}

/// The anchor, same as the card dump's: a dump from another build has another
/// hash, and a table regenerated against one is the failure this prevents.
#[test]
fn the_dump_came_from_this_version() {
    let header = MONSTERS.lines().next().expect("the dump has a header");
    let hash: u32 = header
        .rsplit_once(' ')
        .expect("the header names a content hash")
        .1
        .parse()
        .expect("the content hash is a number");

    assert_eq!(
        hash, VERSION.content_hash,
        "monsters.txt was dumped from a different build than this module claims"
    );
}

/// The eleven ascension levels, densely numbered, because a level is a
/// threshold compared against the run's scalar level — so the discriminants
/// are the thresholds and a renumber would silently move every step.
#[test]
fn the_ascension_levels_are_the_thresholds() {
    let levels: Vec<Vec<&str>> = rows("enum").filter(|r| r[1] == "AscensionLevel").collect();
    assert_eq!(levels.len(), 11, "ascension level count changed");

    for (i, l) in levels.iter().enumerate() {
        assert_eq!(l[2], i.to_string(), "AscensionLevel is no longer dense");
    }
    assert_eq!(levels[8][3], "ToughEnemies");
    assert_eq!(levels[9][3], "DeadlyEnemies");
}

/// Ascension moves numbers, never a monster's graph.
///
/// The dump reads every value at every level; a `structurechange` row would
/// mean a monster gains or loses a state or a branch as ascension rises, which
/// the table has no way to express. Codegen refuses on one, and this says so
/// out loud rather than leaving it implied.
#[test]
fn ascension_never_changes_a_graph() {
    assert_eq!(rows("structurechange").count(), 0);
}

/// Where ascension actually moves something, measured rather than assumed.
///
/// 102 of 126 monsters gain HP at `ToughEnemies`. Damage almost always steps at
/// `DeadlyEnemies` — except Gremlin Merc, whose three attacks step at
/// `ToughEnemies` instead. That exception is the reason the dump sweeps every
/// level instead of reading two names out of the source.
#[test]
fn the_ascension_thresholds_are_the_two_enemy_levels() {
    let hp: Vec<&str> = rows("scalehp").map(|r| r[2]).collect();
    assert_eq!(hp.len(), 102, "monsters whose HP scales");
    assert!(hp.iter().all(|l| *l == "8"), "HP steps somewhere new");

    let mut early: Vec<&str> = rows("scaleintent")
        .filter(|r| r[4] != "9")
        .map(|r| r[1])
        .collect();
    early.sort_unstable();
    early.dedup();
    assert_eq!(
        early,
        ["GREMLIN_MERC"],
        "a new monster steps off DeadlyEnemies"
    );

    // Weights are the one dumped number ascension never touches.
    assert_eq!(rows("scalebranch").count(), 0);
}

/// `Scaled::at` walks the thresholds the way the game's `>=` does.
#[test]
fn a_scaled_value_steps_at_its_threshold() {
    let hp = monster("GREMLIN_MERC").data().min_hp;
    assert_eq!(hp.at(0), 47);
    assert_eq!(hp.at(7), 47);
    assert_eq!(hp.at(8), 51);
    assert_eq!(hp.at(10), 51);
    // The game tests `_level >= (int)level`, so past the top is still the top.
    assert_eq!(hp.at(255), 51);

    let flat: Scaled<u16> = Scaled {
        base: 3,
        step: None,
    };
    assert!(flat.is_flat());
    assert_eq!(flat.at(10), 3);
}

#[test]
fn every_monster_is_generated() {
    let dumped: Vec<&str> = rows("monster").map(|r| r[1]).collect();
    assert_eq!(dumped.len(), 126, "the build has 126 monsters");
    assert_eq!(Monster::ALL.len(), dumped.len());

    for id in dumped {
        assert_eq!(monster(id).id(), id);
    }
}

/// The four acts, in the order a run visits them — which is the order
/// `ModelDb.Acts` yields them and therefore the order the dump wrote them in.
/// The discriminants encode that order, so a reshuffle is a silent remapping.
#[test]
fn the_acts_are_in_run_order() {
    let mut seen: Vec<&str> = Vec::new();
    for row in rows("act") {
        if !seen.contains(&row[1]) {
            seen.push(row[1]);
        }
    }

    assert_eq!(seen, ["OVERGROWTH", "UNDERDOCKS", "HIVE", "GLORY"]);
    assert_eq!(
        seen.iter().map(|a| act(a) as u8).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
}

fn act(name: &str) -> Act {
    match name {
        "OVERGROWTH" => Act::Overgrowth,
        "UNDERDOCKS" => Act::Underdocks,
        "HIVE" => Act::Hive,
        "GLORY" => Act::Glory,
        other => panic!("unknown act {other}"),
    }
}

#[test]
fn a_monsters_acts_are_the_ones_whose_encounters_name_it() {
    for row in rows("monster") {
        let expected: Vec<Act> = rows("act")
            .filter(|a| a[2] == row[1])
            .map(|a| act(a[1]))
            .collect();

        assert_eq!(monster(row[1]).acts(), expected, "{}: acts", row[1]);
    }

    // Not every monster has one. These arrive only by being summoned, and no
    // encounter table names them — an empty slice is a fact about the game
    // rather than a gap in the dump, which is why `ModelDb.Monsters` alone was
    // not enough to enumerate them.
    let summoned = Monster::ALL.iter().filter(|m| m.acts().is_empty()).count();
    assert_eq!(summoned, 24, "monsters reachable only as summons");
}

/// One rule has one spelling.
///
/// `Cooldown(One)` and `InARow(One)` are the same predicate — "the last move
/// was this branch" — so a table free to use either would compare two identical
/// rules unequal. Codegen canonicalises to `InARow(One)`; this is what stops
/// that from being a comment.
///
/// This build has no cooldown of 1 at all, so the assertion would hold even if
/// codegen forgot. That is why the mutation matters more than the pass: flip
/// the canonicalisation and this test still goes green, which is exactly what
/// makes it worth pinning before a balance patch introduces one.
#[test]
fn a_window_of_one_is_always_spelled_in_a_row() {
    for m in Monster::ALL {
        for state in m.data().states {
            let State::Random(branches) = state else {
                continue;
            };
            for b in *branches {
                assert_ne!(
                    b.repeat,
                    Repeat::Cooldown(RepeatCount::One),
                    "{}: a cooldown of one that should be InARow(One)",
                    m.id()
                );
            }
        }
    }
}

/// The graph closes.
///
/// Every index the table holds — the initial state, a move's follow-up, a
/// branch target — has to land inside its own monster's `states` slice. This is
/// what makes the name-to-index translation safe to rely on, so nothing
/// downstream has to bounds-check a transition.
#[test]
fn every_state_index_is_in_range() {
    for m in Monster::ALL {
        let data = m.data();
        let n = data.states.len();
        let check = |i: StateIndex, what: &str| {
            assert!(
                (i as usize) < n,
                "{}: {what} points at state {i} of {n}",
                m.id()
            );
        };

        check(data.initial, "initial");
        for state in data.states {
            match state {
                State::Move(mv) => match mv.follow_up {
                    FollowUp::Next(next) | FollowUp::OncePerformed(next) => {
                        check(next, "a follow-up");
                    }
                    FollowUp::Never => {}
                },
                State::Random(branches) => {
                    for b in *branches {
                        check(b.target, "a random branch");
                    }
                }
                State::Conditional(targets) => {
                    for t in *targets {
                        check(*t, "a conditional branch");
                    }
                }
            }
        }
    }
}

/// The table against the dump, row by row.
///
/// Branch order is compared unsorted, and that is the point rather than
/// laziness: `RandomBranchState.GetNextState` subtracts weights walking the
/// list, so reordering two branches changes which move a given roll produces.
#[test]
fn the_generated_table_matches_the_dump() {
    let mut states = 0;
    let mut branches = 0;
    let mut intents = 0;

    for row in rows("monster") {
        let id = row[1];
        let data = monster(id).data();
        let names = state_names(id);

        check_scaled(
            &format!("{id}.min_hp"),
            data.min_hp,
            row[2],
            &updates("scalehp", &[id], 3),
        );
        check_scaled(
            &format!("{id}.max_hp"),
            data.max_hp,
            row[3],
            &updates("scalehp", &[id], 4),
        );
        assert_eq!(data.states.len().to_string(), row[5], "{id}: state count");
        assert_eq!(names.len(), data.states.len(), "{id}: state rows");

        // The dump names the initial state; the table indexes it. Checking the
        // name at that index is what proves the two agree.
        assert_eq!(names[data.initial as usize], row[4], "{id}: initial");

        for (i, state) in data.states.iter().enumerate() {
            let name = names[i];
            match state {
                State::Move(mv) => {
                    // Two dump columns, one field. Restated as a pair so that
                    // both halves are pinned: `OncePerformed` differs from
                    // `Next` only in the bool, and a codegen that dropped the
                    // bool would still name the right state.
                    let d = one("move", id, name);
                    let expected = match mv.follow_up {
                        FollowUp::Never => ("-", false),
                        FollowUp::Next(f) => (names[f as usize], false),
                        FollowUp::OncePerformed(f) => (names[f as usize], true),
                    };
                    assert_eq!(
                        expected,
                        (d[3], d[4] == "1"),
                        "{id}.{name}: follow-up and must-perform-once"
                    );
                    intents += check_intents(id, name, &mv.intents, d[5]);
                }
                State::Random(list) => {
                    let d = one("rand", id, name);
                    assert_eq!(list.len().to_string(), d[3], "{id}.{name}: branch count");

                    for (j, b) in list.iter().enumerate() {
                        let d = rows("branch")
                            .find(|r| r[1] == id && r[2] == name && r[3] == j.to_string())
                            .unwrap_or_else(|| panic!("{id}.{name} has no branch {j}"));

                        assert_eq!(names[b.target as usize], d[4], "{id}.{name}[{j}]: target");
                        assert_eq!(
                            b.repeat,
                            repeat(d[5], d[6], d[7]),
                            "{id}.{name}[{j}]: repeat"
                        );
                        match b.weight {
                            Field::Computed => {
                                assert_eq!(d[8], "dynamic", "{id}.{name}[{j}]: weight");
                            }
                            Field::Constant(w) => {
                                let dumped: f32 = d[8].parse().expect("a weight");
                                assert_eq!(w, dumped, "{id}.{name}[{j}]: weight");
                            }
                        }
                        branches += 1;
                    }
                }
                State::Conditional(targets) => {
                    let d = one("cond", id, name);
                    assert_eq!(targets.len().to_string(), d[3], "{id}.{name}: branch count");

                    for (j, t) in targets.iter().enumerate() {
                        let d = rows("condbranch")
                            .find(|r| r[1] == id && r[2] == name && r[3] == j.to_string())
                            .unwrap_or_else(|| panic!("{id}.{name} has no condbranch {j}"));
                        assert_eq!(names[*t as usize], d[4], "{id}.{name}[{j}]: target");
                    }
                }
            }
            states += 1;
        }
    }

    // Every dumped row was reached, not just every generated one — otherwise a
    // state the codegen dropped would pass everything above.
    assert_eq!(
        states,
        rows("move").count() + rows("rand").count() + rows("cond").count()
    );
    assert_eq!(branches, rows("branch").count());
    assert_eq!(intents, rows("intent").count());
}

/// The dump row of a given kind for one monster's state.
fn one(kind: &'static str, monster: &str, state: &str) -> Vec<&'static str> {
    rows(kind)
        .find(|r| r[1] == monster && r[2] == state)
        .unwrap_or_else(|| panic!("no {kind} row for {monster}.{state}"))
}

/// Every `scale*` row matching a slot, as `(level, value)` in dump order.
///
/// Raw: a row is included even when the column it names did not move, because
/// replaying all of them is what makes this independent of the codegen rule
/// that decides which ones become steps.
fn updates(kind: &'static str, key: &[&str], column: usize) -> Vec<(u8, &'static str)> {
    let level = key.len() + 1;
    rows(kind)
        .filter(|r| key.iter().enumerate().all(|(i, k)| r[i + 1] == *k))
        .map(|r| (r[level].parse().expect("an ascension level"), r[column]))
        .collect()
}

fn check_field<T: Copy + std::fmt::Display>(
    where_: &str,
    field: Field<Scaled<T>>,
    base: &str,
    updates: &[(u8, &str)],
) {
    match field {
        Field::Computed => assert_eq!(base, "dynamic", "{where_}: computed but dumped {base}"),
        Field::Constant(s) => check_scaled(where_, s, base, updates),
    }
}

/// A `Scaled` against the dump, by replaying it.
///
/// Rather than comparing `steps` to the dump's rows — which would only prove
/// the codegen agrees with its own rule about which rows become steps — this
/// asks the value at every ascension level and checks it against the dump
/// replayed to that level. What is compared is the meaning, not the encoding.
fn check_scaled<T: Copy + std::fmt::Display>(
    where_: &str,
    scaled: Scaled<T>,
    base: &str,
    updates: &[(u8, &str)],
) {
    for ascension in 0..=10u8 {
        let mut expected = base;
        for (level, value) in updates {
            if *level <= ascension {
                expected = value;
            }
        }
        assert_eq!(
            scaled.at(ascension).to_string(),
            expected,
            "{where_} at ascension {ascension}"
        );
    }
}

/// The dump's intent-type name as the [`IntentKind`] it should be, spelled out
/// here rather than derived from `Debug` — otherwise a renamed variant would
/// rename both sides at once and prove nothing.
fn intent_kind(name: &str) -> IntentKind {
    match name {
        "Attack" => IntentKind::Attack,
        "Buff" => IntentKind::Buff,
        "Debuff" => IntentKind::Debuff,
        "DebuffStrong" => IntentKind::DebuffStrong,
        "Defend" => IntentKind::Defend,
        "Escape" => IntentKind::Escape,
        "Heal" => IntentKind::Heal,
        "Hidden" => IntentKind::Hidden,
        "Summon" => IntentKind::Summon,
        "Sleep" => IntentKind::Sleep,
        "Stun" => IntentKind::Stun,
        "StatusCard" => IntentKind::StatusCard,
        "CardDebuff" => IntentKind::CardDebuff,
        "DeathBlow" => IntentKind::DeathBlow,
        "Unknown" => IntentKind::Unknown,
        other => panic!("unknown intent kind {other}"),
    }
}

/// A move's telegraph against the dump's intent rows.
///
/// The table holds a set and two payloads where the dump holds an ordered list,
/// so this walks the rows and rebuilds what the set should be — the same
/// posture as `repeat`, restating the mapping instead of comparing spellings.
///
/// Order is the one thing not checked, because it is the one thing the table
/// deliberately drops: the game's array order reaches only the intent icons and
/// the hover tips. Everything else is checked per row, including that the two
/// payloads sit beside the right bits.
fn check_intents(monster: &str, state: &str, intents: &Intents, count: &str) -> usize {
    let rows: Vec<Vec<&str>> = rows("intent")
        .filter(|r| r[1] == monster && r[2] == state)
        .collect();
    assert_eq!(
        rows.len().to_string(),
        count,
        "{monster}.{state}: intent count"
    );

    let mut expected: Vec<IntentKind> = Vec::new();
    for row in &rows {
        let where_ = format!("{monster}.{state}[{}]", row[3]);
        let key = [monster, state, row[3]];
        let (kind, count, damage) = (row[4], row[5], row[6]);

        let found = intent_kind(kind);
        assert!(
            intents.kinds.contains(found),
            "{where_}: {kind} is not in the set"
        );
        assert!(
            !expected.contains(&found),
            "{where_}: {kind} telegraphed twice"
        );
        expected.push(found);

        match kind {
            "Attack" | "DeathBlow" => {
                let a = intents.attack.expect("an attacking intent carries numbers");
                check_field(&where_, a.damage, damage, &updates("scaleintent", &key, 6));
                check_field(&where_, a.repeats, count, &updates("scaleintent", &key, 5));
            }
            "StatusCard" => {
                assert_eq!(damage, "-", "{where_}: a status intent with damage");
                let c = intents.cards.expect("a status intent carries a count");
                check_field(&where_, c, count, &updates("scaleintent", &key, 5));
            }
            // Everything else carries no numbers, and the dump has to agree. A
            // number in a column with no field for it would otherwise be
            // dropped without a word.
            _ => {
                assert_eq!(count, "-", "{where_}: {kind} carries a count");
                assert_eq!(damage, "-", "{where_}: {kind} carries damage");
            }
        }
    }

    // The other direction: a bit the dump never asked for would otherwise sit
    // in the set unnoticed, and so would a payload with no intent to belong to.
    let mut got: Vec<IntentKind> = intents.kinds.iter().collect();
    let mut want = expected.clone();
    got.sort_by_key(|k| *k as u8);
    want.sort_by_key(|k| *k as u8);
    assert_eq!(got, want, "{monster}.{state}: the set has extra kinds");

    let attacks = expected
        .iter()
        .filter(|k| matches!(k, IntentKind::Attack | IntentKind::DeathBlow))
        .count();
    assert_eq!(
        intents.attack.is_some(),
        attacks == 1,
        "{monster}.{state}: attack payload"
    );
    assert_eq!(
        intents.cards.is_some(),
        expected.contains(&IntentKind::StatusCard),
        "{monster}.{state}: cards payload"
    );

    rows.len()
}

/// Exactly the branch weights the game works out from combat state, and no
/// others.
///
/// All four are Two-Tailed Rat's `() => !CanSummon() ? 1f : 1f/12f`. The
/// remaining 64 are literal constants the dump recovered. If a build makes one
/// of those conditional, this fails rather than letting the simulator keep
/// rolling against a number that is no longer true.
#[test]
fn only_the_known_weights_are_computed() {
    let mut computed: Vec<(&str, &str)> = rows("branch")
        .filter(|r| r[8] == "dynamic")
        .map(|r| (r[1], r[4]))
        .collect();
    computed.sort_unstable();

    assert_eq!(
        computed,
        [
            ("TWO_TAILED_RAT", "CALL_FOR_BACKUP_MOVE"),
            ("TWO_TAILED_RAT", "DISEASE_BITE_MOVE"),
            ("TWO_TAILED_RAT", "SCRATCH_MOVE"),
            ("TWO_TAILED_RAT", "SCREECH_MOVE"),
        ],
        "the set of branches whose weight reads combat state has changed"
    );
}

/// The intents whose numbers the game works out at combat time. Same argument
/// as the weights.
#[test]
fn only_the_known_intents_are_computed() {
    let mut computed: Vec<(&str, &str, &str)> = rows("intent")
        .filter(|r| r[5] == "dynamic" || r[6] == "dynamic")
        .map(|r| (r[1], r[2], r[4]))
        .collect();
    computed.sort_unstable();

    assert_eq!(
        computed,
        [
            ("GAS_BOMB", "EXPLODE_MOVE", "DeathBlow"),
            ("TEST_SUBJECT", "MULTI_CLAW_MOVE", "Attack"),
            ("THE_FORGOTTEN", "DREAD", "Attack"),
            ("WATERFALL_GIANT", "EXPLODE_MOVE", "DeathBlow"),
            ("WATERFALL_GIANT", "PRESSURE_GUN_MOVE", "Attack"),
        ],
        "the set of intents reading combat state has changed"
    );
}
