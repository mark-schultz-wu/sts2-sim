//! What each card does when it is played or upgraded, from `card_plays.txt`.
//!
//! The dump names every card. A card whose method matched gets the commands it
//! listed, spelled the way a hand port spells them. Any other card gets
//! `unimplemented!()` and a comment naming the statement that stopped it, and
//! the build's `behaviour/` ports it by hand.
//!
//! One file per card pool, beside the table's own.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::DumpError;
use super::cards::{POOLS, card_path};
use super::split::Split;
use super::{Build, Dump, camel, sanitize, version_module};
use crate::error::FileError;

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
    #[error("{what} of {words:?}")]
    Unread {
        what: &'static str,
        words: Vec<String>,
    },
    #[error("{at}: {source}")]
    In { at: String, source: Box<Error> },
    #[error(transparent)]
    NotAPool(#[from] super::cards::NotAPool),
    #[error("card_plays.txt names {0}, which is not a card")]
    NotACard(String),
    #[error("card_plays.txt has a {0} row")]
    UnknownRow(String),
    #[error("card_plays.txt names {named} cards, and cards.txt {cards}")]
    Count { named: usize, cards: usize },
}

/// What the dump says of one card's method.
#[derive(Default)]
struct Method {
    /// The rows it matched, each split into words.
    rows: Vec<Vec<String>>,
    /// The statement nothing matched, when one did not.
    blocked: Option<String>,
}

/// One card's play and upgrade.
#[derive(Default)]
struct Card {
    play: Method,
    upgrade: Method,
    /// Whether it acts when a turn ends with it in hand.
    turn_end: bool,
}

/// The number an amount names, as a play reads it.
fn amount(words: &[String]) -> Result<String, Error> {
    match words {
        [kind, key] if kind == "var" => Ok(format!("fight.var(CardVar::{})", sanitize(key))),
        [kind, n] if kind == "number" => Ok(DumpError::number::<i64>("a number", n)?.to_string()),
        _ => Err(Error::Unread {
            what: "an amount",
            words: words.to_vec(),
        }),
    }
}

/// `Scaling` for a command's `ValueProp`.
///
/// Powered is `Move` without `Unpowered`, which `IsPoweredAttack` and
/// `IsPoweredCardOrMonsterMoveBlock` both ask.
fn scaling(props: &str) -> Result<&'static str, Error> {
    match props {
        "Move" => Ok("Scaling::Dynamic"),
        "Move+Unpowered" | "Unpowered" => Ok("Scaling::Static"),
        other => Err(Error::Unread {
            what: "the props",
            words: vec![other.to_owned()],
        }),
    }
}

/// One hit, from the acting creature to `target`, for the card's own damage.
fn hit(out: &mut String, key: &str, scaling: &str) {
    writeln!(
        out,
        "let attack = fight.resolve(Effect::Attack {{ dealer: fight.actor(), target, damage: Amount::Var(CardVar::{}), scaling: {scaling} }});\nfight.apply(attack);",
        sanitize(key)
    )
    .unwrap();
}

/// The Rust for one command a play runs.
fn command(out: &mut String, words: &[String]) -> Result<(), Error> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match &words[..] {
        // One `AttackCommand`, which raises its own hooks around its hits.
        ["attack", target, key, props, hits @ ..] => {
            let scaling = scaling(props)?;
            let hits: Vec<String> = hits.iter().map(|w| w.to_string()).collect();
            let once = hits == ["number", "1"];
            writeln!(out, "fight.attack({scaling}, |fight| {{").unwrap();
            if !once {
                writeln!(out, "for _ in 0..{} {{", amount(&hits)?).unwrap();
            }
            match *target {
                "chosen" => hit(out, key, scaling),
                // The list is taken before the first hit lands, and again for
                // each hit after it. Each hit is one `CreatureCmd.Damage` over
                // the lot.
                "opponents" => {
                    writeln!(out, "let targets = fight.opponents();").unwrap();
                    writeln!(out, "fight.damage_command(|fight| {{").unwrap();
                    writeln!(out, "for target in targets {{").unwrap();
                    hit(out, key, scaling);
                    writeln!(out, "}}").unwrap();
                    writeln!(out, "}});").unwrap();
                }
                // Drawn for each hit, and the hits stop once no opponent is
                // left to draw.
                "random" if once => {
                    writeln!(out, "if let Some(target) = fight.random_target() {{").unwrap();
                    hit(out, key, scaling);
                    writeln!(out, "}}").unwrap();
                }
                "random" => {
                    writeln!(out, "let Some(target) = fight.random_target() else {{ break }};").unwrap();
                    hit(out, key, scaling);
                }
                other => {
                    return Err(Error::Unread {
                        what: "an attack at",
                        words: vec![other.to_string()],
                    });
                }
            }
            if !once {
                writeln!(out, "}}").unwrap();
            }
            writeln!(out, "}});").unwrap();
        }
        ["block", key, props] => writeln!(
            out,
            "let block = fight.resolve(Effect::GainBlock {{ recipient: fight.actor(), amount: Amount::Var(CardVar::{}), scaling: {} }});\nfight.apply(block);",
            sanitize(key),
            scaling(props)?
        )
        .unwrap(),
        ["power", target, power, rest @ ..] => {
            let rest: Vec<String> = rest.iter().map(|w| w.to_string()).collect();
            let power = format!("Power::{}", camel(power));
            // A power's amount is the number as printed. The power runs its
            // own hooks where it lands, so none of the number's class runs.
            let amount = match rest.as_slice() {
                [kind, key] if kind == "var" => {
                    format!("PowerAmount::Counted(fight.power_var(CardVar::{}))", sanitize(key))
                }
                _ => format!("PowerAmount::Counted({})", amount(&rest)?),
            };
            match *target {
                "chosen" => writeln!(out, "fight.apply_power(target, {power}, {amount});"),
                "self" => writeln!(out, "fight.apply_to_actor({power}, {amount});"),
                "opponents" => writeln!(out, "fight.apply_to_opponents({power}, {amount});"),
                // The amount is read once, before the first lands.
                "held" => writeln!(
                    out,
                    "let amount = {amount};\nfor &target in &held {{\nfight.apply_power(target, {power}, amount);\n}}"
                ),
                other => {
                    return Err(Error::Unread {
                        what: "a power on",
                        words: vec![other.to_string()],
                    });
                }
            }
            .unwrap();
        }
        ["channel", orb] => writeln!(out, "fight.channel(Orb::{});", camel(orb)).unwrap(),
        // `HittableEnemies`, taken now for a command further on.
        ["hold", "opponents"] => writeln!(out, "let held = fight.opponents();").unwrap(),
        ["summon", rest @ ..] => {
            let rest: Vec<String> = rest.iter().map(|w| w.to_string()).collect();
            writeln!(out, "fight.summon_osty({});", amount(&rest)?).unwrap();
        }
        [what @ ("draw" | "energy" | "stars" | "forge"), rest @ ..] => {
            let rest: Vec<String> = rest.iter().map(|w| w.to_string()).collect();
            let amount = amount(&rest)?;
            match *what {
                "draw" => writeln!(out, "fight.draw({amount});"),
                "energy" => writeln!(out, "fight.modify_energy({amount});"),
                "stars" => writeln!(out, "fight.gain_stars({amount});"),
                // `ForgeCmd.Forge` adds to the damage of every Sovereign Blade.
                _ => writeln!(
                    out,
                    "fight.forge({}, {amount});",
                    card_path("TOKEN_CARD_POOL", "SOVEREIGN_BLADE")?
                ),
            }
            .unwrap();
        }
        _ => {
            return Err(Error::Unread {
                what: "a play",
                words: words.iter().map(|w| w.to_string()).collect(),
            });
        }
    }
    Ok(())
}

/// The Rust for one change an upgrade makes.
fn change(words: &[String]) -> Result<String, Error> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let number = |n: &str| DumpError::number::<i64>("a number", n);
    Ok(match &words[..] {
        ["raise", key, by] => format!("Change::Raise(CardVar::{}, {})", sanitize(key), number(by)?),
        ["cost", by] => format!("Change::ChangeCost(CostPool::Energy, {})", number(by)?),
        ["keyword", keyword] => format!("Change::AddKeyword(CardKeyword::{})", sanitize(keyword)),
        ["unkeyword", keyword] => {
            format!("Change::RemoveKeyword(CardKeyword::{})", sanitize(keyword))
        }
        _ => {
            return Err(Error::Unread {
                what: "an upgrade",
                words: words.iter().map(|w| w.to_string()).collect(),
            });
        }
    })
}

/// A statement as a comment, cut to a line's worth.
fn comment(out: &mut String, stmt: &str) {
    let mut cut: String = stmt.chars().take(90).collect();
    if cut.len() < stmt.len() {
        cut.push_str(" ...");
    }
    writeln!(out, "// `{cut}`").unwrap();
}

/// Every card's play and upgrade, one module per pool.
pub fn generate(root: &Path, build: &Build) -> Result<(), Error> {
    let mut dump = Dump::read_any("card_plays.txt")?;
    super::mad_science::expand(&mut dump);
    // No content hash: the dump is read from the assembly, and the game
    // works the hash out at runtime.
    if (
        &dump.build.version,
        &dump.build.commit,
        dump.build.assembly_hash,
    ) != (&build.version, &build.commit, build.assembly_hash)
    {
        return Err(DumpError::ReadFrom {
            dump: "card_plays.txt",
            read: Box::new(dump.build),
            tables: Box::new(build.clone()),
        }
        .into());
    }

    let mut cards_dump = Dump::read("cards.txt")?;
    super::mad_science::expand(&mut cards_dump);
    let mut cards: BTreeMap<String, (String, Card)> = BTreeMap::new();
    for row in cards_dump.of("card") {
        cards.insert(row[1].clone(), (row[2].clone(), Card::default()));
    }
    let mut named = 0;
    for row in &dump.rows {
        let (kind, id) = (row[0].as_str(), &row[1]);
        let (_, card) = cards
            .get_mut(id)
            .ok_or_else(|| Error::NotACard(id.clone()))?;
        match kind {
            "card" => named += 1,
            "play" => card.play.rows.push(row[2..].to_vec()),
            "upgrade" => card.upgrade.rows.push(row[2..].to_vec()),
            "unplayed" => card.play.blocked = Some(row[2..].join(" ")),
            "unupgraded" => card.upgrade.blocked = Some(row[2..].join(" ")),
            "turnend" => card.turn_end = true,
            other => return Err(Error::UnknownRow(other.to_owned())),
        }
    }
    if named != cards.len() {
        return Err(Error::Count {
            named,
            cards: cards.len(),
        });
    }

    // The build's type, `V0_107_1` for the module `v0_107_1`.
    let version = version_module().map_err(DumpError::from)?.to_uppercase();
    let mut split = Split::new(root, "card_plays", "card_plays.txt", &dump.build)?;
    // Each pool, and whether it has a play that can stand still.
    let mut pausing: Vec<(&str, &str, bool)> = Vec::new();
    for (pool_id, module, enum_name, _) in POOLS {
        // In the order the pool's enum declares them, which is the dump's.
        let mut members: Vec<(String, &Card)> = Vec::new();
        for row in cards_dump.of("card").filter(|row| row[2] == pool_id) {
            // `Cards::Regent(RegentCard::Strike)`, of which a match on the
            // pool's own enum wants the inside.
            let path = card_path(pool_id, &row[1])?;
            let variant = path
                .split_once('(')
                .map(|(_, inner)| inner.trim_end_matches(')').to_owned())
                .unwrap_or(path);
            members.push((variant, &cards[&row[1]].1));
        }

        let out = split.body(module);
        writeln!(out, "type V = super::super::super::{version};\n").unwrap();

        // Each card the dump matched, as the commands it runs. A card with
        // one command never stands still on a question, so `play` runs it.
        // A card with more is the pool's coroutine's.
        let mut plays: Vec<(&str, Vec<&Vec<String>>, bool)> = Vec::new();
        let mut blocked: Vec<(&str, String)> = Vec::new();
        for (variant, card) in &members {
            if let Some(why) = &card.play.blocked {
                let mut note = String::new();
                comment(&mut note, why);
                blocked.push((variant, note));
                continue;
            }
            // A list nothing reads is a list the C# only animates over.
            let read = card
                .play
                .rows
                .iter()
                .any(|row| row.get(1).is_some_and(|t| t == "held"));
            let rows: Vec<&Vec<String>> = card
                .play
                .rows
                .iter()
                .filter(|row| read || row.as_slice() != ["hold", "opponents"])
                .collect();
            // A list taken before a command that can wait, and read after it,
            // would be taken again when the play goes on. So it is ported by
            // hand.
            let held = rows
                .iter()
                .position(|row| row.as_slice() == ["hold", "opponents"]);
            let reads = rows
                .iter()
                .rposition(|row| row.get(1).is_some_and(|t| t == "held"));
            if let (Some(held), Some(reads)) = (held, reads)
                && reads > held
            {
                blocked.push((
                    variant,
                    "// A list held across a command that can wait on a question.\n".to_owned(),
                ));
                continue;
            }
            let targets = card
                .play
                .rows
                .iter()
                .any(|row| row.get(1).is_some_and(|t| t == "chosen"));
            plays.push((variant, rows, targets));
        }
        let (single, multiple): (Vec<_>, Vec<_>) =
            plays.iter().partition(|(_, rows, _)| rows.len() <= 1);
        let unblocked = |variant: &str| blocked.iter().all(|(v, _)| *v != variant);
        debug_assert!(plays.iter().all(|(v, ..)| unblocked(v)));

        // `play`, for the cards with one command and those the dump left.
        let reads = single.iter().any(|(_, rows, _)| !rows.is_empty());
        let fight = if reads { "fight" } else { "_fight" };
        match multiple.is_empty() {
            true => writeln!(
                out,
                "/// `OnPlay`, for every card whose statements the dump matched. Each\n\
                 /// is one command, which never stands still on a question.\n\
                 ///\n\
                 /// Any other card panics, and `behaviour/` ports it by hand.\n\
                 pub fn play<F: Fight<V>>(card: {enum_name}, {fight}: &mut F) {{\n\
                 match card {{"
            ),
            false => writeln!(
                out,
                "/// `OnPlay` of a card whose play is one command, which never stands\n\
                 /// still on a question. `None` for a card with more, which [`paused`]\n\
                 /// plays.\n\
                 ///\n\
                 /// Any other card panics, and `behaviour/` ports it by hand.\n\
                 pub fn play<F: Fight<V>>(card: {enum_name}, {fight}: &mut F) -> Option<()> {{\n\
                 match card {{"
            ),
        }
        .unwrap();
        let done = if multiple.is_empty() { "" } else { "Some(())" };
        for (variant, note) in &blocked {
            out.push_str(note);
            writeln!(
                out,
                "{variant} => unimplemented!(\"what {{card:?}} does when it is played\"),"
            )
            .unwrap();
        }
        for (variant, rows, targets) in &single {
            writeln!(out, "{variant} => {{").unwrap();
            if *targets {
                writeln!(out, "let target = fight.target();").unwrap();
            }
            for row in rows {
                command(out, row).map_err(|source| Error::In {
                    at: variant.to_string(),
                    source: Box::new(source),
                })?;
            }
            writeln!(out, "{done}\n}}").unwrap();
        }
        if !multiple.is_empty() {
            let names: Vec<&str> = multiple.iter().map(|(v, ..)| *v).collect();
            writeln!(out, "{} => None,", names.join(" | ")).unwrap();
        }
        writeln!(out, "}}\n}}\n").unwrap();

        // `paused`, for the cards with more than one command.
        if !multiple.is_empty() {
            writeln!(
                out,
                "/// `OnPlay` of a card whose play is more than one command, from its\n\
                 /// start to its end. A command that leaves a question waiting stands\n\
                 /// the rest still, as the game's `await` does. Each step is lent the\n\
                 /// fight, and after a question the answer.\n\
                 #[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]\n\
                 #[derive(Debug, Clone, PartialEq, Eq, Hash)]\n\
                 pub fn paused<K: FightKind<V>>(card: {enum_name}) {{\n\
                 match card {{"
            )
            .unwrap();
            for (variant, rows, targets) in &multiple {
                writeln!(out, "{variant} => {{").unwrap();
                if *targets {
                    writeln!(
                        out,
                        "let target: sts2_core::CombatId = resumed!().fight.target();"
                    )
                    .unwrap();
                }
                let last = rows.len() - 1;
                for (at, row) in rows.iter().enumerate() {
                    writeln!(out, "{{\nlet fight = &mut *resumed!().fight;").unwrap();
                    command(out, row).map_err(|source| Error::In {
                        at: variant.to_string(),
                        source: Box::new(source),
                    })?;
                    writeln!(out, "}}").unwrap();
                    if at < last {
                        writeln!(
                            out,
                            "if resumed!().fight.waiting() {{\nyield_!(Ask::Waiting);\n}}"
                        )
                        .unwrap();
                    }
                }
                writeln!(out, "}}").unwrap();
            }
            writeln!(
                out,
                "card => unreachable!(\"{{card:?}} is not a play of more than one command\"),\n}}\n}}\n"
            )
            .unwrap();
        }
        pausing.push((module, enum_name, !multiple.is_empty()));

        writeln!(
            out,
            "/// `OnUpgrade`, which writes on the copy being upgraded, for every card\n\
             /// whose statements the dump matched.\n\
             pub fn upgrade(card: {enum_name}) -> &'static [Change<V>] {{\n\
             match card {{"
        )
        .unwrap();
        for (variant, card) in &members {
            if let Some(blocked) = &card.upgrade.blocked {
                comment(out, blocked);
                writeln!(
                    out,
                    "{variant} => unimplemented!(\"card upgrade: {{card:?}}\"),"
                )
                .unwrap();
                continue;
            }
            let changes = card
                .upgrade
                .rows
                .iter()
                .map(|row| change(row))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| Error::In {
                    at: variant.to_string(),
                    source: Box::new(source),
                })?;
            writeln!(out, "{variant} => &[{}],", changes.join(", ")).unwrap();
        }
        writeln!(out, "}}\n}}\n").unwrap();

        // `OnTurnEndInHand` runs between pile moves the generator does not
        // write, so every card with one is ported by hand.
        let acting: Vec<&str> = members
            .iter()
            .filter(|(_, card)| card.turn_end)
            .map(|(variant, _)| variant.as_str())
            .collect();
        writeln!(
            out,
            "/// `OnTurnEndInHand`, for a card left in hand when its owner's turn\n\
             /// ends. Nothing, for a card that does not say it acts there."
        )
        .unwrap();
        match acting.is_empty() {
            true => writeln!(
                out,
                "pub fn on_turn_end<F: Fight<V>>(_card: {enum_name}, _fight: &mut F) {{}}"
            )
            .unwrap(),
            false => writeln!(
                out,
                "pub fn on_turn_end<F: Fight<V>>(card: {enum_name}, _fight: &mut F) {{\n\
                 match card {{\n\
                 {} => unimplemented!(\"what {{card:?}} does when a turn ends with it in hand\"),\n\
                 _ => {{}}\n\
                 }}\n\
                 }}",
                acting.join(" | ")
            )
            .unwrap(),
        }
    }

    let mut shape = super::banner(
        "What each card does when it is played or upgraded, one module per pool.",
        "card_plays.txt",
        &dump.build,
    );
    shape.push('\n');
    for (_, module, _, _) in POOLS {
        writeln!(shape, "pub mod {module};").unwrap();
    }
    shape.push_str(&dispatch(&version, &pausing));
    split.finish(
        &shape,
        |module| {
            let (pool_id, ..) = POOLS
                .iter()
                .find(|(_, m, _, _)| *m == module)
                .expect("every bucket is a pool");
            format!("What the cards in `{pool_id}` do when played or upgraded.")
        },
        &[
            (
                "super::super::cards",
                &[
                    "CardKeyword",
                    "CardVar",
                    "Cards",
                    "ColorlessCard",
                    "CurseCard",
                    "DefectCard",
                    "DeprecatedCard",
                    "EventCard",
                    "IroncladCard",
                    "NecrobinderCard",
                    "QuestCard",
                    "RegentCard",
                    "SilentCard",
                    "StatusCard",
                    "TokenCard",
                ],
            ),
            ("crate::behaviour", &["Played"]),
            ("crate::behaviour::paused", &["Ask", "Lent"]),
            ("super::super::orbs", &["Orb"]),
            ("super::super::powers", &["Power"]),
            (
                "sts2_core",
                &[
                    "Amount",
                    "Change",
                    "CostPool",
                    "Effect",
                    "Fight",
                    "FightKind",
                    "PowerAmount",
                    "Scaling",
                ],
            ),
        ],
    )?;
    Ok(())
}

/// The parent's `Paused`, `play` and `resume`, over every pool. `pausing`
/// names each pool's module and enum, and whether it has a play that can
/// stand still.
fn dispatch(version: &str, pausing: &[(&str, &str, bool)]) -> String {
    let pool = |module: &str| camel(module);
    let mut out = format!(
        "\ntype V = super::super::{version};\n\n\
         /// A generated play standing still on a question, by its pool.\n\
         #[derive(Debug, Clone, PartialEq, Eq, Hash)]\n\
         pub enum Paused<K: sts2_core::FightKind<V>> {{\n"
    );
    for (module, _, pauses) in pausing {
        if *pauses {
            writeln!(out, "{}({module}::paused::State<K>),", pool(module)).unwrap();
        }
    }
    out.push_str(
        "}\n\n\
         /// `OnPlay`, for every card whose statements the dump matched.\n\
         ///\n\
         /// Any other card panics, and `behaviour/` ports it by hand.\n\
         pub fn play<'a, K: sts2_core::FightKind<V>>(\n\
         card: super::cards::Cards,\n\
         fight: &mut K::Fight<'a>,\n\
         ) -> crate::behaviour::Played<V, K> {\n\
         match card {\n",
    );
    for (module, enum_name, pauses) in pausing {
        let variant = enum_name.trim_end_matches("Card");
        match pauses {
            true => writeln!(
                out,
                "super::cards::Cards::{variant}(card) => match {module}::play(card, fight) {{\n\
                 Some(()) => crate::behaviour::Played::Done,\n\
                 None => {{\n\
                 let mut play = {module}::paused::<K>(card);\n\
                 let state = play.start_with(crate::behaviour::paused::Lent {{ fight, taken: None }});\n\
                 settle(Paused::{}(play), state)\n\
                 }}\n\
                 }},",
                pool(module)
            ),
            false => writeln!(
                out,
                "super::cards::Cards::{variant}(card) => {{\n\
                 {module}::play(card, fight);\n\
                 crate::behaviour::Played::Done\n\
                 }}"
            ),
        }
        .unwrap();
    }
    out.push_str(
        "}\n}\n\n\
         /// A generated play that stood still, with the answer it waited for.\n\
         pub fn resume<'a, K: sts2_core::FightKind<V>>(\n\
         fight: &mut K::Fight<'a>,\n\
         paused: Paused<K>,\n\
         taken: sts2_core::Taken<V>,\n\
         ) -> crate::behaviour::Played<V, K> {\n\
         let lent = crate::behaviour::paused::Lent { fight, taken: Some(taken) };\n\
         match paused {\n",
    );
    for (module, _, pauses) in pausing {
        if *pauses {
            let name = pool(module);
            writeln!(
                out,
                "Paused::{name}(mut play) => {{\n\
                 let state = diapause::Coroutine::resume(&mut play, lent);\n\
                 settle(Paused::{name}(play), state)\n\
                 }}"
            )
            .unwrap();
        }
    }
    out.push_str(
        "}\n}\n\n\
         /// How far a step of `play` got.\n\
         fn settle<K: sts2_core::FightKind<V>>(\n\
         play: Paused<K>,\n\
         state: diapause::CoroutineState<crate::behaviour::paused::Ask<V>, ()>,\n\
         ) -> crate::behaviour::Played<V, K> {\n\
         match state {\n\
         diapause::CoroutineState::Complete(()) => crate::behaviour::Played::Done,\n\
         diapause::CoroutineState::Yielded(ask) => {\n\
         ask.at(crate::behaviour::Resume::Build(play.into()))\n\
         }\n\
         }\n\
         }\n",
    );
    out
}
