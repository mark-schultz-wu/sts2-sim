//! Every encounter: what room it is, which monsters it fields, and where.
//!
//! An encounter is how a fight starts. The monster table says what a monster
//! does once it is on the board. This says which monsters are on the board.
//!
//! The xtask README says why a roster has three answers, why slots are two
//! different lists, and what typing the monsters buys.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr, sets};

/// Members the game declares that are not values of the field they name.
const DROPPED: [(&str, &str, &str); 2] = [
    (
        "Family",
        "None",
        "no encounter has it and nothing in the game names it: zero references \
         in the whole assembly, its own declaration aside. `Tags` defaults to \
         an empty sequence rather than to this, so the absence of a tag is \
         already spelled as no tag.",
    ),
    (
        "RoomType",
        "Unassigned",
        "all eight uses are an absent answer, and one of them throws. \
         `TryGetRoomTypeForTutorial` assigns it to its `out` before returning \
         false; `RollRoomTypeFor` returns it for `MapPointType.Unassigned` and \
         the reverse switch maps it back — not injectively, since `RoomType.Map` \
         maps there too; `IBootstrapSettings` throws \
         `ArgumentOutOfRangeException` on it; the three debug bootstrappers read \
         it as `no room was chosen, enter the act`; and `AutoSlayer` waits for a \
         room to stop being it, timing out with `Room type not assigned`. So it \
         is `Option<RoomType>` at whatever site asks, and no encounter is ever \
         in one — all 96 are Monster, Elite or Boss.",
    ),
];

/// The flags an encounter carries, in the order they become bits.
///
/// Not a game enum, but three separate `EncounterModel` overrides. So the
/// numbering is ours, and the dump's column order is what fixes it.
const FLAGS: [(&str, usize, &str); 3] = [
    ("Weak", 3, "One of the first fights of an act."),
    ("GivesRewards", 4, "Winning it pays gold and a card."),
    (
        "Debug",
        5,
        "Not content: the eight mocks and `DEPRECATED_ENCOUNTER`.",
    ),
];

/// What the game's enum is called here.
///
/// `EncounterTag` is the only rename, and it earns it: see the doc the enum
/// carries. `RoomType` keeps its name because that is what it is.
fn rust_name(game: &str) -> &str {
    match game {
        "EncounterTag" => "Family",
        other => other,
    }
}

/// The most monsters a fight opens with, and the most one can ever field.
///
/// Caps rather than guesses. The build's largest are 4 and 6, and generation
/// fails rather than truncating if one ever grows past these. They are what
/// lets a roster be inline instead of a pointer. See `sts2_content::Few`.
const SPAWN_CAP: usize = 4;
const POSSIBLE_CAP: usize = 6;

/// The most slots an encounter can name.
///
/// A cap rather than a `u8` for the same reason `sts2_content::Few` has one,
/// and for one more. A slot index sits beside a `Monster` inside an `Option`,
/// and only a type with spare values leaves the `Option` a niche. The build's
/// largest vocabulary is six, so this is headroom.
const SLOT_CAP: usize = 8;

/// What each slot position is called. Ordinals, because that is what a slot
/// index is, and what the game itself calls its own common slots.
const SLOT_NAMES: [&str; SLOT_CAP] = [
    "First", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth",
];

fn dropped(enum_name: &str, member: &str) -> bool {
    DROPPED
        .iter()
        .any(|&(e, m, _)| e == enum_name && m == member)
}

/// The file an encounter's row lands in, from its room type.
///
/// The game's own grouping: `ActModel` caches `AllRegularEncounters`,
/// `AllEliteEncounters` and `AllBossEncounters`, each a filter on exactly this.
fn bucket(room: &str) -> Result<&'static str, String> {
    Ok(match room {
        "Monster" => "normal",
        "Elite" => "elite",
        "Boss" => "boss",
        other => return Err(format!("no bucket for an encounter in a {other} room")),
    })
}

fn bucket_doc(file: &str) -> &'static str {
    match file {
        "normal" => "Encounters in a monster room, which is most of them.",
        "elite" => "Encounters in an elite room.",
        "boss" => "Encounters in a boss room.",
        other => panic!("no description for the {other} bucket"),
    }
}

/// One encounter's rows, gathered from the dump's several kinds.
#[derive(Default)]
struct Encounter {
    room: String,
    flags: Vec<&'static str>,
    gold: (Steps, Steps),
    tags: Vec<String>,
    slots: Vec<String>,
    possible: Vec<String>,
    roster: String,
    spawns: Vec<(String, String)>,
}

/// A value at ascension 0 plus the level it steps at, which for gold is Poverty
/// and nothing else.
#[derive(Default)]
struct Steps {
    base: String,
    step: Option<(u8, String)>,
}

impl Steps {
    fn literal(&self, where_: &str) -> Result<String, String> {
        let number = |v: &str| {
            v.parse::<u16>()
                .map(|n| format!("{n}"))
                .map_err(|e| format!("{where_}: {v}: {e}"))
        };

        let step = match &self.step {
            None => "None".to_string(),
            Some((level, v)) => format!("Some(({level}, {}))", number(v)?),
        };

        Ok(format!(
            "Scaled {{ base: {}, step: {step} }}",
            number(&self.base)?
        ))
    }
}

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("encounters.txt")?;
    let build = dump.build.clone();

    let mut enums: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    for f in dump.of("enum") {
        let discriminant = f[2]
            .parse::<u8>()
            .map_err(|e| format!("{} {}: discriminant {}: {e}", f[1], f[3], f[2]))?;
        enums
            .entry(f[1].clone())
            .or_default()
            .push((discriminant, f[3].clone()));
    }

    // Encounters in dump order, which is by id, and their rows folded in.
    let mut encounters: BTreeMap<String, Encounter> = BTreeMap::new();
    for row in dump.of("encounter") {
        if row.len() != 9 {
            return Err(format!("{}: {} columns, not 9", row[1], row.len() - 1));
        }
        let e = encounters.entry(row[1].clone()).or_default();
        e.room = row[2].clone();
        e.flags = FLAGS
            .iter()
            .filter(|(_, column, _)| row[*column] == "1")
            .map(|(name, ..)| *name)
            .collect();
        e.gold = (Steps::new(&row[6]), Steps::new(&row[7]));
        e.roster = row[8].clone();
    }

    let get = |id: &str, encounters: &mut BTreeMap<String, Encounter>| -> Result<(), String> {
        if encounters.contains_key(id) {
            Ok(())
        } else {
            Err(format!("{id} has rows but no encounter row"))
        }
    };

    for row in dump.of("slot") {
        get(&row[1], &mut encounters)?;
        encounters
            .get_mut(&row[1])
            .unwrap()
            .slots
            .push(row[3].clone());
    }
    for row in dump.of("tag") {
        get(&row[1], &mut encounters)?;
        encounters
            .get_mut(&row[1])
            .unwrap()
            .tags
            .push(row[2].clone());
    }
    for row in dump.of("possible") {
        get(&row[1], &mut encounters)?;
        encounters
            .get_mut(&row[1])
            .unwrap()
            .possible
            .push(row[2].clone());
    }
    for row in dump.of("spawn") {
        get(&row[1], &mut encounters)?;
        encounters
            .get_mut(&row[1])
            .unwrap()
            .spawns
            .push((row[3].clone(), row[4].clone()));
    }
    for row in dump.of("scalegold") {
        get(&row[1], &mut encounters)?;
        let level = row[2]
            .parse::<u8>()
            .map_err(|e| format!("{}: ascension {}: {e}", row[1], row[2]))?;
        let e = encounters.get_mut(&row[1]).unwrap();
        for (steps, value) in [(&mut e.gold.0, &row[3]), (&mut e.gold.1, &row[4])] {
            if steps.step.is_some() {
                return Err(format!("{}: gold steps at more than one level", row[1]));
            }
            steps.step = Some((level, value.clone()));
        }
    }

    if encounters.is_empty() {
        return Err("the dump declares no encounters".into());
    }

    // Which acts can roll an encounter. Empty for one only an event starts, and
    // for the mocks — the same shape as a monster no encounter can roll.
    let mut acts: Vec<String> = Vec::new();
    let mut act_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in dump.of("act") {
        if !acts.contains(&row[1]) {
            acts.push(row[1].clone());
        }
        act_of
            .entry(row[2].clone())
            .or_default()
            .push(row[1].clone());
    }

    let mut out = banner(
        "Every encounter's room, roster and rewards.",
        "encounters.txt",
        &build,
    );
    writeln!(out, "\nuse super::acts::Act;").unwrap();
    writeln!(out, "use super::monsters::Monster;").unwrap();
    writeln!(out, "use crate::{{Few, Scaled}};\nuse sts2_core::Table;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    for (enum_name, member, why) in DROPPED {
        writeln!(out, "// `{enum_name}::{member}` is not generated: {why}").unwrap();
    }
    writeln!(out).unwrap();

    for (game_name, members) in &enums {
        let name = rust_name(game_name);
        if name == "Family" {
            writeln!(
                out,
                "/// The creature family an encounter fields.\n\
                 ///\n\
                 /// The game calls this `EncounterTag`, which says nothing about\n\
                 /// what it is for. It is a family: every family more than one\n\
                 /// encounter carries names a monster all of them field —\n\
                 /// `Burrower` is `TUNNELER`, `Slimes` is the two slimes,\n\
                 /// `Workers` is `BOWLBUG_ROCK`. Not act-scoped, though it looks\n\
                 /// it, because a creature family lives in one act.\n\
                 ///\n\
                 /// One reader: `ActModel` will not place an encounter next to\n\
                 /// one it shares a family with, so a run does not fight the weak\n\
                 /// slimes and then the full ones."
            )
            .unwrap();
        }
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            if dropped(name, m) {
                continue;
            }
            writeln!(out, "    #[serde(rename = \"{m}\")]").unwrap();
            writeln!(out, "    {m} = {discriminant},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    for e in encounters.values() {
        if e.spawns.len() > SPAWN_CAP {
            return Err(format!(
                "an encounter opens with {} monsters, past the {SPAWN_CAP} a Spawns can hold",
                e.spawns.len()
            ));
        }
        if e.possible.len() > POSSIBLE_CAP {
            return Err(format!(
                "an encounter can field {} monsters, past the {POSSIBLE_CAP} a Possible can hold",
                e.possible.len()
            ));
        }
    }

    for (id, e) in &encounters {
        let vocabulary = vocabulary(e);
        if vocabulary.len() > SLOT_CAP {
            return Err(format!(
                "{id} names {} slots, past the {SLOT_CAP} a Slot can spell",
                vocabulary.len()
            ));
        }
    }

    emit_sets(&mut out, &enums)?;
    emit_shape(&mut out);
    emit_encounters(&mut out, &encounters, &act_of)?;

    let mut split = Split::new(root, "encounters", "encounters.txt", &build)?;
    for (id, encounter) in &encounters {
        let literal = literal(id, encounter)?;
        writeln!(
            split.body(bucket(&encounter.room)?),
            "pub(super) const {id}: EncounterData = {literal};\n"
        )
        .unwrap();
    }

    let rooms = split.names().count();
    for name in split.names() {
        writeln!(out, "mod {name};").unwrap();
    }
    writeln!(out).unwrap();

    split.finish(
        &out,
        |file| bucket_doc(file).to_string(),
        &[
            (
                "super",
                &[
                    "EncounterData",
                    "EncounterFlag",
                    "Family",
                    "FamilySet",
                    "FlagSet",
                    "Gold",
                    "Possible",
                    "RoomType",
                    "Roster",
                    "Slot",
                    "Spawn",
                    "Spawns",
                ],
            ),
            ("super::super::monsters", &["Monster"]),
            ("crate", &["Few", "Scaled"]),
        ],
    )?;

    let random = encounters.values().filter(|e| e.roster == "random").count();
    println!(
        "  encounters: {} across {rooms} rooms and {} acts, {random} roll their roster",
        encounters.len(),
        acts.len()
    );

    Ok(build)
}

impl Steps {
    fn new(base: &str) -> Steps {
        Steps {
            base: base.to_string(),
            step: None,
        }
    }
}

/// The two bitsets: tags, which are the game's enum, and flags, which are not.
/// The two bitsets: families, which are the game's enum, and flags, which are
/// not.
fn emit_sets(out: &mut String, enums: &BTreeMap<String, Vec<(u8, String)>>) -> Result<(), String> {
    let families: Vec<sets::Member<'_>> = enums
        .get("EncounterTag")
        .ok_or("the dump reflects no EncounterTag")?
        .iter()
        .filter(|(_, m)| !dropped("Family", m))
        .map(|(bit, m)| sets::Member {
            name: m,
            bit: *bit as usize,
            doc: "",
        })
        .collect();

    sets::plain(
        out,
        &sets::Set {
            flag: "Family",
            set: "FamilySet",
            flag_doc: "",
            set_doc: "The families an encounter fields.\n\
                      \n\
                      A set rather than the sequence the game declares, because the one\n\
                      reader cannot see an order: `SharesTagsWith` is\n\
                      `Tags.Intersect(other.Tags).Any()`, which is `intersects` here.",
            members: &families,
            declare: false,
            extras: &[sets::Extra::Intersects],
        },
    )?;

    sets::plain(
        out,
        &sets::Set {
            flag: "EncounterFlag",
            set: "FlagSet",
            flag_doc: "\n\
                       Three separate `EncounterModel` overrides rather than a game enum,\n\
                       so the numbering is the dump's column order. Unlike a power's\n\
                       flags these are never computed — nothing here asks the encounter —\n\
                       so the set is one word and `contains` can answer plainly.",
            set_doc: "The flags an encounter has.",
            members: &sets::counted(FLAGS.map(|(name, _, doc)| (name, doc))),
            declare: true,
            extras: &[],
        },
    )?;

    Ok(())
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// Which of an encounter's slots a monster stands in."
    )
    .unwrap();
    writeln!(
        out,
        "///\n\
         /// An index, not a name: slot names are encounter-local scene\n\
         /// positions with no shared vocabulary — 44 of them across 96\n\
         /// encounters — and nothing outside the game ever sees one. Capped\n\
         /// rather than a `u8` so that the spare values leave a niche, which is\n\
         /// what makes `Option<Slot>` cost nothing beside a `Monster`."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum Slot {{").unwrap();
    for (i, name) in SLOT_NAMES.iter().enumerate() {
        writeln!(out, "    {name} = {i},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// What winning pays.\n\
         ///\n\
         /// Scaled because Poverty multiplies both by\n\
         /// `AscensionHelper.PovertyAscensionGoldMultiplier`. Every encounter\n\
         /// but one steps at that level and nowhere else, and the exception is\n\
         /// the point of sweeping rather than assuming:\n\
         /// `FAKE_MERCHANT_EVENT_ENCOUNTER` overrides both properties with a\n\
         /// flat 300 and so owes the rule nothing.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct Gold {{\n\
         \x20   pub min: Scaled<u16>,\n\
         \x20   pub max: Scaled<u16>,\n\
         }}\n\
         \n\
         /// One monster on the board when the fight starts.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct Spawn {{\n\
         \x20   pub monster: Monster,\n\
         \x20   /// Which of the encounter's slots it stands in, or `None` for\n\
         \x20   /// an encounter that positions nothing.\n\
         \x20   pub slot: Option<Slot>,\n\
         }}\n\
         \n\
         /// The monsters a fight opens with, in order.\n\
         pub type Spawns = Few<Spawn, {SPAWN_CAP}>;\n\
         \n\
         /// Every monster an encounter could ever field.\n\
         pub type Possible = Few<Monster, {POSSIBLE_CAP}>;\n\
         \n\
         /// Who is on the board when the fight starts.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum Roster {{\n\
         \x20   /// The same fight every time.\n\
         \x20   Fixed(Spawns),\n\
         \x20   /// Rolled from an `Rng` seeded from the run's seed, the floor,\n\
         \x20   /// and the encounter's own id. Ported by hand.\n\
         \x20   Random,\n\
         \x20   /// Chosen by whatever starts the fight, which is one event.\n\
         \x20   Configured,\n\
         }}\n\
         \n\
         /// What is true of an encounter with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct EncounterData {{\n\
         \x20   pub room: RoomType,\n\
         \x20   pub flags: FlagSet,\n\
         \x20   pub families: FamilySet,\n\
         \x20   pub gold: Gold,\n\
         \x20   /// How many slots a creature added mid-combat can take.\n\
         \x20   ///\n\
         \x20   /// `GetNextSlot` returns the first of the encounter's slots no\n\
         \x20   /// living enemy stands in, so what matters is how many there\n\
         \x20   /// are and in what order — slot 0 is searched first. The names\n\
         \x20   /// themselves are dropped: they are encounter-local scene\n\
         \x20   /// positions like `wriggler3` and `bomb5`, no two encounters\n\
         \x20   /// share a vocabulary, and nothing outside the game ever sees\n\
         \x20   /// one.\n\
         \x20   ///\n\
         \x20   /// A spawn's slot may be at or past this: three encounters\n\
         \x20   /// declare no arrival slots at all and still start monsters in\n\
         \x20   /// named positions, so indices from `arrivals` up are starting\n\
         \x20   /// positions and nothing else.\n\
         \x20   pub arrivals: u8,\n\
         \x20   /// Every monster this could ever field, summons included.\n\
         \x20   pub possible: Possible,\n\
         \x20   pub roster: Roster,\n\
         }}\n"
    )
    .unwrap();
}

/// One encounter's row, as the `const` its bucket file declares.
///
/// The slot names become indices here. A name means nothing outside its own
/// encounter, being a scene position like `wriggler3`, so what survives is the
/// position `GetNextSlot` needs.
///
/// The index space is the declared slots, followed by any a spawn names and the
/// encounter does not. `arrivals` is where the first kind stops. Three
/// encounters need that second half.
fn vocabulary(e: &Encounter) -> Vec<&str> {
    let mut vocabulary: Vec<&str> = e.slots.iter().map(String::as_str).collect();
    for (_, slot) in &e.spawns {
        if slot != "-" && !vocabulary.contains(&slot.as_str()) {
            vocabulary.push(slot);
        }
    }
    vocabulary
}

fn literal(id: &str, e: &Encounter) -> Result<String, String> {
    let vocabulary = vocabulary(e);
    let arrivals = e.slots.len();

    let mut out = String::new();
    writeln!(out, "EncounterData {{").unwrap();
    writeln!(out, "    room: RoomType::{},", e.room).unwrap();
    writeln!(
        out,
        "    flags: FlagSet::new(&[{}]),",
        e.flags
            .iter()
            .map(|f| format!("EncounterFlag::{f}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    writeln!(
        out,
        "    families: FamilySet::new(&[{}]),",
        e.tags
            .iter()
            .map(|t| format!("Family::{t}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    writeln!(
        out,
        "    gold: Gold {{ min: {}, max: {} }},",
        e.gold.0.literal(&format!("{id}.minGold"))?,
        e.gold.1.literal(&format!("{id}.maxGold"))?
    )
    .unwrap();
    writeln!(out, "    arrivals: {arrivals},").unwrap();
    writeln!(
        out,
        "    possible: Possible::new(&[{}]),",
        e.possible
            .iter()
            .map(|m| format!("Monster::{}", camel(m)))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();

    let roster = match e.roster.as_str() {
        "random" => "Roster::Random".to_string(),
        "configured" => "Roster::Configured".to_string(),
        "fixed" => {
            let spawns = e
                .spawns
                .iter()
                .map(|(monster, slot)| {
                    let slot = match slot.as_str() {
                        "-" => "None".to_string(),
                        name => format!(
                            "Some(Slot::{})",
                            SLOT_NAMES[vocabulary
                                .iter()
                                .position(|s| *s == name)
                                .expect("just interned")]
                        ),
                    };
                    format!(
                        "Spawn {{ monster: Monster::{}, slot: {slot} }}",
                        camel(monster)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("Roster::Fixed(Spawns::new(&[{spawns}]))")
        }
        other => return Err(format!("{id}: {other} is not a roster")),
    };

    // A roll that also listed a roster would be two answers to one question.
    if e.roster != "fixed" && !e.spawns.is_empty() {
        return Err(format!("{id}: {} but has spawn rows", e.roster));
    }

    writeln!(out, "    roster: {roster},").unwrap();
    write!(out, "}}").unwrap();
    Ok(out)
}

fn emit_encounters(
    out: &mut String,
    encounters: &BTreeMap<String, Encounter>,
    act_of: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    writeln!(
        out,
        "/// Every encounter in the build.\n\
         ///\n\
         /// Flat, and ordered by the game's own id. The generated files group by\n\
         /// room type, which is what `ActModel` itself groups them by — it\n\
         /// caches `AllRegularEncounters`, `AllEliteEncounters` and\n\
         /// `AllBossEncounters` as three filters on exactly that.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(encounters.len())?).unwrap();
    writeln!(out, "pub enum Encounter {{").unwrap();
    for id in encounters.keys() {
        writeln!(out, "    #[serde(rename = \"{id}\")]").unwrap();
        writeln!(out, "    {},", camel(id)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    let mut extras = String::new();
    writeln!(
        extras,
        "    /// The acts whose map can roll this encounter. Empty for one an\n\
         \x20   /// event starts instead, and for the mocks."
    )
    .unwrap();
    writeln!(extras, "    fn acts(self) -> &'static [Act] {{").unwrap();
    writeln!(extras, "        match self {{").unwrap();
    for id in encounters.keys() {
        let list = act_of
            .get(id)
            .map(|acts| {
                acts.iter()
                    .map(|a| format!("Act::{}", camel(a)))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        writeln!(extras, "            Self::{} => &[{list}],", camel(id)).unwrap();
    }
    writeln!(extras, "        }}").unwrap();
    writeln!(extras, "    }}").unwrap();

    writeln!(
        extras,
        "\n    /// Monster, elite or boss — the room the encounter is."
    )
    .unwrap();
    writeln!(extras, "    fn room(self) -> RoomType {{").unwrap();
    writeln!(extras, "        self.data().room").unwrap();
    writeln!(extras, "    }}").unwrap();

    writeln!(out, "impl sts2_core::Table for Encounter {{").unwrap();
    writeln!(out, "    type Data = EncounterData;\n").unwrap();
    writeln!(
        out,
        "    /// Every encounter, so callers can sweep the table without a way\n\
         \x20   /// to enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    const ALL: &'static [Self] = &[").unwrap();
    for id in encounters.keys() {
        writeln!(out, "        Encounter::{},", camel(id)).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(
        out,
        "    /// The room, the roster and the rewards.\n\
         \x20   ///\n\
         \x20   /// One arm per encounter, naming the file its row lives in."
    )
    .unwrap();
    writeln!(out, "    fn data(self) -> EncounterData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for (id, e) in encounters {
        writeln!(
            out,
            "            Self::{} => {}::{id},",
            camel(id),
            bucket(&e.room)?
        )
        .unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl sts2_core::EncounterTable for Encounter {{").unwrap();
    writeln!(out, "    type Act = Act;").unwrap();
    writeln!(out, "    type Room = RoomType;\n").unwrap();
    out.push_str(&extras);
    writeln!(out, "}}").unwrap();

    Ok(())
}
