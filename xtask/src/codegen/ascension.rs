//! The ascension ladder.
//!
//! Eleven named levels, and ascension is a *scalar* over them rather than a set
//! of flags. `AscensionManager.HasLevel` is `_level >= (int)level`, so a run at
//! `Scarcity` also has everything below it, and each name is a threshold.
//!
//! The xtask README says why this is its own domain rather than the monster
//! dump's, and what a level does that is not here.

use std::fmt::Write as _;
use std::path::Path;

use super::DumpError;
use super::{Build, DERIVES, Dump, banner, repr};
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
    #[error("{name} is level {value} but sits at position {at}; the ladder is not dense")]
    NotDense { name: String, value: u8, at: usize },
    #[error(
        "the dump reflects no AscensionLevel::{0}, which a ported rule names: see what this build calls it"
    )]
    Unreflected(&'static str),
}

pub fn generate(root: &Path) -> Result<Build, Error> {
    let dump = Dump::read("ascension.txt")?;
    let build = dump.build.clone();

    let mut levels: Vec<(u8, String)> = Vec::new();
    for row in dump.of("level") {
        if row.len() != 3 {
            return Err(DumpError::Columns {
                id: row[2].clone(),
                got: row.len() - 1,
                want: 2,
            }
            .into());
        }
        let value: u8 = DumpError::number(format_args!("{}: level", row[2]), &row[1])?;
        levels.push((value, row[2].clone()));
    }

    if levels.is_empty() {
        return Err(DumpError::Empty("ascension levels").into());
    }

    // Dense and ascending, because the whole model is `>=` on the discriminant.
    // A gap or a reorder would make `has` quietly wrong rather than fail.
    for (at, (value, name)) in levels.iter().enumerate() {
        if usize::from(*value) != at {
            return Err(Error::NotDense {
                name: name.clone(),
                value: *value,
                at,
            });
        }
    }

    let mut out = banner("The ascension ladder.", "ascension.txt", &build);
    writeln!(out, "\nuse serde::{{Deserialize, Serialize}};\n").unwrap();

    emit(&mut out, &levels)?;

    let path = root.join("ascension.rs");
    error::write(&path, &out)?;

    println!("  ascension: {} levels", levels.len());

    Ok(build)
}

fn emit(out: &mut String, levels: &[(u8, String)]) -> Result<(), Error> {
    writeln!(
        out,
        "/// One rung of the ascension ladder.\n\
         ///\n\
         /// A run has a single level, and every name at or below it applies:\n\
         /// `AscensionManager.HasLevel` is `_level >= (int)level`. So this is a\n\
         /// scalar and not a set, and the discriminant order is the rule rather\n\
         /// than a listing convention.\n\
         ///\n\
         /// The variant name is the game's own, and the serde rename is the only\n\
         /// place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[derive(PartialOrd, Ord)]").unwrap();
    writeln!(out, "#[repr({})]", repr(levels.len())?).unwrap();
    writeln!(out, "pub enum AscensionLevel {{").unwrap();
    for (value, name) in levels {
        writeln!(out, "    #[serde(rename = \"{name}\")]").unwrap();
        writeln!(out, "    {name} = {value},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl AscensionLevel {{").unwrap();
    writeln!(
        out,
        "    /// Every level, from none to the highest.\n\
         \x20   pub const ALL: [AscensionLevel; {}] = [",
        levels.len()
    )
    .unwrap();
    for (_, name) in levels {
        writeln!(out, "        AscensionLevel::{name},").unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(
        out,
        "    /// Whether a run at this level has the rule `level` names.\n\
         \x20   ///\n\
         \x20   /// The game's own test, which is `>=` and not equality — a run at\n\
         \x20   /// `Scarcity` has Poverty's gold cut as well.\n\
         \x20   pub const fn has(self, level: AscensionLevel) -> bool {{\n\
         \x20       self as u8 >= level as u8\n\
         \x20   }}\n\
         }}\n"
    )
    .unwrap();

    emit_ladder(out, levels)?;
    Ok(())
}

/// The rungs a ported rule names, as the consts it names them by.
///
/// A generated enum is per build, so a rule holding only a `ContentVersion`
/// cannot write `AscensionLevel::SwarmingElites`. This is the bridge, and a
/// build that renames a rung stops here rather than reading as no ascension.
fn emit_ladder(out: &mut String, levels: &[(u8, String)]) -> Result<(), Error> {
    // `(const, rung)`, in the trait's own order.
    const NAMED: [(&str, &str); 7] = [
        ("INFLATION", "Inflation"),
        ("NONE", "None"),
        ("SCARCITY", "Scarcity"),
        ("SWARMING_ELITES", "SwarmingElites"),
        ("WEARY_TRAVELER", "WearyTraveler"),
        ("POVERTY", "Poverty"),
        ("TIGHT_BELT", "TightBelt"),
    ];

    for (_, rung) in &NAMED {
        if !levels.iter().any(|(_, have)| have == rung) {
            return Err(Error::Unreflected(rung));
        }
    }

    writeln!(
        out,
        "\nimpl sts2_core::AscensionKinds for AscensionLevel {{"
    )
    .unwrap();
    for (name, rung) in NAMED {
        writeln!(
            out,
            "    const {name}: AscensionLevel = AscensionLevel::{rung};"
        )
        .unwrap();
    }
    writeln!(
        out,
        "\n    fn has(self, rung: AscensionLevel) -> bool {{\n\
         \x20       AscensionLevel::has(self, rung)\n\
         \x20   }}\n\
         }}"
    )
    .unwrap();
    Ok(())
}
