//! Every relic's static data, and which pool drafts it.
//! A relic is a permanent passive, and nearly all of what one *does* is hooks, ported by hand. What is table is where it comes from, what it costs, and the handful of yes-or-no facts the engine reads off it without asking.
//! # `Price` rather than a number
//! `RelicModel.MerchantCost` returns 999999999 for a relic that is not for sale. This is a sentinel and not a price, nothing must do arithmetic on it. A shop comparing gold against it is comparing against a number that means "never". So it is `Price::NotSold`, the same move `Cost::None` makes for a card's -1.
//! It has to be read per relic rather than derived from rarity. The base implementation maps Ancient, Starter and Event to the sentinel, but nine relics override `MerchantCost` outright. Therefore of the 35 Event-rarity relics, 26 are unsellable and 9 have a real price. Deriving has got those nine wrong.
//! The price is flat. Both properties were read at all eleven ascension levels and nothing moved, so unlike an encounter's gold there is no `Scaled` here. Which is a measurement rather than an assumption, and the reason the dump sweeps at all.
//! # `Undrafted` rather than `None`
//! `RelicRarity.None` is not the C# zero standing in for "unset": two relics carry it, and it means the rarity system does not reach them. `RelicGrabBag` gives it no deque, `rarity == None ? Null : GetDeque(rarity)` , so no roll can produce one. They arrive only when something hands them over. `CIRCLET` is the fallback pool's only member, which is what you get when there is nothing left to get, and `DEPRECATED_RELIC` is the deprecated pool's. The wire keeps the game's spelling. The Rust name says what it does.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::sets;
use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr};

/// The sentinel `MerchantCost` returns for a relic that is not for sale.
const NOT_SOLD: &str = "999999999";

/// Enum members whose Rust name differs from the game's.
///
/// The rename is Rust-side only: the serde spelling stays the game's. Therefore the wire and the dump are unchanged and the generated file is still diffable against the assembly.
const RENAMED: [(&str, &str, &str); 1] = [("RelicRarity", "None", "Undrafted")];

/// The yes-or-no columns, in the order they become bits.
///
/// Not a game enum, nine separate `RelicModel` members, so the numbering is ours and this order is the whole of it. `column` is where the dump writes each one.
const FLAGS: [(&str, usize, &str); 9] = [
    ("UsedUp", 4, "Spent, and gone once it fires."),
    (
        "UponPickup",
        5,
        "Does something the moment it is picked up, rather than in combat.",
    ),
    ("SpawnsPets", 6, "Brings a pet along."),
    (
        "Stackable",
        7,
        "More than one can be held. Two relics stack, both of them\n\
         `Undrafted`.",
    ),
    ("AddsPet", 8, "Adds a pet to the party."),
    (
        "AllowedInShops",
        9,
        "A merchant may stock it. True of all but five.",
    ),
    (
        "ShowsCounter",
        10,
        "Displays a number, which is `display_amount`.",
    ),
    (
        "GatedInRun",
        12,
        "Has a condition on being offered at all: `IsAllowed` takes the\n\
         run state and 20 relics override it. What the condition *is* is\n\
         code and gets ported by hand; that there is one is the fact a\n\
         reward roll needs before it can offer anything.",
    ),
    (
        "GatedAtNeow",
        13,
        "The same, for Neow's opening choice. Two relics override it.",
    ),
];

/// The bucket a relic's row lands in, from the pools that draft it.
///
/// A genuine partition: every relic is in at least one pool, and three are in two. Multi-pool relics get their own bucket instead of an arbitrary first pool. Each bucket therefore agrees with `pools()`.
///
/// `crossover` and not `shared`, because `SHARED_RELIC_POOL` is itself a pool and a file named for both will name neither.
fn bucket(pools: &[String]) -> Result<String, String> {
    match pools {
        [] => Err("a relic is in no pool at all".into()),
        [_, _, ..] => Ok("crossover".into()),
        [only] => Ok(only
            .strip_suffix("_RELIC_POOL")
            .unwrap_or(only)
            .to_lowercase()),
    }
}

fn bucket_doc(file: &str) -> String {
    match file {
        "crossover" => "Relics more than one pool drafts.".into(),
        pool => format!("Relics only `{}_RELIC_POOL` drafts.", pool.to_uppercase()),
    }
}

/// What the game calls a member, as this crate spells it.
fn rust_member(enum_name: &str, member: &str) -> String {
    RENAMED
        .iter()
        .find(|&&(e, m, _)| e == enum_name && m == member)
        .map_or_else(|| member.to_string(), |&(_, _, rust)| rust.to_string())
}

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("relics.txt")?;
    let build = dump.build.clone();

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

    let width = 14;
    let relics: Vec<&Vec<String>> = dump.of("relic").collect();
    if relics.is_empty() {
        return Err("the dump declares no relics".into());
    }
    for row in &relics {
        if row.len() != width {
            return Err(format!(
                "{}: {} columns, but the table has {}",
                row[1],
                row.len() - 1,
                width - 1
            ));
        }
    }

    // The dump swept `MerchantCost` at every ascension and found nothing, which
    // is why `Price` is flat. A build that starts scaling it will generate a
    // table that silently describes ascension 0 only, so it fails here instead.
    if dump.of("scalecost").next().is_some() {
        return Err(
            "a relic's merchant cost now scales with ascension, which Price cannot spell".into(),
        );
    }

    let mut pools: Vec<String> = Vec::new();
    let mut pools_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in dump.of("pool") {
        if !pools.contains(&row[1]) {
            pools.push(row[1].clone());
        }
        pools_of
            .entry(row[2].clone())
            .or_default()
            .push(row[1].clone());
    }

    let mut out = banner(
        "Every relic's rarity, price and flags.",
        "relics.txt",
        &build,
    );
    writeln!(out, "\nuse crate::Field;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    writeln!(out, "/// The pools a relic can be drafted from.").unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(pools.len())?).unwrap();
    writeln!(out, "pub enum RelicPool {{").unwrap();
    for (i, pool) in pools.iter().enumerate() {
        writeln!(out, "    #[serde(rename = \"{pool}\")]").unwrap();
        writeln!(
            out,
            "    {} = {i},",
            camel(pool.strip_suffix("_RELIC_POOL").unwrap_or(pool))
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    for (name, members) in &enums {
        for (_, member, rust) in RENAMED.iter().filter(|&&(e, _, _)| e == name) {
            writeln!(
                out,
                "// `{name}::{member}` is generated as `{rust}`: it is not the C# zero standing in \
                 for an unset value. Two relics carry it, and `RelicGrabBag` gives it no deque — \
                 `rarity == None ? null : GetDeque(rarity)` — so no roll can produce one. The \
                 serde spelling stays the game's."
            )
            .unwrap();
        }
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            let rust = rust_member(name, m);
            if rust != *m {
                writeln!(out, "    #[serde(rename = \"{m}\")]").unwrap();
            }
            writeln!(out, "    {rust} = {discriminant},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    emit_flags(&mut out)?;
    emit_shape(&mut out);
    emit_relics(&mut out, &relics, &pools_of)?;

    let mut split = Split::new(root, "relics", "relics.txt", &build)?;
    for row in &relics {
        let empty = Vec::new();
        let of = pools_of.get(&row[1]).unwrap_or(&empty);
        let literal = literal(row)?;
        writeln!(
            split.body(&bucket(of)?),
            "pub(super) const {}: RelicData = {literal};\n",
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
                    "Price",
                    "RelicData",
                    "RelicFlag",
                    "RelicFlags",
                    "RelicRarity",
                ],
            ),
            ("crate", &["Field"]),
        ],
    )?;

    let not_sold = relics.iter().filter(|r| r[3] == NOT_SOLD).count();
    println!(
        "  relics: {} across {} pools in {files} files, {not_sold} not for sale",
        relics.len(),
        pools.len()
    );

    Ok(build)
}

/// The flag member enum and the set that carries them.
///
/// Two `u8`s, like a power's, because a flag has three answers: `IsUsedUp` and `ShowCounter` are `virtual` and some relics work them out.
fn emit_flags(out: &mut String) -> Result<(), String> {
    sets::computed(
        out,
        &sets::Set {
            flag: "RelicFlag",
            set: "RelicFlags",
            flag_doc: "\n\
                       Nine separate `RelicModel` members rather than a game enum, so the\n\
                       numbering is the order the dump writes the columns in.",
            set_doc: "The flags a relic has, and the ones it works out at the time.\n\
                      \n\
                      `IsUsedUp` and `ShowCounter` are `virtual` and some relics decide\n\
                      them from combat state — ten and eighteen of them.",
            members: &FLAGS.map(|(name, _, doc)| (name, doc)),
            declare: true,
            bits: FLAGS.len() - 1,
            extras: &[],
        },
    )
}

fn emit_shape(out: &mut String) {
    writeln!(
        out,
        "/// What a merchant charges, or that there is no such thing.\n\
         ///\n\
         /// `MerchantCost` returns 999999999 for a relic that is not for sale,\n\
         /// which is a sentinel and not a price: nothing must do arithmetic\n\
         /// on it, and a shop comparing a player's gold against it is\n\
         /// comparing against a number that means never.\n\
         ///\n\
         /// Read per relic rather than derived from rarity. The base\n\
         /// implementation maps Ancient, Starter and Event to the sentinel, but\n\
         /// nine relics override it, so nine of the 35 Event-rarity relics do\n\
         /// have a price.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub enum Price {{\n\
         \x20   Gold(u16),\n\
         \x20   NotSold,\n\
         }}\n\
         \n\
         /// What is true of a relic with no run in progress.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct RelicData {{\n\
         \x20   pub rarity: RelicRarity,\n\
         \x20   /// Flat: both cost properties were read at all eleven ascension\n\
         \x20   /// levels and neither moved.\n\
         \x20   pub price: Price,\n\
         \x20   pub flags: RelicFlags,\n\
         \x20   /// The number a relic with `ShowsCounter` displays.\n\
         \x20   ///\n\
         \x20   /// `Constant(0)` or `Computed` and nothing else: every relic\n\
         \x20   /// that shows a number works it out, and the 260 that do not\n\
         \x20   /// show one all report zero.\n\
         \x20   pub display_amount: Field<u16>,\n\
         }}\n"
    )
    .unwrap();
}

/// One relic's row, as the `const` its bucket file declares.
fn literal(row: &[String]) -> Result<String, String> {
    let id = &row[1];

    let price = match row[3].as_str() {
        NOT_SOLD => "Price::NotSold".to_string(),
        "dynamic" => {
            return Err(format!(
                "{id}: merchant cost is computed, which Price cannot spell"
            ));
        }
        n => format!(
            "Price::Gold({})",
            n.parse::<u16>()
                .map_err(|e| format!("{id}: merchant cost {n}: {e}"))?
        ),
    };

    let (mut yes, mut computed) = (Vec::new(), Vec::new());
    for (name, column, _) in FLAGS {
        match row[column].as_str() {
            "1" => yes.push(format!("RelicFlag::{name}")),
            "0" => {}
            "dynamic" => computed.push(format!("RelicFlag::{name}")),
            other => return Err(format!("{id}.{name}: {other} is not a flag")),
        }
    }

    let display = match row[11].as_str() {
        "dynamic" => "Field::Computed".to_string(),
        n => format!(
            "Field::Constant({})",
            n.parse::<u16>()
                .map_err(|e| format!("{id}: display amount {n}: {e}"))?
        ),
    };

    let mut out = String::new();
    writeln!(out, "RelicData {{").unwrap();
    writeln!(
        out,
        "    rarity: RelicRarity::{},",
        rust_member("RelicRarity", &row[2])
    )
    .unwrap();
    writeln!(out, "    price: {price},").unwrap();
    writeln!(
        out,
        "    flags: RelicFlags::new(&[{}], &[{}]),",
        yes.join(", "),
        computed.join(", ")
    )
    .unwrap();
    writeln!(out, "    display_amount: {display},").unwrap();
    write!(out, "}}").unwrap();
    Ok(out)
}

fn emit_relics(
    out: &mut String,
    relics: &[&Vec<String>],
    pools_of: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    writeln!(
        out,
        "/// Every relic in the build.\n\
         ///\n\
         /// Flat, and ordered by the game's own id. The generated files group by\n\
         /// pool, which is what a character drafts from.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(relics.len())?).unwrap();
    writeln!(out, "pub enum Relic {{").unwrap();
    for row in relics {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl Relic {{").unwrap();
    writeln!(
        out,
        "    /// Every relic, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    pub const ALL: [Relic; {}] = [", relics.len()).unwrap();
    for row in relics {
        writeln!(out, "        Relic::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(
        out,
        "    /// The pools that draft this relic.\n\
         \x20   ///\n\
         \x20   /// Never empty — unlike a monster's acts or an encounter's,\n\
         \x20   /// every relic is in at least one pool — and three relics are in\n\
         \x20   /// two."
    )
    .unwrap();
    writeln!(
        out,
        "    pub const fn pools(self) -> &'static [RelicPool] {{"
    )
    .unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in relics {
        let list = pools_of
            .get(&row[1])
            .map(|ps| {
                ps.iter()
                    .map(|p| {
                        format!(
                            "RelicPool::{}",
                            camel(p.strip_suffix("_RELIC_POOL").unwrap_or(p))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        writeln!(out, "            Self::{} => &[{list}],", camel(&row[1])).unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}\n").unwrap();

    writeln!(
        out,
        "    /// The rarity, the price and the flags.\n\
         \x20   ///\n\
         \x20   /// One arm per relic, naming the file its row lives in."
    )
    .unwrap();
    writeln!(out, "    pub const fn data(self) -> RelicData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for row in relics {
        let empty = Vec::new();
        let of = pools_of.get(&row[1]).unwrap_or(&empty);
        writeln!(
            out,
            "            Self::{} => {}::{},",
            camel(&row[1]),
            bucket(of)?,
            row[1]
        )
        .unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(())
}
