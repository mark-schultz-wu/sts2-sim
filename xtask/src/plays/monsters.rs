//! What each monster's moves do, for `dump monstermovedump`.
//!
//! A move is the method its `MoveState` names in `GenerateMoveStateMachine`.
//! The list is [`command`]. An attack matches only when it hits for exactly
//! the numbers its intent advertises, which the table already carries.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use super::csharp::{self, Stmt, Tok, spell};
use super::{
    Blocked, Call, Error, LOOKS, Locals, arguments, chain, literal, presentation, pure, squash,
};
use crate::error;

/// A number a move reads.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Amount {
    /// One written into the class.
    Number(i64),
    /// `AscensionHelper.GetValueIfAscension(level, at, below)`.
    Ascension { level: String, at: i64, below: i64 },
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Amount::Number(n) => write!(f, "number {n}"),
            Amount::Ascension { level, at, below } => write!(f, "ascension {level} {at} {below}"),
        }
    }
}

/// The hit an attack intent advertises, as the class spells it.
struct Advertised {
    damage: String,
    /// `None` for a `SingleAttackIntent`.
    hits: Option<String>,
}

/// One `new MoveState(id, method, intents...)`.
struct MoveState {
    /// `None` when the state performs a lambda rather than a named method.
    method: Option<String>,
    /// Every attack intent the state advertises. An attack intent this reads
    /// no numbers off is `None`.
    attacks: Vec<Option<Advertised>>,
}

/// One monster's class, and its parent's when that is not `MonsterModel`.
struct Monster<'a> {
    /// The class first, then its parent.
    classes: Vec<Vec<Tok>>,
    /// Numbers the classes name, own first.
    members: HashMap<String, Vec<Tok>>,
    powers: &'a HashMap<String, String>,
    cards: &'a HashMap<String, String>,
}

/// Every monster's moves, as the dump's rows.
pub fn monsters(game: &Path, version: &str, version_dir: &Path) -> Result<String, Error> {
    let (release, source) = super::source(game, version)?;
    // The game's test monsters sit in a namespace of their own, and the table
    // holds them too.
    let mut by_name = super::classes(&source, "MegaCrit.Sts2.Core.Models.Monsters")?;
    by_name.extend(super::classes(
        &source,
        "MegaCrit.Sts2.Core.Models.Monsters.Mocks",
    )?);
    let ids = |dump: &str, kind: &str| -> Result<HashMap<String, String>, Error> {
        let text = error::read(&version_dir.join("dumps").join(dump))?;
        Ok(text
            .lines()
            .filter_map(|line| line.strip_prefix(kind))
            .filter_map(|row| row.split_whitespace().next())
            .map(|id| (squash(id), id.to_owned()))
            .collect())
    };
    let powers = ids("powers.txt", "power ")?;
    let cards = ids("cards.txt", "card ")?;

    let table = error::read(&version_dir.join("dumps/monsters.txt"))?;
    let mut moves: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in table.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[..] {
            ["monster", id, ..] => moves.push((id, Vec::new())),
            ["move", monster, state, ..] => moves
                .iter_mut()
                .find(|(id, _)| *id == monster)
                .ok_or_else(|| Error::BeforeMonster(monster.to_owned()))?
                .1
                .push(state),
            _ => {}
        }
    }

    let mut out = super::header("monster moves", &release);
    out.push_str(
        "# monster <id>\n\
         # move <monster> <move>\n\
         # does <monster> <move> attack, for the hit its intent advertises\n\
         # does <monster> <move> power <self|opponents> <power> <amount>\n\
         # does <monster> <move> block <amount>\n\
         # does <monster> <move> cards <card> <amount>, into each opponent's discard pile\n\
         # unmoved <monster> <move> <the first statement no command matches>\n\
         # An amount is `number <n>` or `ascension <level> <at> <below>`.\n",
    );
    for (id, states) in moves {
        let read = |path: &Path| -> Result<Vec<Tok>, Error> {
            csharp::tokens(&error::read(path)?).map_err(Error::syntax(path.display()))
        };
        let path = by_name.get(&squash(id)).ok_or_else(|| Error::NoClass {
            kind: "monster",
            id: id.to_owned(),
        })?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let own = read(path)?;
        let mut classes = vec![own];
        match csharp::base_class(&classes[0], name).as_deref() {
            Some("MonsterModel") => {}
            Some(parent) => {
                let path = by_name
                    .get(&squash(parent))
                    .ok_or_else(|| Error::NotAMonster {
                        class: name.to_owned(),
                        parent: parent.to_owned(),
                    })?;
                classes.push(read(path)?);
            }
            None => return Err(Error::NoParent(name.to_owned())),
        }
        let mut members = HashMap::new();
        for class in classes.iter().rev() {
            members.extend(numbers(class));
        }
        let monster = Monster {
            classes,
            members,
            powers: &powers,
            cards: &cards,
        };

        writeln!(out, "monster {id}").unwrap();
        let machine = monster.states().map_err(Error::syntax(name))?;
        for state in states {
            writeln!(out, "move {id} {state}").unwrap();
            match monster.performs(machine.get(state)) {
                Ok(commands) => {
                    for command in commands {
                        writeln!(out, "does {id} {state} {command}").unwrap();
                    }
                }
                Err(blocked) => writeln!(out, "unmoved {id} {state} {blocked}").unwrap(),
            }
        }
    }
    Ok(out)
}

/// The numbers a class names at its own level: `Name => value;` and
/// `const Type Name = value;`. A field that is not constant is left out,
/// because a move can change it.
fn numbers(class: &[Tok]) -> HashMap<String, Vec<Tok>> {
    let mut out = HashMap::new();
    let mut depth = 0;
    for (at, tok) in class.iter().enumerate() {
        match tok {
            Tok::Punct("{") => depth += 1,
            Tok::Punct("}") => depth -= 1,
            _ => {}
        }
        if depth != 1 || at == 0 {
            continue;
        }
        let declared = tok.is("=>")
            || (tok.is("=")
                && class[..at]
                    .iter()
                    .rev()
                    .take_while(|tok| !(tok.is(";") || tok.is("{") || tok.is("}")))
                    .any(|tok| tok.is("const")));
        if !declared {
            continue;
        }
        let Some(name) = class[at - 1].ident() else {
            continue;
        };
        let value: Vec<Tok> = class[at + 1..]
            .iter()
            .take_while(|tok| !tok.is(";"))
            .cloned()
            .collect();
        out.insert(name.to_owned(), value);
    }
    out
}

impl Monster<'_> {
    /// The body of a method, from the class or else its parent.
    fn body(&self, name: &str) -> Result<Option<Vec<Tok>>, csharp::Error> {
        for class in &self.classes {
            if let Some(body) = csharp::body(class, name)? {
                return Ok(Some(body));
            }
        }
        Ok(None)
    }

    /// Every `MoveState` the state machine builds, by id.
    fn states(&self) -> Result<HashMap<String, MoveState>, csharp::Error> {
        let Some(body) = self.body("GenerateMoveStateMachine")? else {
            return Ok(HashMap::new());
        };
        let mut out = HashMap::new();
        for at in 0..body.len().saturating_sub(2) {
            if !(body[at].is("new") && body[at + 1].is("MoveState") && body[at + 2].is("(")) {
                continue;
            }
            let Some(args) = arguments(&body[at + 2..]) else {
                continue;
            };
            let Some([Tok::Str(id)]) = args.first().map(Vec::as_slice) else {
                continue;
            };
            let method = match args.get(1).map(Vec::as_slice) {
                Some([Tok::Ident(method)]) => Some(method.clone()),
                _ => None,
            };
            let attacks = args
                .iter()
                .skip(2)
                .filter(|arg| {
                    arg.iter()
                        .any(|tok| tok.ident().is_some_and(|name| name.contains("Attack")))
                })
                .map(|arg| match &arg[..] {
                    [Tok::Ident(new), Tok::Ident(intent), rest @ ..] if new == "new" => {
                        let inner = arguments(rest)?;
                        let spelled: Vec<String> = inner.iter().map(|arg| spell(arg)).collect();
                        match (intent.as_str(), &spelled[..]) {
                            ("SingleAttackIntent", [damage]) => Some(Advertised {
                                damage: damage.clone(),
                                hits: None,
                            }),
                            ("MultiAttackIntent", [damage, hits]) => Some(Advertised {
                                damage: damage.clone(),
                                hits: Some(hits.clone()),
                            }),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect();
            out.insert(id.clone(), MoveState { method, attacks });
        }
        Ok(out)
    }

    /// A number a move reads, followed through the members it names.
    fn amount(&self, toks: &[Tok]) -> Option<Amount> {
        if let Some(n) = literal(toks) {
            return Some(Amount::Number(n));
        }
        if let [Tok::Ident(name)] = toks {
            return self.amount(self.members.get(name)?);
        }
        let (root, calls) = chain(toks)?;
        let [(_, args)] = &calls[..] else { return None };
        match (root.as_str(), &args[..]) {
            ("AscensionHelper.GetValueIfAscension", [level, at, below]) => match &level[..] {
                [Tok::Ident(ty), Tok::Punct("."), Tok::Ident(level)] if ty == "AscensionLevel" => {
                    Some(Amount::Ascension {
                        level: level.clone(),
                        at: literal(at)?,
                        below: literal(below)?,
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// An amount that is never below zero, on any rung.
    fn count(&self, toks: &[Tok]) -> Option<Amount> {
        let amount = self.amount(toks)?;
        match amount {
            Amount::Number(n) if n >= 0 => Some(amount),
            Amount::Ascension { at, below, .. } if at >= 0 && below >= 0 => Some(amount),
            _ => None,
        }
    }

    /// The commands a move runs, or the statement none matches.
    fn performs(&self, state: Option<&MoveState>) -> Result<Vec<String>, Blocked> {
        let state = state.ok_or(Blocked::NoState)?;
        let method = state.method.as_deref().ok_or(Blocked::Lambda)?;
        let body = self
            .body(method)?
            .ok_or_else(|| Blocked::NoMethod(method.to_owned()))?;
        let mut locals = Locals::default();
        // The move's own parameter, `combatState.PlayerCreatures`.
        locals.plain.insert("targets".to_owned());
        let mut out = Vec::new();
        for stmt in super::statements(&body)? {
            if presentation(&stmt, &mut locals) {
                continue;
            }
            match self.command(state, &stmt, &locals) {
                Some(command) => out.push(command),
                None => return Err(Blocked::Statement(stmt.text())),
            }
        }
        Ok(out)
    }

    /// The command one statement of a move runs, as the dump writes it.
    fn command(&self, state: &MoveState, stmt: &Stmt, locals: &Locals) -> Option<String> {
        let Stmt::Simple(toks) = stmt else {
            return None;
        };
        let [Tok::Ident(wait), toks @ .., Tok::Punct(";")] = &toks[..] else {
            return None;
        };
        if wait != "await" {
            return None;
        }
        let (root, calls) = chain(toks)?;
        let spelled: Vec<String> = calls[0].1.iter().map(|arg| spell(arg)).collect();
        let generic = |prefix: &str| {
            root.strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix('<'))
                .and_then(|rest| rest.strip_suffix('>'))
                .map(squash)
        };
        match (root.as_str(), calls.len()) {
            ("DamageCmd.Attack", _) => self.attack(state, &calls, locals),
            ("CreatureCmd.GainBlock", 1) => match &spelled[..] {
                [creature, _, props, play]
                    if creature == "base.Creature"
                        && props == "ValueProp.Move"
                        && play == "null" =>
                {
                    Some(format!("block {}", self.count(&calls[0].1[1])?))
                }
                _ => None,
            },
            (_, 1) if root.starts_with("PowerCmd.Apply<") => {
                let power = self.powers.get(&generic("PowerCmd.Apply")?)?;
                let [context, target, _, applier, source] = &spelled[..] else {
                    return None;
                };
                if !(context == "new ThrowingPlayerChoiceContext()"
                    && applier == "base.Creature"
                    && source == "null")
                {
                    return None;
                }
                let target = match target.as_str() {
                    "targets" => "opponents",
                    "base.Creature" => "self",
                    _ => return None,
                };
                let amount = self.count(&calls[0].1[2])?;
                Some(format!("power {target} {power} {amount}"))
            }
            (_, 1) if root.starts_with("CardPileCmd.AddToCombatAndPreview<") => {
                let card = self
                    .cards
                    .get(&generic("CardPileCmd.AddToCombatAndPreview")?)?;
                match &spelled[..] {
                    [targets, pile, _, creator]
                        if targets == "targets"
                            && pile == "PileType.Discard"
                            && creator == "null" =>
                    {
                        Some(format!("cards {card} {}", self.count(&calls[0].1[2])?))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `DamageCmd.Attack(damage).FromMonster(this)`, when it hits for what
    /// the state's one attack intent advertises.
    fn attack(&self, state: &MoveState, calls: &[Call], locals: &Locals) -> Option<String> {
        let [Some(advertised)] = &state.attacks[..] else {
            return None;
        };
        let [damage] = &calls[0].1[..] else {
            return None;
        };
        // The same member, or the same number written twice: `1m` and `1`.
        let same = |spelled: &str, toks: &[Tok]| {
            spell(toks) == spelled
                || csharp::tokens(spelled)
                    .ok()
                    .and_then(|advertised| literal(&advertised))
                    .is_some_and(|n| literal(toks) == Some(n))
        };
        if !same(&advertised.damage, damage) {
            return None;
        }
        let (mut from, mut hits, mut run) = (false, None, false);
        for (at, (name, args)) in calls.iter().enumerate().skip(1) {
            let spelled: Vec<String> = args.iter().map(|arg| spell(arg)).collect();
            match (name.as_str(), &spelled[..]) {
                ("FromMonster", [this]) if this == "this" && !from => from = true,
                ("WithHitCount", [_]) if hits.is_none() => hits = Some(&args[0]),
                ("Execute", [context]) if context == "null" && at == calls.len() - 1 => run = true,
                (look, _) if LOOKS.contains(&look) => {
                    if !args.iter().all(|arg| pure(arg, locals)) {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        let hits_match = match (hits, &advertised.hits) {
            (None, None) => true,
            (Some(hits), Some(advertised)) => same(advertised, hits),
            _ => false,
        };
        (from && run && hits_match).then(|| "attack".to_owned())
    }
}

/// How many monsters' moves the dump lets the generator write, for the
/// summary.
pub fn tally(dump: &str) -> String {
    let count = |kind: &str| {
        dump.lines()
            .filter(|line| line.split_whitespace().next() == Some(kind))
            .count()
    };
    format!(
        "{} of {} moves matched",
        count("move") - count("unmoved"),
        count("move")
    )
}
