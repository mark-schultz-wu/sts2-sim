//! Every monster's HP range and move state machine.
//!
//! The state machine is a graph the game builds at runtime, and the dump walks
//! it. What comes out is data: nodes, edges, branch order, repeat rules,
//! cooldowns, and intents. The only holes are the weight lambdas and the
//! conditional predicates, which arrive marked `dynamic` and become
//! `Field::Computed`.
//!
//! The xtask README says why state ids become indices, how the files are split,
//! and why branch order is an RNG contract rather than presentation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use super::sets;
use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr, sanitize};

/// Intents that carry numbers, and which numbers.
///
/// Taken from the C# types rather than from whichever columns happen to be
/// filled in. `AttackIntent` has `DamageCalc` and `Repeats`, `DeathBlowIntent`
/// extends it, and `StatusIntent` has `CardCount`. Every other intent's numbers
/// live in the move body.
///
/// Every dumped row is checked against this, so a build that starts putting a
/// number on a `HealIntent` fails generation rather than dropping it.
const INTENT_PAYLOAD: [(&str, Payload); 3] = [
    ("Attack", Payload::Attack),
    ("DeathBlow", Payload::Attack),
    ("StatusCard", Payload::Count),
];

/// The `MoveRepeatType` members this knows how to translate, and how each reads
/// the `maxTimes` beside it.
///
/// `CannotRepeat` and `CanRepeatXTimes` are the same rule with the number left
/// implicit: `GetStateWeight` computes `num2 = CannotRepeat ? 1 : maxTimes` and
/// then runs one loop for both.
///
/// A build that grows a fifth member fails generation here rather than
/// generating a graph that quietly ignores it.
const REPEAT_LIMIT: [(&str, Limit); 4] = [
    ("CanRepeatForever", Limit::Forever),
    ("CanRepeatXTimes", Limit::InARow),
    ("CannotRepeat", Limit::InARowOnce),
    ("UseOnlyOnce", Limit::Once),
];

/// How a `MoveRepeatType` reads `maxTimes`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Limit {
    /// No limit. Carries no number.
    Forever,
    /// `InARow(maxTimes)`.
    InARow,
    /// `InARow(1)`. The number is implied, so the dump writes 0.
    InARowOnce,
    /// `Once`. Carries no number.
    Once,
}

/// The largest window `RepeatCount` can spell, for either kind of rule.
///
/// A capped type rather than a `u8`, because `Repeat` needs two fieldless
/// discriminants beside it. Only a payload with fewer than 255 inhabitants
/// leaves room. Anything up to 254 serves. The build's largest is 3, and
/// generation fails rather than truncating if one exceeds this.
const REPEAT_CAP: u8 = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Payload {
    /// `damage` and `repeats`.
    Attack,
    /// `count` alone.
    Count,
    /// No numbers at all.
    None,
}

fn payload(intent: &str) -> Payload {
    INTENT_PAYLOAD
        .iter()
        .find(|(name, _)| *name == intent)
        .map_or(Payload::None, |(_, p)| *p)
}

/// One monster's states, in the order the dump emitted them, which is sorted by
/// state id and is therefore the index each state gets.
struct Machine {
    min_hp: Steps,
    max_hp: Steps,
    initial: String,
    states: Vec<State>,
}

enum State {
    Move {
        id: String,
        follow_up: String,
        must_perform_once: bool,
        intents: Vec<Intent>,
    },
    Random {
        id: String,
        branches: Vec<Branch>,
    },
    Conditional {
        id: String,
        targets: Vec<String>,
    },
}

struct Intent {
    kind: String,
    count: Steps,
    damage: Steps,
}

struct Branch {
    target: String,
    repeat: String,
    max_times: String,
    cooldown: String,
    weight: Steps,
}

/// A value at ascension 0, plus the levels at which it changes.
///
/// `dynamic` at the base means the game computes it at combat time. Such a
/// value never carries steps, because there is nothing constant to scale.
#[derive(Default)]
struct Steps {
    base: String,
    at: Vec<(u8, String)>,
}

impl Steps {
    fn new(base: &str) -> Steps {
        Steps {
            base: base.to_string(),
            at: Vec::new(),
        }
    }

    fn is_dynamic(&self) -> bool {
        self.base == "dynamic"
    }

    fn is_absent(&self) -> bool {
        self.base == "-"
    }

    /// The `Scaled<T>` literal, or `Field::Computed`.
    fn literal(&self, where_: &str, ty: &str) -> Result<String, String> {
        if self.is_dynamic() {
            return Ok("Field::Computed".into());
        }

        let number = |v: &str| {
            v.parse::<u32>()
                .map(|n| format!("{n}{ty}"))
                .map_err(|e| format!("{where_}: {v}: {e}"))
        };

        // One threshold, because `AscensionHelper.GetValueIfAscension` takes
        // one. A value that moved twice would need a monster nesting two calls,
        // and `Scaled` has nowhere to put the second — so this fails rather
        // than dropping it.
        let step = match self.at.as_slice() {
            [] => "None".to_string(),
            [(level, v)] => format!("Some(({level}, {}))", number(v)?),
            more => {
                let levels: Vec<u8> = more.iter().map(|(l, _)| *l).collect();
                return Err(format!(
                    "{where_}: steps at more than one level: {levels:?}"
                ));
            }
        };

        Ok(format!(
            "Field::Constant(Scaled {{ base: {}, step: {step} }})",
            number(&self.base)?
        ))
    }
}

/// Every state has an id in the game, and only moves keep theirs in the table.
/// Branch nodes still need one here, because transitions target them by name.
fn state_id(state: &State) -> &str {
    match state {
        State::Move { id, .. } | State::Random { id, .. } | State::Conditional { id, .. } => id,
    }
}

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("monsters.txt")?;
    let build = dump.build.clone();

    // Reflected from the game, so a member no monster happens to use is still
    // a member — the same reason the card dump reflects its own enums.
    let mut enums: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    for f in dump.of("enum") {
        let discriminant = f[2]
            .parse::<u8>()
            .map_err(|e| format!("{} {}: discriminant {}: {e}", f[1], f[3], f[2]))?;
        enums
            .entry(f[1].clone())
            .or_default()
            .push((discriminant, f[3].clone()));
    }

    // Act order is the order `ModelDb.Acts` yields them, which is the order the
    // dump wrote them in and the order a run visits them.
    let mut acts: Vec<String> = Vec::new();
    let mut act_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in dump.of("act") {
        if !acts.contains(&f[1]) {
            acts.push(f[1].clone());
        }
        act_of.entry(f[2].clone()).or_default().push(f[1].clone());
    }

    let machines = machines(&dump)?;

    let mut out = banner(
        "Every monster's HP range and move state machine.",
        "monsters.txt",
        &build,
    );
    // `Act` is the acts domain's, not this one's. It was declared here when
    // nothing else described an act, which meant the enum was whichever ids
    // happened to turn up in this dump — four of the build's five.
    writeln!(out, "\nuse super::acts::Act;").unwrap();
    writeln!(out, "use crate::{{Field, Scaled}};").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    for (name, members) in &enums {
        // `IntentType` becomes `Intent`, whose variants carry the numbers that
        // intent has; `MoveRepeatType` becomes `RepeatLimit`, which carries
        // `maxTimes`. Both are emitted below from the members, and re-emitting
        // the bare enum as well would leave a second spelling nothing uses.
        if name == "IntentType" || name == "MoveRepeatType" {
            continue;
        }
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            writeln!(out, "    {} = {discriminant},", sanitize(m)).unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    emit_intent(&mut out, &enums)?;
    emit_repeat(&mut out, &enums, &machines)?;
    let largest = machines.values().map(|m| m.states.len()).max().unwrap_or(0);
    if largest > u8::MAX as usize {
        return Err(format!(
            "a monster has {largest} states, past what a u8 StateIndex can hold"
        ));
    }
    emit_shape(&mut out, largest);
    emit_moves(&mut out, &machines, &installed(&dump))?;
    // The hooks each monster's class overrides. `IterateHookListeners` asks
    // a monster's own model beside the powers on its creature.
    let overrides = super::overrides(&build)?;
    emit_monsters(&mut out, &machines, &act_of, &overrides)?;

    // The graphs, one file per bucket. `mod.rs` keeps the shape and the enum;
    // a monster's data is a named const it points at, so `data()` stays a
    // readable index instead of eight thousand lines of table.
    let mut split = Split::new(root, "monsters", "monsters.txt", &build)?;
    for (id, machine) in &machines {
        let literal = machine_literal(id, machine)?;
        writeln!(
            split.body(&bucket(act_of.get(id))),
            "pub(super) const {id}: MonsterData = {literal};\n"
        )
        .unwrap();
    }

    for name in split.names() {
        writeln!(out, "mod {name};").unwrap();
    }
    writeln!(out).unwrap();

    split.finish(
        &out,
        describe,
        &[
            (
                "super",
                &[
                    "Attack",
                    "Branch",
                    "FollowUp",
                    "IntentKind",
                    "IntentSet",
                    "Intents",
                    "MonsterData",
                    "Move",
                    "MoveId",
                    "Repeat",
                    "RepeatCount",
                    "State",
                ],
            ),
            ("crate", &["Field", "Scaled"]),
        ],
    )?;

    let computed = machines
        .values()
        .flat_map(|m| &m.states)
        .filter_map(|s| match s {
            State::Random { branches, .. } => Some(branches),
            _ => None,
        })
        .flatten()
        .filter(|b| b.weight.is_dynamic())
        .count();

    println!(
        "  monsters: {} across {} acts, {computed} computed branch weights",
        machines.len(),
        acts.len()
    );

    Ok(build)
}

/// The moves the game installs on a monster that no machine declares.
///
/// `Creature.Stun` swaps in a `STUNNED` state, and `IllusionPower.AfterDeath`
/// installs a `REVIVE_MOVE`. The observation reports a name that walking every
/// machine in the build never produces. The fuzzer found the second after 288
/// moves.
///
/// They are names and nothing else, because whatever installs one carries the
/// rest. So they reach [`emit_moves`] and stop.
fn installed(dump: &Dump) -> BTreeSet<&str> {
    dump.rows
        .iter()
        .filter(|row| row[0] == "installed")
        .map(|row| row[1].as_str())
        .collect()
}

/// Groups the dump's rows by monster.
///
/// Row order carries meaning twice over. States arrive sorted by id, and their
/// position becomes their index. Branches arrive in the order the game
/// subtracts their weights.
fn machines(dump: &Dump) -> Result<BTreeMap<String, Machine>, String> {
    let mut machines: BTreeMap<String, Machine> = BTreeMap::new();
    let mut current: Option<String> = None;

    for row in &dump.rows {
        match row[0].as_str() {
            "monster" => {
                machines.insert(
                    row[1].clone(),
                    Machine {
                        min_hp: Steps::new(&row[2]),
                        max_hp: Steps::new(&row[3]),
                        initial: row[4].clone(),
                        states: Vec::new(),
                    },
                );
                current = Some(row[1].clone());
            }
            "move" | "rand" | "cond" | "branch" | "condbranch" | "intent" | "nograph"
            | "unknownstate" => {
                let monster = current
                    .as_deref()
                    .ok_or_else(|| format!("{} row before any monster row", row[0]))?;
                if monster != row[1] {
                    return Err(format!(
                        "{} row for {} appeared under {monster}",
                        row[0], row[1]
                    ));
                }
                let machine = machines.get_mut(monster).expect("just inserted");

                match row[0].as_str() {
                    "move" => machine.states.push(State::Move {
                        id: row[2].clone(),
                        follow_up: row[3].clone(),
                        must_perform_once: row[4] == "1",
                        intents: Vec::new(),
                    }),
                    "rand" => machine.states.push(State::Random {
                        id: row[2].clone(),
                        branches: Vec::new(),
                    }),
                    "cond" => machine.states.push(State::Conditional {
                        id: row[2].clone(),
                        targets: Vec::new(),
                    }),
                    "intent" => match machine.states.last_mut() {
                        Some(State::Move { intents, .. }) => intents.push(Intent {
                            kind: row[4].clone(),
                            count: Steps::new(&row[5]),
                            damage: Steps::new(&row[6]),
                        }),
                        _ => return Err(format!("intent row outside a move: {row:?}")),
                    },
                    "branch" => match machine.states.last_mut() {
                        Some(State::Random { branches, .. }) => branches.push(Branch {
                            target: row[4].clone(),
                            repeat: row[5].clone(),
                            max_times: row[6].clone(),
                            cooldown: row[7].clone(),
                            weight: Steps::new(&row[8]),
                        }),
                        _ => return Err(format!("branch row outside a rand: {row:?}")),
                    },
                    "condbranch" => match machine.states.last_mut() {
                        Some(State::Conditional { targets, .. }) => targets.push(row[4].clone()),
                        _ => return Err(format!("condbranch row outside a cond: {row:?}")),
                    },
                    // The dump reports these rather than staying silent, so
                    // generation stops on them rather than emitting a monster
                    // with a graph nobody built.
                    other => return Err(format!("{other} for {monster}: {row:?}")),
                }
            }
            _ => {}
        }
    }

    scaling(dump, &mut machines)?;

    Ok(machines)
}

/// Folds the `scale*` rows into the values they move.
///
/// The dump emits one row per threshold, in ascending level order, so these
/// append in order and `Scaled::at` can walk them without sorting. A row that
/// names a value the base rows never declared means the two halves of the dump
/// disagree, which is worth stopping for.
fn scaling(dump: &Dump, machines: &mut BTreeMap<String, Machine>) -> Result<(), String> {
    // Ascension changing a monster's *graph* is not something the table models,
    // and the dump reports it rather than flattening it.
    if let Some(row) = dump.of("structurechange").next() {
        return Err(format!("ascension changes a monster's graph: {row:?}"));
    }

    for row in dump.of("scalehp") {
        let machine = machines
            .get_mut(&row[1])
            .ok_or_else(|| format!("scalehp for unknown monster {}", row[1]))?;
        let level = level(&row[2])?;
        machine.min_hp.at.push((level, row[3].clone()));
        machine.max_hp.at.push((level, row[4].clone()));
    }

    for row in dump.of("scaleintent") {
        let intent = intent_mut(machines, &row[1], &row[2], &row[3])?;
        let level = level(&row[4])?;
        push(&mut intent.count, level, &row[5], "count", &row[1])?;
        push(&mut intent.damage, level, &row[6], "damage", &row[1])?;
    }

    for row in dump.of("scalebranch") {
        let branch = branch_mut(machines, &row[1], &row[2], &row[3])?;
        let level = level(&row[4])?;
        push(&mut branch.weight, level, &row[5], "weight", &row[1])?;
    }

    Ok(())
}

/// A step is only recorded where the value actually moved, so a column that
/// matches the base is skipped. The two columns of a `scaleintent` row move
/// independently.
fn push(
    steps: &mut Steps,
    level: u8,
    value: &str,
    what: &str,
    monster: &str,
) -> Result<(), String> {
    let current = steps
        .at
        .last()
        .map_or(steps.base.as_str(), |(_, v)| v.as_str());
    if value == current {
        return Ok(());
    }
    if steps.is_dynamic() || steps.is_absent() {
        return Err(format!(
            "{monster}: {what} is `{}` at ascension 0 but `{value}` at {level}",
            steps.base
        ));
    }
    steps.at.push((level, value.to_string()));
    Ok(())
}

fn level(value: &str) -> Result<u8, String> {
    value
        .parse()
        .map_err(|e| format!("ascension level {value}: {e}"))
}

fn intent_mut<'a>(
    machines: &'a mut BTreeMap<String, Machine>,
    monster: &str,
    state: &str,
    index: &str,
) -> Result<&'a mut Intent, String> {
    let i: usize = index
        .parse()
        .map_err(|e| format!("{monster}.{state}: intent index {index}: {e}"))?;

    machines
        .get_mut(monster)
        .and_then(|m| {
            m.states.iter_mut().find_map(|s| match s {
                State::Move { id, intents, .. } if id == state => intents.get_mut(i),
                _ => None,
            })
        })
        .ok_or_else(|| format!("no intent {i} on {monster}.{state}"))
}

fn branch_mut<'a>(
    machines: &'a mut BTreeMap<String, Machine>,
    monster: &str,
    state: &str,
    index: &str,
) -> Result<&'a mut Branch, String> {
    let i: usize = index
        .parse()
        .map_err(|e| format!("{monster}.{state}: branch index {index}: {e}"))?;

    machines
        .get_mut(monster)
        .and_then(|m| {
            m.states.iter_mut().find_map(|s| match s {
                State::Random { id, branches } if id == state => branches.get_mut(i),
                _ => None,
            })
        })
        .ok_or_else(|| format!("no branch {i} on {monster}.{state}"))
}

/// The intent enum: one variant per `IntentType` member, carrying whatever
/// numbers that intent actually has.
///
/// A variant rather than a type-plus-two-nullable-columns, because "a Buff has
/// no damage" is a fact about buffs and belongs in the type. The dump spells it
/// `-`. Here it is absent.
fn emit_intent(
    out: &mut String,
    enums: &BTreeMap<String, Vec<(u8, String)>>,
) -> Result<(), String> {
    let members = enums
        .get("IntentType")
        .ok_or("the dump declares no IntentType")?;

    writeln!(
        out,
        "/// What a monster telegraphs before a move resolves.\n\
         ///\n\
         /// Not what the move does — that is the move body, ported by hand. The\n\
         /// game declares the two separately, so this is only what is displayed."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum IntentKind {{").unwrap();
    for (discriminant, m) in members {
        writeln!(out, "    {m} = {discriminant},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    // A set rather than a list, which is what the game's own use of it makes
    // it. `MoveState.Intents` is an ordered array, but the only two things that
    // read it are `NCreature.UpdateIntent`, which lays icons left to right into
    // a Godot container, and `Creature.HoverTips`, which collects tooltips —
    // both presentation. Nothing in the rules reads them, and no move telegraphs
    // the same kind twice, so the order is the only thing a set loses and the
    // order is not a fact about the game.
    sets::plain(
        out,
        &sets::Set {
            flag: "IntentKind",
            set: "IntentSet",
            flag_doc: "",
            set_doc: "A set of [`IntentKind`]s: one bit per kind, at its discriminant.\n\
                      \n\
                      A set rather than a list because the order is presentation. The\n\
                      game stores an ordered `AbstractIntent[]`, but the only readers are\n\
                      the node that lays out intent icons and the one that gathers hover\n\
                      tips; nothing in the rules looks at it, and no move in the build\n\
                      telegraphs one kind twice, so nothing else is lost.",
            members: &members
                .iter()
                .map(|(bit, m)| sets::Member {
                    name: m,
                    bit: *bit as usize,
                    doc: "",
                })
                .collect::<Vec<_>>(),
            declare: false,
            extras: &[],
        },
    )?;

    writeln!(
        out,
        "/// The numbers an attacking intent displays.\n\
         ///\n\
         /// One type for `Attack` and `DeathBlow` alike, because\n\
         /// `DeathBlowIntent` extends `SingleAttackIntent` and carries the same\n\
         /// two numbers, and no move telegraphs both.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct Attack {{\n\
         \x20   pub damage: Field<Scaled<u16>>,\n\
         \x20   /// `SingleAttackIntent.Repeats` is 1; `MultiAttackIntent` computes\n\
         \x20   /// or stores its own.\n\
         \x20   pub repeats: Field<Scaled<u8>>,\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// Everything a move telegraphs.\n\
         ///\n\
         /// The kinds as a set, and beside it the numbers the three kinds that\n\
         /// carry any actually carry. Held here rather than one payload per\n\
         /// intent because it saves the slice: a move's telegraph is now inline,\n\
         /// and the whole table lost a separate array of 433 intents.\n\
         ///\n\
         /// The two payloads and the set agree, which codegen checks: `attack`\n\
         /// is present exactly when the set holds `Attack` or `DeathBlow`, and\n\
         /// `cards` exactly when it holds `StatusCard`.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct Intents {{\n\
         \x20   pub kinds: IntentSet,\n\
         \x20   /// The numbers for whichever of `Attack` or `DeathBlow` is in\n\
         \x20   /// `kinds`.\n\
         \x20   pub attack: Option<Attack>,\n\
         \x20   /// `StatusIntent.CardCount` — how many cards it will add.\n\
         \x20   pub cards: Option<Field<Scaled<u8>>>,\n\
         }}\n"
    )
    .unwrap();

    Ok(())
}

/// English for a small number, which is what a `RepeatCount` variant is
/// called. Only ever asked for `1..=REPEAT_CAP`.
fn word(n: u8) -> &'static str {
    const WORDS: [&str; 9] = [
        "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight",
    ];
    WORDS[n as usize]
}

/// The repeat rule: when a branch stops being eligible.
///
/// Three of the game's fields become one enum. `maxTimes` folds in because it
/// is only meaningful for one `MoveRepeatType`. `cooldown` folds in because it
/// is redundant with whichever rule sits beside it. See [`repeat_literal`] for
/// the argument and for the translation itself.
fn emit_repeat(
    out: &mut String,
    enums: &BTreeMap<String, Vec<(u8, String)>>,
    machines: &BTreeMap<String, Machine>,
) -> Result<(), String> {
    let members = enums
        .get("MoveRepeatType")
        .ok_or("the dump declares no MoveRepeatType")?;

    for (_, m) in members {
        if !REPEAT_LIMIT.iter().any(|(name, _)| name == m) {
            return Err(format!(
                "MoveRepeatType::{m} is new; teach REPEAT_LIMIT what it means"
            ));
        }
    }

    // Counted rather than written down, because a generated doc that states a
    // number the dump no longer supports is worse than one that states none.
    let all = || {
        machines
            .values()
            .flat_map(|m| &m.states)
            .filter_map(|s| match s {
                State::Random { branches, .. } => Some(branches),
                _ => None,
            })
            .flatten()
    };
    let branches = all().count();
    let cooldowns = all().filter(|b| b.cooldown != "0").count();
    let largest = all()
        .filter_map(|b| {
            let n = b.max_times.parse::<u8>().ok()?;
            let c = b.cooldown.parse::<u8>().ok()?;
            Some(n.max(c))
        })
        .max()
        .unwrap_or(0);

    writeln!(
        out,
        "/// How far back a [`Repeat`] rule looks.\n\
         ///\n\
         /// A capped count rather than a `u8` so that `Repeat` stays small: it\n\
         /// needs two fieldless discriminants beside this one, and a `u8` payload\n\
         /// uses every pattern there is. The cap is {REPEAT_CAP}, against a build\n\
         /// whose largest window is {largest} — `cargo xtask codegen` fails rather\n\
         /// than truncating if one ever exceeds it.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]\n\
         #[repr(u8)]\n\
         pub enum RepeatCount {{"
    )
    .unwrap();
    for n in 1..=REPEAT_CAP {
        writeln!(out, "    {} = {n},", word(n)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "impl RepeatCount {{\n\
         \x20   /// The count as a number.\n\
         \x20   pub const fn get(self) -> u8 {{\n\
         \x20       self as u8\n\
         \x20   }}\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// What bars a branch the machine would otherwise be free to take.\n\
         ///\n\
         /// The game spends three fields on this — `MoveRepeatType`, `maxTimes`\n\
         /// and `cooldown` — and `GetStateWeight` blocks the branch if *either*\n\
         /// the repeat type or the cooldown says so. One of the two is always\n\
         /// redundant, because each implies the other in one direction:\n\
         ///\n\
         /// - `InARow(n)` blocking means the last `n` log entries are all this\n\
         ///   branch, and `n` is never zero, so the most recent entry is this\n\
         ///   branch — which is exactly what a cooldown of 1 or more already\n\
         ///   blocks on. A cooldown beside an in-a-row limit swallows it.\n\
         /// - A cooldown blocking means the branch is somewhere in the log, which\n\
         ///   is what [`Once`](Self::Once) blocks on. `Once` swallows a cooldown.\n\
         ///\n\
         /// So the pair collapses to whichever half is the stronger, and no\n\
         /// behaviour is lost. Of this build's {branches} branches, {cooldowns}\n\
         /// carry a cooldown at all.\n\
         ///\n\
         /// The two windowed variants are different questions, not the same one\n\
         /// with different numbers: [`InARow`](Self::InARow) asks whether *every*\n\
         /// entry of a recent window is this branch, [`Cooldown`](Self::Cooldown)\n\
         /// whether *any* is. They coincide only at `n = 1`, which generates as\n\
         /// `InARow(One)`.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum Repeat {{\n\
         \x20   /// Never barred by its own history.\n\
         \x20   Forever,\n\
         \x20   /// Barred while the last `n` moves were all this branch — so it may\n\
         \x20   /// be taken at most `n` times in a row.\n\
         \x20   InARow(RepeatCount),\n\
         \x20   /// Barred while this branch is anywhere in the last `n` moves — so\n\
         \x20   /// `n` other moves must pass before it comes round again.\n\
         \x20   Cooldown(RepeatCount),\n\
         \x20   /// Barred once this branch has been taken at all, ever.\n\
         \x20   Once,\n\
         }}\n"
    )
    .unwrap();

    Ok(())
}

/// Every name a move state has, across the build.
///
/// One enum rather than one per monster, which is exactly as precise as the
/// wire. Eighty-four of the 355 moves share a name, and a reader always has the
/// monster beside it.
///
/// Move states only. `NextMove` is typed `MoveState`, and the machine walks
/// through random and conditional nodes without stopping on one.
fn emit_moves(
    out: &mut String,
    machines: &BTreeMap<String, Machine>,
    installed: &BTreeSet<&str>,
) -> Result<(), String> {
    let mut names: BTreeSet<&str> = BTreeSet::new();
    for machine in machines.values() {
        for state in &machine.states {
            if let State::Move { id, .. } = state {
                names.insert(id);
            }
        }
    }

    // The moves no machine declares, which the observation can still report.
    // A row here that a machine *does* declare would mean the dump found the
    // same move twice and one of the two is not what it says it is.
    for id in installed {
        if !names.insert(id) {
            return Err(format!("{id} is both declared and installed"));
        }
    }

    // Two names that camel-case the same way would be one variant standing for
    // two moves, which is a divergence nothing downstream could see.
    let mut variants: BTreeMap<String, &str> = BTreeMap::new();
    for name in &names {
        if let Some(other) = variants.insert(camel(name), name) {
            return Err(format!(
                "{other} and {name} are both {}, so one variant would stand for two moves",
                camel(name)
            ));
        }
    }

    writeln!(
        out,
        "/// The name of a move, as the game and the observation both spell it.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(names.len())?).unwrap();
    writeln!(out, "pub enum MoveId {{").unwrap();
    for name in &names {
        writeln!(out, "    #[serde(rename = \"{name}\")]").unwrap();
        writeln!(out, "    {},", camel(name)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl MoveId {{").unwrap();
    writeln!(
        out,
        "    /// Every move name in the build, so a caller can sweep them."
    )
    .unwrap();
    writeln!(out, "    pub const ALL: [MoveId; {}] = [", names.len()).unwrap();
    for name in &names {
        writeln!(out, "        MoveId::{},", camel(name)).unwrap();
    }
    writeln!(out, "    ];").unwrap();
    writeln!(out, "}}\n").unwrap();

    // The one move name the simulator writes itself, for a creature that
    // has not rolled one.
    if !names.iter().any(|name| *name == "UNSET_MOVE") {
        return Err("the dump reflects no UNSET_MOVE, which the simulator names".into());
    }
    writeln!(
        out,
        "impl sts2_core::MoveKinds for MoveId {{\n    const UNSET: MoveId = MoveId::UnsetMove;\n}}\n"
    )
    .unwrap();

    Ok(())
}

fn emit_shape(out: &mut String, states: usize) {
    writeln!(
        out,
        "/// A state's position in its monster's own `states` slice.\n\
         ///\n\
         /// The game keys states by string and looks them up in a dictionary;\n\
         /// nothing outside a monster's own graph refers to one, so an index is\n\
         /// enough and costs no hashing. Every index a generated table contains\n\
         /// is in range for its monster, which the tests check.\n\
         ///\n\
         /// A `u8` and not a capped enum. Capping would buy nothing: the only\n\
         /// thing that could use the spare discriminants is a niche, and\n\
         /// [`FollowUp`] has two variants carrying one of these, so it is a tag\n\
         /// plus a payload either way. The build's largest graph is {states}\n\
         /// states; codegen refuses one that would not fit here.\n\
         pub type StateIndex = u8;\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// Where the machine goes when it leaves a move.\n\
         ///\n\
         /// The game spells this as a nullable `FollowUpStateId` beside a\n\
         /// `MustPerformOnceBeforeTransitioning` bool, but both are about\n\
         /// leaving: `CanTransitionAway` returns `_performedAtLeastOnce` when the\n\
         /// bool is set, and `FindNextMoveState` returns early while it is false.\n\
         /// So the bool gates the transition and belongs to it.\n\
         ///\n\
         /// A missing follow-up is not \"no next state\" either —\n\
         /// `MoveState.GetNextState` throws on one. That is\n\
         /// [`Never`](Self::Never), and it makes the combination the game can\n\
         /// express but never uses — must perform once, then go nowhere —\n\
         /// unrepresentable here.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum FollowUp {{\n\
         \x20   /// Leaves for this state.\n\
         \x20   Next(StateIndex),\n\
         \x20   /// Leaves for this state, but not before the move has been\n\
         \x20   /// performed at least once; the machine stays here until it has.\n\
         \x20   OncePerformed(StateIndex),\n\
         \x20   /// Never leaves. Asking is an error, not a stop.\n\
         \x20   Never,\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// One branch of a random choice.\n\
         ///\n\
         /// A single target, because the game's own `StateWeight` has one: a\n\
         /// random node is a list of these and picking one picks its target.\n\
         /// The branch is where the repeat rules live rather than the target,\n\
         /// because two nodes can offer the same target on different terms.\n\
         #[derive(Debug, Clone, Copy, PartialEq)]\n\
         pub struct Branch {{\n\
         \x20   pub target: StateIndex,\n\
         \x20   pub repeat: Repeat,\n\
         \x20   /// `Computed` when the game works it out from combat state.\n\
         \x20   pub weight: Field<f32>,\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// A move the monster can make.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct Move {{\n\
         \x20   /// The name the game calls this state, which is what an\n\
         \x20   /// observation reports as a monster's intent.\n\
         \x20   ///\n\
         \x20   /// Here so that the intent can be found in the graph: a move is\n\
         \x20   /// a state at a position, the wire names it, and without this\n\
         \x20   /// there is nothing to match the name against.\n\
         \x20   pub id: MoveId,\n\
         \x20   pub follow_up: FollowUp,\n\
         \x20   pub intents: Intents,\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// A node in a monster's move graph.\n\
         ///\n\
         /// `Random` takes exactly one RNG draw and walks its branches in order;\n\
         /// `Conditional` takes none and picks the first branch whose predicate\n\
         /// holds. Both are transparent to `StateLog` — the machine loops until\n\
         /// it lands on a `Move`.\n\
         #[derive(Debug, Clone, Copy, PartialEq)]\n\
         pub enum State {{\n\
         \x20   Move(Move),\n\
         \x20   Random(&'static [Branch]),\n\
         \x20   /// Targets in order; the predicates are ported by hand.\n\
         \x20   Conditional(&'static [StateIndex]),\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// What is true of a monster before it is put into a combat.\n\
         #[derive(Debug, Clone, Copy, PartialEq)]\n\
         pub struct MonsterData {{\n\
         \x20   /// Rolled between inclusive bounds, and both step with ascension\n\
         \x20   /// — 102 of 126 monsters gain HP at `ToughEnemies`.\n\
         \x20   pub min_hp: Scaled<u16>,\n\
         \x20   pub max_hp: Scaled<u16>,\n\
         \x20   pub initial: StateIndex,\n\
         \x20   pub states: &'static [State],\n\
         }}\n"
    )
    .unwrap();
}

/// What a bucket file holds, for its banner.
fn describe(file: &str) -> String {
    match file {
        "summoned" => "Monsters no encounter can roll — only another monster summons them.".into(),
        "shared" => "Monsters more than one act's encounters can roll.".into(),
        act => format!(
            "Monsters only `{}`'s encounters can roll.",
            act.to_uppercase()
        ),
    }
}

/// The file a monster's data lands in, from the acts it can be met in.
///
/// A genuine partition, unlike "the act it is in". 24 monsters are in no act,
/// because only another monster summons them, and two `*_CONSTRUCT` turn up in
/// Glory as well as an earlier act. Those get their own buckets, rather than
/// being filed somewhere their own `acts()` does not mention.
pub(super) fn bucket(acts: Option<&Vec<String>>) -> String {
    match acts {
        None => "summoned".to_string(),
        Some(acts) if acts.len() > 1 => "shared".to_string(),
        Some(acts) => acts[0].to_lowercase(),
    }
}

fn emit_monsters(
    out: &mut String,
    machines: &BTreeMap<String, Machine>,
    act_of: &BTreeMap<String, Vec<String>>,
    overrides: &BTreeMap<String, Vec<sts2_core::ModelHook>>,
) -> Result<(), String> {
    writeln!(out, "/// Every monster in the build.").unwrap();
    writeln!(
        out,
        "///\n\
         /// Flat rather than nested by act. Card ids are pool-scoped — `Strike`\n\
         /// exists in two pools — so nesting them solves a real collision;\n\
         /// monster ids are unique across the build, so nesting would cost a\n\
         /// wrapper at every call site and buy nothing. Acts would make a poor\n\
         /// nesting anyway: 24 monsters are in none and two are in two.\n\
         ///\n\
         /// The generated files still group by act, because a monster's graph is\n\
         /// long and a balance patch touches one act. See `data`."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(machines.len())?).unwrap();
    writeln!(out, "pub enum Monster {{").unwrap();
    for id in machines.keys() {
        writeln!(out, "    #[serde(rename = \"{id}\")]").unwrap();
        writeln!(out, "    {},", camel(id)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    // The one monster the simulator stands up by name, as a pet.
    if !machines.contains_key("OSTY") {
        return Err("the dump has no OSTY, which the simulator names".into());
    }
    writeln!(
        out,
        "impl sts2_core::MonsterKinds for Monster {{\n    const OSTY: Monster = Monster::Osty;\n}}\n"
    )
    .unwrap();

    let mut extras = String::new();
    writeln!(
        extras,
        "    /// The acts whose encounters can roll this monster. Empty for one\n\
         \x20   /// that only ever arrives as a summon."
    )
    .unwrap();
    writeln!(extras, "    fn acts(self) -> &'static [Act] {{").unwrap();
    writeln!(extras, "        match self {{").unwrap();
    for id in machines.keys() {
        let list = act_of
            .get(id)
            .map(|acts| {
                acts.iter()
                    .map(|a| format!("Act::{}", camel(a)))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        writeln!(extras, "            Self::{} => &[{list}],", camel(id)).unwrap();
    }
    writeln!(extras, "        }}").unwrap();
    writeln!(extras, "    }}").unwrap();
    writeln!(
        extras,
        "\n    fn overrides(self) -> sts2_core::Overrides {{\n\
         \x20       OVERRIDES[self as usize]\n\
         \x20   }}"
    )
    .unwrap();

    writeln!(out, "impl sts2_core::Table for Monster {{").unwrap();
    writeln!(out, "    type Data = MonsterData;\n").unwrap();

    writeln!(
        out,
        "    /// Every monster, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    const ALL: &'static [Self] = &[").unwrap();
    for id in machines.keys() {
        writeln!(out, "        Monster::{},", camel(id)).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// The HP range and the move graph.").unwrap();
    let mut table: Vec<String> = Vec::new();
    for id in machines.keys() {
        table.push(format!("{}::{id}", bucket(act_of.get(id))));
    }
    writeln!(out, "    fn data(self) -> &'static MonsterData {{").unwrap();
    writeln!(out, "        &ROWS[self as usize]").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    super::rows(out, "MonsterData", &table);
    super::emit_overrides(
        out,
        "indexed by the monster's place in the enum.",
        machines.keys().map(String::as_str),
        overrides,
    );

    writeln!(out, "impl sts2_core::MonsterTable for Monster {{").unwrap();
    writeln!(out, "    type Act = Act;\n").unwrap();
    out.push_str(&extras);
    writeln!(out, "}}").unwrap();

    Ok(())
}

fn machine_literal(id: &str, machine: &Machine) -> Result<String, String> {
    let mut out = String::new();
    let out = &mut out;
    // The dump names states; the table indexes them. Built here so that a name
    // the graph cannot resolve is a generation failure rather than a panic in
    // the simulator.
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, state) in machine.states.iter().enumerate() {
        if index.insert(state_id(state), i).is_some() {
            return Err(format!("{id}: two states named {}", state_id(state)));
        }
    }

    let resolve = |name: &str| -> Result<String, String> {
        index
            .get(name)
            .map(|i| i.to_string())
            .ok_or_else(|| format!("{id}: no state named {name}"))
    };

    let initial = resolve(&machine.initial)?;
    let min_hp = scaled(id, "minHp", &machine.min_hp, "u16")?;
    let max_hp = scaled(id, "maxHp", &machine.max_hp, "u16")?;

    writeln!(out, "MonsterData {{").unwrap();
    writeln!(out, "                min_hp: {min_hp},").unwrap();
    writeln!(out, "                max_hp: {max_hp},").unwrap();
    writeln!(out, "                initial: {initial},").unwrap();
    writeln!(out, "                states: &[").unwrap();

    for state in &machine.states {
        match state {
            State::Move {
                id: move_id,
                follow_up,
                must_perform_once,
                intents,
            } => {
                // The bool gates the transition, so it lands inside it. A move
                // that must be performed once and then goes nowhere is a
                // combination `FollowUp` cannot spell — and one the game never
                // uses, since the only move without a follow-up does not set it.
                let next = match (follow_up.as_str(), must_perform_once) {
                    ("-", false) => "FollowUp::Never".to_string(),
                    ("-", true) => {
                        return Err(format!(
                            "{id}.{move_id}: must perform once, but has no follow-up state"
                        ));
                    }
                    (name, false) => format!("FollowUp::Next({})", resolve(name)?),
                    (name, true) => format!("FollowUp::OncePerformed({})", resolve(name)?),
                };
                writeln!(out, "                    State::Move(Move {{").unwrap();
                writeln!(
                    out,
                    "                        id: MoveId::{},",
                    camel(move_id)
                )
                .unwrap();
                writeln!(out, "                        follow_up: {next},").unwrap();
                writeln!(
                    out,
                    "                        intents: {},",
                    intents_literal(id, move_id, intents)?
                )
                .unwrap();
                writeln!(out, "                    }}),").unwrap();
            }
            State::Random { branches, .. } => {
                writeln!(out, "                    State::Random(&[").unwrap();
                for b in branches {
                    if !b.weight.at.is_empty() {
                        return Err(format!("{id}: a branch weight steps with ascension"));
                    }
                    let weight = match b.weight.base.as_str() {
                        "dynamic" => "Field::Computed".to_string(),
                        w => format!("Field::Constant({w}f32)"),
                    };
                    writeln!(out, "                        Branch {{").unwrap();
                    writeln!(
                        out,
                        "                            target: {},",
                        resolve(&b.target)?
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "                            repeat: {},",
                        repeat_literal(id, b)?
                    )
                    .unwrap();
                    writeln!(out, "                            weight: {weight},").unwrap();
                    writeln!(out, "                        }},").unwrap();
                }
                writeln!(out, "                    ]),").unwrap();
            }
            State::Conditional { targets, .. } => {
                let list = targets
                    .iter()
                    .map(|t| resolve(t))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                writeln!(out, "                    State::Conditional(&[{list}]),").unwrap();
            }
        }
    }

    writeln!(out, "                ],").unwrap();
    write!(out, "            }}").unwrap();

    Ok(out.clone())
}

/// One branch's three repeat columns as the single `Repeat` they mean.
///
/// One of the two rules is always the weaker, so the three columns collapse
/// without losing behaviour. The xtask README works through the cases.
///
/// Everything the columns say that the type cannot is an error rather than a
/// value. A `maxTimes` on a member that does not read one, a missing one on
/// `CanRepeatXTimes`, or a window past [`REPEAT_CAP`].
fn repeat_literal(monster: &str, branch: &Branch) -> Result<String, String> {
    let where_ = format!("{monster}.{} -> {}", branch.repeat, branch.target);

    let (_, kind) = REPEAT_LIMIT
        .iter()
        .find(|(name, _)| *name == branch.repeat)
        .ok_or_else(|| format!("{where_}: unknown MoveRepeatType"))?;

    let count = |what: &str, n: u8| {
        if n == 0 || n > REPEAT_CAP {
            return Err(format!("{where_}: {what} {n} is outside 1..={REPEAT_CAP}"));
        }
        Ok(format!("RepeatCount::{}", word(n)))
    };

    let max_times: u8 = branch
        .max_times
        .parse()
        .map_err(|e| format!("{where_}: maxTimes {}: {e}", branch.max_times))?;
    let cooldown: u8 = branch
        .cooldown
        .parse()
        .map_err(|e| format!("{where_}: cooldown {}: {e}", branch.cooldown))?;

    if *kind != Limit::InARow && max_times != 0 {
        return Err(format!(
            "{where_}: maxTimes {max_times} on a member that does not read one"
        ));
    }

    Ok(match (kind, cooldown) {
        (Limit::Once, _) => "Repeat::Once".to_string(),
        (_, 0) => match kind {
            Limit::Forever => "Repeat::Forever".to_string(),
            Limit::InARowOnce => "Repeat::InARow(RepeatCount::One)".to_string(),
            _ => format!("Repeat::InARow({})", count("maxTimes", max_times)?),
        },
        (_, 1) => "Repeat::InARow(RepeatCount::One)".to_string(),
        (_, c) => format!("Repeat::Cooldown({})", count("cooldown", c)?),
    })
}

/// A move's whole telegraph: the set of kinds, and the numbers beside it.
///
/// The dump's rows are an ordered list, and this is a set plus two payloads.
/// Every way the two fail to line up fails generation. A kind twice, a second
/// attacking intent, or a number on a kind with nowhere to put it.
fn intents_literal(monster: &str, state: &str, intents: &[Intent]) -> Result<String, String> {
    let mut kinds: Vec<&str> = Vec::new();
    let mut attack: Option<String> = None;
    let mut cards: Option<String> = None;

    for intent in intents {
        let where_ = format!("{monster}.{state} {}", intent.kind);
        let kind = intent.kind.as_str();

        // A set keeps one bit per kind, so a repeat would be invisible. No move
        // in this build telegraphs one twice, and that is what makes the set
        // lossless rather than lucky.
        if kinds.contains(&kind) {
            return Err(format!("{where_}: telegraphed twice"));
        }
        kinds.push(kind);

        match payload(&intent.kind) {
            Payload::Attack => {
                if intent.count.is_absent() || intent.damage.is_absent() {
                    return Err(format!("{where_}: an attack with no damage or repeats"));
                }
                let literal = format!(
                    "Attack {{ damage: {}, repeats: {} }}",
                    intent.damage.literal(&where_, "u16")?,
                    intent.count.literal(&where_, "u8")?
                );
                if attack.replace(literal).is_some() {
                    return Err(format!("{where_}: a second attacking intent on one move"));
                }
            }
            Payload::Count => {
                if intent.count.is_absent() {
                    return Err(format!("{where_}: a status intent with no count"));
                }
                if !intent.damage.is_absent() {
                    return Err(format!("{where_}: a status intent with damage"));
                }
                let literal = intent.count.literal(&where_, "u8")?;
                if cards.replace(literal).is_some() {
                    return Err(format!("{where_}: a second status intent on one move"));
                }
            }
            Payload::None => {
                // A number where the type has no field for it means the game
                // grew one, and dropping it silently is how a table starts
                // lying.
                if !intent.count.is_absent() || !intent.damage.is_absent() {
                    return Err(format!(
                        "{where_}: carries numbers ({} {}) but has no payload; \
                         add it to INTENT_PAYLOAD",
                        intent.count.base, intent.damage.base
                    ));
                }
            }
        }
    }

    let set = kinds
        .iter()
        .map(|k| format!("IntentKind::{k}"))
        .collect::<Vec<_>>()
        .join(", ");

    Ok(format!(
        "Intents {{ kinds: IntentSet::new(&[{set}]), attack: {}, cards: {} }}",
        attack.map_or("None".to_string(), |a| format!("Some({a})")),
        cards.map_or("None".to_string(), |c| format!("Some({c})")),
    ))
}

/// A bare `Scaled<T>`, with no `Field`, for values the game never computes.
fn scaled(monster: &str, what: &str, steps: &Steps, ty: &str) -> Result<String, String> {
    if steps.is_dynamic() {
        return Err(format!("{monster}: {what} is computed, which has no table"));
    }

    steps
        .literal(&format!("{monster}.{what}"), ty)?
        .strip_prefix("Field::Constant(")
        .and_then(|s| s.strip_suffix(')'))
        .map(String::from)
        .ok_or_else(|| format!("{monster}: {what} did not produce a Scaled"))
}
