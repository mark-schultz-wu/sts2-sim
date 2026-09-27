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
                // each hit after it.
                "opponents" => {
                    writeln!(out, "for target in fight.opponents() {{").unwrap();
                    hit(out, key, scaling);
                    writeln!(out, "}}").unwrap();
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
            let amount = format!("PowerAmount::Counted({})", amount(&rest)?);
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
                // `ForgeCmd.Forge` raises the damage on every Sovereign Blade.
                _ => writeln!(
                    out,
                    "fight.forge({}, CardVar::Damage, {amount});",
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
    let dump = Dump::read_any("card_plays.txt")?;
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

    let cards_dump = Dump::read("cards.txt")?;
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

        // A pool with nothing generated reads nothing from the fight.
        let fight = match members
            .iter()
            .any(|(_, card)| card.play.blocked.is_none() && !card.play.rows.is_empty())
        {
            true => "fight",
            false => "_fight",
        };
        writeln!(
            out,
            "/// `OnPlay`, for every card whose statements the dump matched.\n\
             ///\n\
             /// Any other card panics, and `behaviour/` ports it by hand.\n\
             pub fn on_play<F: Fight<V>>(card: {enum_name}, {fight}: &mut F) -> Played<V> {{\n\
             match card {{"
        )
        .unwrap();
        for (variant, card) in &members {
            if let Some(blocked) = &card.play.blocked {
                comment(out, blocked);
                writeln!(
                    out,
                    "{variant} => unimplemented!(\"what {{card:?}} does when it is played\"),"
                )
                .unwrap();
                continue;
            }
            writeln!(out, "{variant} => {{").unwrap();
            if card
                .play
                .rows
                .iter()
                .any(|row| row.get(1).is_some_and(|t| t == "chosen"))
            {
                writeln!(out, "let target = fight.target();").unwrap();
            }
            // A list nothing reads is a list the C# only animates over.
            let read = card
                .play
                .rows
                .iter()
                .any(|row| row.get(1).is_some_and(|t| t == "held"));
            for row in &card.play.rows {
                if row == &["hold", "opponents"] && !read {
                    continue;
                }
                command(out, row).map_err(|source| Error::In {
                    at: variant.to_string(),
                    source: Box::new(source),
                })?;
            }
            writeln!(out, "Played::Done\n}}").unwrap();
        }
        writeln!(out, "}}\n}}\n").unwrap();

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
                    "Played",
                    "PowerAmount",
                    "Scaling",
                ],
            ),
        ],
    )?;
    Ok(())
}
