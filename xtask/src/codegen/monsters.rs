//! Every monster's HP range and move state machine.
//!
//! The state machine is a graph the game builds at runtime, and the dump walks
//! it. What comes out is data — nodes, edges, branch order, repeat rules,
//! cooldowns, intents — and the only holes are the weight lambdas and the
//! conditional predicates, which arrive marked `dynamic` and become
//! [`Field::Computed`](sts2_content::Field).
//!
//! # State ids become indices
//!
//! The game addresses states by string id and looks them up in a
//! `Dictionary<string, MonsterState>`. Here a state is its position in the
//! monster's own `states` slice, so a transition is a `u8` rather than a hash
//! lookup. The ids survive on moves only, because `StateLog` records moves and
//! the observation reports them; branch nodes have `ShouldAppearInLogs => false`
//! and nothing outside the graph ever names them.
//!
//! # Why the branch order matters
//!
//! `RandomBranchState.GetNextState` sums the weights, draws exactly one
//! `rng.NextFloat(max)`, and subtracts weights walking the list in declaration
//! order. So the order branches are emitted in is not presentation — it decides
//! which move a given roll produces. Branch rows keep dump order for that
//! reason, while states are sorted by id, which is what the dump already did.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::{DERIVES, Dump, banner, camel, sanitize};

/// Intents that carry numbers, and which numbers.
///
/// Taken from the C# types rather than from whichever columns happen to be
/// filled in: `AttackIntent` has `DamageCalc` and `Repeats`, `DeathBlowIntent`
/// extends `SingleAttackIntent` so it has both too, and `StatusIntent` has
/// `CardCount`. Every other intent's numbers live in the move body, which is
/// ported by hand — the intent is only what the player is shown.
///
/// Every dumped row is checked against this, so a build that starts putting a
/// number on a `HealIntent` fails generation rather than dropping it.
const INTENT_PAYLOAD: [(&str, Payload); 3] = [
    ("Attack", Payload::Attack),
    ("DeathBlow", Payload::Attack),
    ("StatusCard", Payload::Count),
];

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
/// `dynamic` at the base means the game computes it at combat time, and such a
/// value never carries steps — there is nothing constant to scale.
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

        let steps = self
            .at
            .iter()
            .map(|(level, v)| Ok(format!("({level}, {})", number(v)?)))
            .collect::<Result<Vec<_>, String>>()?
            .join(", ");

        Ok(format!(
            "Field::Constant(Scaled {{ base: {}, steps: &[{steps}] }})",
            number(&self.base)?
        ))
    }
}

/// Every state has an id in the game; only moves keep theirs in the table.
/// Branch nodes still need one here, because transitions target them by name.
fn state_id(state: &State) -> &str {
    match state {
        State::Move { id, .. } | State::Random { id, .. } | State::Conditional { id, .. } => id,
    }
}

pub fn generate(root: &Path) -> Result<String, String> {
    let dump = Dump::read("monsters.txt")?;
    let hash = dump.hash.clone();

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
        &hash,
    );
    writeln!(out, "\nuse crate::{{Field, Scaled}};").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    // The act a monster can be met in. Not every monster has one: 24 arrive
    // only by being summoned mid-combat, and no encounter table names them.
    writeln!(out, "/// The acts, in the order a run visits them.").unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum Act {{").unwrap();
    for (i, act) in acts.iter().enumerate() {
        writeln!(out, "    #[serde(rename = \"{act}\")]").unwrap();
        writeln!(out, "    {} = {i},", camel(act)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    for (name, members) in &enums {
        if name == "IntentType" {
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
    emit_shape(&mut out);
    emit_monsters(&mut out, &machines, &act_of)?;

    let dir = root.join("monsters");
    fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let path = dir.join("mod.rs");
    fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;

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

    Ok(hash)
}

/// Groups the dump's rows by monster.
///
/// Row order carries meaning twice over: states arrive sorted by id and their
/// position becomes their index, and branches arrive in the order the game
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
/// matches the base is skipped — the two columns of a `scaleintent` row move
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
/// `-`; here it is simply absent.
fn emit_intent(
    out: &mut String,
    enums: &BTreeMap<String, Vec<(u8, String)>>,
) -> Result<(), String> {
    let members = enums
        .get("IntentType")
        .ok_or("the dump declares no IntentType")?;

    writeln!(
        out,
        "/// What the monster telegraphs before a move resolves."
    )
    .unwrap();
    writeln!(
        out,
        "///\n\
         /// Not what the move does — that is the move body, ported by hand. The\n\
         /// game declares the two separately, so these are the displayed numbers."
    )
    .unwrap();
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]").unwrap();
    writeln!(out, "pub enum Intent {{").unwrap();
    for (_, m) in members {
        match payload(m) {
            Payload::Attack => writeln!(
                out,
                "    {m} {{ damage: Field<Scaled<u16>>, repeats: Field<Scaled<u8>> }},"
            )
            .unwrap(),
            Payload::Count => writeln!(out, "    {m} {{ count: Field<Scaled<u8>> }},").unwrap(),
            Payload::None => writeln!(out, "    {m},").unwrap(),
        }
    }
    writeln!(out, "}}\n").unwrap();

    Ok(())
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// A state's position in its monster's own `states` slice.\n\
         ///\n\
         /// The game keys states by string and looks them up in a dictionary;\n\
         /// nothing outside a monster's own graph refers to one, so an index is\n\
         /// enough and costs no hashing. Every index a generated table contains\n\
         /// is in range for its monster, which the tests check.\n\
         pub type StateIndex = u8;\n"
    )
    .unwrap();

    writeln!(
        out,
        "/// One branch of a random choice.\n\
         #[derive(Debug, Clone, Copy, PartialEq)]\n\
         pub struct Branch {{\n\
         \x20   pub target: StateIndex,\n\
         \x20   pub repeat: MoveRepeatType,\n\
         \x20   /// Only meaningful for `CanRepeatXTimes`.\n\
         \x20   pub max_times: u8,\n\
         \x20   /// Turns this branch is barred after being taken. 0 is no cooldown.\n\
         \x20   pub cooldown: u8,\n\
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
         \x20   /// The game's own id. Moves carry one because `StateLog` records\n\
         \x20   /// them and the observation reports them; branch nodes do not,\n\
         \x20   /// because `ShouldAppearInLogs` is false for those.\n\
         \x20   pub id: &'static str,\n\
         \x20   /// Where the machine goes after this move. `None` only for a move\n\
         \x20   /// that ends the monster.\n\
         \x20   pub follow_up: Option<StateIndex>,\n\
         \x20   pub must_perform_once: bool,\n\
         \x20   pub intents: &'static [Intent],\n\
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

fn emit_monsters(
    out: &mut String,
    machines: &BTreeMap<String, Machine>,
    act_of: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    writeln!(out, "/// Every monster in the build.").unwrap();
    writeln!(
        out,
        "///\n\
         /// Flat rather than grouped by act, because acts are not a partition:\n\
         /// a monster can appear in two, and 24 appear in none because they only\n\
         /// ever arrive by being summoned."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u16)]").unwrap();
    writeln!(out, "pub enum Monster {{").unwrap();
    for id in machines.keys() {
        writeln!(out, "    #[serde(rename = \"{id}\")]").unwrap();
        writeln!(out, "    {},", camel(id)).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl Monster {{").unwrap();

    writeln!(
        out,
        "    /// Every monster, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    pub const ALL: [Monster; {}] = [", machines.len()).unwrap();
    for id in machines.keys() {
        writeln!(out, "        Monster::{},", camel(id)).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// The game's own id, and the wire form.").unwrap();
    writeln!(out, "    pub const fn id(self) -> &'static str {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for id in machines.keys() {
        writeln!(out, "            Self::{} => \"{id}\",", camel(id)).unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}\n").unwrap();

    writeln!(
        out,
        "    /// The acts whose encounters can roll this monster. Empty for one\n\
         \x20   /// that only ever arrives as a summon."
    )
    .unwrap();
    writeln!(out, "    pub const fn acts(self) -> &'static [Act] {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
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
        writeln!(out, "            Self::{} => &[{list}],", camel(id)).unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}\n").unwrap();

    writeln!(out, "    /// The HP range and the move graph.").unwrap();
    writeln!(out, "    pub const fn data(self) -> MonsterData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for (id, machine) in machines {
        emit_machine(out, id, machine)?;
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(())
}

fn emit_machine(out: &mut String, id: &str, machine: &Machine) -> Result<(), String> {
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

    writeln!(out, "            Self::{} => MonsterData {{", camel(id)).unwrap();
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
                let next = if follow_up == "-" {
                    "None".to_string()
                } else {
                    format!("Some({})", resolve(follow_up)?)
                };
                writeln!(out, "                    State::Move(Move {{").unwrap();
                writeln!(out, "                        id: \"{move_id}\",").unwrap();
                writeln!(out, "                        follow_up: {next},").unwrap();
                writeln!(
                    out,
                    "                        must_perform_once: {must_perform_once},"
                )
                .unwrap();
                writeln!(out, "                        intents: &[").unwrap();
                for intent in intents {
                    writeln!(
                        out,
                        "                            {},",
                        intent_literal(id, move_id, intent)?
                    )
                    .unwrap();
                }
                writeln!(out, "                        ],").unwrap();
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
                        "                            repeat: MoveRepeatType::{},",
                        b.repeat
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "                            max_times: {},",
                        &b.max_times
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "                            cooldown: {},",
                        &b.cooldown
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
    writeln!(out, "            }},").unwrap();

    Ok(())
}

/// One intent, checked against what its type is allowed to carry.
fn intent_literal(monster: &str, state: &str, intent: &Intent) -> Result<String, String> {
    let where_ = format!("{monster}.{state} {}", intent.kind);

    match payload(&intent.kind) {
        Payload::Attack => {
            if intent.count.is_absent() || intent.damage.is_absent() {
                return Err(format!("{where_}: an attack with no damage or repeats"));
            }
            Ok(format!(
                "Intent::{} {{ damage: {}, repeats: {} }}",
                intent.kind,
                intent.damage.literal(&where_, "u16")?,
                intent.count.literal(&where_, "u8")?
            ))
        }
        Payload::Count => {
            if intent.count.is_absent() {
                return Err(format!("{where_}: a status intent with no count"));
            }
            if !intent.damage.is_absent() {
                return Err(format!("{where_}: a status intent with damage"));
            }
            Ok(format!(
                "Intent::{} {{ count: {} }}",
                intent.kind,
                intent.count.literal(&where_, "u8")?
            ))
        }
        Payload::None => {
            // A number where the type has no field for it means the game grew
            // one, and dropping it silently is how a table starts lying.
            if !intent.count.is_absent() || !intent.damage.is_absent() {
                return Err(format!(
                    "{where_}: carries numbers ({} {}) but has no payload; \
                     add it to INTENT_PAYLOAD",
                    intent.count.base, intent.damage.base
                ));
            }
            Ok(format!("Intent::{}", intent.kind))
        }
    }
}

/// A bare `Scaled<T>` — no `Field`, for values the game never computes.
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
