//! Every power's static data.
//!
//! A power is a status a creature carries: Strength, Vulnerable, Artifact. What
//! it *does* is hooks — `BeforeCardPlayed`, `AfterApplied` — and hooks are code,
//! ported by hand. What is table is the handful of facts the engine consults
//! without asking the power anything: whether it is a buff or a debuff, how it
//! stacks, whether several can coexist, and what happens to it when its owner
//! dies.
//!
//! # Why some cells are `Computed`
//!
//! `PowerModel` declares all of these `virtual` or `abstract`, so any of them
//! could be worked out from combat state, and a few are. `ShrinkPower` stacks
//! as a counter or as a single depending on the sign of its amount;
//! `MonologuePower` reads a dynamic var; the twenty `ITemporaryPower`
//! descendants decide buff-or-debuff from a `protected IsPositive`. The dump
//! marks those `dynamic` and they generate as [`Field::Computed`], which is the
//! simulator's cue to ask rather than look up.
//!
//! # The flags
//!
//! Five of the eight columns are yes-or-no, and five booleans in a row is a
//! shape where every call site has to remember an order nothing enforces. They
//! are one [`PowerFlags`] bitset instead, on the same pattern as `KeywordSet`
//! and `IntentSet` — with one difference worth knowing: those reuse a game enum
//! and its discriminants, and this one has no game enum behind it. The members
//! are five separate `PowerModel` overrides, so the numbering is ours, chosen
//! as the order the dump writes them.
//!
//! The set carries the computed-ness too, which is why it is two `u8`s rather
//! than one. A flag is true, false, or worked out at the time, and `get`
//! returns that as a `Field<bool>` — so there is no way to read a computed flag
//! as a plain `false`, which is what a single bitset would have offered.
//!
//! # The wire form
//!
//! The id is the serde rename and nothing else spells it. There is no `id`
//! accessor beside it: two spellings of one string is a way for them to
//! disagree, and the derive is the one the wire already depends on.
//!
//! # Why the files are split by `kind`
//!
//! The game groups powers by nothing. They are one C# namespace and one source
//! directory; the class hierarchy is flat but for three abstract bases;
//! `ModelDb.AllPowers` is a reflection sweep over every `PowerModel` subclass
//! rather than a registry; and every asset path is derived from the id. Powers
//! have no pools the way cards do, and no acts the way monsters do — any power
//! can be applied to anything that can carry it.
//!
//! What the game does classify a power by is `Type`, and it is the one fact
//! everything branches on: `PowerCmd` checks it before Artifact eats a debuff,
//! `Rend` and `Misery` count debuffs, and the badge and hover tips sort by it.
//! That is the same kind of thing `pool` is for cards — a column of the dump
//! that the game itself switches on — so it is what the buckets are.
//!
//! It partitions cleanly, and the third bucket is a family rather than a
//! remainder: the twenty powers whose `Type` is computed are exactly the
//! `ITemporaryPower` descendants — fourteen `TemporaryStrengthPower`, three
//! `TemporaryDexterityPower`, three `TemporaryFocusPower` — which read
//! buff-or-debuff off the sign of their own amount.
//!
//! `Power` itself stays one flat enum. A power's kind is a `Field` and twenty
//! of them compute it, so a `BuffPower` type would be a claim the table cannot
//! make; and `GetTypeForAmount` flips the answer on a negative amount anyway.
//! The split is where the rows are written, not what they are.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::split::Split;
use super::{DERIVES, Dump, banner, camel, repr};

/// Members the game declares that are not values of the field they name.
///
/// A C# enum has a zero, so a `None` gets declared whether or not it means
/// anything. Dropping one does not renumber the rest — every member is emitted
/// at the game's own discriminant, so the generated enum stays a subset of the
/// game's rather than a reshuffle of it.
///
/// The reason is emitted into the generated file, because a member missing for
/// a good reason and one missing by accident look identical from the Rust side.
const DROPPED: [(&str, &str, &str); 3] = [
    (
        "PowerType",
        "None",
        "no power has it and nothing in the game names it: zero references in \
         the whole assembly, its own declaration aside. It is the C# zero and \
         nothing else.",
    ),
    (
        "PowerInstanceType",
        "None",
        "252 powers have it, but all four places that read `InstanceType` ask \
         the same yes-or-no question. `PowerCmd` switches on it to decide \
         between modifying the power already there and applying another, \
         `Creature` refuses a second instance while it is set, and `HoverTip` \
         derives `IsInstanced` from it. That is the absence of an instancing \
         rule, which is `Option` here.",
    ),
    (
        "PowerStackType",
        "None",
        "one power has it, and nothing in the assembly can tell it apart from \
         `Single`: all three readers test `== Counter` and nothing else. \
         `NPower` shows an amount label for a counter; `Creature` and \
         `GetTypeForAmount` both ask `Counter && AllowNegative && amount < 0`. \
         A third state nothing distinguishes is not a state.",
    ),
];

fn dropped(enum_name: &str, member: &str) -> bool {
    DROPPED
        .iter()
        .any(|&(e, m, _)| e == enum_name && m == member)
}

/// The dump's first three columns, as the table's own fields.
///
/// The ones whose answer is a value rather than a yes or a no. Everything after
/// them is a flag; see [`FLAGS`].
const VALUES: [(&str, &str, Wrap); 3] = [
    ("kind", "PowerType", Wrap::Field),
    ("instance", "PowerInstanceType", Wrap::Option),
    ("stack", "PowerStackType", Wrap::FieldOption),
];

/// The dump's remaining columns, as the members of [`PowerFlags`].
///
/// In dump order, which is the order `PowerModel` declares them, and which
/// becomes each flag's bit. Unlike the card keywords, these are not a game
/// enum — there is nothing to inherit a number from — so this order is the
/// whole of the numbering and reordering it renumbers the set.
const FLAGS: [&str; 5] = [
    "AllowNegative",
    "RemovedOnOwnerDeath",
    "OwnerDeathIsFatal",
    "ScalesInMultiplayer",
    "OwnerIsSecondaryEnemy",
];

/// How a value column is spelled once its enum's `None` is gone.
///
/// Two different absences, and they stack. `Field::Computed` is "the power
/// works it out, so ask it"; `Option::None` is "there is no rule here at all".
/// A column can need both, and `stack` is that column: two powers compute it,
/// `DAMPEN_POWER` does not stack, and 225 are counters.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wrap {
    /// Some power computes this, so every power's cell is a `Field`.
    Field,
    /// The game's `None` meant absence, so it is an `Option` here.
    Option,
    /// Both: computed for some powers, absent for others.
    FieldOption,
}

impl Wrap {
    /// The field's type, given the enum its values come from.
    fn ty(self, inner: &str) -> String {
        match self {
            Wrap::Field => format!("Field<{inner}>"),
            Wrap::Option => format!("Option<{inner}>"),
            Wrap::FieldOption => format!("Field<Option<{inner}>>"),
        }
    }

    /// Whether a cell may say "the power works it out".
    fn computes(self) -> bool {
        matches!(self, Wrap::Field | Wrap::FieldOption)
    }

    /// Whether a cell may say "there is no rule here".
    fn absent(self) -> bool {
        matches!(self, Wrap::Option | Wrap::FieldOption)
    }
}

/// The file a power's row lands in, from its `kind` cell.
///
/// A real partition: every power has exactly one `kind`, and the ones that
/// compute it are a family rather than a leftover — see the module docs. An
/// unrecognised cell is an error rather than a fourth bucket, because a new
/// `PowerType` member is a thing to look at and not a thing to file.
fn bucket(kind: &str) -> Result<&'static str, String> {
    Ok(match kind {
        "Buff" => "buffs",
        "Debuff" => "debuffs",
        "dynamic" => "temporary",
        other => return Err(format!("no bucket for a power whose kind is {other}")),
    })
}

/// What a bucket's own module doc says.
///
/// One line, because [`banner`] puts the source and hash on the next one.
fn bucket_doc(file: &str) -> &'static str {
    match file {
        "buffs" => "Powers the game reads as a buff.",
        "debuffs" => {
            "Powers the game reads as a debuff: what Artifact eats, and what `Rend` and `Misery` count."
        }
        "temporary" => {
            "The `ITemporaryPower` family, which reads buff-or-debuff off the sign of its own amount — so `kind` is computed for every power here and no other."
        }
        other => panic!("no description for the {other} bucket"),
    }
}

/// What each column's doc comment says, beyond its name.
fn describe(field: &str) -> &'static str {
    match field {
        "kind" => {
            "Buff or debuff — what `PowerCmd` checks before Artifact eats it,\n\
                   and what `Rend` and `Misery` count.\n\
                   \n\
                   Not the same as `TypeForCurrentAmount`, which the game derives from\n\
                   this, `stack` and `AllowNegative` together: a debuff that does not\n\
                   allow negatives reads as a buff while its amount is below zero."
        }
        "instance" => {
            "Whether several of these coexist on one creature, and whether\n\
                       they are kept apart by who applied them.\n\
                       \n\
                       `None` is the game's `PowerInstanceType.None`: not instanced, so\n\
                       a second application finds the power already there and modifies\n\
                       it rather than adding another."
        }
        "stack" => {
            "How a second application combines with the first.\n\
                    \n\
                    `Constant(None)` is the game's `PowerStackType.None`, which one\n\
                    power has and which no reader distinguishes from `Single`;\n\
                    `Computed` is a power that decides at the time, like `ShrinkPower`\n\
                    switching to `Single` once its amount goes negative."
        }
        "AllowNegative" => {
            "The amount may go below zero.\n\
             \n\
             Load-bearing twice over: `ShouldRemoveDueToAmount` removes a power\n\
             at zero only when this is set, and `GetTypeForAmount` reads it to\n\
             decide whether a negative amount flips the type."
        }
        "RemovedOnOwnerDeath" => "The power goes away when its owner dies.",
        "OwnerDeathIsFatal" => {
            "The owner dying is a real death rather than something the power\n\
             intercepts.\n\
             \n\
             `REATTACH_POWER` computes it: a segment of its owner dying is only\n\
             fatal once every other segment is already dead."
        }
        "ScalesInMultiplayer" => "The applied amount scales with the party size.",
        "OwnerIsSecondaryEnemy" => {
            "The owner counts as a secondary enemy while this is on it. One\n\
             power in the build."
        }
        other => panic!("no description for {other}"),
    }
}

pub fn generate(root: &Path) -> Result<String, String> {
    let dump = Dump::read("powers.txt")?;
    let hash = dump.hash.clone();

    // Reflected from the game, so a member no power happens to use is still a
    // member — the same reason the card and monster dumps reflect their own.
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

    let powers: Vec<&Vec<String>> = dump.of("power").collect();
    if powers.is_empty() {
        return Err("the dump declares no powers".into());
    }
    let width = VALUES.len() + FLAGS.len();
    for row in &powers {
        if row.len() != width + 2 {
            return Err(format!(
                "{}: {} columns, but the table has {width}",
                row[1],
                row.len() - 2,
            ));
        }
    }

    let mut out = banner("Every power's static data.", "powers.txt", &hash);
    writeln!(out, "\nuse crate::Field;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    // Why three members the game declares are missing here. Written into the
    // generated file rather than left in the generator, because that is where
    // somebody notices the hole in the discriminants.
    for (enum_name, member, why) in DROPPED {
        writeln!(out, "// `{enum_name}::{member}` is not generated: {why}").unwrap();
    }
    writeln!(out).unwrap();

    for (name, members) in &enums {
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            if dropped(name, m) {
                continue;
            }
            // The game's own number, spelled out: a dropped member leaves a
            // hole rather than pulling everything after it down one.
            writeln!(out, "    {m} = {discriminant},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    emit_shape(&mut out);
    emit_powers(&mut out, &powers)?;

    // The rows, one file per kind. `mod.rs` keeps the shape and the enum; a
    // power's data is a named const it points at, so `data()` stays a readable
    // index instead of three thousand lines of table.
    let mut split = Split::new(root, "powers", "powers.txt", &hash)?;
    for row in &powers {
        let literal = power_literal(row)?;
        writeln!(
            split.body(bucket(&row[2])?),
            "pub(super) const {}: PowerData = {literal};\n",
            row[1]
        )
        .unwrap();
    }

    let kinds = split.names().count();
    for name in split.names() {
        writeln!(out, "mod {name};").unwrap();
    }
    writeln!(out).unwrap();

    split.finish(
        &out,
        |file| bucket_doc(file).to_string(),
        &[
            (
                "super",
                &[
                    "PowerData",
                    "PowerFlag",
                    "PowerFlags",
                    "PowerInstanceType",
                    "PowerStackType",
                    "PowerType",
                ],
            ),
            ("crate", &["Field"]),
        ],
    )?;

    let computed: usize = powers
        .iter()
        .map(|r| r[2..].iter().filter(|c| *c == "dynamic").count())
        .sum();
    println!(
        "  powers: {} across {kinds} kinds, {computed} computed cells",
        powers.len()
    );

    Ok(hash)
}

/// Doc lines for one member, indented to sit inside a type.
///
/// Blank doc lines are `///` with nothing after, not `/// `; rustfmt strips the
/// trailing space anyway and `-D warnings` does not care, but the generated
/// file reads better without it.
fn doc(out: &mut String, name: &str) {
    for line in describe(name).lines() {
        match line.trim() {
            "" => writeln!(out, "    ///").unwrap(),
            text => writeln!(out, "    /// {text}").unwrap(),
        }
    }
}

fn emit_shape(out: &mut String) {
    emit_flags(out);

    writeln!(
        out,
        "/// What is true of a power with no combat running.\n\
         ///\n\
         /// Not what it does: that is hooks, and hooks are ported by hand. These\n\
         /// are the facts the engine reads off a power without asking it\n\
         /// anything.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct PowerData {{"
    )
    .unwrap();

    for (field, ty, wrap) in VALUES {
        doc(out, field);
        writeln!(out, "    pub {field}: {},", wrap.ty(ty)).unwrap();
    }

    writeln!(
        out,
        "    /// The yes-or-no facts, which are a set rather than five fields\n\
         \x20   /// in a row nothing keeps in order."
    )
    .unwrap();
    writeln!(out, "    pub flags: PowerFlags,").unwrap();
    writeln!(out, "}}\n").unwrap();
}

/// The flag member enum and the set that carries them.
///
/// Two `u8`s rather than one, because a flag has three answers and not two:
/// `PowerModel` declares every one of these `virtual`, so any of them can be
/// worked out from combat state, and two are. Keeping the computed ones in
/// their own word is what lets [`PowerFlags::get`] hand back a `Field<bool>`
/// instead of quietly answering `false` for a question the table cannot answer.
fn emit_flags(out: &mut String) {
    writeln!(
        out,
        "/// A yes-or-no fact about a power.\n\
         ///\n\
         /// Five separate `PowerModel` overrides rather than a game enum, so\n\
         /// unlike the card keywords there is no discriminant to inherit. The\n\
         /// numbering is the order the dump writes the columns in, which is the\n\
         /// order `PowerModel` declares them.\n\
         {DERIVES}\n\
         #[repr(u8)]\n\
         pub enum PowerFlag {{"
    )
    .unwrap();
    for (bit, flag) in FLAGS.iter().enumerate() {
        doc(out, flag);
        writeln!(out, "    {flag} = {bit},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// The flags a power has, and the ones it works out at the time.\n\
         ///\n\
         /// Two words, because a flag is true, false, or computed. A single\n\
         /// bitset would have to spell the third as one of the first two, and\n\
         /// there is no honest choice between them — so `get` returns a\n\
         /// [`Field<bool>`] and there is no way to ask the question that\n\
         /// cannot be answered.\n\
         ///\n\
         /// [`Field<bool>`]: crate::Field\n\
         #[derive(Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct PowerFlags {{\n\
         \x20   /// The flags that are set. A computed flag is never in here.\n\
         \x20   yes: u8,\n\
         \x20   /// The flags the power decides at the time.\n\
         \x20   computed: u8,\n\
         }}\n\
         \n\
         impl PowerFlags {{\n\
         \x20   /// Every flag, in bit order.\n\
         \x20   const MEMBERS: [PowerFlag; {n}] = [{members}];\n\
         \n\
         \x20   /// The flags a power sets, and the ones it computes.\n\
         \x20   ///\n\
         \x20   /// A flag in both lists is a contradiction, and this is a `const\n\
         \x20   /// fn` called from a `const`, so saying so fails the build rather\n\
         \x20   /// than the run.\n\
         \x20   pub const fn new(yes: &[PowerFlag], computed: &[PowerFlag]) -> Self {{\n\
         \x20       let mut set = 0u8;\n\
         \x20       let mut i = 0;\n\
         \x20       while i < yes.len() {{\n\
         \x20           set |= 1u8 << yes[i] as u8;\n\
         \x20           i += 1;\n\
         \x20       }}\n\
         \n\
         \x20       let mut asked = 0u8;\n\
         \x20       i = 0;\n\
         \x20       while i < computed.len() {{\n\
         \x20           asked |= 1u8 << computed[i] as u8;\n\
         \x20           i += 1;\n\
         \x20       }}\n\
         \n\
         \x20       assert!(\n\
         \x20           set & asked == 0,\n\
         \x20           \"a flag cannot be both set and computed\"\n\
         \x20       );\n\
         \x20       Self {{ yes: set, computed: asked }}\n\
         \x20   }}\n\
         \n\
         \x20   /// One flag: the answer, or the cue to ask the power for it.\n\
         \x20   pub const fn get(self, flag: PowerFlag) -> Field<bool> {{\n\
         \x20       let bit = 1u8 << flag as u8;\n\
         \x20       if self.computed & bit != 0 {{\n\
         \x20           return Field::Computed;\n\
         \x20       }}\n\
         \x20       Field::Constant(self.yes & bit != 0)\n\
         \x20   }}\n\
         }}\n\
         \n\
         impl std::fmt::Debug for PowerFlags {{\n\
         \x20   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n\
         \x20       let mut set = f.debug_set();\n\
         \x20       let mut i = 0;\n\
         \x20       while i < Self::MEMBERS.len() {{\n\
         \x20           let flag = Self::MEMBERS[i];\n\
         \x20           match self.get(flag) {{\n\
         \x20               Field::Constant(true) => {{\n\
         \x20                   set.entry(&flag);\n\
         \x20               }}\n\
         \x20               Field::Computed => {{\n\
         \x20                   set.entry(&format_args!(\"{{flag:?}}: computed\"));\n\
         \x20               }}\n\
         \x20               Field::Constant(false) => {{}}\n\
         \x20           }}\n\
         \x20           i += 1;\n\
         \x20       }}\n\
         \x20       set.finish()\n\
         \x20   }}\n\
         }}\n",
        n = FLAGS.len(),
        members = FLAGS
            .iter()
            .map(|f| format!("PowerFlag::{f}"))
            .collect::<Vec<_>>()
            .join(", "),
    )
    .unwrap();
}

/// One value cell, as the literal it should generate.
///
/// Every way a cell could fail to fit its column is an error rather than a
/// value. A `dynamic` in a column with no `Field` would generate a constant
/// that is not one; a dropped member in a column with no `Option` has no
/// spelling at all and would otherwise emit `PowerType::None`, naming a variant
/// that no longer exists. Both name the power and the column, because the fix
/// is always to change [`VALUES`] or [`DROPPED`] rather than to look away.
fn cell(power: &str, field: &str, ty: &str, wrap: Wrap, raw: &str) -> Result<String, String> {
    if raw == "dynamic" {
        if !wrap.computes() {
            return Err(format!(
                "{power}.{field} is computed, but the column is not a Field; change VALUES"
            ));
        }
        return Ok("Field::Computed".into());
    }

    if dropped(ty, raw) {
        if !wrap.absent() {
            return Err(format!(
                "{power}.{field} is {ty}::{raw}, which is dropped and the column \
                 has no Option to put it in; change VALUES or DROPPED"
            ));
        }
        return Ok(match wrap {
            Wrap::FieldOption => "Field::Constant(None)".into(),
            _ => "None".into(),
        });
    }

    let value = format!("{ty}::{raw}");

    Ok(match wrap {
        Wrap::Field => format!("Field::Constant({value})"),
        Wrap::Option => format!("Some({value})"),
        Wrap::FieldOption => format!("Field::Constant(Some({value}))"),
    })
}

fn emit_powers(out: &mut String, powers: &[&Vec<String>]) -> Result<(), String> {
    writeln!(out, "/// Every power in the build.").unwrap();
    writeln!(
        out,
        "///\n\
         /// Flat, because powers have no pools: a card belongs to one character's\n\
         /// deck and a power belongs to whatever is carrying it. Ordered by the\n\
         /// game's own id, which is the order the dump wrote them in.\n\
         ///\n\
         /// The generated files still group by `kind`, because that is the one\n\
         /// thing the game classifies a power by. It is a filing decision and\n\
         /// not a type: a power's kind is a [`Field`] and twenty powers work it\n\
         /// out at the time. See `data`.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(powers.len())?).unwrap();
    writeln!(out, "pub enum Power {{").unwrap();
    for row in powers {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl Power {{").unwrap();

    writeln!(
        out,
        "    /// Every power, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    pub const ALL: [Power; {}] = [", powers.len()).unwrap();
    for row in powers {
        writeln!(out, "        Power::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    // By value, like `CardData`: the struct fits in a register, so a reference
    // would be larger than the thing it points at.
    writeln!(
        out,
        "    /// What is true of the power with no combat running.\n\
         \x20   ///\n\
         \x20   /// One arm per power, naming the file its row lives in — buffs,\n\
         \x20   /// debuffs, and the temporary family that decides at the time."
    )
    .unwrap();
    writeln!(out, "    pub const fn data(self) -> PowerData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in powers {
        writeln!(
            out,
            "            Self::{} => {}::{},",
            camel(&row[1]),
            bucket(&row[2])?,
            row[1]
        )
        .unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(())
}

/// One power's row, as the `const` its bucket file declares.
fn power_literal(row: &[String]) -> Result<String, String> {
    let mut out = String::new();
    writeln!(out, "PowerData {{").unwrap();
    for (i, (field, ty, wrap)) in VALUES.iter().enumerate() {
        writeln!(
            out,
            "    {field}: {},",
            cell(&row[1], field, ty, *wrap, &row[i + 2])?
        )
        .unwrap();
    }

    // The dump spells a flag `1` or `0` rather than `true` or `false`, so that
    // a cell reading `dynamic` is visibly not a value in the same alphabet.
    let (mut yes, mut computed) = (Vec::new(), Vec::new());
    for (i, flag) in FLAGS.iter().enumerate() {
        match row[i + 2 + VALUES.len()].as_str() {
            "1" => yes.push(format!("PowerFlag::{flag}")),
            "0" => {}
            "dynamic" => computed.push(format!("PowerFlag::{flag}")),
            other => return Err(format!("{}.{flag}: {other} is not a flag", row[1])),
        }
    }
    writeln!(
        out,
        "    flags: PowerFlags::new(&[{}], &[{}]),",
        yes.join(", "),
        computed.join(", ")
    )
    .unwrap();

    write!(out, "}}").unwrap();
    Ok(out)
}
