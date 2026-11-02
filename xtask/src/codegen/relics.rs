//! Every relic's static data, and which pool drafts it.
//!
//! A relic is a permanent passive, and nearly all of what one *does* is hooks,
//! ported by hand. What is table is where it comes from, what it costs, and the
//! handful of yes-or-no facts the engine reads off it without asking.
//!
//! The xtask README says why the price is a `Price` rather than a number, and
//! why the rarity is `Undrafted` rather than `None`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::DumpError;
use super::sets;
use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr};
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
    #[error("a relic is in no pool at all")]
    NoPool,
    #[error("the dump's relics are not in their ids' ordinal order, which `order` reads")]
    Unordered,
    #[error("{0}: merchant cost is computed, which Price cannot spell")]
    ComputedPrice(String),
    #[error("{relic}.{flag}: {value} is not a flag")]
    NotAFlag {
        relic: String,
        flag: String,
        value: String,
    },
    #[error(
        "the dump reflects no SHARED_RELIC_POOL, which run setup names: the pools it does have are {0:?}"
    )]
    NoSharedPool(Vec<String>),
    #[error(
        "the dump reflects no RelicRarity::{0}, which a relic bag deals from: see what this build calls it"
    )]
    Unreflected(&'static str),
    #[error("a relic's merchant cost now scales with ascension, which Price cannot spell")]
    ScalingCost,
    #[error("the dump reflects no RelicRarity")]
    NoRarity,
}

/// The sentinel `MerchantCost` returns for a relic that is not for sale.
const NOT_SOLD: &str = "999999999";

/// Enum members whose Rust name differs from the game's.
///
/// The rename is Rust-side only. The serde spelling stays the game's, so the
/// wire and the dump are unchanged, and the generated file is still diffable
/// against the assembly.
const RENAMED: [(&str, &str, &str); 1] = [("RelicRarity", "None", "Undrafted")];

/// The yes-or-no columns, in the order they become bits.
///
/// Not a game enum, but nine separate `RelicModel` members, so the numbering
/// is ours and this order is the whole of it. `column` is where the dump writes
/// each one.
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
/// A genuine partition, unlike a monster's act or an encounter's: every relic
/// is in at least one pool. Three are in two, and those get their own bucket
/// rather than being filed somewhere their own `pools()` does not lead.
///
/// `crossover` and not `shared`, because `SHARED_RELIC_POOL` is itself a pool
/// and a file named for both names neither.
fn bucket(pools: &[String]) -> Result<String, Error> {
    match pools {
        [] => Err(Error::NoPool),
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

pub fn generate(root: &Path) -> Result<Build, Error> {
    let dump = Dump::read("relics.txt")?;
    let build = dump.build.clone();

    let mut enums: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    for f in dump.of("enum") {
        let discriminant =
            DumpError::number::<u8>(format_args!("{} {}: discriminant", f[1], f[3]), &f[2])?;
        enums
            .entry(f[1].clone())
            .or_default()
            .push((discriminant, f[3].clone()));
    }

    let width = 14;
    let relics: Vec<&Vec<String>> = dump.of("relic").collect();
    if relics.is_empty() {
        return Err(DumpError::Empty("relics").into());
    }
    for row in &relics {
        if row.len() != width {
            return Err(DumpError::Columns {
                id: row[1].clone(),
                got: row.len() - 1,
                want: width - 1,
            }
            .into());
        }
    }

    // The dump swept `MerchantCost` at every ascension and found nothing, which
    // is why `Price` is flat. A build that starts scaling it would generate a
    // table that silently describes ascension 0 only, so it fails here instead.
    if dump.of("scalecost").next().is_some() {
        return Err(Error::ScalingCost);
    }

    let mut pools: Vec<String> = Vec::new();
    let mut pools_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Each pool's relics in the dump's order, which is the pool's own.
    let mut members: Vec<Vec<String>> = Vec::new();
    for row in dump.of("pool") {
        let at = match pools.iter().position(|pool| *pool == row[1]) {
            Some(at) => at,
            None => {
                pools.push(row[1].clone());
                members.push(Vec::new());
                pools.len() - 1
            }
        };
        members[at].push(row[2].clone());
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
    writeln!(out, "\nuse crate::Field;\nuse sts2_core::Table;").unwrap();
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
    let overrides = super::overrides(&build, "RELIC")?;
    emit_relics(&mut out, &relics, &pools_of, &members, &overrides)?;
    emit_bridges(&mut out, &pools, &enums)?;

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
/// Two `u8`s, like a power's, because a flag has three answers: `IsUsedUp` and
/// `ShowCounter` are `virtual` and some relics work them out.
fn emit_flags(out: &mut String) -> Result<(), sets::Error> {
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
            members: &sets::counted(FLAGS.map(|(name, _, doc)| (name, doc))),
            declare: true,
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
         /// which is a sentinel and not a price: nothing should do arithmetic\n\
         /// on it, and a shop comparing a player's gold against it would be\n\
         /// comparing against a number that means never.\n\
         ///\n\
         /// Read per relic rather than derived from rarity. The base\n\
         /// implementation maps Ancient, Starter and Event to the sentinel, but\n\
         /// nine relics override it — so nine of the 35 Event-rarity relics do\n\
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
fn literal(row: &[String]) -> Result<String, Error> {
    let id = &row[1];

    let price = match row[3].as_str() {
        NOT_SOLD => "Price::NotSold".to_string(),
        "dynamic" => {
            return Err(Error::ComputedPrice(id.to_owned()));
        }
        n => format!(
            "Price::Gold({})",
            DumpError::number::<u16>(format_args!("{id}: merchant cost"), n)?
        ),
    };

    let (mut yes, mut computed) = (Vec::new(), Vec::new());
    for (name, column, _) in FLAGS {
        match row[column].as_str() {
            "1" => yes.push(format!("RelicFlag::{name}")),
            "0" => {}
            "dynamic" => computed.push(format!("RelicFlag::{name}")),
            other => {
                return Err(Error::NotAFlag {
                    relic: id.to_owned(),
                    flag: name.to_string(),
                    value: other.to_owned(),
                });
            }
        }
    }

    let display = match row[11].as_str() {
        "dynamic" => "Field::Computed".to_string(),
        n => format!(
            "Field::Constant({})",
            DumpError::number::<u16>(format_args!("{id}: display amount"), n)?
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
    members: &[Vec<String>],
    overrides: &BTreeMap<String, Vec<sts2_core::ModelHook>>,
) -> Result<(), Error> {
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

    // `order` is the discriminant, which holds while the dump lists the
    // relics as `ModelId.CompareTo` orders them: ordinal over the id.
    if !relics.windows(2).all(|pair| pair[0][1] < pair[1][1]) {
        return Err(Error::Unordered);
    }

    let mut extras = String::new();
    writeln!(
        extras,
        "    /// The pools that draft this relic.\n\
         \x20   ///\n\
         \x20   /// Never empty — unlike a monster's acts or an encounter's,\n\
         \x20   /// every relic is in at least one pool — and three relics are in\n\
         \x20   /// two."
    )
    .unwrap();
    writeln!(extras, "    fn pools(self) -> &'static [RelicPool] {{").unwrap();
    writeln!(extras, "        POOLS[self as usize]").unwrap();
    writeln!(extras, "    }}\n").unwrap();
    writeln!(
        extras,
        "    /// The relics `pool` drafts, in the pool's own order."
    )
    .unwrap();
    writeln!(
        extras,
        "    fn members(pool: RelicPool) -> &'static [Relic] {{"
    )
    .unwrap();
    writeln!(extras, "        MEMBERS[pool as usize]").unwrap();
    writeln!(extras, "    }}\n").unwrap();

    let mut pool_lists: Vec<String> = Vec::new();
    for row in relics {
        pool_lists.push(
            pools_of
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
                .unwrap_or_default(),
        );
    }

    writeln!(
        extras,
        "\n    /// Its rarity, which is also which screen offers it."
    )
    .unwrap();
    writeln!(extras, "    fn rarity(self) -> RelicRarity {{").unwrap();
    writeln!(extras, "        self.data().rarity").unwrap();
    writeln!(extras, "    }}\n").unwrap();

    writeln!(
        extras,
        "    /// Whether taking it does something there and then."
    )
    .unwrap();
    writeln!(extras, "    fn has_pickup_effect(self) -> bool {{").unwrap();
    writeln!(
        extras,
        "        self.data().flags.get(RelicFlag::UponPickup) == Field::Constant(true)"
    )
    .unwrap();
    writeln!(extras, "    }}\n").unwrap();

    writeln!(
        extras,
        "    /// Where this relic sorts: `ModelId.CompareTo`, ordinal over its id."
    )
    .unwrap();
    writeln!(extras, "    fn order(self) -> u16 {{").unwrap();
    writeln!(extras, "        self as u16").unwrap();
    writeln!(extras, "    }}\n").unwrap();

    writeln!(extras, "    /// What a merchant charges before the haggle.").unwrap();
    writeln!(extras, "    fn merchant_price(self) -> Option<u16> {{").unwrap();
    writeln!(extras, "        match self.data().price {{").unwrap();
    writeln!(extras, "            Price::Gold(gold) => Some(gold),").unwrap();
    writeln!(extras, "            Price::NotSold => None,").unwrap();
    writeln!(extras, "        }}").unwrap();
    writeln!(extras, "    }}\n").unwrap();

    writeln!(extras, "    /// Whether a merchant is allowed to stock it.").unwrap();
    writeln!(extras, "    fn allowed_in_shops(self) -> bool {{").unwrap();
    writeln!(
        extras,
        "        self.data().flags.get(RelicFlag::AllowedInShops) == Field::Constant(true)"
    )
    .unwrap();
    writeln!(extras, "    }}\n").unwrap();

    writeln!(extras, "    /// Whether a run can hold more than one.").unwrap();
    writeln!(extras, "    fn stackable(self) -> bool {{").unwrap();
    writeln!(
        extras,
        "        match self.data().flags.get(RelicFlag::Stackable) {{\n            Field::Constant(stackable) => stackable,\n            Field::Computed => unimplemented!(\"whether {{self:?}} stacks\"),\n        }}"
    )
    .unwrap();
    writeln!(extras, "    }}").unwrap();

    writeln!(out, "impl sts2_core::Table for Relic {{").unwrap();
    writeln!(out, "    type Data = RelicData;\n").unwrap();
    writeln!(
        out,
        "    /// Every relic, so callers can sweep the table without a way to\n\
         \x20   /// enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    const ALL: &'static [Self] = &[").unwrap();
    for row in relics {
        writeln!(out, "        Relic::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// The rarity, the price and the flags.").unwrap();
    let mut table: Vec<String> = Vec::new();
    for row in relics {
        let empty = Vec::new();
        let of = pools_of.get(&row[1]).unwrap_or(&empty);
        table.push(format!("{}::{}", bucket(of)?, row[1]));
    }
    writeln!(out, "    fn data(self) -> &'static RelicData {{").unwrap();
    writeln!(out, "        &ROWS[self as usize]").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    super::rows(out, "RelicData", &table);

    writeln!(out, "impl sts2_core::RelicTable for Relic {{").unwrap();
    writeln!(out, "    type Pool = RelicPool;").unwrap();
    writeln!(out, "    type Rarity = RelicRarity;\n").unwrap();
    out.push_str(&extras);
    writeln!(
        out,
        "\n    fn overrides(self) -> sts2_core::Overrides {{\n\
         \x20       OVERRIDES[self as usize]\n\
         \x20   }}"
    )
    .unwrap();
    writeln!(out, "}}\n").unwrap();
    super::emit_overrides(
        out,
        "indexed by the relic's discriminant.",
        relics.iter().map(|row| row[1].as_str()),
        overrides,
    );
    super::lists(out, "POOLS", "RelicPool", &pool_lists);
    let member_lists: Vec<String> = members
        .iter()
        .map(|relics| {
            relics
                .iter()
                .map(|relic| format!("Relic::{}", camel(relic)))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect();
    super::lists(out, "MEMBERS", "Relic", &member_lists);

    Ok(())
}

/// The pool and the rarities run setup names, as the consts it names them by.
///
/// A generated enum is per build, so a rule holding only a `ContentVersion`
/// cannot write `RelicPool::Shared`. A build that renamed either stops here.
fn emit_bridges(
    out: &mut String,
    pools: &[String],
    enums: &BTreeMap<String, Vec<(u8, String)>>,
) -> Result<(), Error> {
    if !pools.iter().any(|pool| pool == "SHARED_RELIC_POOL") {
        return Err(Error::NoSharedPool(pools.to_vec()));
    }

    // `RelicGrabBag._rarities`, which is what a bag is allowed to hold.
    const DRAFTED: [&str; 4] = ["Common", "Uncommon", "Rare", "Shop"];
    let rarities = enums.get("RelicRarity").ok_or(Error::NoRarity)?;
    // `RelicRarity.None` under the name the rename gives it, which a reward
    // slot carries when it picks no rarity of its own.
    for rarity in DRAFTED.iter().copied().chain(["None", "Starter"]) {
        if !rarities.iter().any(|(_, have)| have == rarity) {
            return Err(Error::Unreflected(rarity));
        }
    }

    writeln!(out, "\nimpl sts2_core::RelicPoolKinds for RelicPool {{").unwrap();
    writeln!(out, "    const SHARED: RelicPool = RelicPool::Shared;").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl sts2_core::RarityKinds for RelicRarity {{").unwrap();
    writeln!(out, "    const DRAFTED: [RelicRarity; 4] = [").unwrap();
    for rarity in DRAFTED {
        writeln!(out, "        RelicRarity::{rarity},").unwrap();
    }
    writeln!(out, "    ];").unwrap();
    for rarity in DRAFTED {
        writeln!(
            out,
            "    const {}: RelicRarity = RelicRarity::{rarity};",
            rarity.to_uppercase()
        )
        .unwrap();
    }
    writeln!(
        out,
        "    const UNDRAFTED: RelicRarity = RelicRarity::Undrafted;"
    )
    .unwrap();
    writeln!(
        out,
        "    const STARTER: RelicRarity = RelicRarity::Starter;"
    )
    .unwrap();
    writeln!(out, "}}").unwrap();
    Ok(())
}
