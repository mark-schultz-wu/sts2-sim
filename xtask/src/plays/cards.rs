//! What each card's `OnPlay` and `OnUpgrade` do, for `dump cardplaydump`.
//!
//! The list is [`game`] for a play and [`upgrade`] for an upgrade. The xtask
//! README says what else a card overrides that stops its play.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use super::csharp::{self, Stmt, Tok, spell};
use super::{
    Blocked, Call, Error, LOOKS, Locals, arguments, chain, literal, presentation, pure, squash,
};
use crate::error;

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
    /// `TargetingRandomOpponents`: one living opponent for each hit.
    Random,
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Target::Chosen => "chosen",
            Target::Actor => "self",
            Target::Opponents => "opponents",
            Target::Held => "held",
            Target::Random => "random",
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
    /// Orb ids, by the name of the class that is each one.
    orbs: HashMap<String, String>,
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
pub fn cards(game: &Path, version: &str, version_dir: &Path) -> Result<String, Error> {
    let (release, source) = super::source(game, version)?;
    let build = Build::read(&source, version_dir)?;
    let by_name = super::classes(&source, "MegaCrit.Sts2.Core.Models.Cards")?;

    let cards_txt = error::read(&version_dir.join("dumps/cards.txt"))?;
    let ids: Vec<&str> = cards_txt
        .lines()
        .filter_map(|line| line.strip_prefix("card "))
        .filter_map(|row| row.split_whitespace().next())
        .collect();

    let mut out = super::header("card plays", &release);
    out.push_str(
        "# card <id>\n\
         # play <card> attack <chosen|opponents> <damage var> <props> <hits>\n\
         # play <card> block <block var> <props>\n\
         # play <card> hold opponents, which a later `held` reads as they stood\n\
         # play <card> power <chosen|self|opponents|held> <power> <amount>\n\
         # play <card> <draw|energy|stars|forge> <amount>\n\
         # play <card> channel <orb>\n\
         # play <card> summon <amount>, which is Osty for the player\n\
         # unplayed <card> <what the first statement no command matches is>\n\
         # upgrade <card> raise <var> <by>\n\
         # upgrade <card> cost <by>\n\
         # upgrade <card> keyword <keyword>\n\
         # upgrade <card> unkeyword <keyword>\n\
         # unupgraded <card> <what the first statement no change matches is>\n\
         # turnend <card>, which acts when a turn ends with it in hand\n\
         # An amount is `var <key>` or `number <n>`.\n",
    );
    for id in ids {
        let path = by_name.get(&squash(id)).ok_or_else(|| Error::NoClass {
            kind: "card",
            id: id.to_owned(),
        })?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let class = csharp::tokens(&error::read(path)?).map_err(Error::syntax(name))?;
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
                csharp::method(&class, "OnPlay").map_err(Error::syntax(name))?,
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
            csharp::method(&class, "OnUpgrade").map_err(Error::syntax(name))?,
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

impl Build {
    fn read(source: &Path, version_dir: &Path) -> Result<Build, Error> {
        // `public BlockVar Block => (BlockVar)_vars["Block"];`
        let set = source.join("MegaCrit.Sts2.Core.Localization.DynamicVars/DynamicVarSet.cs");
        let toks = csharp::tokens(&error::read(&set)?).map_err(Error::syntax("DynamicVarSet"))?;
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

        let cards = error::read(&version_dir.join("dumps/cards.txt"))?;
        let mut vars: HashMap<String, HashMap<String, (String, bool)>> = HashMap::new();
        for row in cards.lines().filter_map(|line| line.strip_prefix("var ")) {
            let f: Vec<&str> = row.split_whitespace().collect();
            let [card, key, base, calculated, class] = f[..] else {
                return Err(Error::VarRow {
                    fields: f.len(),
                    row: row.to_owned(),
                });
            };
            let printed = calculated == "0" && !base.contains('.');
            vars.entry(card.to_owned())
                .or_default()
                .insert(key.to_owned(), (class.to_owned(), printed));
        }

        let ids = |dump: &str, kind: &str| -> Result<HashMap<String, String>, Error> {
            let text = error::read(&version_dir.join("dumps").join(dump))?;
            Ok(text
                .lines()
                .filter_map(|line| line.strip_prefix(kind))
                .filter_map(|row| row.split_whitespace().next())
                .map(|id| (squash(id), id.to_owned()))
                .collect())
        };

        Ok(Build {
            properties,
            vars,
            powers: ids("powers.txt", "power ")?,
            orbs: ids("orbs.txt", "orb ")?,
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

    /// The class of one of this card's vars.
    fn class(&self, key: &str) -> Option<&str> {
        self.build
            .vars
            .get(self.id)?
            .get(key)
            .map(|(class, _)| class.as_str())
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
            // `?.` changes nothing in a play, which has a fight to stand in.
            "base.CombatState.HittableEnemies" | "base.CombatState?.HittableEnemies" => {
                Some(Target::Opponents)
            }
            local if locals.held.as_deref() == Some(local) => Some(Target::Held),
            _ => None,
        }
    }
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
    matches!(
        spell(value).as_str(),
        "base.CombatState.HittableEnemies" | "base.CombatState?.HittableEnemies"
    )
    .then_some(())?;
    toks[assign - 1].ident().map(String::from)
}

/// The commands a card's `OnPlay` runs, or the statement none matches.
fn plays(card: &Card, body: Option<Vec<Tok>>) -> Result<Vec<String>, Blocked> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    let stmts = super::statements(&body)?;
    let mut locals = Locals::default();
    let mut out = Vec::new();
    for stmt in &stmts {
        // `enemies = base.CombatState.HittableEnemies`, which a later command
        // reads as the list stood here.
        if let Some(name) = holds(stmt) {
            if locals.held.is_some() {
                return Err(Blocked::Statement(stmt.head()));
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
            None => return Err(Blocked::Statement(stmt.head())),
        }
    }
    Ok(out)
}

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
        (_, 1) if root.starts_with("OrbCmd.Channel<") && root.ends_with('>') => {
            let class = &root["OrbCmd.Channel<".len()..root.len() - 1];
            let orb = card.build.orbs.get(&squash(class))?;
            match &first[..] {
                [context, owner]
                    if spell(context) == "choiceContext" && spell(owner) == "base.Owner" =>
                {
                    Some(format!("channel {orb}"))
                }
                _ => None,
            }
        }
        ("OstyCmd.Summon", 1) => match &first[..] {
            [context, owner, amount, this]
                if spell(context) == "choiceContext"
                    && spell(owner) == "base.Owner"
                    && spell(this) == "this" =>
            {
                Some(format!("summon {}", card.number(amount)?))
            }
            _ => None,
        },
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
    // A printed `DamageVar`'s base value, or a `CalculatedDamageVar` handed
    // over whole, which works itself out against each creature it hits.
    let key = match card.var(damage)? {
        (key, [Tok::Punct("."), Tok::Ident(value)])
            if value == "BaseValue" && card.printed(&key)? == "DamageVar" =>
        {
            key
        }
        (key, []) if card.class(&key)? == "CalculatedDamageVar" => key,
        _ => return None,
    };

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
            ("TargetingRandomOpponents", [state])
                if state == "base.CombatState" && target.is_none() =>
            {
                target = Some(Target::Random);
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
fn upgrades(card: &Card, body: Option<Vec<Tok>>) -> Result<Vec<String>, Blocked> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for stmt in csharp::statements(&body)? {
        match upgrade(card, &stmt) {
            Some(change) => out.push(change),
            None => return Err(Blocked::Statement(stmt.head())),
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
        ("RemoveKeyword" | "base.RemoveKeyword", [keyword]) => match &keyword[..] {
            [Tok::Ident(ty), Tok::Punct("."), Tok::Ident(keyword)] if ty == "CardKeyword" => {
                Some(format!("unkeyword {keyword}"))
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
