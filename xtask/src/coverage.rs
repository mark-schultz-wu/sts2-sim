//! How much of one build's content the simulator can play, by act.
//!
//! Read from the build's dumps and its content source, the way `codegen`
//! reads them. Nothing here compiles or runs the content. So the count is of
//! code that exists, and a run can still stop inside it: the census is what
//! says how far runs get.
//!
//! A card's play or a monster's move counts once it has code. It counts as
//! complete once every power it names is ported as well. A power is ported
//! when its behaviour answers for it, or when it overrides no hook at all.
//! A hand-ported card or move reaches its powers through helpers this does
//! not follow, so its powers are not checked.
//!
//! `--missing` names, under each row, what that row lacks. Last comes how
//! far the simulator follows the runs the game played.
//!
//! ```text
//! cargo xtask coverage
//! cargo xtask coverage --missing
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::codegen::{VersionError, camel, version_dir};
use crate::error::FileError;

/// The act pools, in the order a run meets them. The first two are the two
/// first acts.
const ACTS: [&str; 4] = ["OVERGROWTH", "UNDERDOCKS", "HIVE", "GLORY"];

/// Whether a row names content no run can meet. Only the game's tests build
/// its `Mock` models, and no pool offers its `Deprecated` ones. The counts
/// leave them out.
fn test_only(name: &str) -> bool {
    let name = camel(name);
    name.starts_with("Mock") || name.starts_with("Deprecated")
}

/// Content no run with one player can meet, with what keeps it out. The
/// counts leave each out, the way they leave out the multiplayer-only cards.
const SINGLE_PLAYER_UNREACHABLE: [(&str, &str); 6] = [
    ("MassiveScroll", "its IsAllowed needs more than one player"),
    ("GravityPower", "no model applies it"),
    ("LeadershipPower", "no model applies it"),
    ("MagicBombPower", "no model applies it"),
    (
        "GuardedPower",
        "Tank gives it only to a player other than its owner",
    ),
    (
        "InterceptPower",
        "Covered gives it, from the multiplayer-only Intercept",
    ),
];

/// Whether a row names content [`SINGLE_PLAYER_UNREACHABLE`] lists.
fn unreachable_alone(name: &str) -> bool {
    let name = camel(name);
    SINGLE_PLAYER_UNREACHABLE
        .iter()
        .any(|(listed, _)| *listed == name)
}

/// How many of the powers holding the most back the report names.
const BLOCKERS: usize = 15;

/// The card pools a run plays from, by the generated module that holds each.
const CARD_POOLS: [(&str, &str); 11] = [
    ("ironclad", "IroncladCard"),
    ("silent", "SilentCard"),
    ("regent", "RegentCard"),
    ("necrobinder", "NecrobinderCard"),
    ("defect", "DefectCard"),
    ("colorless", "ColorlessCard"),
    ("curse", "CurseCard"),
    ("status", "StatusCard"),
    ("token", "TokenCard"),
    ("event", "EventCard"),
    ("quest", "QuestCard"),
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Version(#[from] VersionError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error("{file} has no `{what}`")]
    Missing { file: PathBuf, what: &'static str },
    #[error(transparent)]
    Process(#[from] crate::error::ProcessError),
}

/// Every file this reads, by its path under the build's directory.
struct Build {
    dir: PathBuf,
}

impl Build {
    fn read(&self, path: impl AsRef<Path>) -> Result<String, FileError> {
        crate::error::read(&self.dir.join(path))
    }

    /// The rows of one dump whose first word is `kind`, split into words.
    fn rows(&self, dump: &str, kind: &str) -> Result<Vec<Vec<String>>, FileError> {
        Ok(self
            .read(Path::new("dumps").join(dump))?
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| {
                line.split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|words| words.first().is_some_and(|word| word == kind))
            .collect())
    }

    /// Every `.rs` file under one directory of the build's source, read whole.
    fn sources(&self, path: impl AsRef<Path>) -> Result<Vec<(PathBuf, String)>, FileError> {
        let mut files = Vec::new();
        for path in crate::error::entries(&self.dir.join(path))? {
            if path.is_dir() {
                let under = path.strip_prefix(&self.dir).unwrap_or(&path).to_path_buf();
                files.extend(self.sources(under)?);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = crate::error::read(&path)?;
                files.push((path, text));
            }
        }
        Ok(files)
    }
}

/// The body of the first function named `name` in `source`, up to the brace
/// that closes it at the start of a line.
fn function<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let start = source.find(&format!("fn {name}"))?;
    let rest = &source[start..];
    let end = rest.find("\n}").map_or(rest.len(), |end| end + 2);
    Some(&rest[..end])
}

/// One match arm: the pattern at the start of a line, and the code up to the
/// next arm at the same indent.
struct Arm<'a> {
    pattern: &'a str,
    body: &'a str,
}

/// The arms of a match whose patterns start with `head`, at `indent` spaces.
///
/// An arm ends at the next line no deeper than its own. That is the next
/// arm, a comment above one, or the end of the match.
fn arms<'a>(body: &'a str, indent: usize, head: &str) -> Vec<Arm<'a>> {
    let prefix = format!("\n{}{head}", " ".repeat(indent));
    body.match_indices(&prefix)
        .map(|(at, _)| {
            let start = at + 1;
            let rest = &body[start..];
            let mut end = rest.len();
            let mut offset = rest.find('\n').map_or(rest.len(), |n| n + 1);
            while offset < rest.len() {
                let line = &rest[offset..];
                let depth = line.len() - line.trim_start_matches(' ').len();
                if !line.starts_with('\n') && depth <= indent {
                    end = offset;
                    break;
                }
                offset += line.find('\n').map_or(line.len(), |n| n + 1);
            }
            let text = &rest[..end];
            let (pattern, body) = text.split_once("=>").unwrap_or((text, ""));
            Arm {
                pattern: pattern.trim(),
                body,
            }
        })
        .collect()
}

/// The names that follow `Enum::` in `text`, each once.
fn named<'a>(text: &'a str, enum_name: &str) -> BTreeSet<&'a str> {
    let head = format!("{enum_name}::");
    text.match_indices(&head)
        .map(|(at, _)| {
            let rest = &text[at + head.len()..];
            let end = rest
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// A count out of a total, and the share it is.
fn share(done: usize, total: usize) -> String {
    let percent = match total {
        0 => 100,
        total => 100 * done / total,
    };
    format!("{done:>3}/{total:<3} {percent:>3}%")
}

#[derive(clap::Args)]
pub struct Args {
    /// Name, under each row, the content it counts as missing.
    #[arg(long)]
    missing: bool,
}

/// Under a row, the names it lacks, when `--missing` asks for them.
fn lacks(out: &mut String, args: &Args, names: impl IntoIterator<Item = String>) {
    if !args.missing {
        return;
    }
    let names: Vec<String> = names.into_iter().collect();
    if names.is_empty() {
        return;
    }
    let mut line = String::from("     ");
    for name in names {
        if line.len() + name.len() > 96 {
            writeln!(out, "{line}").unwrap();
            line = String::from("     ");
        }
        line.push(' ');
        line.push_str(&name);
    }
    writeln!(out, "{line}").unwrap();
}

/// What one card or move needs: whether it has code, and the powers it names.
struct Ported {
    code: bool,
    powers: BTreeSet<String>,
}

impl Ported {
    fn complete(&self, powers: &BTreeSet<String>) -> bool {
        self.code && self.missing(powers).next().is_none()
    }

    /// The powers it names that are not ported.
    fn missing<'a>(&'a self, powers: &'a BTreeSet<String>) -> impl Iterator<Item = &'a String> {
        self.powers.iter().filter(|power| !powers.contains(*power))
    }

    /// `name` when it has no code, or with the powers it waits on when it is
    /// incomplete. Nothing when it is complete.
    fn lack(&self, name: String, powers: &BTreeSet<String>) -> Option<String> {
        if !self.code {
            return Some(name);
        }
        let waits: Vec<&str> = self.missing(powers).map(String::as_str).collect();
        (!waits.is_empty()).then(|| format!("{name}[{}]", waits.join(",")))
    }
}

pub fn run(args: Args) -> Result<String, Error> {
    let build = Build {
        dir: version_dir()?,
    };
    let mut out = String::new();
    writeln!(out, "coverage of {}", build.dir.display()).unwrap();

    let powers = ported_powers(&build)?;
    let moves = moves(&build)?;
    // The unported powers that hold back code that is otherwise there.
    let mut blocking: BTreeMap<String, usize> = BTreeMap::new();
    for ported in moves.values().filter(|p| p.code) {
        for power in ported.missing(&powers) {
            *blocking.entry(power.clone()).or_default() += 1;
        }
    }

    // Each monster by the act pools that field it, and whether every move it
    // has is complete.
    let mut monsters: BTreeMap<String, bool> = BTreeMap::new();
    for ((monster, _), ported) in &moves {
        let done = monsters.entry(monster.clone()).or_insert(true);
        *done &= ported.complete(&powers);
    }
    // A monster that overrides `AfterAddedToRoom` is complete only once its
    // entering is ported too.
    let unentered = unentered(&build)?;
    for monster in &unentered {
        if let Some(done) = monsters.get_mut(monster) {
            *done = false;
        }
    }
    let fielded = build.rows("monsters.txt", "act")?;
    let encounters = encounters(&build, &monsters)?;
    let events = build.rows("events.txt", "act")?;
    let ported_events: BTreeSet<String> = build
        .sources("behaviour/events")?
        .iter()
        .filter_map(|(path, _)| path.file_stem()?.to_str().map(str::to_owned))
        .filter(|stem| stem != "mod")
        .collect();

    writeln!(
        out,
        "\nby act: moves with code, complete moves, complete monsters, complete encounters (normal, elite, boss), events"
    )
    .unwrap();
    for act in ACTS {
        let in_act: BTreeSet<String> = fielded
            .iter()
            .filter(|row| row.get(1).is_some_and(|a| a == act))
            .filter_map(|row| row.get(2).map(|m| camel(m)))
            .collect();
        let act_moves: Vec<(String, &Ported)> = moves
            .iter()
            .filter(|((monster, _), _)| in_act.contains(monster))
            .map(|((monster, id), ported)| (format!("{monster}.{id}"), ported))
            .collect();
        let coded = act_moves.iter().filter(|(_, p)| p.code).count();
        let complete = act_moves
            .iter()
            .filter(|(_, p)| p.complete(&powers))
            .count();
        let done = in_act
            .iter()
            .filter(|m| monsters.get(*m).copied().unwrap_or(false))
            .count();
        let by_room = |room: &str| {
            let of: Vec<_> = encounters
                .iter()
                .filter(|e| e.act == act && e.room == room)
                .collect();
            share(of.iter().filter(|e| e.complete).count(), of.len())
        };
        let act_events: Vec<String> = events
            .iter()
            .filter(|row| row.get(1).is_some_and(|a| a == act))
            .filter_map(|row| row.get(3).map(|e| e.to_lowercase()))
            .collect();
        let ported = act_events
            .iter()
            .filter(|e| ported_events.contains(*e))
            .count();
        let lacking_moves: Vec<String> = act_moves
            .iter()
            .filter_map(|(name, p)| p.lack(name.clone(), &powers))
            .chain(
                unentered
                    .iter()
                    .filter(|m| in_act.contains(*m))
                    .map(|m| format!("{m}.AfterAddedToRoom")),
            )
            .collect();
        let lacking_encounters: Vec<String> = encounters
            .iter()
            .filter(|e| e.act == act && !e.complete)
            .map(|e| e.id.clone())
            .collect();
        let lacking_events: Vec<String> = act_events
            .iter()
            .filter(|e| !ported_events.contains(*e))
            .cloned()
            .collect();
        writeln!(
            out,
            "  {act:<11} moves {}  complete {}  monsters {}\n  {:<11} encounters {}  {}  {}  events {}",
            share(coded, act_moves.len()),
            share(complete, act_moves.len()),
            share(done, in_act.len()),
            "",
            by_room("Monster"),
            by_room("Elite"),
            by_room("Boss"),
            share(ported, act_events.len()),
        )
        .unwrap();
        lacks(&mut out, &args, lacking_moves);
        lacks(&mut out, &args, lacking_encounters);
        lacks(&mut out, &args, lacking_events);
    }
    let shared: Vec<String> = build
        .rows("events.txt", "sharedpool")?
        .iter()
        .filter_map(|row| row.get(2).map(|e| e.to_lowercase()))
        .collect();
    writeln!(
        out,
        "  {:<11} events {}",
        "any act",
        share(
            shared.iter().filter(|e| ported_events.contains(*e)).count(),
            shared.len()
        )
    )
    .unwrap();
    lacks(
        &mut out,
        &args,
        shared
            .iter()
            .filter(|e| !ported_events.contains(*e))
            .cloned(),
    );

    // `CardMultiplayerConstraint.MultiplayerOnly`: a card a pool holds that
    // `GetUnlockedCards` for one player leaves out, which the shop pools are.
    let shop: BTreeSet<String> = build
        .rows("cards.txt", "shoppool")?
        .into_iter()
        .filter_map(|row| row.get(3).map(|c| camel(c)))
        .collect();
    let multiplayer_only: BTreeSet<String> = build
        .rows("cards.txt", "pool")?
        .into_iter()
        .filter_map(|row| row.get(3).map(|c| camel(c)))
        .filter(|card| !shop.contains(card))
        .collect();
    let mut multiplayer_skipped = 0;

    writeln!(out, "\ncards: plays with code, complete plays").unwrap();
    let hand_cards = hand_ported(&build, "behaviour/cards", "on_play")?;
    for (module, enum_name) in CARD_POOLS {
        let source = build.read(format!("generated/card_plays/{module}.rs"))?;
        let body = function(&source, "play_from").ok_or(Error::Missing {
            file: build.dir.join(format!("generated/card_plays/{module}.rs")),
            what: "fn play_from",
        })?;
        let plays: Vec<(String, Ported)> = arms(body, 8, &format!("{enum_name}::"))
            .into_iter()
            .filter(|arm| {
                let card = arm.pattern.trim_start_matches(&format!("{enum_name}::"));
                let kept = !multiplayer_only.contains(card);
                multiplayer_skipped += usize::from(!kept);
                kept
            })
            .map(|arm| {
                let card = arm.pattern.trim_start_matches(&format!("{enum_name}::"));
                let hand = hand_cards.contains(&(enum_name.to_owned(), card.to_owned()));
                let ported = Ported {
                    code: hand || !arm.body.contains("does when it is played"),
                    powers: match hand {
                        true => BTreeSet::new(),
                        false => named(arm.body, "Power")
                            .into_iter()
                            .map(str::to_owned)
                            .collect(),
                    },
                };
                (card.to_owned(), ported)
            })
            .collect();
        for (_, ported) in plays.iter().filter(|(_, p)| p.code) {
            for power in ported.missing(&powers) {
                *blocking.entry(power.clone()).or_default() += 1;
            }
        }
        writeln!(
            out,
            "  {module:<12} {}  complete {}",
            share(plays.iter().filter(|(_, p)| p.code).count(), plays.len()),
            share(
                plays.iter().filter(|(_, p)| p.complete(&powers)).count(),
                plays.len()
            ),
        )
        .unwrap();
        lacks(
            &mut out,
            &args,
            plays
                .iter()
                .filter_map(|(name, p)| p.lack(name.clone(), &powers)),
        );
    }

    writeln!(out, "\nrelics, by pool").unwrap();
    let relic_source = build.read("behaviour/relics/mod.rs")?;
    let relic_states = relic_source
        .find("pub enum State {")
        .map(|at| &relic_source[at..])
        .ok_or(Error::Missing {
            file: build.dir.join("behaviour/relics/mod.rs"),
            what: "pub enum State",
        })?;
    let relic_states = &relic_states[..relic_states.find("\n}").unwrap_or(relic_states.len())];
    let ported_relics: BTreeSet<&str> = relic_states
        .lines()
        .skip(1)
        .filter_map(|line| {
            let name = line.trim().split(['(', ',', ' ']).next()?;
            (!name.is_empty() && name.chars().next()?.is_ascii_uppercase()).then_some(name)
        })
        .collect();
    let mut skipped = 0;
    let mut pools: BTreeMap<String, (usize, usize, Vec<String>)> = BTreeMap::new();
    for row in build.rows("relics.txt", "pool")? {
        let (Some(pool), Some(relic)) = (row.get(1), row.get(2)) else {
            continue;
        };
        if test_only(relic) {
            skipped += 1;
            continue;
        }
        if unreachable_alone(relic) {
            continue;
        }
        let entry = pools.entry(pool.clone()).or_default();
        entry.1 += 1;
        match ported_relics.contains(camel(relic).as_str()) {
            true => entry.0 += 1,
            false => entry.2.push(camel(relic)),
        }
    }
    for (pool, (done, total, lacking)) in pools.into_iter().filter(|(_, (_, total, _))| *total > 0)
    {
        writeln!(out, "  {pool:<24} {}", share(done, total)).unwrap();
        lacks(&mut out, &args, lacking);
    }

    let potion_arms: BTreeSet<String> = build
        .sources("behaviour/potions")?
        .iter()
        .flat_map(|(_, text)| {
            arms(text, 8, "Potion::")
                .into_iter()
                .filter(|arm| !arm.body.contains("unimplemented!"))
                .filter_map(|arm| arm.pattern.strip_prefix("Potion::").map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect();
    let potions: Vec<Vec<String>> = build
        .rows("potions.txt", "potion")?
        .into_iter()
        .filter(|row| match row.get(1) {
            Some(potion) if test_only(potion) => {
                skipped += 1;
                false
            }
            _ => true,
        })
        .collect();
    let ported_potions = potions
        .iter()
        .filter(|row| row.get(1).is_some_and(|p| potion_arms.contains(&camel(p))))
        .count();
    writeln!(out, "\npotions {}", share(ported_potions, potions.len())).unwrap();
    lacks(
        &mut out,
        &args,
        potions
            .iter()
            .filter_map(|row| row.get(1).map(|p| camel(p)))
            .filter(|p| !potion_arms.contains(p)),
    );

    let all_powers: Vec<Vec<String>> = build
        .rows("powers.txt", "power")?
        .into_iter()
        .filter(|row| match row.get(1) {
            Some(power) if test_only(power) => {
                skipped += 1;
                false
            }
            Some(power) => !unreachable_alone(power),
            _ => true,
        })
        .collect();
    let ported = all_powers
        .iter()
        .filter(|row| row.get(1).is_some_and(|p| powers.contains(&camel(p))))
        .count();
    writeln!(
        out,
        "powers  {}  (with behaviour, or overriding no hook)",
        share(ported, all_powers.len())
    )
    .unwrap();
    lacks(
        &mut out,
        &args,
        all_powers
            .iter()
            .filter_map(|row| row.get(1).map(|p| camel(p)))
            .filter(|p| !powers.contains(p)),
    );

    writeln!(
        out,
        "\nleft out of every count: {skipped} test-only relics, potions and powers, and \
         {multiplayer_skipped} cards only a run with more than one player offers"
    )
    .unwrap();
    for (name, why) in SINGLE_PLAYER_UNREACHABLE {
        writeln!(out, "  {name}: {why}").unwrap();
    }

    let mut blocking: Vec<_> = blocking.into_iter().collect();
    blocking.sort_by_key(|(power, count)| (std::cmp::Reverse(*count), power.clone()));
    writeln!(
        out,
        "\nunported powers holding back plays and moves that have code, the most first"
    )
    .unwrap();
    for (power, count) in blocking.iter().take(BLOCKERS) {
        writeln!(out, "  {count:>3}  {power}").unwrap();
    }

    crate::followed::report(&crate::workspace(), &mut out)?;
    Ok(out)
}

/// The powers a run can hold without stopping: those the behaviour answers
/// for, and those that override no hook.
fn ported_powers(build: &Build) -> Result<BTreeSet<String>, Error> {
    let behaviour = build.read("behaviour/powers/mod.rs")?;
    let mut ported: BTreeSet<String> = named(&behaviour, "Power")
        .into_iter()
        .map(str::to_owned)
        .collect();
    let overriding: BTreeSet<String> = build
        .rows("hooks.txt", "implements")?
        .into_iter()
        .filter_map(|row| row.get(1).cloned())
        .collect();
    for row in build.rows("powers.txt", "power")? {
        if let Some(power) = row.get(1)
            && !overriding.contains(power)
        {
            ported.insert(camel(power));
        }
    }
    Ok(ported)
}

/// The monsters that override `AfterAddedToRoom` with no hand port of it.
///
/// A port names the monster in an arm of `starting_powers`, for what it
/// gives itself, or of `starting_opponent_powers`, for what it gives its
/// opponents.
fn unentered(build: &Build) -> Result<BTreeSet<String>, Error> {
    let mut entering = BTreeSet::new();
    for (_, source) in build.sources("behaviour/monsters")? {
        for name in ["starting_powers", "starting_opponent_powers"] {
            if let Some(body) = function(&source, name) {
                entering.extend(named(body, "Monster").into_iter().map(str::to_owned));
            }
        }
    }
    Ok(build
        .rows("monsters.txt", "virtual")?
        .into_iter()
        .filter(|row| {
            row.get(2)
                .is_some_and(|method| method == "AfterAddedToRoom")
        })
        .filter_map(|row| row.get(1).map(|monster| camel(monster)))
        .filter(|monster| !entering.contains(monster))
        .collect())
}

/// Every monster's move, by monster and move, with what it needs.
fn moves(build: &Build) -> Result<BTreeMap<(String, String), Ported>, Error> {
    // Every function a hand port performs through: `perform`, and the
    // `perform_from` it hands a resumed move to.
    let hand: BTreeSet<(String, String)> = build
        .sources("behaviour/monsters")?
        .iter()
        .flat_map(|(_, source)| {
            source
                .match_indices("fn perform")
                .filter_map(|(at, _)| function(&source[at..], "perform"))
                .flat_map(pairs)
                .collect::<Vec<_>>()
        })
        .collect();
    let mut moves = BTreeMap::new();
    for (_, source) in build.sources("generated/monster_moves")? {
        let Some(body) = function(&source, "perform") else {
            continue;
        };
        for arm in arms(body, 8, "(Monster::") {
            let Some(key) = pairs(arm.pattern).into_iter().next() else {
                continue;
            };
            let hand = hand.contains(&key);
            let ported = Ported {
                code: hand || !arm.body.contains("performs it\")"),
                powers: match hand {
                    true => BTreeSet::new(),
                    false => named(arm.body, "Power")
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                },
            };
            moves.insert(key, ported);
        }
    }
    Ok(moves)
}

/// The `(Monster::A, MoveId::B)` pairs in `text`. A pattern can name
/// alternatives on either side, as `(Monster::A | Monster::B, MoveId::C |
/// MoveId::D)`, and spread over lines, and each pair it covers counts.
fn pairs(text: &str) -> Vec<(String, String)> {
    let text: String = text.split_whitespace().collect();
    text.match_indices("(Monster::")
        .filter_map(|(at, _)| {
            let rest = &text[at + 1..];
            let (monsters, rest) = rest.split_once(",MoveId::")?;
            let end = rest.find(')')?;
            Some((monsters.to_owned(), format!("MoveId::{}", &rest[..end])))
        })
        .flat_map(|(monsters, moves)| {
            let names = |side: &str, prefix: &str| -> Vec<String> {
                side.split('|')
                    .map(|name| {
                        name.trim_end_matches(',')
                            .trim_start_matches(prefix)
                            .to_owned()
                    })
                    .collect()
            };
            let moves = names(&moves, "MoveId::");
            names(&monsters, "Monster::")
                .into_iter()
                .flat_map(move |monster| {
                    let moves = moves.clone();
                    moves.into_iter().map(move |id| (monster.clone(), id))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Whether what follows a card's name ends its arm's pattern: `=>` at
/// once, or after more alternatives, as `A | B =>`.
fn leads_to_arm(rest: &str) -> bool {
    let rest = rest.trim_start();
    if rest.starts_with("=>") {
        return true;
    }
    if !rest.starts_with('|') {
        return false;
    }
    let Some(arrow) = rest.find("=>") else {
        return false;
    };
    rest[..arrow]
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || matches!(c, '|' | ':' | '_'))
}

/// The cards a hand port plays, as `(Enum, Variant)`: the arms of every
/// function named `name` under `dir`.
fn hand_ported(build: &Build, dir: &str, name: &str) -> Result<BTreeSet<(String, String)>, Error> {
    let mut ported = BTreeSet::new();
    for (_, source) in build.sources(dir)? {
        let mut rest = source.as_str();
        while let Some(body) = function(rest, name) {
            for (at, _) in body.match_indices("Card::") {
                let before = &body[..at];
                let start = before
                    .rfind(|c: char| !c.is_ascii_alphanumeric())
                    .map_or(0, |i| i + 1);
                let enum_name = format!("{}Card", &before[start..]);
                let after = &body[at + "Card::".len()..];
                let end = after
                    .find(|c: char| !c.is_ascii_alphanumeric())
                    .unwrap_or(after.len());
                if leads_to_arm(&after[end..]) {
                    ported.insert((enum_name, after[..end].to_owned()));
                }
            }
            let consumed = rest.find(body).map_or(rest.len(), |at| at + body.len());
            rest = &rest[consumed..];
        }
    }
    Ok(ported)
}

/// One act's encounter, and whether a run can fight it through.
struct Encounter {
    id: String,
    act: String,
    room: String,
    complete: bool,
}

/// Every encounter an act fields: complete when its roster is fixed or
/// ported, and every monster it can field is complete.
fn encounters(build: &Build, monsters: &BTreeMap<String, bool>) -> Result<Vec<Encounter>, Error> {
    let source = build.read("behaviour/encounters.rs")?;
    let rolled: BTreeSet<&str> = function(&source, "generate")
        .map(|body| named(body, "Encounter"))
        .unwrap_or_default();
    let rows = build.rows("encounters.txt", "encounter")?;
    let possible = build.rows("encounters.txt", "possible")?;
    let mut out = Vec::new();
    for row in build.rows("encounters.txt", "act")? {
        let (Some(act), Some(id)) = (row.get(1), row.get(3)) else {
            continue;
        };
        let Some(encounter) = rows.iter().find(|r| r.get(1) == Some(id)) else {
            continue;
        };
        let room = encounter.get(2).cloned().unwrap_or_default();
        let roster = encounter.last().is_some_and(|spawns| spawns == "fixed")
            || rolled.contains(camel(id).as_str());
        let fielded = possible
            .iter()
            .filter(|p| p.get(1) == Some(id))
            .filter_map(|p| p.get(2));
        let complete = roster
            && fielded
                .into_iter()
                .all(|m| monsters.get(&camel(m)).copied().unwrap_or(false));
        out.push(Encounter {
            id: camel(id),
            act: act.clone(),
            room,
            complete,
        });
    }
    Ok(out)
}
