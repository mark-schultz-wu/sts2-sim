//! The generated types every build spells alike, written once.
//!
//! Behaviour shared between builds names `CardType::Attack` directly, so
//! `CardType` has to be one type, not one per build. A type every build
//! generated alike moves, with its impls, into `sts2_content::generated`,
//! under the path it had in a build. The build re-exports it in its place.
//!
//! Content ids never move: a rule names those through `sts2_content::ids`.
//! Nor does a type naming anything a build keeps. A type a later build
//! changes moves back into each build on the next run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::DumpError;
use crate::error;

/// The content ids, which stay in each build.
pub(super) const CONTENT: &[&str] = &[
    "Act",
    "Affliction",
    "CardVar",
    "Cards",
    "Character",
    "ColorlessCard",
    "CurseCard",
    "DefectCard",
    "DeprecatedCard",
    "Enchantment",
    "Encounter",
    "Event",
    "EventCard",
    "EventOption",
    "IroncladCard",
    "Modifier",
    "Monster",
    "MoveId",
    "NecrobinderCard",
    "Orb",
    "Potion",
    "Power",
    "QuestCard",
    "RegentCard",
    "Relic",
    "RestSiteOption",
    "SilentCard",
    "StatusCard",
    "TokenCard",
];

/// One top-level item, attributes and doc comment included.
#[derive(Clone)]
struct Item {
    text: String,
    what: What,
}

#[derive(Clone, PartialEq, Eq)]
enum What {
    /// `enum`, `struct` or `type`, by name, with its visibility.
    Type { name: String, vis: String },
    /// An `impl` whose self type is this.
    Impl(String),
    /// A `use`.
    Use,
    /// Anything else, by the name it declares if it has one.
    Other(Option<String>),
}

/// A generated file, split into its items.
struct File {
    /// The `//!` banner.
    banner: String,
    items: Vec<Item>,
}

fn word_after<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.split_once(&format!("{keyword} "))?.1;
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

fn what(head: &str) -> What {
    let vis_end = if head.starts_with("pub(") {
        head.find(") ").map_or(0, |at| at + 2)
    } else if head.starts_with("pub ") {
        4
    } else {
        0
    };
    let vis = head[..vis_end].trim().to_owned();
    let rest = &head[vis_end..];
    for keyword in ["enum", "struct", "type"] {
        if rest.starts_with(&format!("{keyword} ")) {
            let name = word_after(rest, keyword).unwrap_or_default().to_owned();
            return What::Type { name, vis };
        }
    }
    if rest.starts_with("use ") {
        return What::Use;
    }
    if let Some(after) = rest.strip_prefix("impl") {
        // `impl<T> Trait for Type<T> {` or `impl Type {`.
        let after = after.trim_end_matches('{').trim();
        let ty = after.rsplit(" for ").next().unwrap_or(after);
        let ty = ty.split('<').next().unwrap_or(ty);
        let ty = ty.rsplit("::").next().unwrap_or(ty).trim();
        return What::Impl(ty.to_owned());
    }
    let name = ["fn", "const", "static", "mod", "trait"]
        .iter()
        .find_map(|keyword| word_after(rest, keyword))
        .map(str::to_owned);
    What::Other(name)
}

fn parse(text: &str) -> File {
    let mut banner = String::new();
    let mut items = Vec::new();
    let mut pending = String::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("//!") && items.is_empty() && pending.is_empty() {
            banner.push_str(line);
            banner.push('\n');
            continue;
        }
        if line.trim().is_empty() {
            pending.clear();
            continue;
        }
        if line.starts_with("#[") || line.starts_with("///") || line.starts_with("//") {
            pending.push_str(line);
            pending.push('\n');
            continue;
        }
        let mut item = std::mem::take(&mut pending);
        item.push_str(line);
        item.push('\n');
        let ends = |l: &str| {
            let l = l.trim_end();
            l.ends_with(';') || l.ends_with('}')
        };
        if !ends(line) {
            for next in lines.by_ref() {
                item.push_str(next);
                item.push('\n');
                let at_zero = !next.starts_with(char::is_whitespace);
                if at_zero && ends(next) {
                    break;
                }
            }
        }
        items.push(Item {
            text: item,
            what: what(line),
        });
    }
    File { banner, items }
}

/// The names `text` refers to: no comment, string, method, field, or path
/// segment past the first, and not an enum's members where it declares them.
fn references(text: &str) -> BTreeSet<&str> {
    let mut code = String::new();
    let mut in_enum = false;
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            in_enum = line.contains("enum ") && line.trim_end().ends_with('{');
        }
        // `    Member,` or `    Member(Type),`: the member is declared here.
        let line = match in_enum && line.starts_with("    ") && !line.starts_with("     ") {
            true => line.trim_start_matches(|c: char| c == ' ' || c.is_alphanumeric() || c == '_'),
            false => line,
        };
        let mut in_string = false;
        let mut escaped = false;
        let mut kept = String::new();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if in_string {
                match (escaped, c) {
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => escaped = false,
                }
                continue;
            }
            match c {
                '"' => {
                    in_string = true;
                    kept.push(' ');
                }
                '/' if chars.peek() == Some(&'/') => break,
                _ => kept.push(c),
            }
        }
        code.push_str(&kept);
        code.push('\n');
    }
    let mut out = BTreeSet::new();
    let mut start = None;
    for (at, c) in code.char_indices().chain([(code.len(), ' ')]) {
        let in_word = c.is_alphanumeric() || c == '_';
        match (start, in_word) {
            (None, true) => start = Some(at),
            (Some(from), false) => {
                start = None;
                let word = &code[from..at];
                let before = code[..from].trim_end();
                let tail = before.ends_with("::") || before.ends_with('.');
                // `const MEMBERS` in an impl declares its own name.
                let declares = ["const", "static", "fn", "type", "let", "mut"]
                    .iter()
                    .any(|k| {
                        before.ends_with(k)
                            && !before[..before.len() - k.len()]
                                .ends_with(|c: char| c.is_alphanumeric() || c == '_')
                    });
                let leads = word.starts_with(|c: char| c.is_alphabetic() || c == '_');
                if leads
                    && !tail
                    && !declares
                    && let Some(found) = text.find(word)
                {
                    out.insert(&text[found..found + word.len()]);
                }
            }
            _ => {}
        }
    }
    out
}

/// A type and its impls, in the order the file gives them.
struct Group {
    name: String,
    vis: String,
    text: String,
}

fn groups(file: &File) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for item in &file.items {
        if let What::Type { name, vis } = &item.what {
            out.push(Group {
                name: name.clone(),
                vis: vis.clone(),
                text: String::new(),
            });
        }
    }
    for group in &mut out {
        for item in &file.items {
            match &item.what {
                What::Type { name, .. } | What::Impl(name) if *name == group.name => {
                    group.text.push_str(&item.text);
                    group.text.push('\n');
                }
                _ => {}
            }
        }
    }
    out
}

/// Every `.rs` file under `root`, relative to it, sorted.
fn files(root: &Path) -> Result<Vec<PathBuf>, DumpError> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for path in error::entries(&dir)? {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
            }
        }
    }
    out.sort();
    Ok(out)
}

/// A file's module path below `generated`, as `cards::ironclad`.
fn module(file: &Path) -> String {
    let mut parts: Vec<String> = file
        .with_extension("")
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if parts.last().is_some_and(|p| p == "mod") {
        parts.pop();
    }
    parts.join("::")
}

/// A `use` line's names, as (the path before them, name).
fn use_names(text: &str) -> Vec<(String, String)> {
    let body = text
        .trim()
        .trim_start_matches("pub ")
        .trim_start_matches("use ")
        .trim_end_matches(';');
    match body.split_once("::{") {
        Some((path, names)) => names
            .trim_end_matches('}')
            .split(',')
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(|n| (path.to_owned(), n.to_owned()))
            .collect(),
        None => {
            let (path, name) = body.rsplit_once("::").unwrap_or(("", body));
            vec![(path.to_owned(), name.to_owned())]
        }
    }
}

/// The `use` lines of `items` still naming something `rest` uses.
///
/// A trait imported for its methods is never named, so with `moved`, the text
/// that left, a name neither text names stays too.
fn uses_for(items: &[Item], rest: &str, moved: Option<&str>) -> String {
    let used = references(rest);
    let gone = moved.map(references);
    let mut out = String::new();
    for item in items.iter().filter(|i| i.what == What::Use) {
        if item.text.trim_start().starts_with("pub ") {
            continue;
        }
        let names: Vec<(String, String)> = use_names(&item.text)
            .into_iter()
            .filter(|(_, n)| {
                let last = n.rsplit(" as ").next().unwrap_or(n);
                let unseen = gone.as_ref().is_some_and(|gone| !gone.contains(last));
                last == "_" || last == "self" || used.contains(last) || unseen
            })
            .collect();
        let Some((path, _)) = names.first() else {
            continue;
        };
        let names: Vec<&str> = names.iter().map(|(_, n)| n.as_str()).collect();
        match names.as_slice() {
            [one] if path.is_empty() => writeln!(out, "use {one};").unwrap(),
            [one] => writeln!(out, "use {path}::{one};").unwrap(),
            many => writeln!(out, "use {path}::{{{}}};", many.join(", ")).unwrap(),
        }
    }
    out
}

/// Move what every build under `src` generated alike into `src/generated`, and
/// return the files written.
pub(super) fn hoist(src: &Path, builds: &[String]) -> Result<Vec<PathBuf>, DumpError> {
    let roots: Vec<PathBuf> = builds
        .iter()
        .map(|b| src.join(b).join("generated"))
        .collect();
    let Some(first) = roots.first() else {
        return Ok(Vec::new());
    };
    let paths = files(first)?;

    // Every build's files, parsed.
    let mut parsed: Vec<BTreeMap<PathBuf, File>> = Vec::new();
    for root in &roots {
        let mut build = BTreeMap::new();
        for path in files(root)? {
            build.insert(path.clone(), parse(&error::read(&root.join(&path))?));
        }
        parsed.push(build);
    }

    // The types every build generated alike, by file.
    let mut alike: BTreeMap<PathBuf, Vec<Group>> = BTreeMap::new();
    for path in &paths {
        let Some(ours) = parsed[0].get(path) else {
            continue;
        };
        let theirs: Vec<Vec<Group>> = parsed[1..]
            .iter()
            .map(|b| b.get(path).map(groups).unwrap_or_default())
            .collect();
        let found = groups(ours)
            .into_iter()
            .filter(|g| !CONTENT.contains(&g.name.as_str()))
            .filter(|g| {
                theirs.iter().all(|t| {
                    t.iter()
                        .any(|o| o.name == g.name && o.text == g.text && o.vis == g.vis)
                })
            })
            .collect();
        alike.insert(path.clone(), found);
    }

    // Everything a build declares, which the shared module cannot name.
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for build in &parsed {
        for file in build.values() {
            for item in &file.items {
                match &item.what {
                    What::Type { name, .. } | What::Other(Some(name)) => {
                        declared.insert(name.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    // Drop a type naming anything left behind, until none does.
    loop {
        let moving: BTreeSet<String> = alike.values().flatten().map(|g| g.name.clone()).collect();
        let before = moving.len();
        for groups in alike.values_mut() {
            groups.retain(|g| {
                !g.text.contains("crate::v")
                    && references(&g.text)
                        .iter()
                        .all(|w| !declared.contains(*w) || moving.contains(*w))
            });
        }
        if alike.values().flatten().count() == before {
            break;
        }
    }
    alike.retain(|_, groups| !groups.is_empty());

    let mut written = Vec::new();

    // The shared copy, under the same paths.
    let shared = src.join("generated");
    error::remove_dir(&shared)?;
    let mut children: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in alike.keys() {
        let module = module(path);
        let mut parent = String::new();
        for part in module.split("::") {
            children
                .entry(parent.clone())
                .or_default()
                .insert(part.to_owned());
            parent = if parent.is_empty() {
                part.to_owned()
            } else {
                format!("{parent}::{part}")
            };
        }
    }
    let mut modules: BTreeSet<String> = children.keys().cloned().collect();
    modules.extend(alike.keys().map(|p| module(p)));
    for module in &modules {
        let file = if module.is_empty() {
            shared.join("mod.rs")
        } else if children.contains_key(module) {
            shared.join(module.replace("::", "/")).join("mod.rs")
        } else {
            shared.join(format!("{}.rs", module.replace("::", "/")))
        };
        let mut out = String::from(
            "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n\
             //! What every build generated alike, which each build re-exports.\n",
        );
        if module.is_empty() {
            writeln!(out, "//! Builds: {}.", builds.join(", ")).unwrap();
        }
        out.push('\n');
        for child in children.get(module).into_iter().flatten() {
            writeln!(out, "pub mod {child};").unwrap();
        }
        let source = alike
            .iter()
            .find(|(path, _)| self::module(path) == *module)
            .map(|(path, groups)| (&parsed[0][path], groups));
        if let Some((file, groups)) = source {
            let body: String = groups.iter().map(|g| g.text.as_str()).collect();
            out.push_str(&uses_for(&file.items, &body, None));
            out.push('\n');
            out.push_str(&body);
        }
        if let Some(parent) = file.parent() {
            error::create_dir(parent)?;
        }
        error::write(&file, &out)?;
        written.push(file);
    }

    // Each build, less what moved, re-exporting it instead.
    for (root, build) in roots.iter().zip(&parsed) {
        for (path, groups) in &alike {
            let Some(file) = build.get(path) else {
                continue;
            };
            let moved: BTreeSet<&str> = groups.iter().map(|g| g.name.as_str()).collect();
            let stays = |item: &Item| match &item.what {
                What::Type { name, .. } | What::Impl(name) => !moved.contains(name.as_str()),
                _ => true,
            };
            let moved_text: String = groups.iter().map(|g| g.text.as_str()).collect();
            let kept: Vec<Item> = file.items.iter().filter(|i| stays(i)).cloned().collect();
            let body: String = kept
                .iter()
                .filter(|i| i.what != What::Use)
                .map(|i| format!("{}\n", i.text))
                .collect();
            let mut out = file.banner.clone();
            out.push('\n');
            for item in kept.iter().filter(|i| i.what == What::Use) {
                if item.text.trim_start().starts_with("pub ") {
                    out.push_str(&item.text);
                }
            }
            out.push_str(&uses_for(&kept, &body, Some(&moved_text)));
            let mut by_vis: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
            for group in groups {
                by_vis.entry(&group.vis).or_default().push(&group.name);
            }
            for (vis, names) in by_vis {
                let vis = if vis.is_empty() { "pub(crate)" } else { vis };
                writeln!(
                    out,
                    "{vis} use crate::generated::{}::{{{}}};",
                    module(path),
                    names.join(", ")
                )
                .unwrap();
            }
            out.push('\n');
            out.push_str(&body);
            let file = root.join(path);
            error::write(&file, &out)?;
            written.push(file);
        }
    }
    Ok(written)
}
