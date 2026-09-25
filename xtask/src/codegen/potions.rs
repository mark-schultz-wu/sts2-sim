//! Every potion's static data, and which pool stocks it.
//!
//! What a potion does is `OnUse`, which 63 of the 65 override, and that is
//! ported by hand. What is table is where it comes from, when it can be drunk,
//! and what it can be pointed at.
//!
//! The xtask README says what each of the game's two `None`s becomes, and why
//! the pools partition.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::DumpError;
use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr, sets};
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
    #[error("{potion} is stocked by both {first} and {second}, which the table has no shape for")]
    TwoPools {
        potion: String,
        first: String,
        second: String,
    },
    #[error("{0} is in no pool at all")]
    NoPool(String),
    #[error("{potion}: {field} is computed, which the table cannot spell")]
    Computed { potion: String, field: &'static str },
    #[error("{potion} has {usage} usage, which is not generated because nothing should have it")]
    DroppedUsage { potion: String, usage: String },
    #[error("{potion}.{flag}: {value} is not a flag")]
    NotAFlag {
        potion: String,
        flag: String,
        value: String,
    },
    #[error("no shared potion pool")]
    NoSharedPool,
    #[error("{character} is stocked from {pool}, which has no rows")]
    EmptyPool { character: String, pool: String },
    #[error("no playable character to stock a reward for")]
    NoPlayable,
}

/// Members the game declares that are not values of the field they name.
const DROPPED: [(&str, &str, &str); 2] = [
    (
        "PotionUsage",
        "None",
        "no potion has it, and `NPotionPopup` throws \"No potions should have \
         'None' usage.\" on the branch that would handle one. The game says it \
         should not exist and the build agrees, so it is not generated.",
    ),
    (
        "PotionRarity",
        "None",
        "`PotionRarityExtensions.ToLocString` throws \
         `ArgumentOutOfRangeException` on it, so it is not a rarity the game \
         can name. Two models carry it — `DEPRECATED_POTION` and a mock — and \
         neither is content, so it is `Option<PotionRarity>` here: no rarity, \
         rather than a rarity called none.",
    ),
];

/// The yes-or-no columns, in the order they become bits.
const FLAGS: [(&str, usize, &str); 2] = [
    (
        "GeneratedInCombat",
        5,
        "A card or a relic may conjure one mid-fight. False for three:\n\
         Fairy in a Bottle, Fruit Juice and Regen Potion.",
    ),
    (
        "PassesUsabilityCheck",
        6,
        "Whether the potion has anything to say about being drinkable\n\
         beyond its `usage`. True for every potion but `FOUL_POTION`,\n\
         which works it out.",
    ),
];

fn dropped(enum_name: &str, member: &str) -> bool {
    DROPPED
        .iter()
        .any(|&(e, m, _)| e == enum_name && m == member)
}

fn bucket(pool: &str) -> String {
    pool.strip_suffix("_POTION_POOL")
        .unwrap_or(pool)
        .to_lowercase()
}

fn bucket_doc(file: &str) -> String {
    format!("Potions stocked by `{}_POTION_POOL`.", file.to_uppercase())
}

pub fn generate(root: &Path) -> Result<Build, Error> {
    let dump = Dump::read("potions.txt")?;
    let build = dump.build.clone();

    let potions: Vec<&Vec<String>> = dump.of("potion").collect();
    if potions.is_empty() {
        return Err(DumpError::Empty("potions").into());
    }
    for row in &potions {
        if row.len() != 7 {
            return Err(DumpError::Columns {
                id: row[1].clone(),
                got: row.len() - 1,
                want: 6,
            }
            .into());
        }
    }

    let mut enums: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    for f in dump.of("enum") {
        let discriminant =
            DumpError::number::<u8>(format_args!("{} {}: discriminant", f[1], f[3]), &f[2])?;
        enums
            .entry(f[1].clone())
            .or_default()
            .push((discriminant, f[3].clone()));
    }

    // Which pool stocks a potion. Exactly one each, which is what makes the
    // buckets a partition — so a second is an error rather than a crossover
    // file nothing would ever land in.
    let mut pools: Vec<String> = Vec::new();
    let mut pool_of: BTreeMap<String, String> = BTreeMap::new();
    // The dump's own order, which is the pool's. A reward indexes into a pool
    // after filtering by rarity, so this is not the id order the flat table is
    // in: the Regent's three potions are dealt Star, Cosmic, King's.
    let mut stocked: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in dump.of("pool") {
        if !pools.contains(&row[1]) {
            pools.push(row[1].clone());
        }
        stocked
            .entry(row[1].clone())
            .or_default()
            .push(row[3].clone());
        if let Some(first) = pool_of.insert(row[3].clone(), row[1].clone()) {
            return Err(Error::TwoPools {
                potion: row[3].clone(),
                first,
                second: row[1].clone(),
            });
        }
    }
    for row in &potions {
        if !pool_of.contains_key(&row[1]) {
            return Err(Error::NoPool(row[1].clone()));
        }
    }

    let mut out = banner(
        "Every potion's rarity, usage and target.",
        "potions.txt",
        &build,
    );
    writeln!(out, "\nuse super::cards::TargetType;").unwrap();
    writeln!(out, "use super::characters::Character;").unwrap();
    writeln!(out, "use sts2_core::Drafted;").unwrap();
    writeln!(out, "use crate::Field;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    for (enum_name, member, why) in DROPPED {
        writeln!(out, "// `{enum_name}::{member}` is not generated: {why}").unwrap();
    }
    writeln!(out).unwrap();

    writeln!(out, "/// The pools a potion can be stocked by.").unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(pools.len())?).unwrap();
    writeln!(out, "pub enum PotionPool {{").unwrap();
    for (i, pool) in pools.iter().enumerate() {
        writeln!(out, "    #[serde(rename = \"{pool}\")]").unwrap();
        writeln!(
            out,
            "    {} = {i},",
            camel(pool.strip_suffix("_POTION_POOL").unwrap_or(pool))
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    // `TargetType` belongs to the cards domain and is used here as it stands.
    for (name, members) in enums.iter().filter(|(n, _)| n.starts_with("Potion")) {
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            if dropped(name, m) {
                continue;
            }
            writeln!(out, "    {m} = {discriminant},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    sets::computed(
        &mut out,
        &sets::Set {
            flag: "PotionFlag",
            set: "PotionFlags",
            flag_doc: "\n\
                       Two separate `PotionModel` members rather than a game enum, so the\n\
                       numbering is the order the dump writes the columns in.",
            set_doc: "The flags a potion has, and the one it works out at the time.\n\
                      \n\
                      `PassesCustomUsabilityCheck` is `virtual` and `FOUL_POTION`\n\
                      overrides it, which is the whole reason this is two words.",
            members: &sets::counted(FLAGS.map(|(name, _, doc)| (name, doc))),
            declare: true,
            extras: &[],
        },
    )?;

    emit_shape(&mut out);
    emit_potions(&mut out, &potions, &pool_of)?;
    emit_stock(&mut out, &pools, &stocked);
    emit_rewards(&mut out, &stocked, &potions)?;

    let mut split = Split::new(root, "potions", "potions.txt", &build)?;
    for row in &potions {
        let literal = literal(row)?;
        writeln!(
            split.body(&bucket(&pool_of[&row[1]])),
            "pub(super) const {}: PotionData = {literal};\n",
            row[1]
        )
        .unwrap();
    }

    let files = split.names().count();
    for name in split.names() {
        writeln!(out, "mod {name};").unwrap();
    }
    writeln!(out).unwrap();

    split.finish(
        &out,
        bucket_doc,
        &[
            (
                "super",
                &[
                    "PotionData",
                    "PotionFlag",
                    "PotionFlags",
                    "PotionRarity",
                    "PotionUsage",
                ],
            ),
            ("super::super::cards", &["TargetType"]),
            ("crate", &["Field"]),
        ],
    )?;

    println!(
        "  potions: {} across {} pools in {files} files",
        potions.len(),
        pools.len()
    );

    Ok(build)
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// What is true of a potion with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct PotionData {{\n\
         \x20   /// How often one turns up. `None` is the game's\n\
         \x20   /// `PotionRarity.None`, which `ToLocString` throws on — so it is\n\
         \x20   /// the absence of a rarity, and only a deprecated model and a\n\
         \x20   /// mock have it.\n\
         \x20   pub rarity: Option<PotionRarity>,\n\
         \x20   /// When it can be drunk. Sixty of the sixty-five are combat\n\
         \x20   /// only.\n\
         \x20   pub usage: PotionUsage,\n\
         \x20   /// What it can be pointed at, which is the cards domain's own\n\
         \x20   /// `TargetType`: a potion targets the way a card does.\n\
         \x20   pub target: Field<TargetType>,\n\
         \x20   pub flags: PotionFlags,\n\
         }}\n"
    )
    .unwrap();
}

fn literal(row: &[String]) -> Result<String, Error> {
    let id = &row[1];

    let rarity = match row[2].as_str() {
        "dynamic" => {
            return Err(Error::Computed {
                potion: id.to_owned(),
                field: "rarity",
            });
        }
        r if dropped("PotionRarity", r) => "None".to_string(),
        r => format!("Some(PotionRarity::{r})"),
    };

    let usage = match row[3].as_str() {
        "dynamic" => {
            return Err(Error::Computed {
                potion: id.to_owned(),
                field: "usage",
            });
        }
        u if dropped("PotionUsage", u) => {
            return Err(Error::DroppedUsage {
                potion: id.to_owned(),
                usage: u.to_owned(),
            });
        }
        u => format!("PotionUsage::{u}"),
    };

    let target = match row[4].as_str() {
        "dynamic" => "Field::Computed".to_string(),
        // `Self` is a reserved word, and the cards domain already renamed it.
        "Self" => "Field::Constant(TargetType::SelfTarget)".to_string(),
        t => format!("Field::Constant(TargetType::{t})"),
    };

    let (mut yes, mut computed) = (Vec::new(), Vec::new());
    for (name, column, _) in FLAGS {
        match row[column].as_str() {
            "1" => yes.push(format!("PotionFlag::{name}")),
            "0" => {}
            "dynamic" => computed.push(format!("PotionFlag::{name}")),
            other => {
                return Err(Error::NotAFlag {
                    potion: id.to_owned(),
                    flag: name.to_string(),
                    value: other.to_owned(),
                });
            }
        }
    }

    let mut out = String::new();
    writeln!(out, "PotionData {{").unwrap();
    writeln!(out, "    rarity: {rarity},").unwrap();
    writeln!(out, "    usage: {usage},").unwrap();
    writeln!(out, "    target: {target},").unwrap();
    writeln!(
        out,
        "    flags: PotionFlags::new(&[{}], &[{}]),",
        yes.join(", "),
        computed.join(", ")
    )
    .unwrap();
    write!(out, "}}").unwrap();
    Ok(out)
}

/// The three questions a potion answers straight off its own row.
///
/// One body each rather than a match per potion, because every answer is a
/// field the table already holds. The generated `PotionUsage` is the game's
/// enum and the trait's is the reduction a rule asks for, so the two are
/// spelled apart.
const READS_ITS_ROW: &str = r#"
    /// When it can be drunk at all.
    fn usage(self) -> sts2_core::PotionUsage {
        match sts2_core::Table::data(self).usage {
            PotionUsage::CombatOnly => sts2_core::PotionUsage::CombatOnly,
            PotionUsage::AnyTime => sts2_core::PotionUsage::AnyTime,
            PotionUsage::Automatic => sts2_core::PotionUsage::Automatic,
        }
    }

    /// What it can be pointed at, for one that prints a target type.
    fn target(self) -> Option<TargetType> {
        match sts2_core::Table::data(self).target {
            Field::Constant(target) => Some(target),
            Field::Computed => None,
        }
    }

    /// Whether its own check lets it be drunk, for one that does not work
    /// that out from a live fight.
    fn passes_own_check(self) -> Option<bool> {
        match sts2_core::Table::data(self)
            .flags
            .get(PotionFlag::PassesUsabilityCheck)
        {
            Field::Constant(passes) => Some(passes),
            Field::Computed => None,
        }
    }
"#;

fn emit_potions(
    out: &mut String,
    potions: &[&Vec<String>],
    pool_of: &BTreeMap<String, String>,
) -> Result<(), Error> {
    writeln!(
        out,
        "/// Every potion in the build.\n\
         ///\n\
         /// Flat, and ordered by the game's own id. The generated files group by\n\
         /// pool, which is what a character is stocked from.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(potions.len())?).unwrap();
    writeln!(out, "pub enum Potion {{").unwrap();
    for row in potions {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    let mut extras = String::new();
    writeln!(
        extras,
        "    /// The pool that stocks this potion.\n\
         \x20   ///\n\
         \x20   /// Exactly one, unlike a relic's: codegen refuses a potion in two\n\
         \x20   /// pools or in none."
    )
    .unwrap();
    writeln!(extras, "    fn pool(self) -> PotionPool {{").unwrap();
    writeln!(extras, "        match self {{").unwrap();
    for row in potions {
        let pool = &pool_of[&row[1]];
        writeln!(
            extras,
            "            Self::{} => PotionPool::{},",
            camel(&row[1]),
            camel(pool.strip_suffix("_POTION_POOL").unwrap_or(pool))
        )
        .unwrap();
    }
    writeln!(extras, "        }}").unwrap();
    writeln!(extras, "    }}").unwrap();

    writeln!(out, "impl sts2_core::Table for Potion {{").unwrap();
    writeln!(out, "    type Data = PotionData;\n").unwrap();
    writeln!(
        out,
        "    /// Every potion, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    const ALL: &'static [Self] = &[").unwrap();
    for row in potions {
        writeln!(out, "        Potion::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(
        out,
        "    /// The rarity, the usage, the target and the flags."
    )
    .unwrap();
    let mut table: Vec<String> = Vec::new();
    for row in potions {
        table.push(format!("{}::{}", bucket(&pool_of[&row[1]]), row[1]));
    }
    writeln!(out, "    fn data(self) -> &'static PotionData {{").unwrap();
    writeln!(out, "        &ROWS[self as usize]").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    super::rows(out, "PotionData", &table);

    writeln!(out, "impl sts2_core::PotionTable for Potion {{").unwrap();
    writeln!(out, "    type Pool = PotionPool;").unwrap();
    writeln!(out, "    type Target = TargetType;\n").unwrap();
    out.push_str(&extras);
    out.push_str(READS_ITS_ROW);
    writeln!(out, "}}").unwrap();

    Ok(())
}

/// What a pool stocks, in the order the game enumerates it.
///
/// A potion reward filters a pool by rarity and then indexes into what is
/// left. So this order is observable, and the flat table's id order is not.
fn emit_stock(out: &mut String, pools: &[String], stocked: &BTreeMap<String, Vec<String>>) {
    writeln!(
        out,
        "/// The potions a pool stocks, in the order the game enumerates them.\n\
         ///\n\
         /// `PotionPoolModel.AllPotions`, which a reward indexes into after\n\
         /// filtering by rarity. Not the id order [`Potion`] is in."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn pool_potions(pool: PotionPool) -> &'static [Potion] {{"
    )
    .unwrap();
    writeln!(out, "    match pool {{").unwrap();
    for pool in pools {
        writeln!(
            out,
            "        PotionPool::{} => &[",
            camel(pool.strip_suffix("_POTION_POOL").unwrap_or(pool))
        )
        .unwrap();
        for potion in &stocked[pool] {
            writeln!(out, "            Potion::{},", camel(potion)).unwrap();
        }
        writeln!(out, "        ],").unwrap();
    }
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
}

/// The potions a reward can roll for each playable character.
///
/// `GetPotionOptions`: the character's own pool and then the shared one, each
/// in its own order. Joined here rather than in the simulator, so a fight ends
/// by reading a slice instead of building a list.
///
/// The character's pool comes from the characters dump, which is the same
/// place the character's row gets it.
fn emit_rewards(
    out: &mut String,
    stocked: &BTreeMap<String, Vec<String>>,
    potions: &[&Vec<String>],
) -> Result<(), Error> {
    let characters = Dump::read("characters.txt")?;
    let shared = stocked
        .get("SHARED_POTION_POOL")
        .ok_or(Error::NoSharedPool)?;

    let rarity_of: BTreeMap<&str, &str> = potions
        .iter()
        .map(|row| (row[1].as_str(), row[2].as_str()))
        .collect();

    writeln!(
        out,
        "/// The potions a reward can roll, split by rarity.\n\
         ///\n\
         /// `GetPotionOptions`: the character's own pool and then the shared\n\
         /// one, each slice keeping that order so an index means what it meant\n\
         /// before the split. Empty for a character no run is played as."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn reward_potions(character: Character) -> Drafted<'static, Potion> {{"
    )
    .unwrap();
    writeln!(out, "    match character {{").unwrap();
    let mut named = 0;
    for row in characters.of("character") {
        if row[2] != "1" {
            continue;
        }
        named += 1;
        let own = stocked.get(&row[11]).ok_or_else(|| Error::EmptyPool {
            character: row[1].clone(),
            pool: row[11].clone(),
        })?;
        writeln!(out, "        Character::{} => Drafted {{", camel(&row[1])).unwrap();
        for (field, rarity) in [
            ("common", "Common"),
            ("uncommon", "Uncommon"),
            ("rare", "Rare"),
        ] {
            writeln!(out, "            {field}: &[").unwrap();
            for potion in own.iter().chain(shared) {
                if rarity_of.get(potion.as_str()).copied() != Some(rarity) {
                    continue;
                }
                writeln!(out, "                Potion::{},", camel(potion)).unwrap();
            }
            writeln!(out, "            ],").unwrap();
        }
        writeln!(out, "        }},").unwrap();
    }
    if named == 0 {
        return Err(Error::NoPlayable);
    }
    writeln!(
        out,
        "        _ => Drafted {{\n\
         \x20           common: &[],\n\
         \x20           uncommon: &[],\n\
         \x20           rare: &[],\n\
         \x20       }},"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    Ok(())
}
