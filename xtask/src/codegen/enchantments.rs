//! Every enchantment: what it can go on, and the flags around it.
//!
//! An enchantment is a modifier stuck to a card. What it *does* is
//! `EnchantDamageAdditive` and friends, which are code.
//!
//! The xtask README says why `enchants` is the probe narrowed rather than the
//! probe, and why `IsStackable` is not a column.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::DumpError;
use super::{Build, DERIVES, Dump, banner, camel, repr, sets};
use crate::error::{self, FileError};

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
    #[error("{0} is stackable, which no enchantment was when the column was dropped")]
    Stackable(String),
    #[error("{0} can be put on nothing at all")]
    Unplaceable(String),
}

/// The card types an enchantment can actually be put on.
///
/// The three `EnchantmentModel.CanEnchant` does not refuse outright. Anything
/// else the probe reports is unreachable. See the module docs.
const ENCHANTABLE: [&str; 3] = ["Attack", "Skill", "Power"];

/// The yes-or-no columns, in the order they become bits.
const FLAGS: [(&str, usize, &str); 4] = [
    ("ExtraCardText", 2, "Adds a line to the card it is on."),
    ("ShowsAmount", 3, "Displays a number beside itself."),
    (
        "StartsAtBottomOfDrawPile",
        4,
        "The card it is on begins the fight at the bottom of the draw\n\
         pile. One enchantment does this.",
    ),
    (
        "RefusesSomeCards",
        6,
        "Has a rule beyond the card's type: `CanEnchant` takes a whole\n\
         `CardModel` and five enchantments override it, so one can\n\
         refuse a card whose type it otherwise accepts. What the rule\n\
         *is* is code; that there is one is what an offer needs.",
    ),
];

pub fn generate(root: &Path) -> Result<Build, Error> {
    let dump = Dump::read("enchantments.txt")?;
    let build = dump.build.clone();

    let enchantments: Vec<&Vec<String>> = dump.of("enchantment").collect();
    if enchantments.is_empty() {
        return Err(DumpError::Empty("enchantments").into());
    }
    for row in &enchantments {
        if row.len() != 7 {
            return Err(DumpError::Columns {
                id: row[1].clone(),
                got: row.len() - 1,
                want: 6,
            }
            .into());
        }
        // The column exists in the dump and is not generated, so a build that
        // starts using it fails here rather than losing it silently.
        if row[5] != "0" {
            return Err(Error::Stackable(row[1].clone()));
        }
    }

    let mut types: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in dump.of("enchants") {
        if ENCHANTABLE.contains(&row[2].as_str()) {
            types
                .entry(row[1].clone())
                .or_default()
                .push(row[2].clone());
        }
    }

    let mut out = banner(
        "Every enchantment's card types and flags.",
        "enchantments.txt",
        &build,
    );
    writeln!(out, "\nuse super::cards::{{CardType, CardTypeSet}};").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    emit_flags(&mut out)?;
    emit_shape(&mut out);
    emit_enchantments(&mut out, &enchantments, &types)?;
    let overrides = super::overrides(&build, "ENCHANTMENT")?;
    super::emit_overrides(
        &mut out,
        "indexed by the enchantment's discriminant.",
        enchantments.iter().map(|row| row[1].as_str()),
        &overrides,
    );
    // Every hook some enchantment's class overrides, so a hook none can hear
    // skips the walk over the deck.
    let mut heard = String::from("sts2_core::Overrides::NONE");
    let mut seen: Vec<sts2_core::ModelHook> = Vec::new();
    for row in &enchantments {
        for hook in overrides.get(&row[1]).into_iter().flatten() {
            if !seen.contains(hook) {
                seen.push(*hook);
                write!(heard, ".with(sts2_core::ModelHook::{hook:?})").unwrap();
            }
        }
    }
    writeln!(
        out,
        "\n/// Every hook some enchantment's class overrides.\n\
         static HEARD_BY_ANY: sts2_core::Overrides = {heard};"
    )
    .unwrap();
    writeln!(
        out,
        "\nimpl sts2_core::EnchantmentTable for Enchantment {{\n    fn overrides(self) -> sts2_core::Overrides {{\n        OVERRIDES[self as usize]\n    }}\n\n    fn heard_by_any() -> sts2_core::Overrides {{\n        HEARD_BY_ANY\n    }}\n}}"
    )
    .unwrap();

    let path = root.join("enchantments.rs");
    error::write(&path, &out)?;

    let restricted = enchantments
        .iter()
        .filter(|r| types.get(&r[1]).is_none_or(|t| t.len() < ENCHANTABLE.len()))
        .count();
    println!(
        "  enchantments: {} ({restricted} restrict their card types)",
        enchantments.len()
    );

    Ok(build)
}

fn emit_flags(out: &mut String) -> Result<(), sets::Error> {
    sets::plain(
        out,
        &sets::Set {
            flag: "EnchantmentFlag",
            set: "EnchantmentFlags",
            flag_doc: "\n\
                       Four separate `EnchantmentModel` members rather than a game enum,\n\
                       so the numbering is the order the dump writes the columns in.",
            set_doc: "The flags an enchantment has.",
            members: &sets::counted(FLAGS.map(|(name, _, doc)| (name, doc))),
            declare: true,
            extras: &[],
        },
    )
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// What is true of an enchantment with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct EnchantmentData {{\n\
         \x20   /// The card types it can be put on. Never empty, and all three\n\
         \x20   /// for seventeen of the twenty-four.\n\
         \x20   pub enchants: CardTypeSet,\n\
         \x20   pub flags: EnchantmentFlags,\n\
         }}\n"
    )
    .unwrap();
}

fn emit_enchantments(
    out: &mut String,
    enchantments: &[&Vec<String>],
    types: &BTreeMap<String, Vec<String>>,
) -> Result<(), Error> {
    writeln!(
        out,
        "/// Every enchantment in the build.\n\
         ///\n\
         /// One file: there is no pool, rarity or act to group them by.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(enchantments.len())?).unwrap();
    writeln!(out, "pub enum Enchantment {{").unwrap();
    for row in enchantments {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl Enchantment {{").unwrap();
    writeln!(
        out,
        "    /// Every enchantment, so callers can sweep the table without a way\n\
         \x20   /// to enumerate an enum."
    )
    .unwrap();
    writeln!(
        out,
        "    pub const ALL: [Enchantment; {}] = [",
        enchantments.len()
    )
    .unwrap();
    for row in enchantments {
        writeln!(out, "        Enchantment::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// The card types it goes on, and the flags.").unwrap();
    writeln!(out, "    pub const fn data(self) -> EnchantmentData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in enchantments {
        let empty = Vec::new();
        let allowed = types.get(&row[1]).unwrap_or(&empty);
        if allowed.is_empty() {
            return Err(Error::Unplaceable(row[1].clone()));
        }

        let set = allowed
            .iter()
            .map(|t| format!("CardType::{t}"))
            .collect::<Vec<_>>()
            .join(", ");
        let flags = FLAGS
            .iter()
            .filter(|(_, column, _)| row[*column] == "1")
            .map(|(name, ..)| format!("EnchantmentFlag::{name}"))
            .collect::<Vec<_>>()
            .join(", ");

        writeln!(
            out,
            "            Self::{} => EnchantmentData {{",
            camel(&row[1])
        )
        .unwrap();
        writeln!(out, "                enchants: CardTypeSet::new(&[{set}]),").unwrap();
        writeln!(
            out,
            "                flags: EnchantmentFlags::new(&[{flags}]),"
        )
        .unwrap();
        writeln!(out, "            }},").unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(())
}
