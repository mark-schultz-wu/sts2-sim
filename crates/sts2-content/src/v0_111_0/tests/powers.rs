//! The power table against the dump it came from.
//!
//! Structured like the card tests: replay the dump, row by row, against the
//! table it generated. What is particular here is the `dynamic` column, because
//! it is the one thing a reader cannot check by eye. Every one of these members
//! is `virtual` or `abstract` on `PowerModel`. Nothing about a generated
//! `Field::Constant` says whether the game really treats it as one.

use super::super::powers::*;
use super::super::{POWERS, VERSION};
use super::{build, rows, wire};
use crate::Field;
use sts2_core::Table;

fn power(id: &str) -> Power {
    *Power::ALL
        .iter()
        .find(|p| wire(p) == id)
        .unwrap_or_else(|| panic!("{id} is in the dump but not generated"))
}

/// The dump's spelling of a column whose enum lost its `None`.
///
/// The absence is Rust's, not the dump's. The game writes the member name it
/// declared, so `None` here has to spell itself back into `None` rather than
/// into JSON's `null`. That is the whole of what dropping the member did. The
/// value survives, and only its spelling moved from a variant to the `Option`.
fn maybe<T: serde::Serialize>(value: &Option<T>) -> String {
    match value {
        Some(v) => wire(v),
        None => "None".to_string(),
    }
}

/// The members `DROPPED` in `xtask/src/codegen/powers.rs`, spelled out again so
/// that dropping another one has to be said twice.
const DROPPED: [(&str, &str); 3] = [
    ("PowerType", "None"),
    ("PowerInstanceType", "None"),
    ("PowerStackType", "None"),
];

fn dropped(enum_name: &str, member: &str) -> bool {
    DROPPED.contains(&(enum_name, member))
}

/// The anchor, same as the other dumps'. A dump from another build has another
/// hash, and a table regenerated against one is the failure this prevents.
#[test]
fn the_dump_came_from_this_version() {
    assert_eq!(
        build(POWERS),
        VERSION,
        "powers.txt was dumped from a different build than this module claims"
    );
}

#[test]
fn every_power_is_generated() {
    let dumped: Vec<&str> = rows(POWERS, "power").map(|r| r[1]).collect();
    assert_eq!(dumped.len(), 283, "the build has 283 powers");
    assert_eq!(Power::ALL.len(), dumped.len());

    for id in dumped {
        assert_eq!(wire(&power(id)), id);
    }
}

/// Ids are the enum variants and the wire form, so a duplicate makes two
/// different powers the same power.
#[test]
fn power_ids_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for row in rows(POWERS, "power") {
        assert!(seen.insert(row[1]), "duplicate power id {}", row[1]);
    }
}

/// The generated table has to agree with the dump it was generated from.
///
/// Every cell is compared through the dump's own spelling. Enums go through
/// serde, and flags against `1` and `0`.
///
/// The flags are checked by column *and* by member, because `PowerFlags` is a
/// bitset whose numbering is ours. A member reordered in `FLAGS` moves every
/// bit after it without changing a name.
#[test]
fn the_generated_table_matches_the_dump() {
    let mut checked = 0;

    for row in rows(POWERS, "power") {
        let id = row[1];
        let d = power(id).data();

        check(id, "kind", d.kind, row[2], wire);
        assert_eq!(maybe(&d.instance), row[3], "{id}: instance");
        check(id, "stack", d.stack, row[4], maybe);

        for (column, flag) in FLAG_COLUMNS {
            check(
                id,
                &format!("{flag:?}"),
                d.flags.get(flag),
                row[column],
                |b| if *b { "1" } else { "0" }.to_string(),
            );
        }

        checked += 1;
    }

    assert_eq!(checked, 283);
}

/// The dump column each flag comes from.
///
/// Spelled out rather than taken as "the columns after the values". That is the
/// mapping `FLAGS` in the generator decides, and the one a reorder changes
/// silently.
const FLAG_COLUMNS: [(usize, PowerFlag); 5] = [
    (5, PowerFlag::AllowNegative),
    (6, PowerFlag::RemovedOnOwnerDeath),
    (7, PowerFlag::OwnerDeathIsFatal),
    (8, PowerFlag::ScalesInMultiplayer),
    (9, PowerFlag::OwnerIsSecondaryEnemy),
];

/// One `Field` cell against one dump column.
///
/// Matched rather than converted to an `Option`. `Computed` means "ask the
/// power" and is not a missing value. The two have to stay distinguishable, for
/// the same reason they do on cards.
fn check<T>(id: &str, field: &str, cell: Field<T>, dumped: &str, spell: impl Fn(&T) -> String) {
    match cell {
        Field::Computed => assert_eq!(dumped, "dynamic", "{id}: {field}"),
        Field::Constant(v) => assert_eq!(spell(&v), dumped, "{id}: {field}"),
    }
}

/// The enums the power fields draw from, reflected from the game rather than
/// inferred from the values in use. A member no power happens to use is still a
/// member. An enum missing one fails to decode the first time content grows
/// into it.
#[test]
fn the_game_declares_three_members_each() {
    for name in ["PowerType", "PowerInstanceType", "PowerStackType"] {
        let members: Vec<_> = rows(POWERS, "enum").filter(|r| r[1] == name).collect();
        assert_eq!(members.len(), 3, "{name} member count changed");

        // Discriminants are the wire and table encoding, so a gap or a reorder
        // in what the *game* declares is a silent remapping rather than an
        // addition. The generated enums are a subset of this, not a reshuffle.
        for (i, m) in members.iter().enumerate() {
            assert_eq!(m[2], i.to_string(), "{name} is no longer densely numbered");
        }
    }
}

/// The generated enums are the game's, minus exactly the dropped members, at
/// the game's own numbers.
///
/// Three things at once, and each is a different way the drop goes wrong. A
/// member that stopped parsing is one silently dropped. One that started
/// parsing is a `DROPPED` entry nobody removed. A member at the wrong
/// discriminant is the renumbering that dropping avoids.
#[test]
fn the_generated_enums_drop_only_what_was_declared_dropped() {
    /// One enum: every member the game declares, against the Rust type.
    macro_rules! check_enum {
        ($ty:ty, $name:literal) => {
            for row in rows(POWERS, "enum").filter(|r| r[1] == $name) {
                let (member, discriminant) = (row[3], row[2].parse::<u8>().unwrap());
                let parsed: Option<$ty> = serde_json::from_str(&format!("\"{member}\"")).ok();

                match parsed {
                    Some(v) => {
                        assert!(
                            !dropped($name, member),
                            "{}::{member} is dropped but still parses",
                            $name
                        );
                        assert_eq!(
                            v as u8, discriminant,
                            "{}::{member} is not at the game's own number",
                            $name
                        );
                    }
                    None => assert!(
                        dropped($name, member),
                        "{}::{member} is declared by the game but does not parse",
                        $name
                    ),
                }
            }
        };
    }

    check_enum!(PowerType, "PowerType");
    check_enum!(PowerInstanceType, "PowerInstanceType");
    check_enum!(PowerStackType, "PowerStackType");
}

/// What each dropped member cost, counted.
///
/// The generator's reasons for dropping them are arguments about how many
/// powers have the member and what reads it. An argument stops being true when
/// the numbers move. `PowerType::None` is the strongest case and the easiest to
/// check. Nothing has it at all, so the `Option` the other two needed is an
/// `Option` that is never `None`.
#[test]
fn the_dropped_members_are_as_rare_as_the_reasons_claim() {
    let count =
        |column: usize, value: &str| rows(POWERS, "power").filter(|r| r[column] == value).count();

    assert_eq!(count(2, "None"), 0, "some power now has PowerType::None");
    assert_eq!(count(3, "None"), 260, "PowerInstanceType::None count moved");
    assert_eq!(count(4, "None"), 1, "PowerStackType::None count moved");
}

/// Exactly the powers the game works something out for, and no others.
///
/// The assertion the whole `dynamic` mechanism rests on, spelled out rather
/// than counted. A table generating a constant for one is wrong invisibly.
///
/// The twenty on `kind` are one family: `ITemporaryPower` descendants whose
/// `Type` follows a `protected IsPositive`. Each is constant per subclass
/// rather than as a literal, and the dump reads IL rather than guessing.
#[test]
fn only_the_known_powers_are_computed() {
    let computed = |column: usize| -> Vec<&str> {
        rows(POWERS, "power")
            .filter(|r| r[column] == "dynamic")
            .map(|r| r[1])
            .collect()
    };

    // `instance` is the one column with no `Field` to put a computed cell in,
    // so codegen fails on a `dynamic` here rather than generating one. Said
    // from this side too, because the generator that produced the table is not
    // the thing that should be certifying it.
    assert!(
        computed(3).is_empty(),
        "InstanceType is computed on {:?}, which the table cannot spell",
        computed(3)
    );

    assert_eq!(
        computed(2),
        [
            "ANTICIPATE_POWER",
            "COORDINATE_POWER",
            "CRUSH_UNDER_POWER",
            "DARK_SHACKLES_POWER",
            "DYING_STAR_POWER",
            "ENFEEBLING_TOUCH_POWER",
            "FADE_POWER",
            "FEEDING_FRENZY_POWER",
            "FLEX_POTION_POWER",
            "FOCUSED_STRIKE_POWER",
            "HELICAL_DART_POWER",
            "HOTFIX_POWER",
            "HYPERBEAM_FOCUS_DOWN_POWER",
            "MANGLE_POWER",
            "MOCK_TEMPORARY_STRENGTH_LOSS_POWER",
            "MONARCHS_GAZE_STRENGTH_DOWN_POWER",
            "PIERCING_WAIL_POWER",
            "REPTILE_TRINKET_POWER",
            "SETUP_STRIKE_POWER",
            "SHACKLING_POTION_POWER",
            "SPEED_POTION_POWER",
            "SYNCHRONIZE_POWER",
        ],
        "the set of powers computing Type has changed"
    );

    // `ShrinkPower` is a counter until its amount goes negative and a single
    // after; `MonologuePower` reads a dynamic var.
    assert_eq!(
        computed(4),
        ["MONOLOGUE_POWER", "SHRINK_POWER"],
        "the set of powers computing StackType has changed"
    );

    // Reattach only makes its owner's death fatal once every other segment is
    // already dead.
    assert_eq!(
        computed(7),
        ["REATTACH_POWER"],
        "the set of powers computing ShouldOwnerDeathTriggerFatal has changed"
    );

    assert_eq!(
        computed(8),
        ["MOCK_SCALE_IN_MULTIPLAYER_POWER"],
        "the set of powers computing ShouldScaleInMultiplayer has changed"
    );

    // The other three flags: nothing works them out in this build. A `Field`
    // is what `PowerFlags` hands back either way, so a power that started
    // computing one of these would generate fine and change nothing visible —
    // which is exactly why it is worth saying out loud.
    for (column, what) in [
        (5, "AllowNegative"),
        (6, "ShouldPowerBeRemovedAfterOwnerDeath"),
        (9, "OwnerIsSecondaryEnemy"),
    ] {
        assert!(
            computed(column).is_empty(),
            "{what} is now computed on {:?}",
            computed(column)
        );
    }
}

/// The wire form round-trips, and it is the only spelling of an id.
///
/// There is no `id` accessor, because the serde rename is the id. So this is
/// the test saying the 273 renames are the 273 ids the dump declares, and that
/// every one of them reads back.
#[test]
fn a_power_round_trips_through_its_wire_form() {
    let dumped: Vec<&str> = rows(POWERS, "power").map(|r| r[1]).collect();

    for (power, id) in Power::ALL.iter().zip(&dumped) {
        let json = serde_json::to_string(power).unwrap();
        assert_eq!(json, format!("\"{id}\""), "the rename is not the dump's id");
        assert_eq!(&serde_json::from_str::<Power>(&json).unwrap(), power);
    }

    assert!(serde_json::from_str::<Power>("\"NOT_A_POWER\"").is_err());
    assert!(serde_json::from_str::<Power>("\"STRENGTH_POWE\"").is_err());
}

/// `GetTypeForAmount`, which the game derives rather than stores.
///
/// Worth pinning because it is the one place these fields combine, and the
/// combination is not obvious. A debuff that does not allow negatives reads as
/// a *buff* while its amount is below zero. A counter that does allow them
/// reads as a debuff. Anything porting `Rend` or `Misery` reproduces this.
#[test]
fn a_negative_amount_can_flip_a_powers_type() {
    fn allows_negative(power: Power) -> bool {
        match power.data().flags.get(PowerFlag::AllowNegative) {
            Field::Constant(b) => b,
            Field::Computed => panic!("{}: computes AllowNegative", wire(&power)),
        }
    }

    fn type_for(power: Power, amount: i32) -> PowerType {
        let d = power.data();
        let (Field::Constant(kind), Field::Constant(stack)) = (d.kind, d.stack) else {
            panic!("{}: computes what this test wants to derive", wire(&power));
        };
        let allow_negative = allows_negative(power);

        if stack == Some(PowerStackType::Counter) && allow_negative && amount < 0 {
            return PowerType::Debuff;
        }
        if !allow_negative && kind == PowerType::Debuff && amount < 0 {
            return PowerType::Buff;
        }
        kind
    }

    // Vulnerable: a debuff that does not allow negatives, so a negative amount
    // reads as a buff.
    let vulnerable = power("VULNERABLE_POWER");
    assert!(!allows_negative(vulnerable));
    assert_eq!(type_for(vulnerable, 2), PowerType::Debuff);
    assert_eq!(type_for(vulnerable, -1), PowerType::Buff);

    // Strength: a buff that does allow negatives, so a negative amount reads as
    // a debuff — which is why `Rend` and `Misery` count it.
    let strength = power("STRENGTH_POWER");
    assert!(allows_negative(strength));
    assert_eq!(type_for(strength, 2), PowerType::Buff);
    assert_eq!(type_for(strength, -1), PowerType::Debuff);
}
