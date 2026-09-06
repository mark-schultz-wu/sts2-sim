//! The vocabulary a position is described in: six enums that are not content.
//!
//! Every other domain here answers *what content does this build have*: which
//! cards, which relics, which encounters. This one answers *what words does the
//! game use to describe where a run is standing*. What a map node leads to, and
//! whose side a creature fights on. Which phase a turn is in, why a card cannot
//! be played, and which random stream a draw came from.
//!
//! None of them is a property of a content item, which is why they are one dump
//! of their own. `RoomType` is the contrast: it lives in `encounters` because an
//! encounter has one.
//!
//! The xtask README says why every member is kept, and why `UnplayableReason`
//! is numbered by bit rather than by value.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::{Build, DERIVES, Dump, banner, sets};

/// Every enum the dump carries, in the order the generated file declares them,
/// with what it is.
///
/// Generation fails on a dump that names one this does not. A build that adds a
/// member of the run's vocabulary stops here rather than dropping it.
const ENUMS: [(&str, &str); 6] = [
    (
        "MapPointType",
        "What a node on the act's map leads to.\n\
         \n\
         Not the room the run ends up in: an `Unknown` node rolls that on\n\
         arrival, so this is what the map *shows* and `RoomType` is what is\n\
         entered.",
    ),
    (
        "CombatSide",
        "Which side of a fight a creature is on.\n\
         \n\
         Not the same question as whether it is a monster. A charmed monster\n\
         fights on the player's side and is still a monster, which is why the\n\
         observation carries both.",
    ),
    (
        "PlayerTurnPhase",
        "Where in their turn a player is.\n\
         \n\
         The two `Auto` phases are when cards play themselves, before and after\n\
         the player's own.",
    ),
    (
        "UnplayableReason",
        "One rule a card in hand fails.\n\
         \n\
         `CardModel.CanPlay` adds one per rule the card fails, so a card can\n\
         carry several and a caller holds an [`UnplayableReasons`] rather\n\
         than one of these.",
    ),
    (
        "RunRngType",
        "One of the twelve random streams a run keeps.\n\
         \n\
         Every stream is seeded from the run's seed and drawn from\n\
         independently, so two implementations agreeing on every visible field\n\
         can still have consumed different numbers of draws. That divergence is\n\
         invisible until it decides something, which is why the counters are\n\
         compared directly.",
    ),
    (
        "PlayerRngType",
        "One of the three random streams each player keeps.\n\
         \n\
         Beside the run's twelve, not among them: these are per-player, so a\n\
         multiplayer run has a set each.",
    ),
];

/// The members worth a note, as `(enum, member, note)`.
///
/// Most names say what they are. These are the ones where the name is not the
/// whole fact.
const NOTES: [(&str, &str, &str); 10] = [
    (
        "MapPointType",
        "Unassigned",
        "The grid before the types are dealt. `StandardActMap` fills every\n\
         point with this and assigns over it; anything still unassigned when\n\
         it finishes becomes `Monster`.",
    ),
    (
        "MapPointType",
        "Unknown",
        "The `?` node. What it becomes is rolled on arrival from `RunOddsSet`,\n\
         which draws on the `UnknownMapPoint` stream.",
    ),
    (
        "MapPointType",
        "Ancient",
        "The node an act starts on. `StandardActMap` assigns it to\n\
         `StartingMapPoint` and to nothing else.",
    ),
    (
        "PlayerTurnPhase",
        "None",
        "Between turns, and after a fight ends.",
    ),
    (
        "UnplayableReason",
        "HasUnplayableKeyword",
        "The card carries `CardKeyword::Unplayable`.",
    ),
    (
        "UnplayableReason",
        "BlockedByHook",
        "Something else said no. `Hook.ShouldPlay` names which model, and that\n\
         is the `preventer` the observation carries beside the reason — a\n\
         power, a monster, a relic, a potion, an orb, a card, an affliction, an\n\
         enchantment, a run modifier or a badge.",
    ),
    (
        "UnplayableReason",
        "BlockedByCardLogic",
        "The card said no about itself: `IsPlayable` is an override, so what\n\
         the condition is belongs to the card's body.",
    ),
    (
        "UnplayableReason",
        "NoLivingAllies",
        "It targets an ally and there is no second living player creature.",
    ),
    (
        "RunRngType",
        "UpFront",
        "Drawn when a run or an act is set up rather than while it is played:\n\
         the relic grab bags and the shuffle of the shared ancients.",
    ),
    (
        "RunRngType",
        "Niche",
        "The stream for draws with no stream of their own — the curse Cursed\n\
         Run adds, Tough Egg's hatchling health, what the Astrolabe transforms\n\
         into, the shuffles Royal Stamp and Sand Castle do.",
    ),
];

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("runstate.txt")?;
    let build = dump.build.clone();

    let mut enums: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    for row in dump.of("enum") {
        if row.len() != 4 {
            return Err(format!("{}: {} columns, not 3", row[1], row.len() - 1));
        }
        let discriminant = row[2]
            .parse::<u8>()
            .map_err(|e| format!("{} {}: discriminant {}: {e}", row[1], row[3], row[2]))?;
        enums
            .entry(row[1].clone())
            .or_default()
            .push((discriminant, row[3].clone()));
    }

    for name in enums.keys() {
        if !ENUMS.iter().any(|(known, _)| known == name) {
            return Err(format!(
                "the dump reflects {name}, which this module does not know what to say \
                 about — add it to ENUMS with what it is"
            ));
        }
    }

    // The `flags` rows say which enums are `[Flags]`. Only one is, and the
    // numbering below turns on it, so a build that made a second one flags has
    // to be looked at rather than generated.
    let flags: Vec<&String> = dump.of("flags").map(|row| &row[1]).collect();
    if flags != ["UnplayableReason"] {
        return Err(format!(
            "the dump calls {flags:?} a [Flags] enum, and this module is written for \
             UnplayableReason alone"
        ));
    }

    let mut out = banner(
        "The words the game describes a run's position in.",
        "runstate.txt",
        &build,
    );
    writeln!(out, "\nuse serde::{{Deserialize, Serialize}};\n").unwrap();

    for (name, doc) in ENUMS {
        let members = enums
            .get(name)
            .ok_or_else(|| format!("the dump reflects no {name}"))?;

        match name {
            "UnplayableReason" => emit_reasons(&mut out, doc, members)?,
            _ => emit_enum(&mut out, name, doc, members),
        }

        if name == "MapPointType" {
            emit_map_points(&mut out, members)?;
        }
    }

    let path = root.join("runstate.rs");
    fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;

    println!(
        "  runstate: {} enums, {} members",
        ENUMS.len(),
        enums.values().map(Vec::len).sum::<usize>()
    );

    Ok(build)
}

/// A note on a member, or nothing where the name is the whole fact.
fn note(enum_name: &str, member: &str) -> &'static str {
    NOTES
        .iter()
        .find(|&&(e, m, _)| e == enum_name && m == member)
        .map_or("", |&(_, _, doc)| doc)
}

/// One enum at the game's own discriminants.
fn emit_enum(out: &mut String, name: &str, doc: &str, members: &[(u8, String)]) {
    for line in doc.lines() {
        writeln!(out, "/// {}", line.trim()).unwrap();
    }
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum {name} {{").unwrap();
    for (discriminant, member) in members {
        for line in note(name, member).lines() {
            writeln!(out, "    /// {}", line.trim()).unwrap();
        }
        writeln!(out, "    {member} = {discriminant},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    emit_all(
        out,
        name,
        &members.iter().map(|(_, m)| m.as_str()).collect::<Vec<_>>(),
    );
}

/// `ALL`, because Rust has no way to walk an enum.
fn emit_all(out: &mut String, name: &str, members: &[&str]) {
    writeln!(out, "impl {name} {{").unwrap();
    writeln!(
        out,
        "    /// Every member, in the game's own order, so a caller can sweep\n\
         \x20   /// them."
    )
    .unwrap();
    writeln!(out, "    pub const ALL: [{name}; {}] = [", members.len()).unwrap();
    for member in members {
        writeln!(out, "        {name}::{member},").unwrap();
    }
    writeln!(out, "    ];").unwrap();
    writeln!(out, "}}\n").unwrap();
}

/// The one `[Flags]` enum, numbered by bit index, and the set over it.
fn emit_reasons(out: &mut String, doc: &str, members: &[(u8, String)]) -> Result<(), String> {
    let mut bits: Vec<sets::Member<'_>> = Vec::new();

    for (value, member) in members {
        // The zero is the empty set, which the set type spells by being empty.
        // Checked by name too: a build that renamed it would be saying
        // something, and generating an enum with a `None` bit in it would not.
        if *value == 0 {
            if member != "None" {
                return Err(format!(
                    "UnplayableReason's zero is called {member}, not None — it used to be \
                     the absence of a reason and may not be any more"
                ));
            }
            continue;
        }

        if value.count_ones() != 1 {
            return Err(format!(
                "UnplayableReason::{member} is {value}, which is more than one bit — \
                 a combination the game named, and not a rule a card fails"
            ));
        }

        bits.push(sets::Member {
            name: member,
            bit: value.trailing_zeros() as usize,
            doc: note("UnplayableReason", member),
        });
    }

    for line in doc.lines() {
        writeln!(out, "/// {}", line.trim()).unwrap();
    }
    writeln!(
        out,
        "///\n\
         /// The discriminant is the bit's index, so the game's own value is\n\
         /// `1 << reason`. Bit 0 is unused: the game skips value 1."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum UnplayableReason {{").unwrap();
    for member in &bits {
        for line in member.doc.lines() {
            writeln!(out, "    /// {}", line.trim()).unwrap();
        }
        writeln!(out, "    {} = {},", member.name, member.bit).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    emit_all(
        out,
        "UnplayableReason",
        &bits.iter().map(|m| m.name).collect::<Vec<_>>(),
    );

    sets::plain(
        out,
        &sets::Set {
            flag: "UnplayableReason",
            set: "UnplayableReasons",
            flag_doc: "",
            set_doc: "Every rule a card in hand fails, which can be more than one.\n\
                      \n\
                      `CardModel.CanPlay` starts at nothing and adds a bit per rule, so\n\
                      an unaffordable card that also has no living ally to target\n\
                      carries both. Empty is the game's `UnplayableReason.None`, and\n\
                      means the card can be played.\n\
                      \n\
                      One bit per reason at its own index, which makes the word the\n\
                      game's own `[Flags]` value.",
            members: &bits,
            declare: false,
            extras: &[sets::Extra::IsEmpty],
        },
    )
}

/// The nine map point kinds, as the consts map generation names them by.
///
/// A generated enum is per build, so the simulator's map rules cannot write
/// `MapPointType::Elite`. This is the bridge, spelled out member by member
/// rather than derived from the order. A build that renames one stops here,
/// where the answer is to look at what it renamed.
fn emit_map_points(out: &mut String, members: &[(u8, String)]) -> Result<(), String> {
    // `(const, member)`, in the trait's own order.
    const NAMED: [(&str, &str); 9] = [
        ("UNASSIGNED", "Unassigned"),
        ("UNKNOWN", "Unknown"),
        ("SHOP", "Shop"),
        ("TREASURE", "Treasure"),
        ("REST_SITE", "RestSite"),
        ("MONSTER", "Monster"),
        ("ELITE", "Elite"),
        ("BOSS", "Boss"),
        ("ANCIENT", "Ancient"),
    ];

    for (_, member) in &NAMED {
        if !members.iter().any(|(_, have)| have == member) {
            return Err(format!(
                "the dump reflects no MapPointType::{member}, which map generation \
                 names — see what this build calls it"
            ));
        }
    }
    if members.len() != NAMED.len() {
        return Err(format!(
            "MapPointType has {} members and the map rules name {} — the new one \
             needs a const on sts2_core::MapPoints",
            members.len(),
            NAMED.len()
        ));
    }

    writeln!(out, "impl sts2_core::MapPoints for MapPointType {{").unwrap();
    for (name, member) in NAMED {
        writeln!(
            out,
            "    const {name}: MapPointType = MapPointType::{member};"
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    Ok(())
}
