//! The options a rest site can offer, by the name each one answers to.
//!
//! Nine names and no columns, because there is nothing else about them that is
//! table. What an option does is `OnSelect`, and whether a given rest site
//! offers it depends on the run's relics. Both are behaviour, and the
//! observation reports the result of both. What the build has is the closed set
//! of names, and that is what an observation's `id` has to be one of.
//!
//! The xtask README says why this is a domain at all.

use std::fmt::Write as _;
use std::path::Path;

use super::DumpError;
use super::{Build, DERIVES, Dump, banner, camel, repr};
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
}

pub fn generate(root: &Path) -> Result<Build, Error> {
    let dump = Dump::read("restsite.txt")?;
    let build = dump.build.clone();

    let options: Vec<&Vec<String>> = dump.of("option").collect();
    if options.is_empty() {
        return Err(DumpError::Empty("rest site options").into());
    }
    for row in &options {
        if row.len() != 2 {
            return Err(DumpError::Columns {
                id: row[1].clone(),
                got: row.len() - 1,
                want: 1,
            }
            .into());
        }
    }

    let mut out = banner("The options a rest site can offer.", "restsite.txt", &build);
    writeln!(out, "\nuse serde::{{Deserialize, Serialize}};\n").unwrap();

    writeln!(
        out,
        "/// Every rest site option in the build.\n\
         ///\n\
         /// Not a model — these are `RestSiteOption` subclasses and the name is\n\
         /// `OptionId` on each — so this is the whole of what the build says\n\
         /// about them.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(options.len())?).unwrap();
    writeln!(out, "pub enum RestSiteOption {{").unwrap();
    for row in &options {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl RestSiteOption {{").unwrap();
    writeln!(
        out,
        "    /// Every option, so callers can sweep the set without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(
        out,
        "    pub const ALL: [RestSiteOption; {}] = [",
        options.len()
    )
    .unwrap();
    for row in &options {
        writeln!(out, "        RestSiteOption::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];").unwrap();
    writeln!(out, "}}").unwrap();

    let path = root.join("restsite.rs");
    error::write(&path, &out)?;

    println!("  restsite: {} options", options.len());

    Ok(build)
}
