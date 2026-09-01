//! Every modifier: whether a run can offer it, what it rules out, and the two things it changes about how a run begins.
//! A modifier is a run-wide rule chosen before the run starts. Almost all of what one *does* is hooks, `ModifyGeneratedMap`, `TryModifyRewardsLate`, `ModifyRestSiteHealAmount`, and hooks are code.
//! # Which hooks are not here
//! Deliberately. A simulator's ported Midas already knows it hooks rewards. Therefore a list of overridden method names will describe the game's API surface rather than the game. A caller can do nothing with it that the ported body does not already do. What is generated is what a caller needs *before* any ported code runs: whether a run can pick this, what picking it rules out. The two things that happen at setup.
//! # `kind` is the two lists, and there is a third state
//! `ModelDb.GoodModifiers` and `BadModifiers` are what a run offers from, and between them they are the whole classification. A modifier in neither is one no run can pick. This is `None` here and is the deprecated one, the same shape the deprecated act takes, and for the same reason.
//! # Exclusion is a set, not a pair
//! `MutuallyExclusiveModifiers` is a list of sets and there is one set in it, holding three. A pair has been the easy read and has been wrong the moment a fourth arrived. Therefore `EXCLUSIVE` is a list of `ModifierSet` and `intersects` is the question a caller has: does what I have chosen already conflict with this.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::{Build, DERIVES, Dump, banner, camel, repr, sets};

/// How the dump spells each list, and the variant it becomes.
///
/// `unused` is not a variant: a modifier no run can offer has no kind, which is `Option::None` rather than a third thing to be.
const KINDS: [(&str, &str, &str); 2] = [
    ("good", "Good", "Offered as a boon. Nine of the seventeen."),
    ("bad", "Bad", "Offered as a drawback. Seven."),
];

/// The yes-or-no columns, in the order they become bits.
const FLAGS: [(&str, usize, &str); 3] = [
    (
        "ClearsDeck",
        3,
        "Empties the starting deck in `OnRunCreated`, before the run\n\
         begins. Three do, and they are exactly the three that rule\n\
         each other out.",
    ),
    (
        "OffersNeowOption",
        4,
        "Puts an option in front of the player at Neow. Five do.",
    ),
    (
        "TakesParameter",
        5,
        "Carries run state of its own — a `[SavedProperty]` — so the\n\
         same modifier means different things in two runs. Only\n\
         `CHARACTER_CARDS`, which names a character. What it names is\n\
         not here: with no run in progress there is no answer.",
    ),
];

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("modifiers.txt")?;
    let build = dump.build.clone();

    let modifiers: Vec<&Vec<String>> = dump.of("modifier").collect();
    if modifiers.is_empty() {
        return Err("the dump declares no modifiers".into());
    }
    for row in &modifiers {
        if row.len() != 6 {
            return Err(format!("{}: {} columns, not 5", row[1], row.len() - 1));
        }
    }

    let exclusive = exclusive(&dump, &modifiers)?;

    let mut out = banner(
        "Every modifier's kind, flags and exclusions.",
        "modifiers.txt",
        &build,
    );
    writeln!(out, "\nuse serde::{{Deserialize, Serialize}};\n").unwrap();

    emit_kind(&mut out);
    emit_flags(&mut out)?;
    emit_shape(&mut out);
    emit_modifiers(&mut out, &modifiers)?;
    emit_exclusive(&mut out, &modifiers, &exclusive)?;

    let path = root.join("modifiers.rs");
    fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;

    println!(
        "  modifiers: {} ({} a run can offer, {} exclusive groups)",
        modifiers.len(),
        modifiers.iter().filter(|r| r[2] != "unused").count(),
        exclusive.len()
    );

    Ok(build)
}

/// The exclusion groups, in the dump's order, each as a list of ids.
fn exclusive(dump: &Dump, modifiers: &[&Vec<String>]) -> Result<Vec<Vec<String>>, String> {
    let mut groups: BTreeMap<usize, Vec<String>> = BTreeMap::new();

    for row in dump.of("exclusive") {
        let group: usize = row[1]
            .parse()
            .map_err(|e| format!("exclusive group {}: {e}", row[1]))?;

        if !modifiers.iter().any(|m| m[1] == row[2]) {
            return Err(format!("{} rules others out but is not a modifier", row[2]));
        }

        groups.entry(group).or_default().push(row[2].clone());
    }

    let ordered: Vec<Vec<String>> = groups.values().cloned().collect();
    if groups.keys().copied().ne(0..ordered.len()) {
        return Err(format!(
            "the exclusion groups are not 0..{}, so one of them is empty",
            ordered.len()
        ));
    }

    // A group of one rules nothing out, and is a sign the dump lost a
    // member rather than that the game has a singleton.
    for (at, group) in ordered.iter().enumerate() {
        if group.len() < 2 {
            return Err(format!(
                "exclusion group {at} holds {} modifier(s), which rules nothing out",
                group.len()
            ));
        }
    }

    Ok(ordered)
}

fn emit_kind(out: &mut String) {
    writeln!(
        out,
        "/// Which list a run offers a modifier from.\n\
         ///\n\
         /// The two are `ModelDb.GoodModifiers` and `BadModifiers`, and a\n\
         /// modifier in neither is one no run can pick at all, that is no kind\n\
         /// rather than a third one."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum ModifierKind {{").unwrap();
    for (dumped, variant, doc) in KINDS {
        writeln!(out, "    /// {doc}").unwrap();
        writeln!(out, "    #[serde(rename = \"{dumped}\")]").unwrap();
        writeln!(out, "    {variant},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();
}

fn emit_flags(out: &mut String) -> Result<(), String> {
    sets::plain(
        out,
        &sets::Set {
            flag: "ModifierFlag",
            set: "ModifierFlags",
            flag_doc: "\n\
                       Three separate `ModifierModel` members rather than a game enum,\n\
                       so the numbering is the order the dump writes the columns in.",
            set_doc: "The flags a modifier has.",
            members: &FLAGS.map(|(name, _, doc)| (name, doc)),
            declare: true,
            extras: &[],
            bits: FLAGS.len() - 1,
        },
    )
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// What is true of a modifier with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct ModifierData {{\n\
         \x20   /// Which list a run offers it from, or nothing where no run\n\
         \x20   /// can.\n\
         \x20   pub kind: Option<ModifierKind>,\n\
         \x20   pub flags: ModifierFlags,\n\
         }}\n"
    )
    .unwrap();
}

fn emit_modifiers(out: &mut String, modifiers: &[&Vec<String>]) -> Result<(), String> {
    writeln!(
        out,
        "/// Every modifier in the build.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(modifiers.len())?).unwrap();
    writeln!(out, "pub enum Modifier {{").unwrap();
    for row in modifiers {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl Modifier {{").unwrap();
    writeln!(
        out,
        "    /// Every modifier, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(
        out,
        "    pub const ALL: [Modifier; {}] = [",
        modifiers.len()
    )
    .unwrap();
    for row in modifiers {
        writeln!(out, "        Modifier::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// Its kind and its flags.").unwrap();
    writeln!(out, "    pub const fn data(self) -> ModifierData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in modifiers {
        let kind = match KINDS.iter().find(|(dumped, ..)| *dumped == row[2]) {
            Some((_, variant, _)) => format!("Some(ModifierKind::{variant})"),
            None if row[2] == "unused" => "None".to_string(),
            None => return Err(format!("{}: {} is no kind this knows", row[1], row[2])),
        };
        let flags = FLAGS
            .iter()
            .filter(|(_, column, _)| row[*column] == "1")
            .map(|(name, ..)| format!("ModifierFlag::{name}"))
            .collect::<Vec<_>>()
            .join(", ");

        writeln!(
            out,
            "            Self::{} => ModifierData {{",
            camel(&row[1])
        )
        .unwrap();
        writeln!(out, "                kind: {kind},").unwrap();
        writeln!(
            out,
            "                flags: ModifierFlags::new(&[{flags}]),"
        )
        .unwrap();
        writeln!(out, "            }},").unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    Ok(())
}

fn emit_exclusive(
    out: &mut String,
    modifiers: &[&Vec<String>],
    exclusive: &[Vec<String>],
) -> Result<(), String> {
    let members: Vec<(String, String)> = modifiers
        .iter()
        .map(|row| (camel(&row[1]), String::new()))
        .collect();
    let members: Vec<(&str, &str)> = members
        .iter()
        .map(|(name, doc)| (name.as_str(), doc.as_str()))
        .collect();

    sets::plain(
        out,
        &sets::Set {
            flag: "Modifier",
            set: "ModifierSet",
            flag_doc: "",
            set_doc: "A set of modifiers — the shape an exclusion group takes.",
            members: &members,
            declare: false,
            extras: &[sets::Extra::IsEmpty, sets::Extra::Intersects],
            bits: modifiers.len() - 1,
        },
    )?;

    writeln!(
        out,
        "/// The groups a run may take at most one modifier from.\n\
         ///\n\
         /// One group of three in this build, and a list of sets rather than a\n\
         /// list of pairs because that is what the game holds: reading it as\n\
         /// pairs is right today and wrong the moment a fourth member\n\
         /// arrived. `ModifierSet::intersects` is the question a caller has.\n\
         pub const EXCLUSIVE: [ModifierSet; {}] = [",
        exclusive.len()
    )
    .unwrap();
    for group in exclusive {
        let set = group
            .iter()
            .map(|id| format!("Modifier::{}", camel(id)))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(out, "    ModifierSet::new(&[{set}]),").unwrap();
    }
    writeln!(out, "];").unwrap();

    Ok(())
}
