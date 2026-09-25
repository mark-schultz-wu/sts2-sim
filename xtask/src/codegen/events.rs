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

use super::DumpError;
use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr, sets};
use crate::error::FileError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Dump(#[from] DumpError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Set(#[from] super::sets::Error),
    #[error(transparent)]
    Bridge(#[from] super::bridge::Error),
    #[error("{0} is in the shared pool and in an act's own list, so a map would offer it twice")]
    Twice(String),
    #[error(
        "{list} lists {event} at index {at}, which is not the {is} it is at: the rows are out of order"
    )]
    OutOfOrder {
        list: String,
        event: String,
        at: usize,
        is: usize,
    },
    #[error("the dump reflects no Neow {0}")]
    NoNeowList(String),
    #[error("{relic} is not one of Neow's {list} any more")]
    NotNeows {
        relic: &'static str,
        list: &'static str,
    },
    #[error("Neow's {0} is not one option")]
    NotOneOption(String),
    #[error("{0} names an event the dump does not list")]
    NoSuchEvent(String),
    #[error("two options are both spelled {0}")]
    SameName(String),
}

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

pub fn generate(root: &Path) -> Result<Build, Error> {
    let dump = Dump::read("events.txt")?;
    let build = dump.build.clone();

    let events: Vec<&Vec<String>> = dump.of("event").collect();
    if events.is_empty() {
        return Err(DumpError::Empty("events").into());
    }
    for row in &events {
        if row.len() != 8 {
            return Err(DumpError::Columns {
                id: row[1].clone(),
                got: row.len() - 1,
                want: 7,
            }
            .into());
        }
    }

    let mut enums: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    for f in dump.of("enum") {
        let discriminant =
            DumpError::number::<u8>(format_args!("{} {}: discriminant", f[1], f[3]), &f[2])?;
        enums
            .entry(f[1].clone())
            .or_default()
            .push((discriminant, f[3].clone()));
    }

    // Four kinds of row, two shapes. An act's own events and its ancients are
    // separate lists the game rolls from separately, and each is ordered: room
    // generation shuffles the event list, and a shuffle permutes what it was
    // handed, so the order is a rule.
    let mut acts: Vec<String> = Vec::new();
    let mut acts_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut offered: Vec<(String, Vec<String>)> = Vec::new();
    let mut ancients: Vec<(String, Vec<String>)> = Vec::new();
    for kind in ["act", "ancient"] {
        let listed = if kind == "act" {
            &mut offered
        } else {
            &mut ancients
        };
        for row in dump.of(kind) {
            if row.len() != 4 {
                return Err(DumpError::Columns {
                    id: format!("{kind} {}", row[1]),
                    got: row.len() - 1,
                    want: 3,
                }
                .into());
            }
            if !acts.contains(&row[1]) {
                acts.push(row[1].clone());
            }
            let of = acts_of.entry(row[3].clone()).or_default();
            if !of.contains(&row[1]) {
                of.push(row[1].clone());
            }
            ordered(listed, &row[1], &row[2], &row[3])?;
        }
    }

    // Neow's own lists, which are code in the event rather than a table.
    let mut neow: Vec<(String, Vec<String>)> = Vec::new();
    for row in dump.of("neow") {
        if row.len() != 4 {
            return Err(DumpError::Columns {
                id: format!("neow {}", row[1]),
                got: row.len() - 1,
                want: 3,
            }
            .into());
        }
        ordered(&mut neow, &row[1], &row[2], &row[3])?;
    }

    let mut shared: Vec<(String, Vec<String>)> = Vec::new();
    for kind in ["sharedpool", "sharedancient"] {
        for row in dump.of(kind) {
            if row.len() != 3 {
                return Err(DumpError::Columns {
                    id: format!("{kind} {}", row[2]),
                    got: row.len() - 1,
                    want: 2,
                }
                .into());
            }
            ordered(&mut shared, kind, &row[1], &row[2])?;
        }
    }
    let pooled: Vec<&str> = shared
        .iter()
        .flat_map(|(_, listed)| listed.iter().map(String::as_str))
        .collect();

    // The two memberships are meant to be disjoint: an event either belongs to
    // an act's list or to the pool every act draws from. Checked rather than
    // assumed, because an event in both would be offered twice.
    for id in &pooled {
        if acts_of.contains_key(*id) {
            return Err(Error::Twice(id.to_string()));
        }
    }

    let mut out = banner("Every event's acts and its shell.", "events.txt", &build);
    writeln!(out, "\nuse super::encounters::Encounter;").unwrap();
    writeln!(out, "use super::relics::Relic;").unwrap();
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
    emit_lists(&mut out, "events_of", "an act's own events", &offered);
    emit_lists(&mut out, "ancients_of", "an act's own ancients", &ancients);
    emit_shared(&mut out, &shared);
    emit_neow(&mut out, &neow)?;
    emit_options(&mut out, &dump, &events)?;

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
fn emit_flags(out: &mut String) -> Result<(), sets::Error> {
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
         \x20   /// The entry name the game hashes into this event's seed.\n\
         \x20   pub seed_name: &'static str,\n\
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

fn literal(row: &[String]) -> Result<String, Error> {
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
    writeln!(out, "    seed_name: {:?},", row[1]).unwrap();
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
) -> Result<(), Error> {
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

    // From the layout rather than from a second list. `AncientEventModel` is
    // the type, and `EventLayoutType::Ancient` is how the dump reports it.
    writeln!(
        extras,
        "\n    /// One of the nine ancients, which an act draws separately.\n\
         \x20   fn ancient(self) -> bool {{\n\
         \x20       <Self as sts2_core::Table>::data(self).layout == EventLayoutType::Ancient\n\
         \x20   }}\n\
         \n\
         \x20   /// The event id hash used by its private RNG.\n\
         \x20   fn seed_name(self) -> &'static str {{\n\
         \x20       <Self as sts2_core::Table>::data(self).seed_name\n\
         \x20   }}"
    )
    .unwrap();

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
        "    /// The layout, the flags and the fight it starts."
    )
    .unwrap();
    let mut table: Vec<String> = Vec::new();
    for row in events {
        let empty = Vec::new();
        let of = acts_of.get(&row[1]).unwrap_or(&empty);
        table.push(format!(
            "{}::{}",
            bucket(of, pooled.contains(&row[1].as_str())),
            row[1]
        ));
    }
    writeln!(out, "    fn data(self) -> &'static EventData {{").unwrap();
    writeln!(out, "        &ROWS[self as usize]").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    super::rows(out, "EventData", &table);

    writeln!(out, "impl sts2_core::EventTable for Event {{").unwrap();
    writeln!(out, "    type Act = Act;\n").unwrap();
    out.push_str(&extras);
    writeln!(out, "}}").unwrap();

    Ok(())
}

/// One id appended to the list it belongs to, at the index it claims.
///
/// Dense and ascending. A gap means the dump dropped one, and an order
/// recovered from rows that arrived out of order is not an order.
fn ordered(
    listed: &mut Vec<(String, Vec<String>)>,
    key: &str,
    index: &str,
    id: &str,
) -> Result<(), Error> {
    let at: usize = DumpError::number(format_args!("{key} {id}: index"), index)?;
    let list = match listed.iter_mut().find(|(held, _)| held == key) {
        Some((_, list)) => list,
        None => {
            listed.push((key.to_string(), Vec::new()));
            &mut listed.last_mut().expect("just pushed").1
        }
    };
    if at != list.len() {
        return Err(Error::OutOfOrder {
            list: key.to_owned(),
            event: id.to_owned(),
            at,
            is: list.len(),
        });
    }
    list.push(id.to_string());
    Ok(())
}

/// A per-act ordered list, as a function over acts.
fn emit_lists(out: &mut String, name: &str, what: &str, listed: &[(String, Vec<String>)]) {
    writeln!(out, "/// Every event in {what}, in the act's own order.").unwrap();
    writeln!(
        out,
        "///\n\
         /// Not sorted. Room generation shuffles this list, and a shuffle\n\
         /// permutes what it was handed, so the order decides the result.\n\
         ///\n\
         /// Empty for an act with none."
    )
    .unwrap();
    writeln!(out, "pub fn {name}(act: Act) -> &'static [Event] {{").unwrap();
    writeln!(out, "    match act {{").unwrap();
    for (act, list) in listed {
        let ids = list
            .iter()
            .map(|id| format!("Event::{}", camel(id)))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(out, "        Act::{} => &[{ids}],", camel(act)).unwrap();
    }
    writeln!(out, "        #[allow(unreachable_patterns)]").unwrap();
    writeln!(out, "        _ => &[],").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
}

/// The two lists every act draws from, in the game's own order.
fn emit_shared(out: &mut String, shared: &[(String, Vec<String>)]) {
    for (kind, name, what) in [
        ("sharedpool", "SHARED_EVENTS", "every act's own list"),
        (
            "sharedancient",
            "SHARED_ANCIENTS",
            "the run, dealt out to the later acts",
        ),
    ] {
        let empty = Vec::new();
        let list = shared
            .iter()
            .find(|(held, _)| held == kind)
            .map(|(_, list)| list)
            .unwrap_or(&empty);
        writeln!(out, "/// The events concatenated onto {what}.").unwrap();
        writeln!(
            out,
            "///\n/// In the game's own order, which is not sorted."
        )
        .unwrap();
        writeln!(out, "pub const {name}: [Event; {}] = [", list.len()).unwrap();
        for id in list {
            writeln!(out, "    Event::{},", camel(id)).unwrap();
        }
        writeln!(out, "];\n").unwrap();
    }
}

/// Neow's option lists, and the pairings its own body spells out.
///
/// The lists are reflected. The pairings are not reachable that way: they are
/// `is` checks inside `GenerateInitialOptions`, so they are named here and
/// checked against the lists. A build that renames one stops generation.
fn emit_neow(out: &mut String, neow: &[(String, Vec<String>)]) -> Result<(), Error> {
    let list = |name: &str| -> Result<&Vec<String>, Error> {
        neow.iter()
            .find(|(held, _)| held == name)
            .map(|(_, list)| list)
            .ok_or_else(|| Error::NoNeowList(name.to_owned()))
    };

    // A curse that takes a positive off the table with it, because the two do
    // the same thing. `GenerateInitialOptions` spells these four out.
    const PAIRS: [(&str, &str); 4] = [
        ("CURSED_PEARL", "GOLDEN_PEARL"),
        ("HEFTY_TABLET", "ARCANE_SCROLL"),
        ("LEAFY_POULTICE", "NEW_LEAF"),
        ("PRECARIOUS_SHEARS", "PRECISE_SCISSORS"),
    ];
    // The curse that suppresses the first coin flip.
    const SUPPRESSES: &str = "LARGE_CAPSULE";

    let curses = list("CurseOptions")?;
    let positives = list("PositiveOptions")?;
    for (curse, positive) in PAIRS {
        if !curses.contains(&curse.to_string()) {
            return Err(Error::NotNeows {
                relic: curse,
                list: "curses",
            });
        }
        if !positives.contains(&positive.to_string()) {
            return Err(Error::NotNeows {
                relic: positive,
                list: "positives",
            });
        }
    }
    if !curses.contains(&SUPPRESSES.to_string()) {
        return Err(Error::NotNeows {
            relic: SUPPRESSES,
            list: "curses",
        });
    }

    writeln!(
        out,
        "/// The curses Neow offers, one of which every blessing comes with.\n\
         ///\n\
         /// In the game's own order, which one draw indexes into."
    )
    .unwrap();
    emit_relics(out, "NEOW_CURSES", curses);

    writeln!(
        out,
        "/// The blessings Neow offers, before the three coin flips add to them.\n\
         ///\n\
         /// In the game's own order, which is what the shuffle permutes."
    )
    .unwrap();
    emit_relics(out, "NEOW_POSITIVES", positives);

    // One of each pair is added, on a coin flip, in this order. The first is
    // skipped when the curse drawn is the one that suppresses it.
    const FLIPS: [(&str, &str); 3] = [
        ("LavaRockOption", "SmallCapsuleOption"),
        ("NutritiousOysterOption", "StoneHumidifierOption"),
        ("NeowsTalismanOption", "PomanderOption"),
    ];
    writeln!(
        out,
        "/// The three pairs a coin flip adds one of, in the order they are\n\
         /// added.\n\
         ///\n\
         /// Heads takes the first. The first pair is skipped when the curse\n\
         /// drawn is [`NEOW_SUPPRESSES_FLIP`]."
    )
    .unwrap();
    writeln!(out, "pub const NEOW_FLIPS: [(Relic, Relic); 3] = [").unwrap();
    for (heads, tails) in FLIPS {
        let one = |name: &str| -> Result<String, Error> {
            let held = list(name)?;
            match held.as_slice() {
                [only] => Ok(format!("Relic::{}", camel(only))),
                _ => Err(Error::NotOneOption(name.to_owned())),
            }
        };
        writeln!(out, "    ({}, {}),", one(heads)?, one(tails)?).unwrap();
    }
    writeln!(out, "];\n").unwrap();

    writeln!(
        out,
        "/// A curse that takes a blessing off the table with it.\n\
         ///\n\
         /// The two do the same thing, so Neow does not offer both."
    )
    .unwrap();
    writeln!(out, "pub const NEOW_PAIRS: [(Relic, Relic); 4] = [").unwrap();
    for (curse, positive) in PAIRS {
        writeln!(
            out,
            "    (Relic::{}, Relic::{}),",
            camel(curse),
            camel(positive)
        )
        .unwrap();
    }
    writeln!(out, "];\n").unwrap();

    writeln!(
        out,
        "/// The curse that suppresses the first of [`NEOW_FLIPS`]."
    )
    .unwrap();
    writeln!(
        out,
        "pub const NEOW_SUPPRESSES_FLIP: Relic = Relic::{};\n",
        camel(SUPPRESSES)
    )
    .unwrap();
    Ok(())
}

/// One named list of relics.
fn emit_relics(out: &mut String, name: &str, relics: &[String]) {
    writeln!(out, "pub const {name}: [Relic; {}] = [", relics.len()).unwrap();
    for id in relics {
        writeln!(out, "    Relic::{},", camel(id)).unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

/// One option a page names: the event, the page, and the option's own name.
struct Option_ {
    event: String,
    page: String,
    name: String,
    /// Whether the table holds a `_LOCKED` twin, which a locked option shows.
    locked: bool,
}

impl Option_ {
    /// The variant, named for the event and the option, with the page
    /// between the two when it is not the first.
    fn variant(&self) -> String {
        match self.page.as_str() {
            "INITIAL" => format!("{}{}", camel(&self.event), camel(&self.name)),
            page => format!("{}{}{}", camel(&self.event), camel(page), camel(&self.name)),
        }
    }
}

/// Every option a page names, one variant each, with its wire spelling.
///
/// A locked option is not an option of its own. The table spells it with
/// `_LOCKED` on the end, and the variant says whether that twin exists.
fn emit_options(out: &mut String, dump: &Dump, events: &[&Vec<String>]) -> Result<(), Error> {
    let mut options: Vec<Option_> = Vec::new();
    for row in dump.of("option") {
        if row.len() != 4 {
            return Err(DumpError::Columns {
                id: format!("option {}", row[1]),
                got: row.len() - 1,
                want: 3,
            }
            .into());
        }
        if !events.iter().any(|event| event[1] == row[1]) {
            return Err(Error::NoSuchEvent(row[1].clone()));
        }
        let (name, locked) = match row[3].strip_suffix("_LOCKED") {
            Some(name) => (name, true),
            None => (row[3].as_str(), false),
        };
        let same = |o: &&mut Option_| o.event == row[1] && o.page == row[2] && o.name == name;
        match options.iter_mut().find(same) {
            Some(option) => option.locked |= locked,
            None => options.push(Option_ {
                event: row[1].clone(),
                page: row[2].clone(),
                name: name.to_owned(),
                locked,
            }),
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for option in &options {
        if !seen.insert(option.variant()) {
            return Err(Error::SameName(option.variant()));
        }
    }

    writeln!(
        out,
        "/// Every option an event's pages name, by its localisation key.\n\
         ///\n\
         /// `<event>.pages.<page>.options.<name>`. A locked option shows the\n\
         /// same key with `_LOCKED` on the end, and is this option still."
    )
    .unwrap();
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]").unwrap();
    writeln!(out, "#[repr({})]", repr(options.len())?).unwrap();
    writeln!(out, "pub enum EventOption {{").unwrap();
    for option in &options {
        writeln!(out, "    {},", option.variant()).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl EventOption {{").unwrap();
    writeln!(out, "    pub const ALL: &'static [Self] = &[").unwrap();
    for option in &options {
        writeln!(out, "        Self::{},", option.variant()).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();
    writeln!(out, "    /// The event whose page names this option.").unwrap();
    writeln!(out, "    pub fn event(self) -> Event {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for option in &options {
        writeln!(
            out,
            "            Self::{} => Event::{},",
            option.variant(),
            camel(&option.event)
        )
        .unwrap();
    }
    writeln!(out, "        }}\n    }}\n").unwrap();
    writeln!(
        out,
        "    /// The page and the name, as the key spells them. A locked option\n\
         \x20   /// has `_LOCKED` on its name, where the table holds that twin."
    )
    .unwrap();
    writeln!(
        out,
        "    pub fn spelled(self, locked: bool) -> (&'static str, &'static str) {{"
    )
    .unwrap();
    writeln!(out, "        match (self, locked) {{").unwrap();
    for option in &options {
        let variant = option.variant();
        if option.locked {
            writeln!(
                out,
                "            (Self::{variant}, true) => ({:?}, \"{}_LOCKED\"),",
                option.page, option.name
            )
            .unwrap();
        }
        writeln!(
            out,
            "            (Self::{variant}, _) => ({:?}, {:?}),",
            option.page, option.name
        )
        .unwrap();
    }
    writeln!(out, "        }}\n    }}\n").unwrap();
    writeln!(
        out,
        "    /// The option a key names, with any `_LOCKED` taken off its name."
    )
    .unwrap();
    writeln!(
        out,
        "    pub fn from_key(event: Event, page: &str, name: &str) -> Option<Self> {{"
    )
    .unwrap();
    writeln!(
        out,
        "        let name = name.strip_suffix(\"_LOCKED\").unwrap_or(name);"
    )
    .unwrap();
    writeln!(
        out,
        "        Self::ALL.iter().copied().find(|option| {{\n\
         \x20           option.event() == event && option.spelled(false) == (page, name)\n\
         \x20       }})"
    )
    .unwrap();
    writeln!(out, "    }}\n}}\n").unwrap();
    println!("  events: {} options", options.len());
    Ok(())
}
