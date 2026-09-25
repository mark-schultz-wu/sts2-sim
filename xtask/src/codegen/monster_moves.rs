//! What each monster's moves do, from `monster_moves.txt`.
//!
//! A move whose statements the dump matched gets the commands it listed,
//! spelled the way a hand port spells them. Any other move gets
//! `unimplemented!()` under a comment naming the statement that stopped it,
//! and the build's `behaviour/` ports it by hand.
//!
//! Filed the way the monster table is: one file per act a monster is met in.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::cards::card_path;
use super::split::Split;
use super::{Build, Dump, camel, sanitize, version_module};

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
fn amount(words: &[String]) -> Result<String, String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let number = |n: &str| {
        n.parse::<i64>()
            .map(|n| n.to_string())
            .map_err(|e| format!("the number {n}: {e}"))
    };
    match &words[..] {
        ["number", n] => number(n),
        ["ascension", level, at, below] => Ok(format!(
            "match fight.ascension().has(AscensionLevel::{}) {{ true => {}, false => {} }}",
            sanitize(level),
            number(at)?,
            number(below)?
        )),
        _ => Err(format!("an amount of {}", words.join(" "))),
    }
}

/// The Rust for one command a move runs.
fn command(
    out: &mut String,
    words: &[String],
    cards: &BTreeMap<String, String>,
) -> Result<(), String> {
    match words {
        [attack] if attack == "attack" => writeln!(out, "fight.attack_opponents();").unwrap(),
        [power, target, id, rest @ ..] if power == "power" => {
            let call = match target.as_str() {
                "self" => "apply_to_actor",
                "opponents" => "apply_to_opponents",
                other => return Err(format!("a power on {other}")),
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
                .ok_or_else(|| format!("{card} is not a card"))?;
            writeln!(
                out,
                "fight.give_cards({}, {});",
                card_path(pool, card)?,
                amount(rest)?
            )
            .unwrap();
        }
        _ => return Err(format!("a move of {}", words.join(" "))),
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
pub fn generate(root: &Path, build: &Build) -> Result<(), String> {
    let dump = Dump::read_any("monster_moves.txt")?;
    if (
        &dump.build.version,
        &dump.build.commit,
        dump.build.assembly_hash,
    ) != (&build.version, &build.commit, build.assembly_hash)
    {
        return Err(format!(
            "monster_moves.txt was read from {}, but the tables come from {build}",
            dump.build
        ));
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
                    .ok_or_else(|| format!("a move of {monster} before the monster"))?;
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
                    .ok_or_else(|| format!("{monster} {id} before its move row"))?;
                match *kind {
                    "does" => held.rows.push(row[3..].to_vec()),
                    _ => held.blocked = Some(row[3..].join(" ")),
                }
            }
            _ => return Err(format!("monster_moves.txt has a row {}", words.join(" "))),
        }
    }

    // The build's type, `V0_107_1` for the module `v0_107_1`.
    let version = version_module()?.to_uppercase();
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
             /// Any other move panics, and `behaviour/` ports it by hand.\n\
             pub fn perform<F: Fight<V>>(monster: Monster, move_id: MoveId, fight: &mut F) {{\n\
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
                for row in &held.rows {
                    command(out, row, &cards).map_err(|e| format!("{id} {}: {e}", held.id))?;
                }
                writeln!(out, "}}").unwrap();
            }
        }
        writeln!(
            out,
            "(monster, move_id) => unimplemented!(\"{{monster:?}} has no {{move_id:?}} in the table\"),\n}}\n}}"
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
        writeln!(shape, "mod {bucket};").unwrap();
    }
    writeln!(
        shape,
        "\ntype V = super::super::{version};\n\n\
         /// One of a monster's moves, from the file its table row is filed in.\n\
         pub fn perform<F: sts2_core::Fight<V>>(\n\
         monster: super::monsters::Monster,\n\
         move_id: super::monsters::MoveId,\n\
         fight: &mut F,\n\
         ) {{\n\
         use super::monsters::Monster;\n\
         match monster {{"
    )
    .unwrap();
    for (bucket, ids) in &buckets {
        let arms: Vec<String> = ids
            .iter()
            .map(|id| format!("Monster::{}", camel(id)))
            .collect();
        writeln!(
            shape,
            "{} => {bucket}::perform(monster, move_id, fight),",
            arms.join(" | ")
        )
        .unwrap();
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
    )
}
