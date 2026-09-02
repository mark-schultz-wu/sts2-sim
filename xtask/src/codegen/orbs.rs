//! Every orb: its two values, when its passive fires, and the list a random
//! orb is drawn from.
//!
//! An orb is almost entirely behaviour. `Passive` and `Evoke` pick targets, deal
//! damage, grant block or energy, and that is code. Three things around them are
//! table, and one of them is a trap.
//!
//! The xtask README says which of the two orb lists a roll indexes. It also
//! says why a value is a number and nothing more, and what `hooked` asks.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::{Build, DERIVES, Dump, banner, camel, repr};

/// How the dump spells each trigger, and the variant it becomes.
///
/// `none` is not a variant: an orb with no passive has no trigger, which is
/// `Option::None` rather than a third way to fire.
const TRIGGERS: [(&str, &str, &str); 2] = [
    (
        "turnEnd",
        "TurnEnd",
        "Before the turn ends. Four of the five.",
    ),
    (
        "turnStart",
        "TurnStart",
        "After the turn starts — a whole turn earlier than the rest,\n\
         which is when Plasma's energy arrives.",
    ),
];

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("orbs.txt")?;
    let build = dump.build.clone();

    let orbs: Vec<&Vec<String>> = dump.of("orb").collect();
    if orbs.is_empty() {
        return Err("the dump declares no orbs".into());
    }
    for row in &orbs {
        if row.len() != 5 {
            return Err(format!("{}: {} columns, not 4", row[1], row.len() - 1));
        }
    }

    let rollable = rollable(&orbs)?;
    let values = values(&dump, &orbs)?;

    let mut out = banner(
        "Every orb's values and when its passive fires.",
        "orbs.txt",
        &build,
    );
    writeln!(out, "\nuse nonmax::NonMaxU8;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    emit_shape(&mut out);
    emit_orbs(&mut out, &orbs, &rollable, &values)?;

    let path = root.join("orbs.rs");
    fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;

    println!(
        "  orbs: {} ({} a roll can produce)",
        orbs.len(),
        rollable.len()
    );

    Ok(build)
}

/// The orbs a roll produces, in the order `NextItem` indexes them.
///
/// Built from the index column rather than from dump order, because that column
/// *is* the array's order and dump order is alphabetical.
fn rollable(orbs: &[&Vec<String>]) -> Result<Vec<String>, String> {
    let mut placed: BTreeMap<usize, String> = BTreeMap::new();

    for row in orbs {
        let index: i32 = row[3]
            .parse()
            .map_err(|e| format!("{}: roll index {}: {e}", row[1], row[3]))?;

        if index < 0 {
            // Not in `_validOrbs`, and so not something a roll ever produces.
            // Its listing has to agree, or the two lists disagree about an orb
            // that plays.
            if row[4] == "1" {
                return Err(format!(
                    "{} is listed in ModelDb.Orbs but no roll can produce it — \
                     the two lists have diverged and this module's docs are wrong",
                    row[1]
                ));
            }
            continue;
        }

        if placed.insert(index as usize, row[1].clone()).is_some() {
            return Err(format!("two orbs claim roll index {index}"));
        }
    }

    let ordered: Vec<String> = placed.values().cloned().collect();
    if placed.keys().copied().ne(0..ordered.len()) {
        return Err(format!(
            "the roll indices are not 0..{}, so the array they came from has gaps",
            ordered.len()
        ));
    }

    Ok(ordered)
}

/// Each orb's two values, keyed by id, as `(passive, evoke)` Rust expressions.
fn values(dump: &Dump, orbs: &[&Vec<String>]) -> Result<BTreeMap<String, [String; 2]>, String> {
    let mut values: BTreeMap<String, [Option<String>; 2]> = BTreeMap::new();

    for row in dump.of("value") {
        if row.len() != 6 {
            return Err(format!(
                "{}: {} value columns, not 5",
                row[1],
                row.len() - 1
            ));
        }

        let slot = match row[2].as_str() {
            "passive" => 0,
            "evoke" => 1,
            other => return Err(format!("{}: {other} is neither passive nor evoke", row[1])),
        };

        // The dump's `stored` column is deliberately not read: the number is
        // the same whether the getter holds it or a field does, and sorting the
        // values by that sorts the C# rather than the orb. See the module docs.
        let amount = match row[3].as_str() {
            "dynamic" => "None".to_string(),
            n => {
                let n = n
                    .parse::<u8>()
                    .map_err(|e| format!("{}: {} value {n}: {e}", row[1], row[2]))?;
                // `NonMaxU8` spends 255 on being the absent marker, and this
                // field's absence already means "the getter names no number".
                // So a real 255 would arrive as `None` and read as no value at
                // all. Every orb value in this build is 0-8, so this is dormant
                // — it is here for the domain that adopts `NonMaxU8` with a
                // range that reaches the top of a byte.
                if n == u8::MAX {
                    return Err(format!(
                        "{}'s {} value is 255, which is the number NonMaxU8 spends on \
                         saying \"absent\" — generating it would turn a real value into \
                         no value at all. Store this domain's values as something wider \
                         than a NonMaxU8.",
                        row[1], row[2]
                    ));
                }
                format!("NonMaxU8::new({n})")
            }
        };

        let entry = values.entry(row[1].clone()).or_default();
        if entry[slot].is_some() {
            return Err(format!("{} has two {} values", row[1], row[2]));
        }
        entry[slot] = Some(format!(
            "OrbValue {{ amount: {amount}, hooked: {} }}",
            row[4] == "1"
        ));
    }

    let mut complete: BTreeMap<String, [String; 2]> = BTreeMap::new();
    for row in orbs {
        let [passive, evoke] = values
            .remove(&row[1])
            .ok_or_else(|| format!("{} has no values at all", row[1]))?;

        complete.insert(
            row[1].clone(),
            [
                passive.ok_or_else(|| format!("{} has no passive value", row[1]))?,
                evoke.ok_or_else(|| format!("{} has no evoke value", row[1]))?,
            ],
        );
    }

    match values.keys().next() {
        Some(stray) => Err(format!("{stray} has values but is not an orb")),
        None => Ok(complete),
    }
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// When an orb's passive fires.\n\
         ///\n\
         /// Both are `virtual` on `OrbModel` and do nothing by default, so which\n\
         /// one an orb overrides is the whole statement. An orb that overrides\n\
         /// neither has no passive, and carries no trigger at all."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum OrbTrigger {{").unwrap();
    for (dumped, variant, doc) in TRIGGERS {
        for line in doc.lines() {
            writeln!(out, "    /// {}", line.trim()).unwrap();
        }
        writeln!(out, "    #[serde(rename = \"{dumped}\")]").unwrap();
        writeln!(out, "    {variant},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// One of an orb's two numbers.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct OrbValue {{\n\
         \x20   /// The number the getter names, or nothing where it names none.\n\
         \x20   ///\n\
         \x20   /// `None` says only that: there is no numeral in the getter to\n\
         \x20   /// take. It is not a claim about the orb. Glass's evoke is the\n\
         \x20   /// only one, and it is `PassiveVal * 2` — the same relationship\n\
         \x20   /// Plasma's evoke has to its passive, which is spelled as a\n\
         \x20   /// plain 2 and so has a number here. How a value is combined,\n\
         \x20   /// decayed or accumulated is the orb's body, and bodies are\n\
         \x20   /// ported by hand.\n\
         \x20   pub amount: Option<NonMaxU8>,\n\
         \x20   /// Whether this getter runs `ModifyOrbValue` — whether Focus is\n\
         \x20   /// applied at this level.\n\
         \x20   ///\n\
         \x20   /// Two models implement that hook: `FocusPower` adds its amount\n\
         \x20   /// to any hooked value, and `InfusedCore` adds to Lightning's.\n\
         \x20   /// At *this* level is the whole of the claim: Glass's evoke is\n\
         \x20   /// unhooked and still moves with Focus, because it doubles a\n\
         \x20   /// passive that is hooked. What the flag rules out is a second\n\
         \x20   /// application on top. Plasma is the one orb Focus does not\n\
         \x20   /// reach at all.\n\
         \x20   pub hooked: bool,\n\
         }}\n\
         \n\
         /// What is true of an orb with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct OrbData {{\n\
         \x20   /// When the passive fires, or nothing where the orb has no\n\
         \x20   /// passive.\n\
         \x20   pub trigger: Option<OrbTrigger>,\n\
         \x20   /// What it does every turn while it sits in a slot.\n\
         \x20   pub passive: OrbValue,\n\
         \x20   /// What it does when it leaves one.\n\
         \x20   pub evoke: OrbValue,\n\
         }}\n"
    )
    .unwrap();
}

fn emit_orbs(
    out: &mut String,
    orbs: &[&Vec<String>],
    rollable: &[String],
    values: &BTreeMap<String, [String; 2]>,
) -> Result<(), String> {
    writeln!(
        out,
        "/// Every orb in the build.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(orbs.len())?).unwrap();
    writeln!(out, "pub enum Orb {{").unwrap();
    for row in orbs {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// The orbs a random channel produces, in the order a roll indexes\n\
         /// them.\n\
         ///\n\
         /// `OrbModel.GetRandomOrb` is `rng.NextItem(_validOrbs)`, so this is\n\
         /// that array and the order is which orb a given roll gives. It is not\n\
         /// `ModelDb.Orbs`, which holds four of these and is used by nothing\n\
         /// that plays — see the module docs.\n\
         pub const ROLLABLE: [Orb; {}] = [",
        rollable.len()
    )
    .unwrap();
    for id in rollable {
        writeln!(out, "    Orb::{},", camel(id)).unwrap();
    }
    writeln!(out, "];\n").unwrap();

    writeln!(out, "impl Orb {{").unwrap();
    writeln!(
        out,
        "    /// Every orb, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    pub const ALL: [Orb; {}] = [", orbs.len()).unwrap();
    for row in orbs {
        writeln!(out, "        Orb::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// Its values and when its passive fires.").unwrap();
    writeln!(out, "    pub const fn data(self) -> OrbData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in orbs {
        let trigger = match TRIGGERS.iter().find(|(dumped, ..)| *dumped == row[2]) {
            Some((_, variant, _)) => format!("Some(OrbTrigger::{variant})"),
            None if row[2] == "none" => "None".to_string(),
            None => return Err(format!("{}: {} is no trigger this knows", row[1], row[2])),
        };
        let [passive, evoke] = &values[&row[1]];

        writeln!(out, "            Self::{} => OrbData {{", camel(&row[1])).unwrap();
        writeln!(out, "                trigger: {trigger},").unwrap();
        writeln!(out, "                passive: {passive},").unwrap();
        writeln!(out, "                evoke: {evoke},").unwrap();
        writeln!(out, "            }},").unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(())
}
