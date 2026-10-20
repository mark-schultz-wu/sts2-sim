//! Rewrite a build's behaviour to be generic over the build.
//!
//! A shared rule names content through `sts2_content::ids`:
//! `V::Orb::LIGHTNING_ORB`, not `Orb::LightningOrb`. A pattern cannot name a
//! generic const. So each content path in a pattern becomes a binding, and
//! the comparison moves into the arm's guard. A match left with no catch-all
//! gains an `unreachable!()` arm.
//!
//! Outside a pattern, a content path becomes the const and a content type
//! `V::Orb`. A relative path into the build becomes absolute. A function
//! naming `V` takes it as its first generic. What the rewrite cannot do it
//! lists, by line.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::error::{self, FileError};

#[derive(clap::Args)]
pub struct Args {
    /// The build the files belong to, as its module.
    #[arg(long, default_value = "v0_107_1")]
    build: String,
    /// The files, each rewritten in place.
    files: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error("{0} is not under crates/sts2-content/src")]
    Outside(PathBuf),
}

/// What the rewrite knows about the build's content.
#[derive(Clone)]
struct Content {
    /// `Orb::LightningOrb`, and `Cards::Ironclad(IroncladCard::Bash)` as well
    /// as `IroncladCard::Bash`, to the const naming it.
    paths: BTreeMap<String, String>,
    /// A content enum to the associated type naming it, `Cards` to `V::Card`.
    types: BTreeMap<String, String>,
    /// A content enum only one card pool is, which has no shared type.
    pools: Vec<String>,
    /// The associated type to the trait in `ids` whose consts name it.
    traits: BTreeMap<String, String>,
    /// The types every build shares, by their module under `generated`.
    shared: BTreeMap<String, Vec<String>>,
}

/// Read the consts off `ids.rs`, as the build implements them.
fn content(build: &str) -> Result<Content, Error> {
    let text = error::read(Path::new("crates/sts2-content/src/ids.rs"))?;
    // A const a core trait names too is ambiguous on `V::Power`, so those
    // name their trait.
    let core = error::read(Path::new("crates/sts2-core/src/content.rs"))?;
    let core_consts: std::collections::BTreeSet<String> = core
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("const ")?;
            let (name, ty) = rest.split_once(':')?;
            (ty.trim() == "Self;").then(|| name.trim().to_owned())
        })
        .collect();
    let mut paths = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut pools = Vec::new();
    let mut traits = BTreeMap::new();
    let prefix = format!("crate::{build}::");
    let mut current: Option<(String, String)> = None;
    // A const rustfmt wrapped over lines, as one.
    let mut lines: Vec<String> = Vec::new();
    let mut open = false;
    for line in text.lines() {
        match open {
            true => {
                let last = lines.last_mut().expect("an open const has a line");
                last.push(' ');
                last.push_str(line.trim());
            }
            false => lines.push(line.to_owned()),
        }
        let trimmed = line.trim();
        open = (open || trimmed.starts_with("const ")) && !trimmed.ends_with(';');
    }
    for line in &lines {
        let line = line.replace(",)", ")");
        let line = line.as_str();
        if let Some(rest) = line.strip_prefix("impl ") {
            current = rest.split_once(" for ").and_then(|(trait_name, ty)| {
                let ty = ty.trim_end_matches(" {").strip_prefix(&prefix)?;
                let name = ty.rsplit("::").next()?.to_owned();
                let assoc = if name == "Cards" {
                    "Card".to_owned()
                } else {
                    name.clone()
                };
                traits.insert(assoc.clone(), trait_name.to_owned());
                Some((name, assoc))
            });
            if let Some((name, assoc)) = &current {
                types.insert(name.clone(), format!("V::{assoc}"));
            }
            continue;
        }
        let Some((name, assoc)) = &current else {
            continue;
        };
        let Some((konst, value)) = line
            .trim()
            .strip_prefix("const ")
            .and_then(|l| l.strip_suffix(';'))
            .and_then(|l| l.split_once(": Self = "))
        else {
            continue;
        };
        let target = match core_consts.contains(konst) {
            true => {
                let trait_name = traits.get(assoc).cloned().unwrap_or_default();
                format!("<V::{assoc} as crate::ids::{trait_name}>::{konst}")
            }
            false => format!("V::{assoc}::{konst}"),
        };
        let value: String = value.chars().filter(|c| !c.is_whitespace()).collect();
        let value = value.replace(",)", ")");
        let value = value.replace(&format!("{prefix}cards::"), "");
        let value = value.strip_prefix("Self::").unwrap_or(&value).to_owned();
        match value.split_once('(') {
            // `Ironclad(IroncladCard::Bash)`.
            Some((pool, inner)) => {
                let inner = inner.trim_end_matches(')');
                paths.insert(format!("{name}::{pool}({inner})"), target.clone());
                paths.insert(inner.to_owned(), target);
                if let Some((pool_enum, _)) = inner.split_once("::")
                    && !pools.iter().any(|p| p == pool_enum)
                {
                    pools.push(pool_enum.to_owned());
                    // A pool's enum as a type is the build's card.
                    types.insert(pool_enum.to_owned(), "V::Card".to_owned());
                }
            }
            None => {
                paths.insert(format!("{name}::{value}"), target);
            }
        }
    }
    Ok(Content {
        paths,
        types,
        pools,
        traits,
        shared: shared()?,
    })
}

/// The types in `sts2_content::generated`, by module.
fn shared() -> Result<BTreeMap<String, Vec<String>>, Error> {
    let root = Path::new("crates/sts2-content/src/generated");
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for path in error::entries(&dir)? {
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(root).unwrap_or(&path).with_extension("");
            let mut parts: Vec<String> = rel
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            if parts.last().is_some_and(|p| p == "mod") {
                parts.pop();
            }
            let names = out.entry(parts.join("::")).or_default();
            for line in error::read(&path)?.lines() {
                for kind in ["pub enum ", "pub struct ", "pub type "] {
                    if let Some(rest) = line.strip_prefix(kind) {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        names.push(name);
                    }
                }
            }
        }
    }
    Ok(out)
}

// --- Scanning past strings, chars and comments. ---

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Past a string starting at `i`.
fn skip_str(s: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < s.len() && s[i] != b'"' {
        i += if s[i] == b'\\' { 2 } else { 1 };
    }
    i + 1
}

/// Past a char literal at `i`, or one past the `'` of a lifetime.
fn skip_char(s: &[u8], i: usize) -> usize {
    match (s.get(i + 1), s.get(i + 2), s.get(i + 3)) {
        (Some(b'\\'), _, _) => {
            let mut j = i + 2;
            while j < s.len() && s[j] != b'\'' {
                j += 1;
            }
            j + 1
        }
        (Some(_), Some(b'\''), _) => i + 3,
        _ => i + 1,
    }
}

/// Past whatever starts at `i` that is not code: a comment, string or char.
fn skip_noise(s: &[u8], i: usize) -> Option<usize> {
    match s[i] {
        b'/' if s.get(i + 1) == Some(&b'/') => Some(
            s[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(s.len(), |at| i + at),
        ),
        b'"' => Some(skip_str(s, i)),
        b'\'' => Some(skip_char(s, i)),
        _ => None,
    }
}

/// From `i`, the first place at depth 0 where `stop` holds, or where a
/// bracket closes past depth 0.
fn scan_to(s: &[u8], mut i: usize, stop: impl Fn(&[u8], usize) -> bool) -> usize {
    let mut depth = 0i32;
    while i < s.len() {
        if let Some(next) = skip_noise(s, i) {
            i = next;
            continue;
        }
        if depth == 0 && stop(s, i) {
            return i;
        }
        match s[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth < 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    s.len()
}

fn at(word: &'static str) -> impl Fn(&[u8], usize) -> bool {
    move |s, i| s[i..].starts_with(word.as_bytes())
}

/// `word` at `i` as a keyword, not part of a longer name.
fn at_kw(s: &[u8], i: usize, word: &str) -> bool {
    s[i..].starts_with(word.as_bytes())
        && (i == 0 || !is_ident(s[i - 1]))
        && s.get(i + word.len()).is_none_or(|&c| !is_ident(c))
}

/// `text` split at depth 0 on `sep`, `|` not matching `||`.
fn split_top(text: &str, sep: u8) -> Vec<String> {
    let s = text.as_bytes();
    let mut parts = Vec::new();
    let mut from = 0;
    let mut i = 0;
    let mut depth = 0i32;
    while i < s.len() {
        if let Some(next) = skip_noise(s, i) {
            i = next;
            continue;
        }
        match s[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            c if c == sep && depth == 0 => {
                let double =
                    sep == b'|' && (s.get(i + 1) == Some(&b'|') || (i > 0 && s[i - 1] == b'|'));
                if !double {
                    parts.push(text[from..i].to_owned());
                    from = i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(text[from..].to_owned());
    parts
}

/// `text` without its comments, for deciding what a pattern is.
fn code_of(text: &str) -> String {
    text.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The content paths in `text`, outside strings and comments, as
/// `(start, end, const)`. The longest path at a place wins, so a pool card
/// inside `Cards::Ironclad(..)` is one path.
fn content_paths(text: &str, content: &Content) -> Vec<(usize, usize, String)> {
    let s = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if let Some(next) = skip_noise(s, i) {
            i = next;
            continue;
        }
        let starts = s[i].is_ascii_uppercase()
            && (i == 0 || !(is_ident(s[i - 1]) || s[i - 1] == b':' || s[i - 1] == b'.'));
        // `Cards::Curse(\n    CurseCard::Clumsy,\n)`, as rustfmt breaks it.
        if starts && s[i..].starts_with(b"Cards::") {
            let open = (i..s.len()).find(|&k| !is_ident(s[k]) && s[k] != b':');
            if let Some(open) = open.filter(|&k| s[k] == b'(') {
                let close = scan_to(s, open + 1, |_, _| false);
                let inner: String = text[open + 1..close]
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                let key = format!("{}({})", &text[i..open], inner.trim_end_matches(','));
                if let Some(konst) = content.paths.get(&key) {
                    out.push((i, close + 1, konst.clone()));
                    i = close + 1;
                    continue;
                }
            }
        }
        if starts {
            let found = content
                .paths
                .iter()
                .filter(|(path, _)| {
                    s[i..].starts_with(path.as_bytes())
                        && s.get(i + path.len()).is_none_or(|&c| !is_ident(c))
                })
                .max_by_key(|(path, _)| path.len());
            if let Some((path, konst)) = found {
                out.push((i, i + path.len(), konst.clone()));
                i += path.len();
                continue;
            }
        }
        i += 1;
    }
    out
}

/// What a rewrite left for a hand, by line.
struct Left {
    notes: Vec<(usize, String)>,
}

impl Left {
    fn note(&mut self, text: &str, at: usize, what: &str) {
        let line = text[..at.min(text.len())].matches('\n').count() + 1;
        self.notes.push((line, what.to_owned()));
    }
}

/// One alternative's pattern with each content path bound, and the
/// comparisons those bindings stand for.
///
/// Paths joined by `|` inside a pattern, `(A | B, _)`, are one binding.
fn bind_paths(alt: &str, content: &Content) -> (String, Vec<String>) {
    let paths = content_paths(alt, content);
    // Runs of paths with nothing but `|` between them.
    let mut runs: Vec<Vec<(usize, usize, String)>> = Vec::new();
    for path in paths {
        match runs.last_mut() {
            Some(run) if code_of(&alt[run[run.len() - 1].1..path.0]).trim() == "|" => {
                run.push(path)
            }
            _ => runs.push(vec![path]),
        }
    }
    let mut skeleton = String::new();
    let mut conds = Vec::new();
    let mut from = 0;
    for (n, run) in runs.into_iter().enumerate() {
        skeleton.push_str(&alt[from..run[0].0]);
        let binding = format!("__c{n}");
        let ors: Vec<String> = run
            .iter()
            .map(|(_, _, konst)| format!("{binding} == {konst}"))
            .collect();
        conds.push(match ors.len() {
            1 => ors[0].clone(),
            _ => format!("({})", ors.join(" || ")),
        });
        skeleton.push_str(&binding);
        from = run[run.len() - 1].1;
    }
    skeleton.push_str(&alt[from..]);
    (skeleton, conds)
}

/// Whether an unguarded pattern matches anything.
fn catches_all(pattern: &str) -> bool {
    let code = code_of(pattern);
    let code = code.trim();
    !code.is_empty()
        && split_top(code, b'|').iter().any(|alt| {
            alt.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|w| !w.is_empty())
                .all(|w| {
                    w == "_"
                        || w == "mut"
                        || w == "ref"
                        || w.starts_with(|c: char| c.is_lowercase() || c == '_')
                })
                && !alt.contains("::")
        })
}

/// The arms of the match body opening at `brace`, rewritten. Returns the new
/// body, braces included, and where the old one ended.
fn rewrite_match(s: &str, brace: usize, content: &Content) -> (String, usize) {
    let b = s.as_bytes();
    let mut out = String::from("{");
    let mut i = brace + 1;
    let mut caught = false;
    let mut changed = false;
    let mut last_indent = String::new();
    loop {
        // Whitespace and comments before an arm.
        let mut j = i;
        loop {
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if b[j..].starts_with(b"//") {
                j = b[j..]
                    .iter()
                    .position(|&c| c == b'\n')
                    .map_or(b.len(), |at| j + at);
                continue;
            }
            break;
        }
        out.push_str(&s[i..j]);
        i = j;
        if b[i] == b'}' {
            if changed && !caught {
                let indent = &s[s[..i].rfind('\n').map_or(0, |at| at + 1)..i];
                out.truncate(out.trim_end().len());
                write!(
                    out,
                    "\n{last_indent}_ => unreachable!(\"every case has an arm\"),\n{indent}"
                )
                .unwrap();
            }
            out.push('}');
            return (out, i + 1);
        }
        // The arm's indent, or a space where it shares its line.
        let line = &s[s[..i].rfind('\n').map_or(0, |at| at + 1)..i];
        last_indent = match line.trim().is_empty() {
            true => line.to_owned(),
            false => " ".to_owned(),
        };
        let pattern_end = scan_to(b, i, |s, k| at("=>")(s, k) || at_kw(s, k, "if"));
        let pattern = &s[i..pattern_end];
        let (guard, arrow) = if at_kw(b, pattern_end, "if") {
            let guard_end = scan_to(b, pattern_end + 2, at("=>"));
            (
                Some(s[pattern_end + 2..guard_end].trim().to_owned()),
                guard_end,
            )
        } else {
            (None, pattern_end)
        };
        let body_start = arrow + 2;
        let mut k = body_start;
        while b[k].is_ascii_whitespace() {
            k += 1;
        }
        let body_end = if b[k] == b'{' {
            let mut end = scan_to(b, k + 1, |_, _| false) + 1;
            let mut m = end;
            while m < b.len() && b[m].is_ascii_whitespace() {
                m += 1;
            }
            if b.get(m) == Some(&b',') {
                end = m + 1;
            }
            end
        } else {
            // To its comma, or to the brace closing the match.
            let stop = scan_to(b, k, at(","));
            if b.get(stop) == Some(&b',') {
                stop + 1
            } else {
                stop
            }
        };
        let body = &s[body_start..body_end];
        if guard.is_none() && catches_all(pattern) {
            caught = true;
        }

        // Alternatives grouped by what is left once their content is bound.
        // Compared without spaces, since rustfmt spaces a pattern by its line.
        let mut groups: Vec<(String, String, Vec<Vec<String>>)> = Vec::new();
        for alt in split_top(pattern, b'|') {
            let (skeleton, conds) = bind_paths(alt.trim(), content);
            let shown = code_of(&skeleton)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let key: String = shown.chars().filter(|c| !c.is_whitespace()).collect();
            match groups.iter_mut().find(|(k, _, _)| *k == key) {
                Some((_, _, all)) => all.push(conds),
                None => groups.push((key, shown, vec![conds])),
            }
        }
        if groups
            .iter()
            .all(|(_, _, all)| all.iter().all(Vec::is_empty))
        {
            out.push_str(&s[i..body_end]);
        } else {
            changed = true;
            let arms: Vec<String> = groups
                .into_iter()
                .map(|(_, skeleton, all)| {
                    let mut parts = Vec::new();
                    if all.iter().any(|c| !c.is_empty()) {
                        let ors: Vec<String> = all
                            .iter()
                            .map(|c| match c.len() {
                                1 => c[0].clone(),
                                _ => format!("({})", c.join(" && ")),
                            })
                            .collect();
                        parts.push(match (ors.len(), guard.is_some()) {
                            (1, _) => ors[0].clone(),
                            (_, false) => ors.join(" || "),
                            (_, true) => format!("({})", ors.join(" || ")),
                        });
                    }
                    if let Some(guard) = &guard {
                        parts.push(match parts.is_empty() {
                            true => guard.clone(),
                            false => format!("({guard})"),
                        });
                    }
                    let guard = match parts.is_empty() {
                        true => String::new(),
                        false => format!(" if {}", parts.join(" && ")),
                    };
                    format!("{skeleton}{guard} =>{body}")
                })
                .collect();
            let joined: Vec<String> = arms
                .iter()
                .enumerate()
                .map(|(n, arm)| match n + 1 < arms.len() {
                    true => {
                        let arm = arm.trim_end();
                        match arm.ends_with(',') || arm.ends_with('}') {
                            true => arm.to_owned(),
                            false => format!("{arm},"),
                        }
                    }
                    false => arm.clone(),
                })
                .collect();
            let separator = match last_indent.as_str() {
                " " => " ".to_owned(),
                indent => format!("\n{indent}"),
            };
            out.push_str(&joined.join(&separator));
        }
        i = body_end;
    }
}

/// Every `match` whose arms name content, innermost first.
fn rewrite_matches(s: &str, content: &Content) -> String {
    let mut s = s.to_owned();
    let starts: Vec<usize> = {
        let b = s.as_bytes();
        let mut found = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if let Some(next) = skip_noise(b, i) {
                i = next;
                continue;
            }
            if at_kw(b, i, "match") {
                found.push(i);
            }
            i += 1;
        }
        found
    };
    for &start in starts.iter().rev() {
        let b = s.as_bytes();
        let brace = scan_to(b, start + 5, |s, k| s[k] == b'{');
        if brace >= b.len() || b[brace] != b'{' {
            continue;
        }
        let end = scan_to(b, brace + 1, |_, _| false) + 1;
        if content_paths(&s[brace..end], content).is_empty() {
            continue;
        }
        let (body, end) = rewrite_match(&s, brace, content);
        s.replace_range(brace..end, &body);
    }
    s
}

/// `matches!(e, P)` naming content, as a match.
fn rewrite_matches_macro(s: &str, content: &Content) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find("matches!(") {
        out.push_str(&rest[..at]);
        let open = at + "matches!(".len();
        let b = rest.as_bytes();
        let comma = scan_to(b, open, |s, k| s[k] == b',');
        let close = scan_to(b, comma + 1, |_, _| false);
        let expr = rest[open..comma].trim();
        let pattern = rest[comma + 1..close].trim().trim_end_matches(',').trim();
        if content_paths(pattern, content).is_empty() {
            out.push_str(&rest[at..=close]);
        } else {
            let simple = expr
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '*');
            let alts: Vec<(String, Vec<String>)> = split_top(pattern, b'|')
                .iter()
                .map(|alt| bind_paths(alt.trim(), content))
                .collect();
            if simple
                && alts
                    .iter()
                    .all(|(skeleton, conds)| skeleton == "__c0" && conds.len() == 1)
            {
                let ors: Vec<String> = alts
                    .iter()
                    .map(|(_, conds)| conds[0].replacen("__c0", expr, 1))
                    .collect();
                // Bare where it stands alone: a block's value, a binding, an
                // argument.
                let before = out.trim_end();
                let after = rest[close + 1..].trim_start();
                let alone = (before.ends_with(['{', '=', ',', '(']) || before.ends_with("return"))
                    && !before.ends_with("==")
                    && after.starts_with(['}', ';', ',', ')']);
                match alone {
                    true => out.push_str(&ors.join(" || ")),
                    false => write!(out, "({})", ors.join(" || ")).unwrap(),
                }
            } else {
                // One `matches!` per shape the alternatives take once bound,
                // each with its comparisons as a guard.
                let mut shapes: Vec<(String, String, Vec<String>)> = Vec::new();
                for (skeleton, conds) in &alts {
                    let shown = code_of(skeleton)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    let key: String = shown.chars().filter(|c| !c.is_whitespace()).collect();
                    let cond = match conds.len() {
                        0 => "true".to_owned(),
                        1 => conds[0].clone(),
                        _ => format!("({})", conds.join(" && ")),
                    };
                    match shapes.iter_mut().find(|(k, _, _)| *k == key) {
                        Some((_, _, ors)) => ors.push(cond),
                        None => shapes.push((key, shown, vec![cond])),
                    }
                }
                let tests: Vec<String> = shapes
                    .iter()
                    .map(|(_, shown, ors)| {
                        format!("matches!({expr}, {shown} if {})", ors.join(" || "))
                    })
                    .collect();
                match tests.as_slice() {
                    [one] => out.push_str(one),
                    many => write!(out, "({})", many.join(" || ")).unwrap(),
                }
            }
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// `if let` and `let .. else` naming content: a lone path becomes `==`.
fn rewrite_lets(s: &str, content: &Content, left: &mut Left) -> String {
    let mut s = s.to_owned();
    let lets: Vec<usize> = {
        let b = s.as_bytes();
        let mut found = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if let Some(next) = skip_noise(b, i) {
                i = next;
                continue;
            }
            if at_kw(b, i, "let") {
                found.push(i);
            }
            i += 1;
        }
        found
    };
    for &start in lets.iter().rev() {
        let b = s.as_bytes();
        let eq = scan_to(b, start + 3, |s, k| {
            s[k] == b'='
                && s.get(k + 1) != Some(&b'=')
                && k > 0
                && s[k - 1] != b'='
                && s[k - 1] != b'!'
                && s[k - 1] != b'<'
                && s[k - 1] != b'>'
        });
        let pattern = &s[start + 3..eq];
        if content_paths(pattern, content).is_empty() {
            continue;
        }
        let before = s[..start].trim_end();
        let alts: Vec<(String, Vec<String>)> = split_top(pattern, b'|')
            .iter()
            .map(|alt| bind_paths(alt.trim(), content))
            .collect();
        let lone = alts
            .iter()
            .all(|(skeleton, conds)| skeleton == "__c0" && conds.len() == 1);
        if before.ends_with("if") && lone {
            // `if let Orb::X = e {` to `if e == X {`.
            let open = scan_to(b, eq + 1, |s, k| {
                s[k] == b'{' || at_kw(s, k, "else") || at("&&")(s, k)
            });
            let expr = s[eq + 1..open].trim().to_owned();
            let ors: Vec<String> = alts
                .iter()
                .map(|(_, conds)| conds[0].replacen("__c0", &expr, 1))
                .collect();
            let cond = match ors.len() {
                1 => format!("{} ", ors[0]),
                _ => format!("({}) ", ors.join(" || ")),
            };
            s.replace_range(start..open, &cond);
        } else if before.ends_with("if") && alts.len() == 1 {
            // `if let Some(Orb::X) = e {` to a chain that binds, then compares.
            let (skeleton, conds) = &alts[0];
            let cond = format!("let {skeleton} =");
            let open = scan_to(b, eq + 1, |s, k| {
                s[k] == b'{' || at_kw(s, k, "else") || at("&&")(s, k)
            });
            let expr = s[eq + 1..open].trim_end().to_owned();
            let chained = format!("{cond}{expr} && {} ", conds.join(" && "));
            s.replace_range(start..open, &chained);
        } else {
            left.note(&s, start, "a `let` pattern naming content");
        }
    }
    s
}

/// What is left: a content path is its const, a content type its associated
/// type.
fn rewrite_rest(s: &str, content: &Content, left: &mut Left) -> String {
    // `Power::ALL` is the table's, through the trait the file imports.
    let mut s = s.to_owned();
    // A pool's card is the build's card, so wrapping one in `Cards` is gone.
    for pool in [
        "Ironclad",
        "Silent",
        "Regent",
        "Necrobinder",
        "Defect",
        "Colorless",
        "Curse",
        "Status",
        "Token",
        "Event",
        "Quest",
        "Deprecated",
    ] {
        let wrap = format!("Cards::{pool}(");
        let mut out = String::new();
        let mut rest = s.as_str();
        while let Some(at) = rest.find(&wrap) {
            let inner_start = at + wrap.len();
            let inner_end = rest[inner_start..].find(')').map(|n| inner_start + n);
            let simple = inner_end.is_some_and(|end| {
                let inner = &rest[inner_start..end];
                !inner.is_empty()
                    && inner
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            });
            out.push_str(&rest[..at]);
            match (simple, inner_end) {
                (true, Some(end)) => {
                    out.push_str(&rest[inner_start..end]);
                    rest = &rest[end + 1..];
                }
                _ => {
                    out.push_str(&wrap);
                    rest = &rest[inner_start..];
                }
            }
        }
        out.push_str(rest);
        s = out;
    }
    for (name, assoc) in &content.types {
        s = replace_word(&s, &format!("{name}::ALL"), &format!("{assoc}::ALL"));
    }
    let s = s.as_str();
    // Paths first, over the whole text: rustfmt breaks a long one over lines.
    let mut whole = s.to_owned();
    for (start, end, konst) in content_paths(s, content).into_iter().rev() {
        let line_start = s[..start].rfind('\n').map_or(0, |at| at + 1);
        if !s[line_start..].trim_start().starts_with("use ") {
            whole.replace_range(start..end, &konst);
        }
    }
    let s = whole.as_str();
    let mut out = String::new();
    let mut in_use = false;
    // Inside an `enum` body, where a line's first name declares a member.
    let mut in_enum = false;
    for (number, line) in s.split_inclusive('\n').enumerate() {
        if !line.starts_with(char::is_whitespace) {
            let item = line
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub(super) ")
                .trim_start_matches("pub ");
            in_enum = item.starts_with("enum ") && line.trim_end().ends_with('{');
        }
        // A `use` over several lines is `rewrite_uses`' to change.
        if in_use || line.trim_start().starts_with("use ") {
            in_use = !line.trim_end().ends_with(';');
            out.push_str(line);
            continue;
        }
        let item = line
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub(super) ")
            .trim_start_matches("pub ");
        if item.starts_with("type ") && line.contains("<V as") {
            left.notes
                .push((number + 1, "a type alias naming `V`".to_owned()));
        }
        if line.trim_start().starts_with("use ") || line.trim_start().starts_with("//") {
            out.push_str(line);
            continue;
        }
        let mut text = line.to_owned();
        for (start, end, konst) in content_paths(line, content).into_iter().rev() {
            text.replace_range(start..end, &konst);
        }
        // Content types.
        let b = text.as_bytes();
        let mut fixed = String::new();
        let mut i = 0;
        while i < b.len() {
            if let Some(next) = skip_noise(b, i) {
                fixed.push_str(&text[i..next]);
                i = next;
                continue;
            }
            if b[i].is_ascii_uppercase()
                && (i == 0 || !(is_ident(b[i - 1]) || b[i - 1] == b':' || b[i - 1] == b'.'))
            {
                let end = (i..b.len()).find(|&k| !is_ident(b[k])).unwrap_or(b.len());
                let word = &text[i..end];
                let path_follows = text[end..].starts_with("::");
                // `    Event(event::Resume),` declares a member named so.
                let declares = in_enum
                    && text[..i].trim().is_empty()
                    && (text[end..].starts_with('(')
                        || text[end..].starts_with(',')
                        || text[end..].starts_with(" {")
                        || text[end..].starts_with(" ="));
                if let Some(assoc) = content
                    .types
                    .get(word)
                    .filter(|_| !path_follows && !declares)
                {
                    fixed.push_str(assoc);
                } else {
                    if content.pools.iter().any(|p| p == word)
                        || (path_follows && content.types.contains_key(word))
                    {
                        left.notes
                            .push((number + 1, format!("`{word}` has no shared name")));
                    }
                    fixed.push_str(word);
                }
                i = end;
                continue;
            }
            fixed.push(b[i] as char);
            i += 1;
        }
        out.push_str(&fixed);
    }
    out
}

/// Each `use`: content types go, since they are `V::Orb` now, and what
/// every build shares is named where it is shared.
fn rewrite_uses(s: &str, content: &Content, build: &str, left: &mut Left) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut from = 0;
    let mut i = 0;
    let private = format!("crate::{build}::generated::");
    while i < b.len() {
        if let Some(next) = skip_noise(b, i) {
            i = next;
            continue;
        }
        // Only spaces between it and the line's start.
        let line_start = s[s[..i].rfind('\n').map_or(0, |at| at + 1)..i]
            .trim()
            .is_empty();
        if !(line_start && at_kw(b, i, "use") && !s[..i].ends_with("pub ")) {
            i += 1;
            continue;
        }
        let end = scan_to(b, i, |s, k| s[k] == b';');
        let body = s[i + 4..end].trim();
        let (prefix, imported): (String, Vec<String>) = match body.split_once("::{") {
            Some((prefix, names)) => (
                prefix.to_owned(),
                names
                    .trim_end_matches('}')
                    .split(',')
                    .map(|n| n.trim().to_owned())
                    .filter(|n| !n.is_empty())
                    .collect(),
            ),
            None => match body.rsplit_once("::") {
                Some((prefix, name)) => (prefix.to_owned(), vec![name.to_owned()]),
                None => (String::new(), vec![body.to_owned()]),
            },
        };
        // A card pool's enum nothing names now goes first, so what is left
        // can be found where every build shares it.
        let code_names = names(
            &s.split_inclusive('\n')
                .filter(|l| !l.trim_start().starts_with("use "))
                .collect::<String>(),
        );
        let imported: Vec<String> = imported
            .into_iter()
            .filter(|n| !(content.pools.contains(n) && !code_names.contains(n)))
            .collect();
        let kept: Vec<String> = imported
            .iter()
            .filter(|n| {
                // Under its own name or another, `EventOption as Said`.
                let alias = n.rsplit(" as ").next().unwrap_or(n);
                let real = n.split(" as ").next().unwrap_or(n);
                !(content.types.contains_key(real) || content.types.contains_key(alias))
                    || prefix.is_empty()
            })
            // `Build` is `Rules`, so its methods need no import.
            .filter(|n| !(prefix == "sts2_core" && *n == "Rules as _"))
            // The build is the generic now.
            .filter(|n| *n != "V")
            .cloned()
            .collect();
        let prefix = match prefix.strip_prefix(&private) {
            Some(module) => {
                let shared = content.shared.get(module);
                let all = kept
                    .iter()
                    .all(|n| shared.is_some_and(|names| names.iter().any(|s| s == n)));
                if all {
                    format!("crate::generated::{module}")
                } else {
                    left.note(s, i, "a `use` of what one build keeps");
                    prefix
                }
            }
            None => prefix,
        };
        out.push_str(&s[from..i]);
        if kept.is_empty() {
            // The line it stood on goes with it.
            while out.ends_with(' ') {
                out.pop();
            }
            from = end + 1;
            if b.get(from) == Some(&b'\n') {
                from += 1;
            }
        } else {
            match kept.as_slice() {
                [one] => write!(out, "use {prefix}::{one};").unwrap(),
                many => write!(out, "use {prefix}::{{{}}};", many.join(", ")).unwrap(),
            }
            from = end + 1;
        }
        i = end + 1;
    }
    out.push_str(&s[from..]);
    out
}

/// A `const` inside a function that names `V` cannot, since an item does not
/// see its function's generics. It becomes a `let`, named in lower case
/// through the rest of its block.
fn local_consts(s: &str) -> String {
    let mut s = s.to_owned();
    loop {
        let found = s.match_indices("\n").find_map(|(at, _)| {
            let line_start = at + 1;
            let rest = &s[line_start..];
            let indent = rest.len() - rest.trim_start_matches(' ').len();
            let rest = rest.trim_start_matches(' ');
            let name = rest.strip_prefix("const ")?;
            let name: String = name
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let end = scan_to(s.as_bytes(), line_start, |s, k| s[k] == b';');
            let item = &s[line_start..end];
            let names_v = item
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|w| w == "V");
            (indent > 0 && names_v).then_some((line_start + indent, name))
        });
        let Some((at, name)) = found else {
            return s;
        };
        // The rest of the block the const stands in.
        let block_end = scan_to(s.as_bytes(), at, |_, _| false);
        let lower = name.to_lowercase();
        let block = s[at..block_end].replacen("const ", "let ", 1);
        let mut renamed = String::new();
        let mut last = 0;
        let b = block.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if let Some(next) = skip_noise(b, i) {
                i = next;
                continue;
            }
            if block[i..].starts_with(name.as_str())
                && (i == 0 || !is_ident(b[i - 1]))
                && b.get(i + name.len()).is_none_or(|&c| !is_ident(c))
                && !block[..i].ends_with("::")
            {
                renamed.push_str(&block[last..i]);
                renamed.push_str(&lower);
                i += name.len();
                last = i;
                continue;
            }
            i += 1;
        }
        renamed.push_str(&block[last..]);
        s.replace_range(at..block_end, &renamed);
    }
}

/// Relative paths up out of the file, made absolute.
fn absolute_paths(s: &str, module: &[String]) -> String {
    // Inline modules, `mod tests { .. }`, each one deeper than the file.
    let b = s.as_bytes();
    let mut inline: Vec<(usize, usize, String)> = Vec::new();
    let mut k = 0;
    while k < b.len() {
        if let Some(next) = skip_noise(b, k) {
            k = next;
            continue;
        }
        if at_kw(b, k, "mod") {
            let name_end = (k + 4..b.len())
                .find(|&n| !is_ident(b[n]))
                .unwrap_or(b.len());
            let rest = s[name_end..].trim_start();
            if rest.starts_with('{') {
                let open = name_end + (s[name_end..].len() - rest.len());
                let close = scan_to(b, open + 1, |_, _| false);
                inline.push((open, close, s[k + 4..name_end].to_owned()));
            }
        }
        k += 1;
    }
    let mut out = String::new();
    let mut offset = 0;
    let mut rest = s;
    while let Some(at) = rest.find("super::") {
        let before = &rest[..at];
        out.push_str(before);
        let absolute = offset + at;
        let mut ups = 0;
        let mut tail = &rest[at..];
        while let Some(next) = tail.strip_prefix("super::") {
            ups += 1;
            tail = next;
        }
        // `super` from the module this stands in, the file's or an inline one.
        let mut here: Vec<String> = module.to_vec();
        for (open, close, name) in &inline {
            if *open < absolute && absolute < *close {
                here.push(name.clone());
            }
        }
        let keep = here.len().saturating_sub(ups);
        let mut path = String::from("crate::");
        for part in &here[..keep] {
            write!(path, "{part}::").unwrap();
        }
        out.push_str(&path);
        offset = s.len() - tail.len();
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// Each function naming `V` takes it as its first generic.
///
/// A call in the file to one it made generic names `V` too, since `V` cannot
/// be read off an argument that only projects it.
fn generic_fns(s: &str) -> (String, Vec<(String, usize)>) {
    let b = s.as_bytes();
    let mut made: Vec<(String, usize)> = Vec::new();
    let mut out = String::new();
    let mut i = 0;
    let mut from = 0;
    let mut skip_until = 0;
    // A test module names its build, so nothing in it is made generic.
    let tests = s.find("\nmod tests {").unwrap_or(s.len());
    while i < b.len() {
        if let Some(next) = skip_noise(b, i) {
            i = next;
            continue;
        }
        // An impl block starts its line; `impl Trait` in a signature does not.
        let starts_line = s[s[..i].rfind('\n').map_or(0, |at| at + 1)..i]
            .trim()
            .is_empty();
        if i > tests {
            break;
        }
        if at_kw(b, i, "impl") && starts_line && i >= skip_until {
            let open = scan_to(b, i + 4, |s, k| s[k] == b'{');
            let header = &s[i..open.min(s.len())];
            let names_v = header
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|w| w == "V");
            let declares_v =
                header.starts_with("impl<V") || header.contains(", V:") || header.contains("<V:");
            if names_v && declares_v {
                skip_until = scan_to(b, open + 1, |_, _| false);
            }
            if names_v && !declares_v {
                out.push_str(&s[from..i + 4]);
                match b[i + 4] {
                    b'<' => {
                        out.push_str("<V: crate::Build, ");
                        from = i + 5;
                    }
                    _ => {
                        out.push_str("<V: crate::Build>");
                        from = i + 4;
                    }
                }
                // Its methods take `V` from the impl.
                skip_until = scan_to(b, open + 1, |_, _| false);
            }
            i += 4;
            continue;
        }
        if at_kw(b, i, "fn") && i < skip_until {
            i += 2;
            continue;
        }
        if at_kw(b, i, "fn") {
            let name_end = (i + 3..b.len())
                .find(|&k| !is_ident(b[k]))
                .unwrap_or(b.len());
            let body = scan_to(b, name_end, |s, k| s[k] == b'{' || s[k] == b';');
            let end = match b.get(body) {
                Some(b'{') => scan_to(b, body + 1, |_, _| false) + 1,
                _ => body,
            };
            let names_v = s[i..end.min(s.len())]
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|w| w == "V");
            // A test takes no generics: its module names the build instead.
            let test = s[..i].trim_end().ends_with("#[test]");
            if names_v && !test {
                let name = s[i + 3..name_end].trim().to_owned();
                let others = match b[name_end] {
                    b'<' => {
                        let close = scan_to(b, name_end + 1, |s, k| s[k] == b'>');
                        split_top(&s[name_end + 1..close], b',')
                            .iter()
                            .filter(|p| !p.trim().is_empty() && !p.trim().starts_with('\''))
                            .count()
                    }
                    _ => 0,
                };
                // A method is marked, so only a method call names it.
                let params = &s[name_end..body.min(s.len())];
                let params = params[params.find('(').map_or(0, |at| at + 1)..].trim_start();
                let method = ["self", "&self", "&mut self", "mut self"]
                    .iter()
                    .any(|own| params.starts_with(own));
                made.push((if method { format!(".{name}") } else { name }, others));
                out.push_str(&s[from..name_end]);
                match b[name_end] {
                    b'<' => out.push_str("<V: crate::Build, "),
                    _ => out.push_str("<V: crate::Build>"),
                }
                from = if b[name_end] == b'<' {
                    name_end + 1
                } else {
                    name_end
                };
            }
            i = name_end;
            continue;
        }
        i += 1;
    }
    out.push_str(&s[from..]);
    (out, made)
}

/// `f(` to `f::<V, _>(` for each function `made` generic, outside its own
/// definition and method calls.
///
/// `modules` holds what each moving module made generic, by the module's own
/// name, for a call spelled `throwing_axe::event_hook(`.
fn turbofish(s: &str, made: &[(String, usize)], modules: &[(String, String, usize)]) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut from = 0;
    let mut i = 0;
    while i < b.len() {
        if let Some(next) = skip_noise(b, i) {
            i = next;
            continue;
        }
        let starts = is_ident(b[i]) && (i == 0 || !is_ident(b[i - 1]));
        if !starts {
            i += 1;
            continue;
        }
        let end = (i..b.len()).find(|&k| !is_ident(b[k])).unwrap_or(b.len());
        let word = &s[i..end];
        let call = b.get(end) == Some(&b'(');
        let before = s[..i].trim_end();
        let defines = before.ends_with("fn");
        // A method the file made generic is called on its own type, so a
        // method call to one of its names is that one.
        let method = before.ends_with('.');
        let own = made
            .iter()
            .find(|(name, _)| match method {
                true => name.strip_prefix('.') == Some(word),
                false => name == word,
            })
            .map(|(_, n)| *n);
        let pathed = before.strip_suffix("::").and_then(|path| {
            let module = path
                .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()?;
            modules
                .iter()
                .find(|(m, name, _)| m == module && name == word)
                .map(|(_, _, n)| *n)
        });
        // A path names some other module's function, unless it is one that moved.
        let own = if before.ends_with("::") { None } else { own };
        if let Some(others) = pathed.or(own)
            && call
            && !defines
        {
            let others = &others;
            out.push_str(&s[from..end]);
            out.push_str("::<V");
            for _ in 0..*others {
                out.push_str(", _");
            }
            out.push('>');
            from = end;
        }
        i = end;
    }
    out.push_str(&s[from..]);
    out
}

/// The `ids` traits whose consts the file names, imported after its other
/// `use` lines.
fn import_ids(s: &str, content: &Content) -> String {
    let used: Vec<String> = content
        .traits
        .iter()
        .filter(|(assoc, _)| s.contains(&format!("V::{assoc}::")))
        .map(|(_, name)| format!("{name} as _"))
        .collect();
    if used.is_empty() {
        return s.to_owned();
    }
    let line = match used.as_slice() {
        [one] => format!("use crate::ids::{one};\n"),
        many => format!("use crate::ids::{{{}}};\n", many.join(", ")),
    };
    // After the last `use` before any item, or after the file's own docs.
    let mut at = 0;
    let mut docs_end = 0;
    let mut offset = 0;
    let mut in_use = false;
    for l in s.split_inclusive('\n') {
        offset += l.len();
        if l.starts_with("//!") && docs_end == offset - l.len() {
            docs_end = offset;
        }
        if l.starts_with("use ") || in_use {
            in_use = !l.trim_end().ends_with(';');
            at = offset;
        } else if !l.trim().is_empty() && !l.starts_with("//") {
            break;
        }
    }
    let mut out = s.to_owned();
    let line = match at {
        0 if docs_end > 0 => format!("\n{line}"),
        _ => line,
    };
    out.insert_str(at.max(docs_end), &line);
    out
}

/// The number types every build holds alike, which `BuildContent` pins.
const PINNED: &[(&str, &str)] = &[
    ("Number", "i32"),
    ("Health", "i32"),
    ("Count", "u32"),
    ("Index", "u16"),
    ("CardSerial", "u16"),
    (
        "AscensionLevel",
        "crate::generated::ascension::AscensionLevel",
    ),
    ("CardKeyword", "crate::generated::cards::CardKeyword"),
    ("CardKeywordSet", "crate::generated::cards::KeywordSet"),
    ("CardTag", "crate::generated::cards::CardTag"),
    ("TargetType", "crate::generated::cards::TargetType"),
    ("RoomType", "crate::generated::encounters::RoomType"),
    ("MapPointType", "crate::generated::runstate::MapPointType"),
    ("CombatSide", "crate::generated::runstate::CombatSide"),
    (
        "PlayerTurnPhase",
        "crate::generated::runstate::PlayerTurnPhase",
    ),
    (
        "UnplayableReason",
        "crate::generated::runstate::UnplayableReason",
    ),
    (
        "UnplayableReasons",
        "crate::generated::runstate::UnplayableReasons",
    ),
    ("RunRngType", "crate::generated::runstate::RunRngType"),
    ("PlayerRngType", "crate::generated::runstate::PlayerRngType"),
    ("IntentKind", "crate::generated::monsters::IntentKind"),
    ("DeckWrite", "crate::behaviour::DeckWrite"),
];

/// `<V as sts2_core::ContentVersion>::Count` is `u32` in every build, and
/// `CombatSide` the one shared type.
fn pinned(s: &str) -> String {
    let mut s = s.to_owned();
    for (name, ty) in PINNED {
        for spelled in [
            format!("<V as sts2_core::ContentVersion>::{name}"),
            format!("<V as ContentVersion>::{name}"),
            format!("V::{name}"),
        ] {
            let b = s.clone();
            let mut out = String::new();
            let mut rest = b.as_str();
            while let Some(at) = rest.find(&spelled) {
                let end = at + spelled.len();
                let whole = rest
                    .as_bytes()
                    .get(end)
                    .is_none_or(|&c| !is_ident(c) && c != b':');
                out.push_str(&rest[..at]);
                out.push_str(if whole { ty } else { &spelled });
                rest = &rest[end..];
            }
            out.push_str(rest);
            s = out;
        }
    }
    s
}

/// One list a file hands out whole, as it moves into the file's `lists!`.
struct List {
    /// Its name on `Lists`.
    name: String,
    /// Its type there, with `Self` for the build.
    declared: String,
    /// A function returning it, for a list built on first use.
    lazy: bool,
}

/// Each top-level `const` or `static` naming `V`, as a const on `Lists`.
///
/// An item does not see a generic, and a list of a generic type cannot live
/// as long as the program. The file's `lists!` macro writes each for a build,
/// and the file reads it as `V::NAME`. A `LazyLock` becomes a function there
/// instead, with the `static` inside it.
fn lists(s: &str, prefix: &str, module: &str, left: &mut Left) -> (String, Vec<List>) {
    let mut out = String::new();
    let mut found: Vec<(String, String, String, String, String)> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    let mut from = 0;
    while i < b.len() {
        let line_start = i == 0 || b[i - 1] == b'\n';
        if !line_start {
            i += 1;
            continue;
        }
        let rest = &s[i..];
        let item = rest
            .trim_start_matches("pub(crate) ")
            .trim_start_matches("pub(super) ")
            .trim_start_matches("pub ");
        let kind = ["const ", "static "]
            .into_iter()
            .find(|k| item.starts_with(k));
        let Some(kind) = kind else {
            i += 1;
            continue;
        };
        let head = i + (rest.len() - item.len()) + kind.len();
        let end = scan_to(b, head, |s, k| s[k] == b';');
        let text = &s[head..end];
        let names_v = text
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|w| w == "V");
        let Some((name, rest_of)) = text.split_once(':') else {
            i = end;
            continue;
        };
        let Some((ty, value)) = rest_of.split_once('=') else {
            i = end;
            continue;
        };
        if !names_v {
            i = end;
            continue;
        }
        // Its docs, which go with it.
        let mut doc_start = i;
        loop {
            let prev = s[..doc_start.saturating_sub(1)]
                .rfind('\n')
                .map_or(0, |at| at + 1);
            if doc_start == 0 || !s[prev..doc_start].trim_start().starts_with("///") {
                break;
            }
            doc_start = prev;
        }
        let name = name.trim().to_owned();
        let ty = ty.trim().to_owned();
        // `[T; N]` by value becomes a slice the program keeps.
        let (ty, value) = match ty
            .strip_prefix('[')
            .and_then(|t| t.strip_suffix(']'))
            .and_then(|t| t.rsplit_once(';'))
        {
            Some((element, _)) => (
                format!("&'static [{}]", element.trim()),
                format!("&{}", value.trim()),
            ),
            None => (ty.replacen("&[", "&'static [", 1), value.trim().to_owned()),
        };
        if !ty.starts_with('&') {
            left.note(s, i, "a `const` naming `V` that is not a list");
        }
        found.push((name, ty, value, s[doc_start..i].to_owned(), String::new()));
        out.push_str(&s[from..doc_start]);
        from = end + 1;
        if b.get(from) == Some(&b'\n') {
            from += 1;
        }
        i = from;
    }
    out.push_str(&s[from..]);
    if found.is_empty() {
        return (out, Vec::new());
    }
    // A macro's names resolve where it expands, so each name the file imports
    // is spelled out.
    let imported = imports(s);
    let qualify = |text: &str| {
        let mut text = text.to_owned();
        for (name, path) in &imported {
            text = replace_word(&text, name, path);
        }
        text
    };
    // The file's own functions, which a list's value can call.
    let local_fns: Vec<String> = s
        .lines()
        .filter_map(|l| {
            let l = l
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub(super) ")
                .trim_start_matches("pub ");
            let rest = l.strip_prefix("fn ")?;
            Some(
                rest.chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect(),
            )
        })
        .collect();
    // The file's own consts that stay, which a list's value can name.
    let moved: Vec<&str> = found.iter().map(|f| f.0.as_str()).collect();
    let local_consts: Vec<String> = s
        .lines()
        .filter_map(|l| {
            let l = l
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub(super) ")
                .trim_start_matches("pub ");
            let rest = l.strip_prefix("const ")?;
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!moved.contains(&name.as_str())).then_some(name)
        })
        .collect();
    let mut called: Vec<String> = Vec::new();
    let mut declared = Vec::new();
    let mut body = String::new();
    for (name, ty, value, docs, _) in &found {
        let ty = qualify(ty);
        let mut value = qualify(value);
        for local in &local_fns {
            let call = format!("{local}(");
            if value.contains(&call) {
                value = replace_word(&value, local, &format!("{module}::{local}::<V>"));
                called.push(local.clone());
            }
        }
        for local in &local_consts {
            if names(&value).contains(local) {
                value = replace_word(&value, local, &format!("{module}::{local}"));
                called.push(local.clone());
            }
        }
        let full = format!("{prefix}{name}");
        let lazy_inner = ty
            .strip_prefix("std::sync::LazyLock<")
            .or_else(|| ty.strip_prefix("LazyLock<"))
            .and_then(|t| t.strip_suffix('>'))
            .map(str::to_owned);
        let as_build = |text: &str| {
            text.replace("<V>", "<$build>")
                .replace("V::", "<$build as sts2_core::ContentVersion>::")
        };
        // `V::Card::BASH` names its trait for a concrete build.
        let value = as_build(&value);
        let docs: String = docs
            .lines()
            .map(|l| format!("        {}\n", l.trim()))
            .collect();
        if let Some(inner) = lazy_inner {
            let function = full.to_lowercase();
            let value = value
                .strip_prefix("LazyLock::new")
                .map_or(value.clone(), |rest| {
                    format!("std::sync::LazyLock::new{rest}")
                });
            let inner_build = as_build(&inner);
            writeln!(
                body,
                "{docs}        fn {function}() -> &'static {inner_build} {{\n            static LIST: std::sync::LazyLock<{inner_build}> = {value};\n            &LIST\n        }}"
            )
            .unwrap();
            declared.push(List {
                name: function.clone(),
                declared: inner.replace("<V>", "<Self>").replace("V::", "Self::"),
                lazy: true,
            });
            out = replace_word(&out, name, &format!("V::{function}()"));
            out = out.replace(&format!("&V::{function}()"), &format!("V::{function}()"));
            continue;
        }
        writeln!(
            body,
            "{docs}        const {full}: {} = {value};",
            as_build(&ty)
        )
        .unwrap();
        declared.push(List {
            name: full.clone(),
            declared: ty.replace("<V>", "<Self>").replace("V::", "Self::"),
            lazy: false,
        });
        // The file reads it off the build.
        out = replace_word(&out, name, &format!("V::{full}"));
        // `&LIST` is the slice itself; `&LIST[at]` borrows an element still.
        out = out
            .replace(&format!("&V::{full}["), &format!("&V::{full}\u{0}["))
            .replace(&format!("&V::{full}"), &format!("V::{full}"))
            .replace(&format!("V::{full}\u{0}["), &format!("&V::{full}["));
        // An array iterated by value is a slice now.
        out = out.replace(
            &format!("V::{full}.into_iter()"),
            &format!("V::{full}.iter().copied()"),
        );
    }
    // A function a list calls is the build's to reach.
    for local in &called {
        for (vis, item) in [
            ("\nfn ", "fn"),
            ("\npub(super) fn ", "fn"),
            ("\nconst ", "const"),
            ("\npub(super) const ", "const"),
        ] {
            // The name whole: what follows it opens its generics, its
            // parameters or its type.
            for after in ['<', '(', ':'] {
                out = out.replace(
                    &format!("{vis}{local}{after}"),
                    &format!("\npub(crate) {item} {local}{after}"),
                );
            }
        }
    }
    // A name only the lists used goes from the file's imports.
    let listed: String = found.iter().map(|f| format!("{} {}", f.1, f.2)).collect();
    out = prune_imports(&out, &names(&listed));
    let names: Vec<&str> = found.iter().map(|f| f.0.as_str()).collect();
    let mut macro_text = String::new();
    write!(
        macro_text,
        "\n/// {}, written for one build's own types.\nmacro_rules! lists {{\n    ($build:ty) => {{\n{body}    }};\n}}\n\npub(crate) use lists;\n",
        names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
    )
    .unwrap();
    // Above the tests, which clippy wants last.
    match out.find("\n#[cfg(test)]") {
        Some(at) => out.insert_str(at + 1, macro_text.trim_start()),
        None => out.push_str(&macro_text),
    }
    (out, declared)
}

/// The names `text` refers to, outside strings and comments, and not a path's
/// later segment, a field or a method.
fn names(text: &str) -> std::collections::BTreeSet<String> {
    let b = text.as_bytes();
    let mut out = std::collections::BTreeSet::new();
    let mut i = 0;
    while i < b.len() {
        if let Some(next) = skip_noise(b, i) {
            i = next;
            continue;
        }
        if is_ident(b[i]) && (i == 0 || !is_ident(b[i - 1])) {
            let end = (i..b.len()).find(|&k| !is_ident(b[k])).unwrap_or(b.len());
            let before = text[..i].trim_end();
            if !before.ends_with("::") && !before.ends_with('.') {
                out.insert(text[i..end].to_owned());
            }
            i = end;
            continue;
        }
        i += 1;
    }
    out
}

/// Each `use` of a name in `gone` that nothing outside the `use` lines still
/// names, dropped.
fn prune_imports(s: &str, gone: &std::collections::BTreeSet<String>) -> String {
    let code: String = s
        .split_inclusive('\n')
        .filter(|l| !l.trim_start().starts_with("use "))
        .collect();
    let used = names(&code);
    let b = s.as_bytes();
    let mut out = String::new();
    let mut from = 0;
    let mut at = 0;
    while let Some(i) = s[at..].find("use ").map(|i| at + i) {
        let line_start = s[..i].rfind('\n').map_or(0, |n| n + 1);
        let end = scan_to(b, i, |s, k| s[k] == b';');
        at = end.max(i + 4);
        if !s[line_start..i].trim().is_empty() {
            continue;
        }
        let body = s[i + 4..end].trim();
        let (prefix, names): (&str, Vec<&str>) = match body.split_once("::{") {
            Some((prefix, names)) => (
                prefix,
                names
                    .trim_end_matches('}')
                    .split(',')
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .collect(),
            ),
            None => match body.rsplit_once("::") {
                Some((prefix, name)) => (prefix, vec![name]),
                None => continue,
            },
        };
        let drop = |n: &&str| {
            let name = n.rsplit(" as ").next().unwrap_or(n);
            gone.contains(name) && !used.contains(name)
        };
        if !names.iter().any(drop) {
            continue;
        }
        let kept: Vec<&str> = names.iter().filter(|n| !drop(n)).copied().collect();
        out.push_str(&s[from..i]);
        match kept.as_slice() {
            [] => {
                while out.ends_with(' ') {
                    out.pop();
                }
                from = end + 1;
                if b.get(from) == Some(&b'\n') {
                    from += 1;
                }
            }
            [one] => {
                write!(out, "use {prefix}::{one};").unwrap();
                from = end + 1;
            }
            many => {
                write!(out, "use {prefix}::{{{}}};", many.join(", ")).unwrap();
                from = end + 1;
            }
        }
    }
    out.push_str(&s[from..]);
    out
}

/// Each name the file's `use` lines import, with its whole path.
fn imports(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut at = if s.starts_with("use ") { Some(0) } else { None };
    let mut i = 0;
    loop {
        let start = match at.take() {
            Some(start) => start,
            None => match s[i..].find("\nuse ") {
                Some(found) => i + found + 1,
                None => return out,
            },
        };
        let end = scan_to(b, start, |s, k| s[k] == b';');
        let body = s[start + 4..end].trim();
        let (prefix, names): (&str, Vec<&str>) = match body.split_once("::{") {
            Some((prefix, names)) => (
                prefix,
                names
                    .trim_end_matches('}')
                    .split(',')
                    .map(str::trim)
                    .collect(),
            ),
            None => match body.rsplit_once("::") {
                Some((prefix, name)) => (prefix, vec![name]),
                None => ("", vec![]),
            },
        };
        for name in names {
            if name.is_empty() || name.ends_with(" as _") || name == "self" || name == "V" {
                continue;
            }
            match name.split_once(" as ") {
                Some((real, alias)) => out.push((alias.to_owned(), format!("{prefix}::{real}"))),
                None => out.push((name.to_owned(), format!("{prefix}::{name}"))),
            }
        }
        i = end;
    }
}

/// `word` as a whole name outside strings and comments, replaced.
fn replace_word(s: &str, word: &str, with: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut from = 0;
    let mut i = 0;
    while i < b.len() {
        if let Some(next) = skip_noise(b, i) {
            i = next;
            continue;
        }
        if b[i..].starts_with(word.as_bytes())
            // A range's `..` is not a field's `.`.
            && (i == 0
                || !(is_ident(b[i - 1])
                    || b[i - 1] == b':'
                    || (b[i - 1] == b'.' && !(i >= 2 && b[i - 2] == b'.'))))
            && b.get(i + word.len()).is_none_or(|&c| !is_ident(c))
        {
            out.push_str(&s[from..i]);
            out.push_str(with);
            i += word.len();
            from = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&s[from..]);
    out
}

/// Each `lists!` the run wrote, declared on `Lists` and written for every
/// build, in `build.rs`.
fn declare_lists(lists: &[(String, Vec<List>)]) -> Result<(), Error> {
    if lists.is_empty() {
        return Ok(());
    }
    let path = Path::new("crates/sts2-content/src/build.rs");
    let mut text = error::read(path)?;
    let mut decls = String::new();
    let mut calls = String::new();
    for (module, declared) in lists {
        for list in declared {
            match list.lazy {
                true => writeln!(
                    decls,
                    "    fn {}() -> &'static {};",
                    list.name, list.declared
                )
                .unwrap(),
                false => writeln!(decls, "    const {}: {};", list.name, list.declared).unwrap(),
            }
        }
        writeln!(calls, "            {module}::lists!($build);").unwrap();
    }
    let trait_at = text
        .find("pub trait Lists")
        .ok_or_else(|| Error::Outside(path.to_path_buf()))?;
    let open = trait_at + text[trait_at..].find('{').unwrap_or(0);
    let close = scan_to(text.as_bytes(), open + 1, |_, _| false);
    text.insert_str(close, &decls);
    let impl_at = text
        .find("impl Lists for $build {")
        .ok_or_else(|| Error::Outside(path.to_path_buf()))?;
    let open = impl_at + text[impl_at..].find('{').unwrap_or(0);
    let close = scan_to(text.as_bytes(), open + 1, |_, _| false);
    let line_start = text[..close].rfind('\n').map_or(close, |at| at + 1);
    text.insert_str(line_start, &calls);
    error::write(path, &text)?;
    Ok(())
}

/// Whether the name at `at` stands where a type does, as opposed to a
/// struct literal or a path to a member.
fn type_position(s: &str, at: usize, end: usize) -> bool {
    // Past the path the name is spelled under, `crate::behaviour::`.
    let mut before = s[..at].trim_end();
    while let Some(rest) = before.strip_suffix("::") {
        let segment = rest.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
        if segment.len() == rest.len() {
            break;
        }
        before = segment.trim_end();
    }
    let after = s[end..].trim_start();
    if after.starts_with("::") || after.starts_with('<') || after.starts_with('(') {
        return false;
    }
    let keyword = |k: &str| {
        before.ends_with(k)
            && !before[..before.len() - k.len()]
                .ends_with(|c: char| c.is_alphanumeric() || c == '_')
    };
    if keyword("impl") || keyword("for") {
        return true;
    }
    // `type State = State;`
    let line = &s[s[..at].rfind('\n').map_or(0, |n| n + 1)..at];
    if before.ends_with('=') && line.trim_start().starts_with("type ") {
        return true;
    }
    if after.starts_with('{') {
        return before.ends_with("->");
    }
    before.ends_with(':') && !before.ends_with("::")
        || before.ends_with("->")
        || before.ends_with('&')
        || keyword("mut")
        || before.ends_with('<')
        || before.ends_with(',')
        || before.ends_with('(')
        || before.ends_with('[')
}

/// Each type naming `V` takes it, and so does each type holding one. A use in
/// another file names it by its module path.
///
/// `files` holds each file's module path and text.
fn generic_types(files: &mut [(Vec<String>, String)]) {
    let mut generic: Vec<(Vec<String>, String)> = Vec::new();
    loop {
        let mut added = false;
        for (module, text) in files.iter_mut() {
            // Uses of what is already generic.
            for (owner, name) in &generic {
                // Imported under its own name, it is used bare, as at home.
                let home = format!("{}::{name}", owner.join("::"));
                let imported = imports(text)
                    .iter()
                    .any(|(alias, path)| alias == name && path.ends_with(&home));
                let here = owner == module || imported;
                let qualified = match here {
                    true => name.clone(),
                    false => match owner.last() {
                        Some(last) => format!("{last}::{name}"),
                        None => continue,
                    },
                };
                let b = text.as_bytes().to_vec();
                let mut out = String::new();
                let mut from = 0;
                let mut i = 0;
                while i < b.len() {
                    if let Some(next) = skip_noise(&b, i) {
                        i = next;
                        continue;
                    }
                    let end = i + qualified.len();
                    let whole = b[i..].starts_with(qualified.as_bytes())
                        && (i == 0 || !(is_ident(b[i - 1]) || (here && b[i - 1] == b':')))
                        && b.get(end).is_none_or(|&c| !is_ident(c));
                    if whole
                        && type_position(text, i, end)
                        && !text[..i].trim_end().ends_with("struct")
                        && !text[..i].trim_end().ends_with("enum")
                    {
                        out.push_str(&text[from..end]);
                        out.push_str("<V>");
                        from = end;
                    }
                    i += if whole { qualified.len() } else { 1 };
                }
                out.push_str(&text[from..]);
                *text = out;
            }
            // Definitions naming `V`.
            let b = text.as_bytes().to_vec();
            let mut i = 0;
            let mut inserts = Vec::new();
            while i < b.len() {
                let line_start = i == 0 || b[i - 1] == b'\n';
                if !line_start {
                    i += 1;
                    continue;
                }
                let rest = &text[i..];
                let item = rest
                    .trim_start_matches("pub(crate) ")
                    .trim_start_matches("pub(super) ")
                    .trim_start_matches("pub ");
                let kind = ["struct ", "enum "]
                    .into_iter()
                    .find(|k| item.starts_with(k));
                let (true, Some(kind)) = (line_start, kind) else {
                    i += 1;
                    continue;
                };
                let name_at = i + (rest.len() - item.len()) + kind.len();
                let name_end = (name_at..b.len())
                    .find(|&k| !is_ident(b[k]))
                    .unwrap_or(b.len());
                let name = text[name_at..name_end].to_owned();
                let end = scan_to(&b, name_end, |s, k| s[k] == b';' || s[k] == b'{');
                let end = match b.get(end) {
                    Some(b'{') => scan_to(&b, end + 1, |_, _| false),
                    _ => end,
                };
                let names_v = text[name_end..end.min(text.len())]
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|w| w == "V");
                let known = generic.iter().any(|(m, n)| m == module && *n == name);
                if names_v && !known && b.get(name_end) != Some(&b'<') {
                    inserts.push(name_end);
                    generic.push((module.clone(), name));
                    added = true;
                }
                i = end;
            }
            for at in inserts.into_iter().rev() {
                // `Build`, so a derive's bounds on `V` hold for any build.
                text.insert_str(at, "<V: crate::Build>");
                derive_default_by_hand(text, at);
            }
        }
        if !added {
            return;
        }
    }
}

/// A derived `Default` on a type generic over the build asks the same of
/// each field's type, `V::Power` included, which no build gives. The type
/// whose name ends at `at` gets one written out instead.
fn derive_default_by_hand(text: &mut String, at: usize) {
    let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
    let Some(derive_at) = text[..line_start].rfind("#[derive(") else {
        return;
    };
    // Only the type's own attributes, right above it.
    if text[derive_at..line_start].matches('\n').count() > 3 {
        return;
    }
    let derive_end = derive_at + text[derive_at..].find(")]").unwrap_or(0);
    let derives = &text[derive_at + 9..derive_end];
    if !derives.split(',').any(|d| d.trim() == "Default") {
        return;
    }
    let kept: String = derives
        .split(',')
        .map(str::trim)
        .filter(|d| *d != "Default" && !d.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let name_start = text[line_start..at]
        .rfind(' ')
        .map_or(line_start, |n| line_start + n + 1);
    let name = text[name_start..at].to_owned();
    let b = text.as_bytes().to_vec();
    let open = scan_to(&b, at, |s, k| s[k] == b'{' || s[k] == b';' || s[k] == b'(');
    let body = match b.get(open) {
        Some(b'{') => {
            let close = scan_to(&b, open + 1, |_, _| false);
            let fields: Vec<String> = split_top(&text[open + 1..close], b',')
                .iter()
                .map(|f| code_of(f))
                .filter_map(|f| {
                    let f = f
                        .trim()
                        .trim_start_matches("pub(crate) ")
                        .trim_start_matches("pub ");
                    let (name, _) = f.split_once(':')?;
                    Some(format!("{}: Default::default(),", name.trim()))
                })
                .collect();
            Some((close, format!("Self {{ {} }}", fields.join(" "))))
        }
        Some(b';') => Some((open, "Self".to_owned())),
        _ => None,
    };
    let Some((end, value)) = body else {
        return;
    };
    let item_end = end + 1;
    let impl_text = format!(
        "\n\nimpl<V: crate::Build> Default for {name}<V> {{\n    fn default() -> Self {{\n        {value}\n    }}\n}}"
    );
    text.insert_str(item_end, &impl_text);
    text.replace_range(derive_at + 9..derive_end, &kept);
}

/// The module path of a file under `crates/sts2-content/src`.
fn module_of(path: &Path) -> Result<Vec<String>, Error> {
    let root = Path::new("crates/sts2-content/src");
    let rel = path
        .strip_prefix(root)
        .map_err(|_| Error::Outside(path.to_path_buf()))?;
    let mut parts: Vec<String> = rel
        .with_extension("")
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if parts.last().is_some_and(|p| p == "mod") {
        parts.pop();
    }
    // A path is relative to the module the file is, so `super` leaves it.
    Ok(parts)
}

/// A path into the build to what is shared, or is moving there, made the
/// shared path. A content path from the crate root,
/// `crate::v0_107_1::generated::orbs::Orb::DarkOrb`, starts at its type.
fn shared_paths(s: &str, build: &str, moving: &[String], content: &Content) -> String {
    let mut s = s.to_owned();
    let root = Path::new("crates/sts2-content/src/behaviour");
    let mut shared: Vec<String> = moving.to_vec();
    if let Ok(entries) = error::entries(root) {
        for entry in entries {
            if let Some(stem) = entry.file_stem().and_then(|s| s.to_str())
                && stem != "mod"
            {
                shared.push(stem.to_owned());
            }
        }
    }
    for module in shared {
        s = s.replace(
            &format!("crate::{build}::behaviour::{module}::"),
            &format!("crate::behaviour::{module}::"),
        );
    }
    let prefix = format!("crate::{build}::generated::");
    // A `use` is `rewrite_uses`' to change.
    let uses: Vec<(usize, usize)> = {
        let b = s.as_bytes();
        let mut found = Vec::new();
        let mut at = 0;
        while let Some(i) = s[at..].find("use ").map(|i| at + i) {
            let starts = s[s[..i].rfind('\n').map_or(0, |n| n + 1)..i]
                .trim()
                .is_empty();
            let end = scan_to(b, i, |s, k| s[k] == b';');
            if starts {
                found.push((i, end));
            }
            at = end.max(i + 4);
        }
        found
    };
    let in_use = |at: usize| uses.iter().any(|&(from, to)| from <= at && at < to);
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(at) = rest.find(&prefix) {
        out.push_str(&rest[..at]);
        let after = &rest[at + prefix.len()..];
        if in_use(s.len() - rest.len() + at) {
            out.push_str(&prefix);
            rest = after;
            continue;
        }
        // `orbs::Orb::DarkOrb`: modules, then a type.
        let mut skipped = prefix.len();
        let mut found = None;
        let mut modules = Vec::new();
        for part in after.split("::") {
            let word: String = part
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if word.starts_with(|c: char| c.is_uppercase()) {
                let content_type =
                    content.types.contains_key(&word) || content.pools.contains(&word);
                let module = modules.join("::");
                let vocabulary = content
                    .shared
                    .get(&module)
                    .is_some_and(|names| names.contains(&word));
                if content_type {
                    found = Some((skipped, String::new()));
                } else if vocabulary {
                    found = Some((skipped, format!("crate::generated::{module}::")));
                }
                break;
            }
            // A path's modules are bare lower-case names; anything else ends it.
            if word.is_empty() || word.len() != part.len() {
                break;
            }
            modules.push(word);
            skipped += part.len() + 2;
        }
        match found {
            Some((skip, path)) => {
                out.push_str(&path);
                rest = &rest[at + skip..];
            }
            None => {
                out.push_str(&prefix);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A `use` of a card pool's enum the rewrite left nothing naming.
fn unused_pools(s: &str, content: &Content) -> String {
    // What the code names, its `use` lines aside.
    let code = names(
        &s.split_inclusive('\n')
            .filter(|l| !l.trim_start().starts_with("use "))
            .collect::<String>(),
    );
    let mut out = String::new();
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim();
        let Some(body) = trimmed
            .strip_prefix("use ")
            .and_then(|l| l.strip_suffix(';'))
        else {
            out.push_str(line);
            continue;
        };
        let (prefix, imported): (&str, Vec<&str>) = match body.split_once("::{") {
            Some((prefix, names)) => (
                prefix,
                names
                    .trim_end_matches('}')
                    .split(',')
                    .map(str::trim)
                    .collect(),
            ),
            None => match body.rsplit_once("::") {
                Some((prefix, name)) => (prefix, vec![name]),
                None => (body, vec![]),
            },
        };
        let unused = |name: &&str| content.pools.iter().any(|p| p == name) && !code.contains(*name);
        if imported.is_empty() || !imported.iter().any(unused) {
            out.push_str(line);
            continue;
        }
        let kept: Vec<&str> = imported
            .iter()
            .filter(|n| !unused(n) && !n.is_empty())
            .copied()
            .collect();
        let indent = &line[..line.len() - line.trim_start().len()];
        match kept.as_slice() {
            [] => {}
            [one] => out.push_str(&format!("{indent}use {prefix}::{one};\n")),
            many => out.push_str(&format!("{indent}use {prefix}::{{{}}};\n", many.join(", "))),
        }
    }
    out
}

pub fn run(args: Args) -> Result<String, Error> {
    let content = content(&args.build)?;
    // The modules moving, by the name under `behaviour`.
    let moving: Vec<String> = args
        .files
        .iter()
        .filter_map(|path| module_of(path).ok())
        .filter_map(|module| module.get(2).cloned())
        .collect();
    let mut lefts = Vec::new();
    let mut files = Vec::new();
    let mut declared = Vec::new();
    for path in &args.files {
        let module = module_of(path)?;
        let text = error::read(path)?;
        let mut left = Left { notes: Vec::new() };
        let text = absolute_paths(&text, &module);
        let text = shared_paths(&text, &args.build, &moving, &content);
        let text = text.replace(&format!("use crate::{}::V0_107_1 as V;\n", args.build), "");
        // The build named outright is `V` too.
        let marker = args.build.to_uppercase();
        let text = text
            .replace(&format!("<{marker} as"), "<V as")
            .replace(&format!("type V = {marker};\n"), "")
            .replace(&format!("use crate::{}::{marker};\n", args.build), "");
        // `Stack<V0_107_1>` outside a `use` is `Stack<V>`.
        let text: String = text
            .split_inclusive('\n')
            .map(|line| match line.trim_start().starts_with("use ") {
                true => line.to_owned(),
                false => replace_word(line, &marker, "V"),
            })
            .collect();
        // `EventOption as Said`: the content type under another name, which
        // this file's paths and types know it by too.
        let mut local = content.clone();
        for (alias, path) in imports(&text) {
            let real = path.rsplit("::").next().unwrap_or("").to_owned();
            if alias == real {
                continue;
            }
            if let Some(assoc) = content.types.get(&real) {
                local.types.insert(alias.clone(), assoc.clone());
                let prefix = format!("{real}::");
                for (path, konst) in &content.paths {
                    if let Some(rest) = path.strip_prefix(&prefix) {
                        local
                            .paths
                            .insert(format!("{alias}::{rest}"), konst.clone());
                    }
                }
            }
        }
        // A name the file imports from elsewhere, `sts2_core::EventOption`, is
        // not the build's content.
        let build_path = format!("crate::{}::", args.build);
        for (alias, path) in imports(&text) {
            if local.types.contains_key(&alias) && !path.starts_with(&build_path) {
                local.types.remove(&alias);
                let prefix = format!("{alias}::");
                local.paths.retain(|path, _| !path.starts_with(&prefix));
            }
        }
        let content = &local;
        let text = rewrite_matches_macro(&text, content);
        let text = rewrite_matches(&text, content);
        let text = rewrite_lets(&text, content, &mut left);
        let text = rewrite_rest(&text, content, &mut left);
        let text = rewrite_uses(&text, content, &args.build, &mut left);
        let text = pinned(&text);
        // Where the module lands, and what its lists are called by.
        let shared: Vec<String> = module.iter().skip(2).cloned().collect();
        let stem = shared.last().cloned().unwrap_or_default().to_uppercase();
        let shared_path = format!("crate::behaviour::{}", shared.join("::"));
        let (text, lists) = lists(&text, &format!("{stem}_"), &shared_path, &mut left);
        if !lists.is_empty() {
            declared.push((format!("crate::behaviour::{}", shared.join("::")), lists));
        }
        lefts.push(left);
        // A path from here on is the one the module has once shared.
        let mut shared_module = vec!["behaviour".to_owned()];
        shared_module.extend(shared);
        files.push((shared_module, text));
    }
    generic_types(&mut files);
    declare_lists(&declared)?;
    let mut report = String::new();
    let mut written = Vec::new();
    for ((path, (module, text)), left) in args.files.iter().zip(files).zip(lefts) {
        // A child whose `lists!` the build reaches is visible to it.
        let mut text = text;
        let here = format!("crate::{}::", module.join("::"));
        for (list_module, _) in &declared {
            if let Some(child) = list_module.strip_prefix(&here)
                && !child.contains("::")
            {
                text = text.replace(
                    &format!("\nmod {child};"),
                    &format!("\npub(crate) mod {child};"),
                );
            }
        }
        let text = local_consts(&text);
        let (text, made) = generic_fns(&text);
        let text = import_ids(&text, &content);
        let text = unused_pools(&text, &content);
        written.push((
            path,
            text,
            made,
            left,
            module.last().cloned().unwrap_or_default(),
        ));
    }
    // A call names `V` where it cannot be read off an argument: to a function
    // the file made generic, or one it imports from a module that moved.
    let all: Vec<(String, usize)> = written.iter().flat_map(|w| w.2.clone()).collect();
    let modules: Vec<(String, String, usize)> = written
        .iter()
        .flat_map(|w| w.2.iter().map(|(name, n)| (w.4.clone(), name.clone(), *n)))
        .collect();
    let build = &args.build;
    for (path, text, made, mut left, _) in written {
        let mut calls = made.clone();
        for (name, _) in imports(&text) {
            if let Some(found) = all.iter().find(|(made, _)| *made == name)
                && !calls.iter().any(|(own, _)| *own == name)
            {
                calls.push(found.clone());
            }
        }
        let text = turbofish(&text, &calls, &modules);
        // A test names a build, which is the one the rule came from.
        let text = match text.find("mod tests {\n") {
            Some(at) if text[at..].contains("<V") || text[at..].contains("V::") => {
                let mut text = text;
                let alias = format!(
                    "    type V = crate::{0}::{1};\n",
                    build,
                    build.to_uppercase()
                );
                text.insert_str(at + "mod tests {\n".len(), &alias);
                // `V` is a concrete build there, whose associated types need
                // their trait named.
                let (before, tests) = text.split_at(at);
                let mut tests = tests.to_owned();
                for assoc in content.types.values() {
                    let name = assoc.trim_start_matches("V::");
                    tests = tests.replace(
                        &format!("V::{name}"),
                        &format!("<V as sts2_core::ContentVersion>::{name}"),
                    );
                }
                format!("{before}{tests}")
            }
            _ => text,
        };
        error::write(path, &text)?;
        left.notes.sort();
        left.notes.dedup();
        writeln!(report, "{}: {} left", path.display(), left.notes.len()).unwrap();
        for (line, what) in left.notes {
            writeln!(report, "  {line}: {what}").unwrap();
        }
    }
    Ok(report)
}
