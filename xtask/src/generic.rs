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
}

/// Read the consts off `ids.rs`, as the build implements them.
fn content(build: &str) -> Result<Content, Error> {
    let text = error::read(Path::new("crates/sts2-content/src/ids.rs"))?;
    let mut paths = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut pools = Vec::new();
    let mut traits = BTreeMap::new();
    let prefix = format!("crate::{build}::");
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
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
        let target = format!("V::{assoc}::{konst}");
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
    })
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
fn bind_paths(alt: &str, content: &Content) -> (String, Vec<String>) {
    let mut skeleton = String::new();
    let mut conds = Vec::new();
    let mut from = 0;
    for (n, (start, end, konst)) in content_paths(alt, content).into_iter().enumerate() {
        skeleton.push_str(&alt[from..start]);
        let binding = format!("__c{n}");
        conds.push(format!("{binding} == {konst}"));
        skeleton.push_str(&binding);
        from = end;
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
        last_indent = s[s[..i].rfind('\n').map_or(0, |at| at + 1)..i].to_owned();
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
        let mut groups: Vec<(String, Vec<Vec<String>>)> = Vec::new();
        for alt in split_top(pattern, b'|') {
            let (skeleton, conds) = bind_paths(alt.trim(), content);
            let key = code_of(&skeleton)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, all)) => all.push(conds),
                None => groups.push((key, vec![conds])),
            }
        }
        if groups.iter().all(|(_, all)| all.iter().all(Vec::is_empty)) {
            out.push_str(&s[i..body_end]);
        } else {
            changed = true;
            let arms: Vec<String> = groups
                .into_iter()
                .map(|(skeleton, all)| {
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
            out.push_str(&joined.join(&format!("\n{last_indent}")));
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
                write!(out, "({})", ors.join(" || ")).unwrap();
            } else {
                write!(out, "match {expr} {{ {pattern} => true, _ => false }}").unwrap();
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
        } else {
            left.note(&s, start, "a `let` pattern naming content");
        }
    }
    s
}

/// What is left: a content path is its const, a content type its associated
/// type.
fn rewrite_rest(s: &str, content: &Content, left: &mut Left) -> String {
    let mut out = String::new();
    for (number, line) in s.split_inclusive('\n').enumerate() {
        if line.starts_with("type ") && line.contains("<V as") {
            left.notes
                .push((number + 1, "a type alias naming `V`".to_owned()));
        }
        if line.starts_with("use ") && !line.contains('{') {
            // `use ..::Orb;` names what is now `V::Orb`.
            let name = line
                .trim_end()
                .trim_end_matches(';')
                .rsplit("::")
                .next()
                .unwrap_or("");
            if content.types.contains_key(name) {
                continue;
            }
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
                if let Some(assoc) = content.types.get(word).filter(|_| !path_follows) {
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

/// Relative paths up out of the file, made absolute.
fn absolute_paths(s: &str, module: &[String]) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find("super::") {
        let before = &rest[..at];
        out.push_str(before);
        let mut ups = 0;
        let mut tail = &rest[at..];
        while let Some(next) = tail.strip_prefix("super::") {
            ups += 1;
            tail = next;
        }
        // `module` is the file's own path, so `super` from it is its parent.
        let keep = module.len().saturating_sub(ups);
        let mut path = String::from("crate::");
        for part in &module[..keep] {
            write!(path, "{part}::").unwrap();
        }
        out.push_str(&path);
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// Each function naming `V` takes it as its first generic.
fn generic_fns(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut from = 0;
    while i < b.len() {
        if let Some(next) = skip_noise(b, i) {
            i = next;
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
            if names_v {
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
    // After the last `use` before any item.
    let mut at = 0;
    let mut offset = 0;
    let mut in_use = false;
    for l in s.split_inclusive('\n') {
        offset += l.len();
        if l.starts_with("use ") || in_use {
            in_use = !l.trim_end().ends_with(';');
            at = offset;
        } else if !l.trim().is_empty() && !l.starts_with("//") {
            break;
        }
    }
    let mut out = s.to_owned();
    out.insert_str(at, &line);
    out
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

pub fn run(args: Args) -> Result<String, Error> {
    let content = content(&args.build)?;
    let mut report = String::new();
    for path in &args.files {
        let module = module_of(path)?;
        let text = error::read(path)?;
        let mut left = Left { notes: Vec::new() };
        let text = absolute_paths(&text, &module);
        let text = text.replace(&format!("use crate::{}::V0_107_1 as V;\n", args.build), "");
        let text = rewrite_matches_macro(&text, &content);
        let text = rewrite_matches(&text, &content);
        let text = rewrite_lets(&text, &content, &mut left);
        let text = rewrite_rest(&text, &content, &mut left);
        let text = generic_fns(&text);
        let text = import_ids(&text, &content);
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
