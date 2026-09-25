//! What each card's `OnPlay` and `OnUpgrade` do, read off the decompiled
//! assembly, for `dump cardplaydump`.
//!
//! A statement either matches one command this knows exactly, or it names the
//! card as one nobody can generate. Nothing in between: a card whose method
//! holds one statement outside the list gets no commands at all. The list is
//! [`game`] for a play and [`upgrade`] for an upgrade.
//!
//! Presentation is the exception. A statement that only animates, plays a
//! sound or waits changes nothing a fight holds. [`presentation`] says which
//! ones those are, by what they call.
//!
//! The xtask README says why the list is short, and what else a card
//! overrides that stops its play.

mod csharp;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use csharp::{Stmt, Tok, spell};

/// The creatures a command points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// `cardPlay.Target`.
    Chosen,
    /// `base.Owner.Creature`.
    Actor,
    /// `base.CombatState.HittableEnemies`, as it stands when the command
    /// runs.
    Opponents,
    /// A local that took `base.CombatState.HittableEnemies` earlier in the
    /// play, which is the list as it stood then.
    Held,
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Target::Chosen => "chosen",
            Target::Actor => "self",
            Target::Opponents => "opponents",
            Target::Held => "held",
        }
    }
}

/// A number a command reads.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Number {
    /// One the card prints, by key.
    Var(String),
    /// One written into the method.
    Literal(i64),
}

impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Number::Var(key) => write!(f, "var {key}"),
            Number::Literal(n) => write!(f, "number {n}"),
        }
    }
}

/// What a build says about its cards, beyond the classes themselves.
struct Build {
    /// `DynamicVarSet`'s named properties, each to the key it reads.
    properties: HashMap<String, String>,
    /// Each card's vars, by key, as `cards.txt` lists them: the class, and
    /// whether the number is whole and printed rather than worked out.
    vars: HashMap<String, HashMap<String, (String, bool)>>,
    /// Power ids, by the name of the class that is each one.
    powers: HashMap<String, String>,
}

/// One card's class, and what the rest of its source says that a command
/// reads.
struct Card<'a> {
    id: &'a str,
    build: &'a Build,
    /// The `ValueProp` of each `BlockVar` the card declares, by key.
    block_props: HashMap<String, String>,
}

/// Every card's play and upgrade, as the dump's rows.
///
/// The game in `game` has to be the build `version` names. The mods refuse a
/// build they were not built against, and this reads no mod, so it checks.
pub fn cards(game: &Path, version: &str, version_dir: &Path) -> Result<String, String> {
    let release = release(game)?;
    if release.version != version {
        return Err(format!(
            "{} holds {}, not {version}",
            game.display(),
            release.version
        ));
    }
    let source = decompiled(game, &release)?;
    let build = Build::read(&source, version_dir)?;

    let classes = source.join("MegaCrit.Sts2.Core.Models.Cards");
    let mut by_name: HashMap<String, PathBuf> = HashMap::new();
    for entry in
        fs::read_dir(&classes).map_err(|e| format!("reading {}: {e}", classes.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(clash) = by_name.insert(squash(stem), path.clone()) {
            return Err(format!(
                "{} and {} name the same card",
                clash.display(),
                path.display()
            ));
        }
    }

    let cards_txt = fs::read_to_string(version_dir.join("dumps/cards.txt"))
        .map_err(|e| format!("reading cards.txt: {e}"))?;
    let ids: Vec<&str> = cards_txt
        .lines()
        .filter_map(|line| line.strip_prefix("card "))
        .filter_map(|row| row.split_whitespace().next())
        .collect();

    let mut out = format!(
        "# card plays\n\
         # build {} commit {} assembly {}\n\
         # card <id>\n\
         # play <card> attack <chosen|opponents> <damage var> <props> <hits>\n\
         # play <card> block <block var> <props>\n\
         # play <card> hold opponents, which a later `held` reads as they stood\n\
         # play <card> power <chosen|self|opponents|held> <power> <amount>\n\
         # play <card> <draw|energy|stars|forge> <amount>\n\
         # unplayed <card> <the first statement no command matches>\n\
         # upgrade <card> raise <var> <by>\n\
         # upgrade <card> cost <by>\n\
         # upgrade <card> keyword <keyword>\n\
         # unupgraded <card> <the first statement no change matches>\n\
         # turnend <card>, which acts when a turn ends with it in hand\n\
         # An amount is `var <key>` or `number <n>`.\n",
        release.version, release.commit, release.assembly
    );
    for id in ids {
        let path = by_name
            .get(&squash(id))
            .ok_or_else(|| format!("no class in the assembly is the card {id}"))?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let text =
            fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let class = csharp::tokens(&text).map_err(|e| format!("{name}: {e}"))?;
        let card = Card {
            id,
            build: &build,
            block_props: block_props(&class),
        };
        writeln!(out, "card {id}").unwrap();

        match csharp::base_class(&class, name).as_deref() {
            Some("CardModel") => {}
            other => {
                // Whatever the parent does is what the card inherits, and this
                // reads only the card's own class.
                let parent = other.unwrap_or("nothing");
                writeln!(out, "unplayed {id} derives from {parent}").unwrap();
                writeln!(out, "unupgraded {id} derives from {parent}").unwrap();
                writeln!(out, "turnend {id}").unwrap();
                continue;
            }
        }

        // `CombatManager` runs `OnTurnEndInHand` for a card that says it has
        // an effect there, inside pile moves of its own.
        if csharp::overrides(&class, "HasTurnEndInHandEffect") {
            writeln!(out, "turnend {id}").unwrap();
        }

        // What decides whether the card can be played, and where it goes
        // after. The simulator reads neither off the card, so a play that
        // runs without them runs wrong.
        if let Some(shaping) = ["IsPlayable", "GetResultPileTypeForCardPlay"]
            .into_iter()
            .find(|name| csharp::overrides(&class, name))
        {
            writeln!(out, "unplayed {id} overrides {shaping}").unwrap();
        } else {
            match plays(
                &card,
                csharp::method(&class, "OnPlay").map_err(|e| format!("{name}: {e}"))?,
            ) {
                Ok(commands) => {
                    for command in commands {
                        writeln!(out, "play {id} {command}").unwrap();
                    }
                }
                Err(blocked) => writeln!(out, "unplayed {id} {blocked}").unwrap(),
            }
        }
        match upgrades(
            &card,
            csharp::method(&class, "OnUpgrade").map_err(|e| format!("{name}: {e}"))?,
        ) {
            Ok(changes) => {
                for change in changes {
                    writeln!(out, "upgrade {id} {change}").unwrap();
                }
            }
            Err(blocked) => writeln!(out, "unupgraded {id} {blocked}").unwrap(),
        }
    }
    Ok(out)
}

/// A name with its case and underscores taken out, which a class and the id
/// the game derives from it agree on.
fn squash(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// `release_info.json`, which names the build a game directory holds.
struct Release {
    version: String,
    commit: String,
    assembly: i64,
}

fn release(game: &Path) -> Result<Release, String> {
    let path = game.join("release_info.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let field = |key: &str| json[key].clone();
    Ok(Release {
        version: field("version").as_str().ok_or("no version")?.to_owned(),
        commit: field("commit").as_str().ok_or("no commit")?.to_owned(),
        assembly: field("main_assembly_hash")
            .as_i64()
            .ok_or("no assembly hash")?,
    })
}

/// The assembly as C#, decompiled once per build into `target/decompiled/`.
///
/// `ilspycmd` needs `-r` pointed at Godot's assemblies. Without it, it writes
/// every async method as raw IL, and `OnPlay` is async.
fn decompiled(game: &Path, release: &Release) -> Result<PathBuf, String> {
    let out = PathBuf::from("target/decompiled")
        .join(format!("{}-{}", release.version, release.assembly));
    let done = out.join(".done");
    if done.is_file() {
        return Ok(out);
    }
    let data = game.join("data_sts2_linuxbsd_x86_64");
    let home = std::env::var("HOME").map_err(|_| "HOME is not set")?;
    let dotnet = std::env::var("DOTNET_ROOT").unwrap_or_else(|_| format!("{home}/.dotnet"));
    let path = std::env::var("PATH").unwrap_or_default();
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let status = Command::new("ilspycmd")
        .env("DOTNET_ROOT", &dotnet)
        // The tool targets an older runtime than the one installed.
        .env("DOTNET_ROLL_FORWARD", "Major")
        .env("PATH", format!("{dotnet}:{path}:{dotnet}/tools"))
        .args(["--disable-updatecheck", "-p", "-r"])
        .arg(&data)
        .arg("-o")
        .arg(&out)
        .arg(data.join("sts2.dll"))
        .status()
        .map_err(|e| format!("running ilspycmd, which this needs installed: {e}"))?;
    if !status.success() {
        return Err(format!("ilspycmd exited {status}"));
    }
    fs::write(&done, "").map_err(|e| format!("writing {}: {e}", done.display()))?;
    Ok(out)
}

impl Build {
    fn read(source: &Path, version_dir: &Path) -> Result<Build, String> {
        // `public BlockVar Block => (BlockVar)_vars["Block"];`
        let set = source.join("MegaCrit.Sts2.Core.Localization.DynamicVars/DynamicVarSet.cs");
        let text =
            fs::read_to_string(&set).map_err(|e| format!("reading {}: {e}", set.display()))?;
        let toks = csharp::tokens(&text)?;
        let mut properties = HashMap::new();
        for (at, tok) in toks.iter().enumerate() {
            if !tok.is("=>") {
                continue;
            }
            let (Some(name), Some(key)) = (
                toks[at - 1].ident(),
                toks[at..]
                    .iter()
                    .take_while(|tok| !tok.is(";"))
                    .find_map(|tok| match tok {
                        Tok::Str(key) => Some(key.clone()),
                        _ => None,
                    }),
            ) else {
                continue;
            };
            properties.insert(name.to_owned(), key);
        }

        let cards = fs::read_to_string(version_dir.join("dumps/cards.txt"))
            .map_err(|e| format!("reading cards.txt: {e}"))?;
        let mut vars: HashMap<String, HashMap<String, (String, bool)>> = HashMap::new();
        for row in cards.lines().filter_map(|line| line.strip_prefix("var ")) {
            let f: Vec<&str> = row.split_whitespace().collect();
            let [card, key, base, calculated, class] = f[..] else {
                return Err(format!("a var row with {} fields: {row}", f.len()));
            };
            let printed = calculated == "0" && !base.contains('.');
            vars.entry(card.to_owned())
                .or_default()
                .insert(key.to_owned(), (class.to_owned(), printed));
        }

        let powers_txt = fs::read_to_string(version_dir.join("dumps/powers.txt"))
            .map_err(|e| format!("reading powers.txt: {e}"))?;
        let powers = powers_txt
            .lines()
            .filter_map(|line| line.strip_prefix("power "))
            .filter_map(|row| row.split_whitespace().next())
            .map(|id| (squash(id), id.to_owned()))
            .collect();

        Ok(Build {
            properties,
            vars,
            powers,
        })
    }
}

/// `new BlockVar([name,] base, props)` in `CanonicalVars`, by the key each
/// declares. A var that names no key is filed under `Block`.
fn block_props(class: &[Tok]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for at in 0..class.len().saturating_sub(2) {
        if !(class[at].is("new") && class[at + 1].is("BlockVar") && class[at + 2].is("(")) {
            continue;
        }
        let Some(args) = arguments(&class[at + 2..]) else {
            continue;
        };
        let key = match args.first().map(Vec::as_slice) {
            Some([Tok::Str(key)]) => key.clone(),
            _ => "Block".to_owned(),
        };
        if let Some(props) = args.last().and_then(|arg| props(arg)) {
            out.insert(key, props);
        }
    }
    out
}

/// `ValueProp.Move`, `ValueProp.Unpowered`, or the two together, as the dump
/// writes them. Any other flag is one no command here reads.
fn props(arg: &[Tok]) -> Option<String> {
    let mut flags = Vec::new();
    for part in arg.split(|tok| tok.is("|")) {
        match part {
            [Tok::Ident(ty), Tok::Punct("."), Tok::Ident(flag)] if ty == "ValueProp" => {
                match flag.as_str() {
                    "Move" | "Unpowered" => flags.push(flag.clone()),
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    flags.sort();
    Some(flags.join("+"))
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

/// Whether the call a chain rooted at `root` makes changes nothing in a fight.
fn presentation_call(root: &[&str], locals: &Locals) -> bool {
    match root {
        [first, ..] if node_class(first) || locals.nodes.contains(*first) => true,
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
                    _ => return Some(Err(())),
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
                match node_class(toks[0].ident().unwrap_or_default()) {
                    true => locals.nodes.insert(name),
                    false => locals.plain.insert(name),
                };
                return true;
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
                match node_class(head[0].ident().unwrap_or_default()) {
                    true => locals.nodes.insert(name.to_owned()),
                    false => locals.plain.insert(name.to_owned()),
                };
            }
            body.iter().all(|stmt| presentation(stmt, locals))
        }
        Stmt::Other(_) => false,
    }
}

impl Card<'_> {
    /// The key a var is read by, from `base.DynamicVars.Name` or
    /// `base.DynamicVars["Key"]`, with what follows it.
    fn var<'t>(&self, toks: &'t [Tok]) -> Option<(String, &'t [Tok])> {
        let rest = match toks {
            [Tok::Ident(b), Tok::Punct("."), Tok::Ident(set), rest @ ..]
                if b == "base" && set == "DynamicVars" =>
            {
                rest
            }
            _ => return None,
        };
        match rest {
            [Tok::Punct("."), Tok::Ident(name), rest @ ..] => {
                Some((self.build.properties.get(name)?.clone(), rest))
            }
            [Tok::Punct("["), Tok::Str(key), Tok::Punct("]"), rest @ ..] => {
                Some((key.clone(), rest))
            }
            _ => None,
        }
    }

    /// A var this card prints as it is, with the class it is.
    fn printed(&self, key: &str) -> Option<&str> {
        match self.build.vars.get(self.id)?.get(key)? {
            (class, true) => Some(class),
            (_, false) => None,
        }
    }

    /// A var's printed number, or a whole number written in.
    fn number(&self, toks: &[Tok]) -> Option<Number> {
        if let Some((key, rest)) = self.var(toks) {
            return match rest {
                [Tok::Punct("."), Tok::Ident(value)]
                    if value == "BaseValue" || value == "IntValue" =>
                {
                    self.printed(&key)?;
                    Some(Number::Var(key))
                }
                _ => None,
            };
        }
        literal(toks).map(Number::Literal)
    }

    /// Who a command's creature argument names.
    fn target(&self, toks: &[Tok], locals: &Locals) -> Option<Target> {
        match spell(toks).as_str() {
            "cardPlay.Target" => Some(Target::Chosen),
            "base.Owner.Creature" => Some(Target::Actor),
            "base.CombatState.HittableEnemies" => Some(Target::Opponents),
            local if locals.held.as_deref() == Some(local) => Some(Target::Held),
            _ => None,
        }
    }
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

/// The local a statement declares to hold `base.CombatState.HittableEnemies`.
fn holds(stmt: &Stmt) -> Option<String> {
    let Stmt::Simple(toks) = stmt else {
        return None;
    };
    let assign = toks.iter().position(|tok| tok.is("="))?;
    // `Type name =`, with a type of at least one token and no member access.
    let ty = &toks[..assign.checked_sub(1)?];
    if ty.is_empty() || ty.iter().any(|tok| tok.is(".")) {
        return None;
    }
    let value = &toks[assign + 1..toks.len() - 1];
    (spell(value) == "base.CombatState.HittableEnemies").then_some(())?;
    toks[assign - 1].ident().map(String::from)
}

/// The commands a card's `OnPlay` runs, or the statement none matches.
fn plays(card: &Card, body: Option<Vec<Tok>>) -> Result<Vec<String>, String> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    let mut stmts = csharp::statements(&body)?;
    // A method that is not async ends by handing back a finished task, and
    // one that runs off its end does the same.
    if stmts
        .last()
        .is_some_and(|stmt| stmt.text() == "return Task.CompletedTask;")
    {
        stmts.pop();
    }
    let mut locals = Locals::default();
    let mut out = Vec::new();
    for stmt in &stmts {
        // `enemies = base.CombatState.HittableEnemies`, which a later command
        // reads as the list stood here.
        if let Some(name) = holds(stmt) {
            if locals.held.is_some() {
                return Err(stmt.text());
            }
            locals.held = Some(name.clone());
            locals.plain.insert(name.clone());
            out.push("hold opponents".to_owned());
            continue;
        }
        if presentation(stmt, &mut locals) {
            continue;
        }
        match game(card, stmt, &locals) {
            Some(command) => out.push(command),
            None => return Err(stmt.text()),
        }
    }
    Ok(out)
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

/// The command one statement of a play runs, as the dump writes it.
fn game(card: &Card, stmt: &Stmt, locals: &Locals) -> Option<String> {
    let Stmt::Simple(toks) = stmt else {
        return None;
    };
    // Every command is async, and one nobody awaits runs beside the play
    // rather than inside it.
    let [Tok::Ident(wait), toks @ .., Tok::Punct(";")] = &toks[..] else {
        return None;
    };
    if wait != "await" {
        return None;
    }
    let (root, calls) = chain(toks)?;
    let first = &calls[0].1;
    match (root.as_str(), calls.len()) {
        ("DamageCmd.Attack", _) => attack(card, first, &calls[1..], locals),
        ("CreatureCmd.GainBlock", 1) => match &first[..] {
            [creature, var, play]
                if spell(creature) == "base.Owner.Creature" && spell(play) == "cardPlay" =>
            {
                let (key, []) = card.var(var)? else {
                    return None;
                };
                (card.printed(&key)? == "BlockVar").then_some(())?;
                let props = card.block_props.get(&key)?;
                Some(format!("block {key} {props}"))
            }
            _ => None,
        },
        (power, 1) if power.starts_with("PowerCmd.Apply<") && power.ends_with('>') => {
            let class = &power["PowerCmd.Apply<".len()..power.len() - 1];
            let id = card.build.powers.get(&squash(class))?;
            let [context, target, amount, applier, this] = &first[..] else {
                return None;
            };
            if !(spell(context) == "choiceContext"
                && spell(applier) == "base.Owner.Creature"
                && spell(this) == "this")
            {
                return None;
            }
            let target = card.target(target, locals)?;
            let amount = card.number(amount)?;
            // A negative number the game writes in is one endless instance
            // for some powers and a debt for others.
            if let Number::Literal(n) = amount
                && n < 0
            {
                return None;
            }
            Some(format!("power {} {id} {amount}", target.name()))
        }
        ("CardPileCmd.Draw", 1) => match &first[..] {
            [context, amount, owner]
                if spell(context) == "choiceContext" && spell(owner) == "base.Owner" =>
            {
                Some(format!("draw {}", card.number(amount)?))
            }
            _ => None,
        },
        ("PlayerCmd.GainEnergy" | "PlayerCmd.GainStars", 1) => match &first[..] {
            [amount, owner] if spell(owner) == "base.Owner" => {
                let what = if root.ends_with("Energy") {
                    "energy"
                } else {
                    "stars"
                };
                Some(format!("{what} {}", card.number(amount)?))
            }
            _ => None,
        },
        ("ForgeCmd.Forge", 1) => match &first[..] {
            [amount, owner, this] if spell(owner) == "base.Owner" && spell(this) == "this" => {
                Some(format!("forge {}", card.number(amount)?))
            }
            _ => None,
        },
        _ => None,
    }
}

/// `DamageCmd.Attack(damage)` and the builder calls after it.
fn attack(card: &Card, damage: &[Vec<Tok>], builders: &[Call], locals: &Locals) -> Option<String> {
    let [damage] = damage else { return None };
    let (key, [Tok::Punct("."), Tok::Ident(value)]) = card.var(damage)? else {
        return None;
    };
    (value == "BaseValue" && card.printed(&key)? == "DamageVar").then_some(())?;

    let (mut from, mut target, mut hits, mut props, mut run) =
        (false, None, Number::Literal(1), "Move", false);
    for (at, (name, args)) in builders.iter().enumerate() {
        let spelled: Vec<String> = args.iter().map(|arg| spell(arg)).collect();
        match (name.as_str(), &spelled[..]) {
            // v0.111.0 hands the play over too, which a hook reads. A play
            // here is always the one this attack is part of.
            ("FromCard", [this] | [this, _]) if this == "this" && !from => {
                if spelled.get(1).is_some_and(|play| play != "cardPlay") {
                    return None;
                }
                from = true;
            }
            ("Targeting", [chosen]) if chosen == "cardPlay.Target" && target.is_none() => {
                target = Some(Target::Chosen);
            }
            ("TargetingAllOpponents", [state])
                if state == "base.CombatState" && target.is_none() =>
            {
                target = Some(Target::Opponents);
            }
            ("WithHitCount", [_]) => hits = card.number(&args[0])?,
            ("Unpowered", []) => props = "Move+Unpowered",
            ("Execute", [context]) if context == "choiceContext" && at == builders.len() - 1 => {
                run = true;
            }
            (look, _) if LOOKS.contains(&look) => {
                if !args.iter().all(|arg| pure(arg, locals)) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    (from && run).then_some(())?;
    Some(format!("attack {} {key} {props} {hits}", target?.name()))
}

/// The changes a card's `OnUpgrade` makes, or the statement none matches.
fn upgrades(card: &Card, body: Option<Vec<Tok>>) -> Result<Vec<String>, String> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for stmt in csharp::statements(&body)? {
        match upgrade(card, &stmt) {
            Some(change) => out.push(change),
            None => return Err(stmt.text()),
        }
    }
    Ok(out)
}

/// The change one statement of an upgrade makes, as the dump writes it.
fn upgrade(card: &Card, stmt: &Stmt) -> Option<String> {
    let Stmt::Simple(toks) = stmt else {
        return None;
    };
    let toks = &toks[..toks.len() - 1];
    if let Some((key, rest)) = card.var(toks) {
        let [Tok::Punct("."), Tok::Ident(method), args @ ..] = rest else {
            return None;
        };
        let [by] = &arguments(args)?[..] else {
            return None;
        };
        (method == "UpgradeValueBy" && card.printed(&key).is_some()).then_some(())?;
        return Some(format!("raise {key} {}", literal(by)?));
    }
    let (root, calls) = chain(toks)?;
    let [(_, args)] = &calls[..] else { return None };
    match (root.as_str(), &args[..]) {
        ("base.EnergyCost.UpgradeBy", [by]) => Some(format!("cost {}", literal(by)?)),
        ("AddKeyword" | "base.AddKeyword", [keyword]) => match &keyword[..] {
            [Tok::Ident(ty), Tok::Punct("."), Tok::Ident(keyword)] if ty == "CardKeyword" => {
                Some(format!("keyword {keyword}"))
            }
            _ => None,
        },
        _ => None,
    }
}

/// How many cards the dump lets the generator write, for the summary.
pub fn tally(dump: &str) -> String {
    let count = |kind: &str| {
        dump.lines()
            .filter(|line| line.split_whitespace().next() == Some(kind))
            .map(|line| line.split_whitespace().nth(1))
            .collect::<BTreeSet<_>>()
            .len()
    };
    let cards = count("card");
    format!(
        "{} of {cards} plays and {} of {cards} upgrades matched",
        cards - count("unplayed"),
        cards - count("unupgraded")
    )
}
