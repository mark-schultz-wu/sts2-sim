//! A generated module that is a directory rather than a file.
//!
//! All three domains outgrew one file, and each outgrew it the same way.
//! `mod.rs` keeps the shape: the field enums, the data struct, and the outer
//! enum. The rows go into buckets beside it. What a bucket *is* differs, and
//! that is the interesting part, so it stays in the domain. A card pool decides
//! what a character can draft. An act decides which encounters can roll a
//! monster. A power's kind is the one thing the game classifies a power by.
//!
//! What is identical is everything after that decision. Banner the file, import
//! exactly what it names, write it, and declare it in the parent. Three times,
//! once each, until this.
//!
//! The xtask README says why the import list is the part worth sharing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use super::{Build, banner};

/// Where a bucket file imports from, and the names to consider from each.
///
/// Candidates, not requirements. A group whose names the file never mentions is
/// not written at all.
pub type Imports<'a> = &'a [(&'a str, &'a [&'a str])];

/// A directory of generated files, accumulated before any of it is written.
pub struct Split {
    dir: PathBuf,
    source: String,
    build: Build,
    buckets: BTreeMap<String, String>,
}

impl Split {
    pub fn new(root: &Path, module: &str, source: &str, build: &Build) -> Result<Split, String> {
        let dir = root.join(module);
        fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;

        Ok(Split {
            dir,
            source: source.to_string(),
            build: build.clone(),
            buckets: BTreeMap::new(),
        })
    }

    /// One bucket's body, to append to.
    ///
    /// Naming a bucket is what creates it, so a bucket that turns out to hold
    /// nothing still gets a file. That is what the parent's `mod` declaration
    /// expects, and an empty pool is a real thing rather than a mistake.
    pub fn body(&mut self, bucket: &str) -> &mut String {
        self.buckets.entry(bucket.to_string()).or_default()
    }

    /// The buckets, in the order their `mod` declarations want to be written.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.buckets.keys().map(String::as_str)
    }

    /// Writes every bucket file, then `mod.rs` from `shape`.
    ///
    /// `shape` is the parent verbatim, `mod` declarations included. Which ones
    /// a domain writes, and whether it re-exports them, is the domain's
    /// business. Cards publish their pool enums and the other two do not.
    pub fn finish(
        self,
        shape: &str,
        doc: impl Fn(&str) -> String,
        imports: Imports<'_>,
    ) -> Result<(), String> {
        for (name, body) in &self.buckets {
            let mut out = banner(&doc(name), &self.source, &self.build);
            out.push('\n');
            out.push_str(&uses(body, imports));
            out.push('\n');
            out.push_str(body);

            let path = self.dir.join(format!("{name}.rs"));
            fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;
        }

        let path = self.dir.join("mod.rs");
        fs::write(&path, shape).map_err(|e| format!("writing {}: {e}", path.display()))
    }
}

/// The `use` lines a body needs, out of the candidates it was offered.
///
/// Whole identifiers. The body is split on everything that cannot be in one, so
/// a candidate matches only where it stands alone.
///
/// A candidate mentioned only in a comment counts, which is the one way this is
/// still wrong. No bucket names a type in prose today.
fn uses(body: &str, imports: Imports<'_>) -> String {
    let named: BTreeSet<&str> = body
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .collect();

    let mut out = String::new();
    for (path, candidates) in imports {
        let used: Vec<&str> = candidates
            .iter()
            .copied()
            .filter(|n| named.contains(n))
            .collect();

        match used.as_slice() {
            [] => {}
            [only] => writeln!(out, "use {path}::{only};").unwrap(),
            many => writeln!(out, "use {path}::{{{}}};", many.join(", ")).unwrap(),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMPORTS: Imports<'static> = &[
        ("super", &["Repeat", "RepeatCount", "PowerType"]),
        ("crate", &["Field", "Scaled"]),
    ];

    /// The reason this tokenises rather than searching for substrings.
    ///
    /// Not currently visible in the generated tree, because every bucket that
    /// names `RepeatCount` names `Repeat` too. So it is pinned here, which is
    /// the only place the difference shows.
    #[test]
    fn a_name_inside_a_longer_one_is_not_a_use() {
        assert_eq!(
            uses("const A: RepeatCount = RepeatCount::One;", IMPORTS),
            "use super::RepeatCount;\n"
        );
        assert_eq!(
            uses("PowerInstanceType::Instanced", IMPORTS),
            "",
            "PowerInstanceType contains the text PowerType"
        );
    }

    #[test]
    fn a_group_the_body_never_names_is_not_written() {
        assert_eq!(
            uses("Repeat::Forever", IMPORTS),
            "use super::Repeat;\n",
            "the crate group has nothing in it and should not appear"
        );
        assert_eq!(uses("nothing at all", IMPORTS), "");
    }

    /// A single name is a plain path, because that is what rustfmt rewrites a
    /// one-element group into anyway.
    #[test]
    fn one_name_is_not_a_group() {
        assert_eq!(uses("Field::Computed", IMPORTS), "use crate::Field;\n");
        assert_eq!(
            uses("Field::Constant(Scaled { base: 1 })", IMPORTS),
            "use crate::{Field, Scaled};\n"
        );
    }

    /// Candidate order, not body order. The list is the one place the spelling
    /// is decided, so two bodies naming the same types import them the same
    /// way.
    #[test]
    fn the_candidate_order_is_the_written_order() {
        assert_eq!(
            uses("Scaled Field", IMPORTS),
            "use crate::{Field, Scaled};\n"
        );
    }
}
