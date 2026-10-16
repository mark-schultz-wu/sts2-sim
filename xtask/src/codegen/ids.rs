//! The content every build has, as consts each build's enum implements.
//!
//! Behaviour shared between builds is generic over the build, and a generic
//! rule cannot name `Power::StrengthPower`: each build generates its own
//! `Power`. It names `V::Power::STRENGTH_POWER` instead, a const of a trait
//! every build's `Power` implements. A trait every build implements can only
//! hold the ids every build has, so content one build lacks stays in that
//! build's own code.
//!
//! Read off the enums each build already generated, where every member
//! carries the game's id as its serde name. So this runs once every build's
//! tables are written, and reads whichever builds `sts2-content` holds.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::DumpError;
use crate::error;

/// One content enum, by the trait its consts go on.
struct Kind {
    /// The trait, in `sts2_content::ids`.
    trait_name: &'static str,
    /// The enum each build generates.
    enum_name: &'static str,
    /// Where it is generated, under a build's `generated/`.
    file: &'static str,
    /// The path a build re-exports it under, after the build's module.
    path: &'static str,
}

const KINDS: &[Kind] = &[
    Kind {
        trait_name: "ActIds",
        enum_name: "Act",
        file: "acts.rs",
        path: "acts::Act",
    },
    Kind {
        trait_name: "AfflictionIds",
        enum_name: "Affliction",
        file: "afflictions.rs",
        path: "afflictions::Affliction",
    },
    Kind {
        trait_name: "CardVarIds",
        enum_name: "CardVar",
        file: "cards/mod.rs",
        path: "cards::CardVar",
    },
    Kind {
        trait_name: "CharacterIds",
        enum_name: "Character",
        file: "characters.rs",
        path: "characters::Character",
    },
    Kind {
        trait_name: "EnchantmentIds",
        enum_name: "Enchantment",
        file: "enchantments.rs",
        path: "enchantments::Enchantment",
    },
    Kind {
        trait_name: "EncounterIds",
        enum_name: "Encounter",
        file: "encounters/mod.rs",
        path: "encounters::Encounter",
    },
    Kind {
        trait_name: "EventIds",
        enum_name: "Event",
        file: "events/mod.rs",
        path: "events::Event",
    },
    Kind {
        trait_name: "EventOptionIds",
        enum_name: "EventOption",
        file: "events/mod.rs",
        path: "events::EventOption",
    },
    Kind {
        trait_name: "ModifierIds",
        enum_name: "Modifier",
        file: "modifiers.rs",
        path: "modifiers::Modifier",
    },
    Kind {
        trait_name: "MonsterIds",
        enum_name: "Monster",
        file: "monsters/mod.rs",
        path: "monsters::Monster",
    },
    Kind {
        trait_name: "MoveIds",
        enum_name: "MoveId",
        file: "monsters/mod.rs",
        path: "monsters::MoveId",
    },
    Kind {
        trait_name: "OrbIds",
        enum_name: "Orb",
        file: "orbs.rs",
        path: "orbs::Orb",
    },
    Kind {
        trait_name: "PotionIds",
        enum_name: "Potion",
        file: "potions/mod.rs",
        path: "potions::Potion",
    },
    Kind {
        trait_name: "PowerIds",
        enum_name: "Power",
        file: "powers/mod.rs",
        path: "powers::Power",
    },
    Kind {
        trait_name: "RelicIds",
        enum_name: "Relic",
        file: "relics/mod.rs",
        path: "relics::Relic",
    },
    Kind {
        trait_name: "RestSiteOptionIds",
        enum_name: "RestSiteOption",
        file: "restsite.rs",
        path: "restsite::RestSiteOption",
    },
];

/// The ids of one build's enum, each with the member that carries it.
type Members = BTreeMap<String, String>;

/// One enum's members in every build, by the build's module.
type PerBuild = Vec<(String, Members)>;

/// `pub enum <name> { ... }` in `source`, as id to member. A member with no
/// serde name is serialised as its own name.
fn members(source: &str, name: &str) -> Option<Members> {
    let head = format!("pub enum {name} {{");
    let start = source.find(&head)? + head.len();
    let end = start + source[start..].find("\n}")?;
    let mut out = Members::new();
    let mut renamed: Option<String> = None;
    for line in source[start..end].lines() {
        let line = line.trim();
        if let Some(id) = line
            .strip_prefix("#[serde(rename = \"")
            .and_then(|rest| rest.strip_suffix("\")]"))
        {
            renamed = Some(id.to_owned());
            continue;
        }
        if line.is_empty() || line.starts_with("//") || line.starts_with("#[") {
            continue;
        }
        let member: String = line
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if member.is_empty() {
            continue;
        }
        // A member holding another enum, as `Ironclad(IroncladCard)`.
        if line[member.len()..].starts_with('(') {
            renamed = None;
            continue;
        }
        let id = renamed.take().unwrap_or_else(|| member.clone());
        out.insert(id, member);
    }
    Some(out)
}

/// The cards of one build, as id to the `Cards` value naming each: every pool
/// enum `Cards` holds, member by member.
fn cards(generated: &Path) -> Result<Members, DumpError> {
    let read = |file: &str| error::read(&generated.join(file)).map_err(DumpError::from);
    let source = read("cards/mod.rs")?;
    let head = "pub enum Cards {";
    let start = source.find(head).map(|at| at + head.len()).unwrap_or(0);
    let end = start + source[start..].find("\n}").unwrap_or(0);
    let mut out = Members::new();
    for line in source[start..end].lines() {
        let line = line.trim().trim_end_matches(',');
        let Some((pool, inner)) = line.split_once('(') else {
            continue;
        };
        let inner = inner.trim_end_matches(')');
        let module = inner.trim_end_matches("Card").to_lowercase();
        let pool_source = read(&format!("cards/{module}.rs"))?;
        let Some(pool_members) = members(&pool_source, inner) else {
            continue;
        };
        for (id, member) in pool_members {
            out.insert(id, format!("Self::{pool}(cards::{inner}::{member})"));
        }
    }
    Ok(out)
}

/// An id as a const's name.
pub(super) fn const_name(id: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in id.chars() {
        if c.is_uppercase() && prev_lower {
            out.push('_');
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        out.push(c.to_ascii_uppercase());
    }
    let out = out.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    match out.chars().next() {
        Some(c) if c.is_ascii_digit() => format!("_{out}"),
        _ => out,
    }
}

/// Every build `sts2-content` holds, by its module, oldest first.
pub(super) fn builds(src: &Path) -> Result<Vec<String>, DumpError> {
    let mut found: Vec<String> = error::entries(src)?
        .into_iter()
        .filter(|path| path.join("generated").is_dir())
        .filter_map(|path| Some(path.file_name()?.to_str()?.to_owned()))
        .filter(|name| name.starts_with('v'))
        .collect();
    found.sort();
    Ok(found)
}

/// Write `ids.rs` from every build's generated tables.
pub fn generate(src: &Path) -> Result<PathBuf, DumpError> {
    let builds = builds(src)?;
    let mut out = String::from(
        "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n\
         //! The content every build has, as consts each build's enum implements.\n\
         //! A rule shared between builds names content through these.\n",
    );
    writeln!(out, "//! Builds: {}.\n", builds.join(", ")).unwrap();

    // Cards first, as `Cards` values, then each flat enum.
    let mut kinds: Vec<(&str, PerBuild, &str)> = Vec::new();
    let mut per_build = Vec::new();
    for build in &builds {
        per_build.push((build.clone(), cards(&src.join(build).join("generated"))?));
    }
    kinds.push(("CardIds", per_build, "cards::Cards"));
    for kind in KINDS {
        let mut per_build = Vec::new();
        for build in &builds {
            let file = src.join(build).join("generated").join(kind.file);
            let source = error::read(&file).map_err(DumpError::from)?;
            let found = members(&source, kind.enum_name).unwrap_or_default();
            let found = found
                .into_iter()
                .map(|(id, member)| (id, format!("Self::{member}")))
                .collect();
            per_build.push((build.clone(), found));
        }
        kinds.push((kind.trait_name, per_build, kind.path));
    }

    for (trait_name, per_build, path) in &kinds {
        // The ids every build has, in id order.
        let shared: BTreeSet<&String> = per_build
            .iter()
            .map(|(_, members)| members.keys().collect::<BTreeSet<_>>())
            .reduce(|a, b| a.intersection(&b).copied().collect())
            .unwrap_or_default();
        writeln!(
            out,
            "/// The `{}` of every build, by id.\npub trait {trait_name}: Copy + Eq + 'static {{",
            path.rsplit("::").next().unwrap_or(path)
        )
        .unwrap();
        for id in &shared {
            writeln!(out, "    const {}: Self;", const_name(id)).unwrap();
        }
        writeln!(out, "}}\n").unwrap();
        for (build, members) in per_build {
            writeln!(out, "impl {trait_name} for crate::{build}::{path} {{").unwrap();
            for id in &shared {
                let value = members[*id].replace("cards::", &format!("crate::{build}::cards::"));
                writeln!(out, "    const {}: Self = {value};", const_name(id)).unwrap();
            }
            writeln!(out, "}}\n").unwrap();
        }
    }
    let path = src.join("ids.rs");
    error::write(&path, &out).map_err(DumpError::from)?;
    Ok(path)
}
