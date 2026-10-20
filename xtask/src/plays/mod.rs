//! Reading the decompiled assembly, for the dump modes that read no running
//! game.
//!
//! Each reader matches statements against a list of commands it knows
//! exactly. A statement either matches one, or it names the model as one
//! nobody can generate. Nothing in between: one statement outside the list
//! leaves the whole method to a hand port.
//!
//! Presentation is the exception. A statement that only animates, plays a
//! sound or waits changes nothing a fight holds. [`presentation`] says which
//! ones those are, by what they call.
//!
//! The xtask README says why each list is short.

pub mod cards;
mod csharp;
pub mod monsters;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

use csharp::{Stmt, Tok, spell};

/// What stops a dump read off the assembly.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("reading {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{} names no {key}", path.display())]
    Release { path: PathBuf, key: &'static str },
    #[error("{} holds {holds}, not {wanted}", game.display())]
    OtherBuild {
        game: PathBuf,
        holds: String,
        wanted: String,
    },
    #[error("{} and {} squash to one name", first.display(), second.display())]
    Squash { first: PathBuf, second: PathBuf },
    #[error("{class}: {source}")]
    Syntax {
        class: String,
        #[source]
        source: csharp::Error,
    },
    #[error("no class in the assembly is the {kind} {id}")]
    NoClass { kind: &'static str, id: String },
    #[error("a var row with {fields} fields: {row}")]
    VarRow { fields: usize, row: String },
    #[error("a move of {0} before the monster")]
    BeforeMonster(String),
    #[error("{class} derives from {parent}, which is no monster")]
    NotAMonster { class: String, parent: String },
    #[error("{0} declares no parent")]
    NoParent(String),
    #[error("HOME is not set, and DOTNET_ROOT is found under it")]
    NoHome,
}

impl Error {
    /// A reader failure, in the class it was reading.
    fn syntax(class: impl std::fmt::Display) -> impl FnOnce(csharp::Error) -> Error {
        move |source| Error::Syntax {
            class: class.to_string(),
            source,
        }
    }
}

/// Why a play, an upgrade or a move is left for a hand to port.
///
/// Written into the dump as its reason, so each reads as the dump says it.
#[derive(Debug, thiserror::Error)]
pub enum Blocked {
    /// The first statement no command matches.
    #[error("{0}")]
    Statement(String),
    #[error(transparent)]
    Syntax(#[from] csharp::Error),
    #[error("no MoveState names it")]
    NoState,
    #[error("its MoveState performs a lambda")]
    Lambda,
    #[error("no method {0}")]
    NoMethod(String),
}

/// A name with its case and underscores taken out, which a class and the id
/// the game derives from it agree on.
fn squash(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// The decompiled source of the build in `game`, and that build.
///
/// The game in `game` has to be the build `version` names. The mods refuse a
/// build they were not built against, and this reads no mod, so it checks.
fn source(game: &Path, version: &str) -> Result<(Release, PathBuf), Error> {
    let release = release(game)?;
    if release.version != version {
        return Err(Error::OtherBuild {
            game: game.to_path_buf(),
            holds: release.version,
            wanted: version.to_owned(),
        });
    }
    let source = decompiled(game, &release)?;
    Ok((release, source))
}

/// Every class in one namespace of the source, by its [`squash`]ed name.
fn classes(source: &Path, namespace: &str) -> Result<HashMap<String, PathBuf>, Error> {
    let dir = source.join(namespace);
    let mut by_name: HashMap<String, PathBuf> = HashMap::new();
    for path in error::entries(&dir)? {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(clash) = by_name.insert(squash(stem), path.clone()) {
            return Err(Error::Squash {
                first: clash,
                second: path,
            });
        }
    }
    Ok(by_name)
}

/// A dump's first two lines. No content hash, which only a running game works
/// out.
fn header(kind: &str, release: &Release) -> String {
    format!(
        "# {kind}\n# build {} commit {} assembly {}\n",
        release.version, release.commit, release.assembly
    )
}

/// `release_info.json`, which names the build a game directory holds.
struct Release {
    version: String,
    commit: String,
    assembly: i64,
}

fn release(game: &Path) -> Result<Release, Error> {
    let path = game.join("release_info.json");
    let json: serde_json::Value =
        serde_json::from_str(&error::read(&path)?).map_err(|source| Error::Json {
            path: path.clone(),
            source,
        })?;
    let missing = |key| Error::Release {
        path: path.clone(),
        key,
    };
    Ok(Release {
        version: json["version"]
            .as_str()
            .ok_or_else(|| missing("version"))?
            .to_owned(),
        commit: json["commit"]
            .as_str()
            .ok_or_else(|| missing("commit"))?
            .to_owned(),
        assembly: json["main_assembly_hash"]
            .as_i64()
            .ok_or_else(|| missing("main_assembly_hash"))?,
    })
}

/// The assembly as C#, decompiled once per build into `target/decompiled/`.
///
/// `ilspycmd` needs `-r` pointed at Godot's assemblies. Without it, it writes
/// every async method as raw IL, and `OnPlay` is async.
fn decompiled(game: &Path, release: &Release) -> Result<PathBuf, Error> {
    let out = PathBuf::from("target/decompiled")
        .join(format!("{}-{}", release.version, release.assembly));
    let done = out.join(".done");
    if done.is_file() {
        return Ok(out);
    }
    let data = game.join("data_sts2_linuxbsd_x86_64");
    let home = std::env::var("HOME").map_err(|_| Error::NoHome)?;
    let dotnet = std::env::var("DOTNET_ROOT").unwrap_or_else(|_| format!("{home}/.dotnet"));
    let path = std::env::var("PATH").unwrap_or_default();
    let _ = std::fs::remove_dir_all(&out);
    error::create_dir(&out)?;
    error::status(
        Command::new("ilspycmd")
            .env("DOTNET_ROOT", &dotnet)
            // The tool targets an older runtime than the one installed.
            .env("DOTNET_ROLL_FORWARD", "Major")
            .env("PATH", format!("{dotnet}:{path}:{dotnet}/tools"))
            .args(["--disable-updatecheck", "-p", "-r"])
            .arg(&data)
            .arg("-o")
            .arg(&out)
            .arg(data.join("sts2.dll")),
    )?;
    error::write(&done, "")?;
    Ok(out)
}

/// The arguments of the call whose parentheses open `toks`, each as its
/// tokens.
fn arguments(toks: &[Tok]) -> Option<Vec<Vec<Tok>>> {
    if !toks.first()?.is("(") {
        return None;
    }
    let mut args = vec![Vec::new()];
    let mut depth = 0;
    for tok in toks {
        match tok {
            Tok::Punct("(" | "[" | "{") => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            Tok::Punct(")" | "]" | "}") => {
                depth -= 1;
                if depth == 0 {
                    if args.len() == 1 && args[0].is_empty() {
                        args.clear();
                    }
                    return Some(args);
                }
            }
            Tok::Punct(",") if depth == 1 => {
                args.push(Vec::new());
                continue;
            }
            _ => {}
        }
        args.last_mut()?.push(tok.clone());
    }
    None
}

/// One call of a chain: the method's name, and each argument's tokens. The
/// first call's name is empty, because the chain's root names it.
type Call = (String, Vec<Vec<Tok>>);

/// A call chain, `Root.Method<T>(args).Next(args)...`, split into its calls.
///
/// The root is everything before the first parenthesis, spelled. `None` for a
/// statement that is not one chain of calls.
fn chain(toks: &[Tok]) -> Option<(String, Vec<Call>)> {
    let open = toks.iter().position(|tok| tok.is("("))?;
    let root = spell(&toks[..open]);
    let mut calls = Vec::new();
    let mut at = open;
    let mut name = String::new();
    loop {
        let args = arguments(&toks[at..])?;
        let close = at + closing_paren(&toks[at..])?;
        calls.push((std::mem::take(&mut name), args));
        match &toks[close + 1..] {
            [] => return Some((root, calls)),
            [Tok::Punct("."), Tok::Ident(next), Tok::Punct("("), ..] => {
                name = next.clone();
                at = close + 3;
            }
            _ => return None,
        }
    }
}

fn closing_paren(toks: &[Tok]) -> Option<usize> {
    let mut depth = 0;
    for (at, tok) in toks.iter().enumerate() {
        if tok.is("(") {
            depth += 1;
        } else if tok.is(")") {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
    }
    None
}

/// What the locals a method declares hold, as far as presentation cares.
#[derive(Default)]
struct Locals {
    /// A Godot node, whose methods only draw.
    nodes: HashSet<String>,
    /// Anything else. Read freely, and never called.
    plain: HashSet<String>,
    /// The local holding `base.CombatState.HittableEnemies`, which a play
    /// takes once at most.
    held: Option<String>,
}

/// A Godot node's class, which the game names with an `N` and a capital.
fn node_class(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next() == Some('N') && chars.next().is_some_and(|c| c.is_ascii_uppercase())
}

/// A type whose value only draws: a Godot node or value type, or one of the
/// Spine bindings the game names with `Mega`.
fn drawing_type(name: &str) -> bool {
    node_class(name)
        || matches!(name, "Node" | "Node2D" | "Control" | "Vector2" | "Color")
        || name.starts_with("Mega")
}

/// Whether the call a chain rooted at `root` makes changes nothing in a fight.
fn presentation_call(root: &[&str], locals: &Locals) -> bool {
    match root {
        [first, ..] if node_class(first) || locals.nodes.contains(*first) => true,
        // `Creature.GetCreatureNode` and `GetVfxContainer` look a node up, and
        // hand back none outside a combat room. What is called on it draws.
        names
            if names
                .iter()
                .any(|name| matches!(*name, "GetCreatureNode" | "GetVfxContainer")) =>
        {
            true
        }
        // A speech or thought bubble, and the line it says.
        ["TalkCmd" | "ThinkCmd", "Play"]
        | ["MonsterModel", "L10NMonsterLookup"]
        | ["L10NMonsterLookup"] => true,
        // `Rng.Chaotic` is seeded from the clock, so nothing a fight holds
        // reads what it draws.
        names if names.windows(2).any(|pair| pair == ["Rng", "Chaotic"]) => true,
        // Whether a list a local holds has anything in it.
        [local, "Any"] if locals.plain.contains(*local) => true,
        ["SfxCmd" | "VfxCmd", ..] => true,
        ["Cmd", "Wait" | "CustomScaledWait"] => true,
        ["CreatureCmd", "TriggerAnim"] => true,
        // `Ironclad.GetHeavyAnimIfApplicable` and its kind, each an
        // animation's name or delay for one character and a default for the
        // rest.
        [
            "Ironclad" | "Silent" | "Regent" | "Necrobinder" | "Defect",
            pick,
        ] if pick.starts_with("Get") && pick.ends_with("IfApplicable") => true,
        // Godot's value types.
        ["new", "Color" | "Vector2"] => true,
        _ => false,
    }
}

/// The names a call's chain is spelled with, from its root, for the call
/// whose parenthesis opens at `open`.
///
/// `None` for a parenthesis that opens no call, which is a cast, a group, or
/// a keyword's.
fn called(toks: &[Tok], open: usize) -> Option<Result<Vec<&str>, ()>> {
    const KEYWORDS: [&str; 13] = [
        "if", "foreach", "while", "for", "switch", "using", "lock", "catch", "return", "typeof",
        "nameof", "sizeof", "delegate",
    ];
    let mut at = open.checked_sub(1)?;
    // A generic method's type arguments.
    if toks[at].is(">") {
        let mut depth = 0;
        loop {
            if toks[at].is(">") {
                depth += 1;
            } else if toks[at].is("<") {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            at = at.checked_sub(1)?;
        }
        at = at.checked_sub(1)?;
    }
    let name = toks[at].ident()?;
    if KEYWORDS.contains(&name) {
        return None;
    }
    let mut names = vec![name];
    while let Some(dot) = at.checked_sub(1) {
        if !(toks[dot].is(".") || toks[dot].is("?.")) {
            if toks[dot].is("new") {
                names.push("new");
            }
            break;
        }
        let Some(before) = dot.checked_sub(1) else {
            return Some(Err(()));
        };
        match &toks[before] {
            Tok::Ident(name) => {
                names.push(name);
                at = before;
            }
            // Something called or indexed, whose own chain is checked
            // where its parenthesis opens. Its root is this one's.
            Tok::Punct(")" | "]") => {
                let mut depth = 0;
                let mut back = before;
                loop {
                    if toks[back].is(")") || toks[back].is("]") {
                        depth += 1;
                    } else if toks[back].is("(") || toks[back].is("[") {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    back = match back.checked_sub(1) {
                        Some(back) => back,
                        None => return Some(Err(())),
                    };
                }
                match back.checked_sub(1).map(|at| &toks[at]) {
                    Some(Tok::Ident(name)) => {
                        names.push(name);
                        at = back - 1;
                    }
                    // A parenthesised expression, whose own calls are checked
                    // where they open. The chain it starts has its root.
                    _ => match toks.get(back + 1) {
                        Some(Tok::Ident(name)) => {
                            names.push(name);
                            break;
                        }
                        _ => return Some(Err(())),
                    },
                }
            }
            _ => return Some(Err(())),
        }
    }
    names.reverse();
    Some(Ok(names))
}

/// Whether every call in `toks` is presentation, and nothing is assigned but
/// a local.
fn pure(toks: &[Tok], locals: &Locals) -> bool {
    for (at, tok) in toks.iter().enumerate() {
        match tok {
            Tok::Punct("(") => match called(toks, at) {
                None => {}
                Some(Ok(root)) if presentation_call(&root, locals) => {}
                Some(_) => return false,
            },
            Tok::Punct("=" | "+=" | "-=" | "*=" | "/=" | "??=" | "|=" | "&=" | "++" | "--") => {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// Whether a statement changes nothing in a fight, which is what it takes to
/// leave it out of a play. Declares whatever local it introduces.
fn presentation(stmt: &Stmt, locals: &mut Locals) -> bool {
    match stmt {
        Stmt::Simple(toks) => {
            let toks = match toks.first() {
                Some(tok) if tok.is("await") => &toks[1..],
                _ => &toks[..],
            };
            let toks = &toks[..toks.len() - 1];
            // Where the method goes next is never only how it looks.
            if toks.first().is_some_and(|tok| {
                ["return", "throw", "break", "continue", "goto", "yield"]
                    .iter()
                    .any(|keyword| tok.is(keyword))
            }) {
                return false;
            }
            // The sim has no play without its target, so the check the game
            // makes is one it has already made.
            if spell(toks)
                == "ArgumentNullException.ThrowIfNull(cardPlay.Target, \"cardPlay.Target\")"
            {
                return true;
            }
            // Tunneler's `IsStunned`, which only its animator reads, to pick
            // its hit animation. v0.111.0 clears it in every move.
            if matches!(
                spell(toks).as_str(),
                "IsStunned = false" | "IsStunned = true"
            ) {
                return true;
            }
            // A declaration, `Type name = value` or `Type name`.
            let assign = toks.iter().position(|tok| tok.is("="));
            let named = assign.unwrap_or(toks.len());
            if named >= 2
                && toks[named - 1].ident().is_some()
                && matches!(
                    &toks[named - 2],
                    Tok::Ident(_) | Tok::Punct(">" | "?" | "]")
                )
                && !toks[..named - 1].iter().any(|tok| tok.is("."))
                && toks[0].ident().is_some_and(|ty| ty != "return")
            {
                let name = toks[named - 1].ident().unwrap_or_default().to_owned();
                let value = assign.map_or(&[][..], |at| &toks[at + 1..]);
                if !pure(value, locals) {
                    return false;
                }
                match drawing_type(toks[0].ident().unwrap_or_default()) {
                    true => locals.nodes.insert(name),
                    false => locals.plain.insert(name),
                };
                return true;
            }
            // `node.Property = value`, on a node a local holds.
            if let [Tok::Ident(root), Tok::Punct("." | "?."), ..] = toks
                && locals.nodes.contains(root)
                && let Some(assign) = toks.iter().position(|tok| tok.is("="))
                && toks[..assign]
                    .iter()
                    .all(|tok| tok.ident().is_some() || tok.is(".") || tok.is("?."))
            {
                return pure(&toks[assign + 1..], locals);
            }
            // `local op= value`, or `local++`.
            if let [Tok::Ident(name), Tok::Punct(op), value @ ..] = toks
                && (locals.plain.contains(name) || locals.nodes.contains(name))
                && matches!(*op, "=" | "+=" | "-=" | "*=" | "/=" | "++" | "--")
            {
                return pure(value, locals);
            }
            pure(toks, locals)
        }
        Stmt::If { cond, then, other } => {
            pure(cond, locals)
                && then.iter().all(|stmt| presentation(stmt, locals))
                && other.iter().all(|stmt| presentation(stmt, locals))
        }
        Stmt::Foreach { head, body } => {
            // `Type name in list`.
            let Some(at) = head.iter().position(|tok| tok.is("in")) else {
                return false;
            };
            if at < 2 || !pure(&head[at + 1..], locals) {
                return false;
            }
            if let Some(name) = head[at - 1].ident() {
                match drawing_type(head[0].ident().unwrap_or_default()) {
                    true => locals.nodes.insert(name.to_owned()),
                    false => locals.plain.insert(name.to_owned()),
                };
            }
            body.iter().all(|stmt| presentation(stmt, locals))
        }
        Stmt::Other(_) => false,
    }
}

/// A method body's statements, less the `return Task.CompletedTask;` that
/// ends one that is not async. A method that runs off its end does the same.
fn statements(body: &[Tok]) -> Result<Vec<Stmt>, csharp::Error> {
    let mut stmts = csharp::statements(body)?;
    if stmts
        .last()
        .is_some_and(|stmt| stmt.text() == "return Task.CompletedTask;")
    {
        stmts.pop();
    }
    Ok(stmts)
}

/// A whole number written into the source, such as `3m` or `-1`.
fn literal(toks: &[Tok]) -> Option<i64> {
    let (sign, num) = match toks {
        [Tok::Num(num)] => (1, num),
        [Tok::Punct("-"), Tok::Num(num)] => (-1, num),
        _ => return None,
    };
    let digits = num.trim_end_matches(['m', 'M']);
    digits.parse::<i64>().ok().map(|n| sign * n)
}

/// The builder calls on an attack that only choose how it looks.
const LOOKS: [&str; 9] = [
    "WithHitFx",
    "WithHitVfxNode",
    "WithNoAttackerAnim",
    "WithAttackerAnim",
    "WithAttackerFx",
    "WithWaitBeforeHit",
    "SpawningHitVfxOnEachCreature",
    "WithHitVfxSpawnedAtBase",
    "OnlyPlayAnimOnce",
];
