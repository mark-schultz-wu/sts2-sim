//! Every affliction: what it can be stuck to, and the rules around it.
//!
//! An affliction is a debuff attached to a single card rather than to a
//! creature. It is the card-side twin of an enchantment, generated the same way.
//! What one *does* is `OnPlay`, `AfterApplied` and `BeforeRemoved`, which are
//! code, and the `Amount` each carries is combat state rather than table.
//!
//! The xtask README says why `afflicts` is the whole type rule, and why
//! `RefusesSomeCards` is not a column.

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
    #[error(
        "{0} refuses cards its type allows, which no affliction did when the column was dropped"
    )]
    RefusesCards(String),
    #[error("{0} can be stuck to nothing at all")]
    Unplaceable(String),
}

/// The card type the probe reports that the cards domain does not generate.
const UNMODELLED: &str = "None";

/// The yes-or-no columns, in the order they become bits.
const FLAGS: [(&str, usize, &str); 3] = [
    (
        "Stackable",
        2,
        "A second copy can go on a card that already has this one.\n\
         Two do; the rest replace nothing and refuse the card.",
    ),
    (
        "AfflictsUnplayableCards",
        3,
        "Can go on a card with the `Unplayable` keyword. The base\n\
         `CanAfflict` refuses those otherwise, whatever the type rule\n\
         says.",
    ),
    ("ExtraCardText", 4, "Adds a line to the card it is on."),
];

/// The column the dump measures and this table does not generate.
const REFUSES_SOME_CARDS: usize = 5;

pub fn generate(root: &Path) -> Result<Build, Error> {
    let dump = Dump::read("afflictions.txt")?;
    let build = dump.build.clone();

    let afflictions: Vec<&Vec<String>> = dump.of("affliction").collect();
    if afflictions.is_empty() {
        return Err(DumpError::Empty("afflictions").into());
    }
    for row in &afflictions {
        if row.len() != 6 {
            return Err(DumpError::Columns {
                id: row[1].clone(),
                got: row.len() - 1,
                want: 5,
            }
            .into());
        }
        // The column exists in the dump and is not generated, so a build that
        // starts using it fails here rather than losing it silently.
        if row[REFUSES_SOME_CARDS] != "0" {
            return Err(Error::RefusesCards(row[1].clone()));
        }
    }

    let mut types: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in dump.of("afflicts") {
        if row[2] != UNMODELLED {
            types
                .entry(row[1].clone())
                .or_default()
                .push(row[2].clone());
        }
    }

    let mut out = banner(
        "Every affliction's card types and flags.",
        "afflictions.txt",
        &build,
    );
    writeln!(out, "\nuse super::cards::{{CardType, CardTypeSet}};").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    emit_flags(&mut out)?;
    emit_shape(&mut out);
    emit_afflictions(&mut out, &afflictions, &types)?;

    let path = root.join("afflictions.rs");
    error::write(&path, &out)?;

    let restricted = afflictions
        .iter()
        .filter(|r| types.get(&r[1]).is_none_or(|t| t.len() < 6))
        .count();
    println!(
        "  afflictions: {} ({restricted} restrict their card types)",
        afflictions.len()
    );

    Ok(build)
}

fn emit_flags(out: &mut String) -> Result<(), sets::Error> {
    sets::plain(
        out,
        &sets::Set {
            flag: "AfflictionFlag",
            set: "AfflictionFlags",
            flag_doc: "\n\
                       Three separate `AfflictionModel` members rather than a game enum,\n\
                       so the numbering is the order the dump writes the columns in.",
            set_doc: "The flags an affliction has.",
            members: &sets::counted(FLAGS.map(|(name, _, doc)| (name, doc))),
            declare: true,
            extras: &[],
        },
    )
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// What is true of an affliction with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct AfflictionData {{\n\
         \x20   /// The card types it can be stuck to. Never empty, and every\n\
         \x20   /// type for all but one.\n\
         \x20   pub afflicts: CardTypeSet,\n\
         \x20   pub flags: AfflictionFlags,\n\
         }}\n"
    )
    .unwrap();
}

fn emit_afflictions(
    out: &mut String,
    afflictions: &[&Vec<String>],
    types: &BTreeMap<String, Vec<String>>,
) -> Result<(), Error> {
    writeln!(
        out,
        "/// Every affliction in the build.\n\
         ///\n\
         /// One file: there is no pool, rarity or act to group them by.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(afflictions.len())?).unwrap();
    writeln!(out, "pub enum Affliction {{").unwrap();
    for row in afflictions {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl Affliction {{").unwrap();
    writeln!(
        out,
        "    /// Every affliction, so callers can sweep the table without a way\n\
         \x20   /// to enumerate an enum."
    )
    .unwrap();
    writeln!(
        out,
        "    pub const ALL: [Affliction; {}] = [",
        afflictions.len()
    )
    .unwrap();
    for row in afflictions {
        writeln!(out, "        Affliction::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// The card types it goes on, and the flags.").unwrap();
    writeln!(out, "    pub const fn data(self) -> AfflictionData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in afflictions {
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
            .map(|(name, ..)| format!("AfflictionFlag::{name}"))
            .collect::<Vec<_>>()
            .join(", ");

        writeln!(
            out,
            "            Self::{} => AfflictionData {{",
            camel(&row[1])
        )
        .unwrap();
        writeln!(out, "                afflicts: CardTypeSet::new(&[{set}]),").unwrap();
        writeln!(
            out,
            "                flags: AfflictionFlags::new(&[{flags}]),"
        )
        .unwrap();
        writeln!(out, "            }},").unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    writeln!(
        out,
        "\nimpl sts2_core::AfflictionTable for Affliction {{\n\
         \x20   fn afflicts(self, kind: sts2_core::CardKind) -> bool {{\n\
         \x20       self.data().afflicts.contains(match kind {{\n\
         \x20           sts2_core::CardKind::Attack => CardType::Attack,\n\
         \x20           sts2_core::CardKind::Skill => CardType::Skill,\n\
         \x20           sts2_core::CardKind::Power => CardType::Power,\n\
         \x20           sts2_core::CardKind::Status => CardType::Status,\n\
         \x20           sts2_core::CardKind::Curse => CardType::Curse,\n\
         \x20           sts2_core::CardKind::Quest => CardType::Quest,\n\
         \x20       }})\n\
         \x20   }}\n\
         \n\
         \x20   fn afflicts_unplayable(self) -> bool {{\n\
         \x20       self.data()\n\
         \x20           .flags\n\
         \x20           .contains(AfflictionFlag::AfflictsUnplayableCards)\n\
         \x20   }}\n\
         \n\
         \x20   fn stackable(self) -> bool {{\n\
         \x20       self.data().flags.contains(AfflictionFlag::Stackable)\n\
         \x20   }}\n\
         }}"
    )
    .unwrap();

    Ok(())
}
