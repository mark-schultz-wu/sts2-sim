//! Every event: which acts offer it, and the shell around its choices.
//!
//! What an event *does* is `GenerateInitialOptions`, which all 69 override and
//! which builds the player's choices out of run state. That is code and gets
//! ported by hand. What is table is which acts can offer one at all. Whether it
//! carries a condition, and the few facts the room reads before the first page.
//!
//! The xtask README says which two things the game calls "shared", and why
//! `deterministic` is not the negation of either.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr, sets};

/// The yes-or-no columns, in the order they become bits.
const FLAGS: [(&str, usize, &str); 4] = [
    (
        "Ancient",
        2,
        "An `AncientEventModel`, which an act offers from its own list\n\
         rather than alongside the rest.",
    ),
    (
        "SharedBetweenPlayers",
        3,
        "Every player in a multiplayer run goes through it together.\n\
         Nothing to do with the shared *pool* — see the module docs.",
    ),
    (
        "Deterministic",
        4,
        "The same event every time it is entered. Defaults to the\n\
         opposite of `SharedBetweenPlayers`, but `CRYSTAL_SPHERE` is\n\
         neither, so the two are separate facts.",
    ),
    (
        "Gated",
        7,
        "Has a condition on being offered: `IsAllowed` takes the run\n\
         state and 35 events override it. What the condition *is* is\n\
         code; that there is one is what a map roll needs.",
    ),
];

/// The bucket an event's row lands in.
///
/// Acts first, because that is where most events come from. The rest is what the
/// act lists do not cover. `anyact` for the nineteen in
/// `ModelDb.AllSharedEvents`, `crossover` for the two an act list names twice,
/// and `unlisted` for the four nothing offers.
fn bucket(acts: &[String], pooled: bool) -> String {
    match acts {
        [] if pooled => "anyact".into(),
        [] => "unlisted".into(),
        [_, _, ..] => "crossover".into(),
        [only] => only.to_lowercase(),
    }
}

fn bucket_doc(file: &str) -> String {
    match file {
        "anyact" => "Events any act can roll, from `ModelDb.AllSharedEvents`.".into(),
        "crossover" => "Events more than one act's list names.".into(),
        "unlisted" => "Events no act offers: two deprecated, one mock, and `THE_ARCHITECT`, \
             which something else starts."
            .into(),
        act => format!("Events only `{}` offers.", act.to_uppercase()),
    }
}

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("events.txt")?;
    let build = dump.build.clone();

    let events: Vec<&Vec<String>> = dump.of("event").collect();
    if events.is_empty() {
        return Err("the dump declares no events".into());
    }
    for row in &events {
        if row.len() != 8 {
            return Err(format!("{}: {} columns, not 7", row[1], row.len() - 1));
        }
    }

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

    let mut acts: Vec<String> = Vec::new();
    let mut acts_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in dump.of("act") {
        if !acts.contains(&row[1]) {
            acts.push(row[1].clone());
        }
        acts_of
            .entry(row[2].clone())
            .or_default()
            .push(row[1].clone());
    }

    let pooled: Vec<&str> = dump.of("sharedpool").map(|r| r[1].as_str()).collect();

    // The two memberships are meant to be disjoint: an event either belongs to
    // an act's list or to the pool every act draws from. Checked rather than
    // assumed, because an event in both would be offered twice.
    for id in &pooled {
        if acts_of.contains_key(*id) {
            return Err(format!(
                "{id} is in the shared pool and in an act's own list, so a map would offer it twice"
            ));
        }
    }

    let mut out = banner("Every event's acts and its shell.", "events.txt", &build);
    writeln!(out, "\nuse super::encounters::Encounter;").unwrap();
    writeln!(out, "use super::acts::Act;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    for (name, members) in &enums {
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            writeln!(out, "    {m} = {discriminant},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    emit_flags(&mut out)?;
    emit_shape(&mut out);
    emit_events(&mut out, &events, &acts_of, &pooled)?;

    let mut split = Split::new(root, "events", "events.txt", &build)?;
    for row in &events {
        let empty = Vec::new();
        let of = acts_of.get(&row[1]).unwrap_or(&empty);
        let literal = literal(row)?;
        writeln!(
            split.body(&bucket(of, pooled.contains(&row[1].as_str()))),
            "pub(super) const {}: EventData = {literal};\n",
            row[1]
        )
        .unwrap();
    }

    let files = split.names().count();
    for name in split.names() {
        writeln!(out, "mod {name};").unwrap();
    }
    writeln!(out).unwrap();

    split.finish(
        &out,
        bucket_doc,
        &[
            (
                "super",
                &["EventData", "EventFlag", "EventFlags", "EventLayoutType"],
            ),
            ("super::super::encounters", &["Encounter"]),
        ],
    )?;

    println!(
        "  events: {} across {} acts in {files} files, {} in the any-act pool",
        events.len(),
        acts.len(),
        pooled.len()
    );

    Ok(build)
}

/// The flags, as a one-word set.
///
/// One word and a `contains`, unlike a power's or a relic's. Nothing here is
/// computed, because every one of these is read off the model with no combat
/// running.
fn emit_flags(out: &mut String) -> Result<(), String> {
    sets::plain(
        out,
        &sets::Set {
            flag: "EventFlag",
            set: "EventFlags",
            flag_doc: "\n\
                       Four separate `EventModel` members rather than a game enum, so the\n\
                       numbering is the order the dump writes the columns in.",
            set_doc: "The flags an event has.",
            members: &sets::counted(FLAGS.map(|(name, _, doc)| (name, doc))),
            declare: true,
            extras: &[],
        },
    )
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// What is true of an event with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct EventData {{\n\
         \x20   pub flags: EventFlags,\n\
         \x20   /// Which layout the room draws. `Ancient` is the nine\n\
         \x20   /// `AncientEventModel`s and `Combat` the three that open into a\n\
         \x20   /// fight.\n\
         \x20   pub layout: EventLayoutType,\n\
         \x20   /// The fight this event starts, for the three that start one.\n\
         \x20   pub encounter: Option<Encounter>,\n\
         }}\n"
    )
    .unwrap();
}

fn literal(row: &[String]) -> Result<String, String> {
    let set = FLAGS
        .iter()
        .filter(|(_, column, _)| row[*column] == "1")
        .map(|(name, ..)| format!("EventFlag::{name}"))
        .collect::<Vec<_>>()
        .join(", ");

    let encounter = match row[6].as_str() {
        "-" => "None".to_string(),
        id => format!("Some(Encounter::{})", camel(id)),
    };

    let mut out = String::new();
    writeln!(out, "EventData {{").unwrap();
    writeln!(out, "    flags: EventFlags::new(&[{set}]),").unwrap();
    writeln!(out, "    layout: EventLayoutType::{},", row[5]).unwrap();
    writeln!(out, "    encounter: {encounter},").unwrap();
    write!(out, "}}").unwrap();
    Ok(out)
}

fn emit_events(
    out: &mut String,
    events: &[&Vec<String>],
    acts_of: &BTreeMap<String, Vec<String>>,
    pooled: &[&str],
) -> Result<(), String> {
    writeln!(
        out,
        "/// Every event in the build.\n\
         ///\n\
         /// Flat, and ordered by the game's own id. The generated files group by\n\
         /// act, with the any-act pool and the two an act names twice in their\n\
         /// own files.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(events.len())?).unwrap();
    writeln!(out, "pub enum Event {{").unwrap();
    for row in events {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    let mut extras = String::new();
    writeln!(
        extras,
        "    /// The acts whose own list offers this event.\n\
         \x20   ///\n\
         \x20   /// Empty for one in the any-act pool — see [`Event::any_act`] —\n\
         \x20   /// and for the four nothing offers."
    )
    .unwrap();
    writeln!(extras, "    fn acts(self) -> &'static [Act] {{").unwrap();
    writeln!(extras, "        match self {{").unwrap();
    for row in events {
        let list = acts_of
            .get(&row[1])
            .map(|a| {
                a.iter()
                    .map(|act| format!("Act::{}", camel(act)))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        writeln!(extras, "            Self::{} => &[{list}],", camel(&row[1])).unwrap();
    }
    writeln!(extras, "        }}").unwrap();
    writeln!(extras, "    }}\n").unwrap();

    writeln!(
        extras,
        "    /// Whether any act can roll this event.\n\
         \x20   ///\n\
         \x20   /// `ModelDb.AllSharedEvents` is concatenated onto every act's own\n\
         \x20   /// list, so these are offered everywhere and `acts` is empty for\n\
         \x20   /// them. Disjoint from `acts` by construction: codegen refuses an\n\
         \x20   /// event that is in both, since a map would offer it twice."
    )
    .unwrap();
    writeln!(extras, "    fn any_act(self) -> bool {{").unwrap();
    writeln!(extras, "        matches!(").unwrap();
    writeln!(extras, "            self,").unwrap();
    let names: Vec<String> = pooled
        .iter()
        .map(|id| format!("Self::{}", camel(id)))
        .collect();
    writeln!(extras, "            {}", names.join("\n                | ")).unwrap();
    writeln!(extras, "        )").unwrap();
    writeln!(extras, "    }}").unwrap();

    writeln!(out, "impl sts2_core::Table for Event {{").unwrap();
    writeln!(out, "    type Data = EventData;\n").unwrap();
    writeln!(
        out,
        "    /// Every event, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    const ALL: &'static [Self] = &[").unwrap();
    for row in events {
        writeln!(out, "        Event::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(
        out,
        "    /// The layout, the flags and the fight it starts.\n\
         \x20   ///\n\
         \x20   /// One arm per event, naming the file its row lives in."
    )
    .unwrap();
    writeln!(out, "    fn data(self) -> EventData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in events {
        let empty = Vec::new();
        let of = acts_of.get(&row[1]).unwrap_or(&empty);
        writeln!(
            out,
            "            Self::{} => {}::{},",
            camel(&row[1]),
            bucket(of, pooled.contains(&row[1].as_str())),
            row[1]
        )
        .unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl sts2_core::EventTable for Event {{").unwrap();
    writeln!(out, "    type Act = Act;\n").unwrap();
    out.push_str(&extras);
    writeln!(out, "}}").unwrap();

    Ok(())
}
