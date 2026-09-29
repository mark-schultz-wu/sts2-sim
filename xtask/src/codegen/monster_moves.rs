//! What each monster's moves do, from `monster_moves.txt`.
//!
//! A move whose statements the dump matched gets the commands it listed,
//! spelled the way a hand port spells them. Any other move gets
//! `unimplemented!()` under a comment naming the statement that stopped it,
//! and the build's `behaviour/` ports it by hand.
//!
//! Filed the way the monster table is: one file per act a monster is met in.
//!
//! A move's attack can make the player answer something, as a relic's draw
//! that shuffles on a hit does. The game's move stands still on its `await`
//! until the answer comes. So a move with more to do after an attack stops
//! there, names the step it goes on from, and is called again with it. Every
//! other command takes a `ThrowingPlayerChoiceContext`, which throws on a
//! question, so none of them is a place to stop.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::DumpError;
use super::cards::card_path;
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
    #[error("{0} is not a card")]
    NotACard(String),
    #[error("a move of {0} before the monster")]
    BeforeMonster(String),
    #[error("{monster} {id} before its move row")]
    BeforeMove { monster: String, id: String },
    #[error("monster_moves.txt has a row {0:?}")]
    UnknownRow(Vec<String>),
}

/// What the dump says of one move.
#[derive(Default)]
struct Move {
    id: String,
    /// The rows it matched, each split into words.
    rows: Vec<Vec<String>>,
    /// The statement nothing matched, when one did not.
    blocked: Option<String>,
}

/// The number an amount names, as a move reads it.
fn amount(words: &[String]) -> Result<String, Error> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let number = |n: &str| DumpError::number::<i64>("a number", n).map(|n| n.to_string());
    match &words[..] {
        ["number", n] => Ok(number(n)?),
        ["ascension", level, at, below] => Ok(format!(
            "match fight.ascension().has(AscensionLevel::{}) {{ true => {}, false => {} }}",
            sanitize(level),
            number(at)?,
            number(below)?
        )),
        _ => Err(Error::Unread {
            what: "an amount",
            words: words.iter().map(|w| w.to_string()).collect(),
        }),
    }
}

/// The Rust for one command a move runs.
fn command(
    out: &mut String,
    words: &[String],
    cards: &BTreeMap<String, String>,
) -> Result<(), Error> {
    match words {
        [attack] if attack == "attack" => writeln!(out, "fight.attack_opponents();").unwrap(),
        [power, target, id, rest @ ..] if power == "power" => {
            let call = match target.as_str() {
                "self" => "apply_to_actor",
                "opponents" => "apply_to_opponents",
                other => {
                    return Err(Error::Unread {
                        what: "a power on",
                        words: vec![other.to_owned()],
                    });
                }
            };
            writeln!(
                out,
                "fight.{call}(Power::{}, PowerAmount::Counted({}));",
                camel(id),
                amount(rest)?
            )
            .unwrap();
        }
        [block, rest @ ..] if block == "block" => writeln!(
            out,
            "let amount = {};\nlet block = fight.resolve(Effect::GainBlock {{ recipient: fight.actor(), amount: Amount::Number(amount), scaling: Scaling::Dynamic }});\nfight.apply(block);",
            amount(rest)?
        )
        .unwrap(),
        [kind, card, rest @ ..] if kind == "cards" => {
            let pool = cards
                .get(card)
                .ok_or_else(|| Error::NotACard(card.clone()))?;
            writeln!(
                out,
                "fight.give_cards({}, {});",
                card_path(pool, card)?,
                amount(rest)?
            )
            .unwrap();
        }
        _ => {
            return Err(Error::Unread {
                what: "a move",
                words: words.to_vec(),
            });
        }
    }
    Ok(())
}

/// A statement as a comment, cut to a line's worth.
fn comment(out: &mut String, stmt: &str) {
    let mut cut: String = stmt.chars().take(90).collect();
    if cut.len() < stmt.len() {
        cut.push_str(" ...");
    }
    writeln!(out, "// `{cut}`").unwrap();
}

/// Every monster's moves, one module per act.
pub fn generate(root: &Path, build: &Build) -> Result<(), Error> {
    let dump = Dump::read_any("monster_moves.txt")?;
    if (
        &dump.build.version,
        &dump.build.commit,
        dump.build.assembly_hash,
    ) != (&build.version, &build.commit, build.assembly_hash)
    {
        return Err(DumpError::ReadFrom {
            dump: "monster_moves.txt",
            read: Box::new(dump.build),
            tables: Box::new(build.clone()),
        }
        .into());
    }

    // Where each monster's table row is filed, which is where its moves go.
    let table = Dump::read("monsters.txt")?;
    let mut act_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in table.of("act") {
        let acts = act_of.entry(row[2].clone()).or_default();
        if !acts.contains(&row[1]) {
            acts.push(row[1].clone());
        }
    }
    let cards: BTreeMap<String, String> = Dump::read("cards.txt")?
        .of("card")
        .map(|row| (row[1].clone(), row[2].clone()))
        .collect();

    // Monsters in the dump's order, each with its moves in the table's.
    let mut monsters: Vec<(String, Vec<Move>)> = Vec::new();
    for row in &dump.rows {
        let words: Vec<&str> = row.iter().map(String::as_str).collect();
        match &words[..] {
            ["monster", id] => monsters.push((id.to_string(), Vec::new())),
            [kind @ ("move" | "does" | "unmoved"), monster, id, ..] => {
                let (_, moves) = monsters
                    .iter_mut()
                    .find(|(named, _)| named == monster)
                    .ok_or_else(|| Error::BeforeMonster(monster.to_string()))?;
                if *kind == "move" {
                    moves.push(Move {
                        id: id.to_string(),
                        ..Move::default()
                    });
                    continue;
                }
                let held = moves
                    .iter_mut()
                    .find(|held| held.id == *id)
                    .ok_or_else(|| Error::BeforeMove {
                        monster: monster.to_string(),
                        id: id.to_string(),
                    })?;
                match *kind {
                    "does" => held.rows.push(row[3..].to_vec()),
                    _ => held.blocked = Some(row[3..].join(" ")),
                }
            }
            _ => {
                return Err(Error::UnknownRow(
                    words.iter().map(|w| w.to_string()).collect(),
                ));
            }
        }
    }

    // The build's type, `V0_107_1` for the module `v0_107_1`.
    let version = version_module().map_err(DumpError::from)?.to_uppercase();
    let mut split = Split::new(root, "monster_moves", "monster_moves.txt", &dump.build)?;
    let mut buckets: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (id, _) in &monsters {
        let bucket = super::monsters::bucket(act_of.get(id));
        buckets.entry(bucket).or_default().push(id);
    }
    for (bucket, ids) in &buckets {
        let out = split.body(bucket);
        writeln!(out, "type V = super::super::super::{version};\n").unwrap();
        writeln!(
            out,
            "/// A move of a monster in this file, for every move whose statements\n\
             /// the dump matched.\n\
             ///\n\
             /// Any other move panics, and `behaviour/` ports it by hand. It\n\
             /// starts at `step`, and hands back the step to go on from where a\n\
             /// command made the player answer something.\n\
             pub fn perform<F: Fight<V>>(\n\
             monster: Monster,\n\
             move_id: MoveId,\n\
             step: u8,\n\
             fight: &mut F,\n\
             ) -> Option<u8> {{\n\
             match (monster, move_id) {{"
        )
        .unwrap();
        for id in ids {
            let (_, moves) = monsters
                .iter()
                .find(|(named, _)| named == id)
                .expect("every bucketed monster is in the dump");
            for held in moves {
                let arm = format!("(Monster::{}, MoveId::{})", camel(id), camel(&held.id));
                if let Some(blocked) = &held.blocked {
                    comment(out, blocked);
                    writeln!(
                        out,
                        "{arm} => unimplemented!(\"what {{move_id:?}} does when {{monster:?}} performs it\"),"
                    )
                    .unwrap();
                    continue;
                }
                writeln!(out, "{arm} => {{").unwrap();
                let last = held.rows.len().saturating_sub(1);
                let stops = held.rows[..last].iter().any(|row| row == &["attack"]);
                if stops {
                    writeln!(out, "if step == 0 {{").unwrap();
                }
                let mut steps = 0;
                for (at, row) in held.rows.iter().enumerate() {
                    command(out, row, &cards).map_err(|source| Error::In {
                        at: format!("{id} {}", held.id),
                        source: Box::new(source),
                    })?;
                    if row == &["attack"] && at < last {
                        steps += 1;
                        writeln!(
                            out,
                            "if fight.waiting() {{ return Some({steps}); }}\n}}\nif step <= {steps} {{"
                        )
                        .unwrap();
                    }
                }
                if stops {
                    writeln!(out, "}}").unwrap();
                }
                writeln!(out, "}}").unwrap();
            }
        }
        writeln!(
            out,
            "(monster, move_id) => unimplemented!(\"{{monster:?}} has no {{move_id:?}} in the table\"),\n}}\nNone\n}}"
        )
        .unwrap();
    }

    let mut shape = super::banner(
        "What each monster's moves do, one module per act it is met in.",
        "monster_moves.txt",
        &dump.build,
    );
    shape.push('\n');
    for bucket in buckets.keys() {
        writeln!(shape, "pub mod {bucket};").unwrap();
    }
    writeln!(
        shape,
        "\n/// The file a monster's moves are filed in, one per act it is met in.\n\
         ///\n\
         /// `behaviour/` files its hand ports the same way, and routes a move by\n\
         /// this.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub enum Filed {{"
    )
    .unwrap();
    for bucket in buckets.keys() {
        writeln!(shape, "{},", camel(bucket)).unwrap();
    }
    writeln!(
        shape,
        "}}\n\n\
         /// Where `monster`'s moves are filed.\n\
         pub fn filed(monster: super::monsters::Monster) -> Filed {{\n\
         use super::monsters::Monster;\n\
         match monster {{"
    )
    .unwrap();
    for (bucket, ids) in &buckets {
        let arms: Vec<String> = ids
            .iter()
            .map(|id| format!("Monster::{}", camel(id)))
            .collect();
        writeln!(shape, "{} => Filed::{},", arms.join(" | "), camel(bucket)).unwrap();
    }
    writeln!(shape, "}}\n}}").unwrap();
    split.finish(
        &shape,
        |bucket| format!("What the moves of the monsters in `{bucket}` do."),
        &[
            ("super::super::ascension", &["AscensionLevel"]),
            (
                "super::super::cards",
                &["Cards", "StatusCard", "CurseCard", "TokenCard"],
            ),
            ("super::super::monsters", &["Monster", "MoveId"]),
            ("super::super::powers", &["Power"]),
            (
                "sts2_core",
                &["Amount", "Effect", "Fight", "PowerAmount", "Scaling"],
            ),
        ],
    )?;
    Ok(())
}
