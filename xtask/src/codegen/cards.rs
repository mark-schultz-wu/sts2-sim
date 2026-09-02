//! Every card's static data.
//!
//! Card identity nests by the game's own twelve card pools, so `Strike` needs
//! no suffix. The game's flat `STRIKE_IRONCLAD` survives as the serde rename,
//! and one file is written per pool.
//!
//! The xtask README says what the stripping rule is for, and why the files are
//! split that way.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use super::sets;
use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr, sanitize};

/// Pool id, module name, Rust enum name, and the id suffix to strip inside it.
pub(super) const POOLS: [(&str, &str, &str, Option<&str>); 12] = [
    (
        "IRONCLAD_CARD_POOL",
        "ironclad",
        "IroncladCard",
        Some("IRONCLAD"),
    ),
    ("SILENT_CARD_POOL", "silent", "SilentCard", Some("SILENT")),
    ("REGENT_CARD_POOL", "regent", "RegentCard", Some("REGENT")),
    (
        "NECROBINDER_CARD_POOL",
        "necrobinder",
        "NecrobinderCard",
        Some("NECROBINDER"),
    ),
    ("DEFECT_CARD_POOL", "defect", "DefectCard", Some("DEFECT")),
    ("COLORLESS_CARD_POOL", "colorless", "ColorlessCard", None),
    ("CURSE_CARD_POOL", "curse", "CurseCard", None),
    ("EVENT_CARD_POOL", "event", "EventCard", None),
    ("QUEST_CARD_POOL", "quest", "QuestCard", None),
    ("STATUS_CARD_POOL", "status", "StatusCard", None),
    ("TOKEN_CARD_POOL", "token", "TokenCard", None),
    ("DEPRECATED_CARD_POOL", "deprecated", "DeprecatedCard", None),
];

#[derive(Default)]
struct Card {
    id: String,
    pool: String,
    kind: String,
    rarity: String,
    target: String,
    cost: String,
    costs_x: bool,
    max_upgrade: String,
    keywords: Vec<String>,
}

/// Members the game declares that are not values of the field they name.
///
/// A C# enum has a zero, so a `None` gets declared whether or not it means
/// anything. `Option<T>` is the honest Rust for the ones that don't.
///
/// Dropping one does not renumber the rest, so the enum stays a subset of the
/// game's. `TargetType::None` stays, because 35 cards declare it.
const DROPPED: [(&str, &str, &str); 3] = [
    (
        "CardKeyword",
        "None",
        "not one keyword in the game names it — not a declaration, a comparison, \
         or a set member. It is the C# zero and nothing else.",
    ),
    (
        "CardRarity",
        "None",
        "no card has it and `CardRarityExtensions.ToLocString` throws on it. \
         Every real use is an absent answer — `CardFactory` returns it when no \
         rarity was selected, `GetNextHighestRarityWithWrapping` when there is \
         no next one — which is `Option<CardRarity>` here.",
    ),
    (
        "CardType",
        "None",
        "the only way to reach it is a Mad Science whose Tinker Time type has \
         not been set, and all three sites that create one assign it on the \
         next line — `CreateCard<MadScience>()` cannot take the type as an \
         argument, so `_tinkerTimeType` sits at the C# zero for two statements. \
         That is a two-phase constructor, not a value: here the type is a \
         constructor argument, and a card that has not chosen one is \
         `Option<CardType>` at the call site that asks.",
    ),
];

fn dropped(enum_name: &str, member: &str) -> bool {
    DROPPED
        .iter()
        .any(|&(e, m, _)| e == enum_name && m == member)
}

/// The dump's two cost columns as the one `Cost` they encode.
///
/// The pairing is checked rather than assumed. `CardEnergyCost` zeroes the
/// canonical cost for X-cost cards, and it treats any negative base as "no
/// cost". An X card with a number, or a negative other than -1, means the game
/// changed under us. The mapping below is then no longer faithful.
fn cost(card: &Card) -> Result<String, String> {
    match (card.costs_x, card.cost.as_str()) {
        (true, "0") => Ok("Cost::X".into()),
        (true, n) => Err(format!("{}: costs X but canonical cost is {n}", card.id)),
        (false, "-1") => Ok("Cost::None".into()),
        (false, n) if n.starts_with('-') => Err(format!(
            "{}: negative cost {n} is not the -1 sentinel",
            card.id
        )),
        (false, n) => n
            .parse::<u8>()
            .map(|n| format!("Cost::Energy({n})"))
            .map_err(|e| format!("{}: cost {n}: {e}", card.id)),
    }
}

/// The variant name a card gets inside its pool's enum.
fn variant(card: &Card, suffix: Option<&str>) -> String {
    match suffix {
        Some(s) if card.id.ends_with(&format!("_{s}")) => {
            camel(&card.id[..card.id.len() - s.len() - 1])
        }
        _ => camel(&card.id),
    }
}

/// One card, as the path another domain has to spell to name it.
///
/// Card identity is pool-scoped. `Strike` is a variant of five different enums,
/// so naming one from outside takes the pool as well as the id.
///
/// The suffix rule lives here because this is the domain that invented it. A
/// second copy of it elsewhere is a second chance to get `STRIKE_IRONCLAD`
/// wrong.
pub(super) fn card_path(pool: &str, id: &str) -> Result<String, String> {
    let (_, _, enum_name, suffix) = POOLS
        .iter()
        .find(|(pool_id, ..)| *pool_id == pool)
        .ok_or_else(|| format!("{id} is in {pool}, which is not a card pool"))?;

    let card = Card {
        id: id.to_string(),
        ..Card::default()
    };

    Ok(format!(
        "Cards::{}({enum_name}::{})",
        enum_name.trim_end_matches("Card"),
        variant(&card, *suffix)
    ))
}

/// Every card, as one module per pool plus the shared field enums.
///
/// Returns the dump's content hash, which stamps the roll-up too.
pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("cards.txt")?;
    let build = dump.build.clone();

    let cards: Vec<Card> = dump
        .of("card")
        .map(|f| Card {
            id: f[1].clone(),
            pool: f[2].clone(),
            kind: f[3].clone(),
            rarity: f[4].clone(),
            target: f[5].clone(),
            cost: f[6].clone(),
            costs_x: f[7] == "1",
            max_upgrade: f[8].clone(),
            keywords: if f[9] == "-" {
                Vec::new()
            } else {
                f[9].split(',').map(String::from).collect()
            },
        })
        .collect();

    // Discriminant and name, because a dropped member leaves a hole that the
    // rest must not slide into.
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

    let mut split = Split::new(root, "cards", "cards.txt", &build)?;

    let mut total = 0usize;
    let mut stripped = 0usize;

    // One file per pool: the enum, its members, and its data table.
    for (pool_id, module, enum_name, suffix) in POOLS {
        let members: Vec<&Card> = cards.iter().filter(|c| c.pool == pool_id).collect();
        total += members.len();

        let out = split.body(module);
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr({})]", repr(members.len())?).unwrap();
        writeln!(out, "pub enum {enum_name} {{").unwrap();
        for card in &members {
            if suffix.is_some_and(|s| card.id.ends_with(&format!("_{s}"))) {
                stripped += 1;
            }
            writeln!(out, "    #[serde(rename = \"{}\")]", card.id).unwrap();
            writeln!(out, "    {},", variant(card, suffix)).unwrap();
        }
        writeln!(out, "}}\n").unwrap();

        writeln!(out, "impl {enum_name} {{").unwrap();

        // By value: `CardData` fits in a register now, so a reference would be
        // strictly larger than the thing it points at — and returning one would
        // force the rows back into a static, since a `const fn` call is not a
        // promotable expression.
        writeln!(
            out,
            "    /// What is true of the card with no run in progress."
        )
        .unwrap();
        writeln!(out, "    pub const fn data(self) -> CardData {{").unwrap();
        writeln!(out, "        match self {{").unwrap();
        for card in &members {
            let kind = if card.kind == "dynamic" {
                "Field::Computed".into()
            } else {
                format!("Field::Constant(CardType::{})", sanitize(&card.kind))
            };
            let target = if card.target == "dynamic" {
                "Field::Computed".into()
            } else {
                format!("Field::Constant(TargetType::{})", sanitize(&card.target))
            };
            let keywords = card
                .keywords
                .iter()
                .map(|k| format!("CardKeyword::{}", sanitize(k)))
                .collect::<Vec<_>>()
                .join(", ");

            writeln!(
                out,
                "            Self::{} => CardData {{",
                variant(card, suffix)
            )
            .unwrap();
            writeln!(out, "                kind: {kind},").unwrap();
            writeln!(
                out,
                "                rarity: CardRarity::{},",
                sanitize(&card.rarity)
            )
            .unwrap();
            writeln!(out, "                target: {target},").unwrap();
            writeln!(out, "                cost: {},", cost(card)?).unwrap();
            writeln!(out, "                max_upgrade: {},", card.max_upgrade).unwrap();
            writeln!(
                out,
                "                keywords: KeywordSet::new(&[{keywords}]),"
            )
            .unwrap();
            writeln!(out, "            }},").unwrap();
        }
        writeln!(out, "        }}").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}").unwrap();
    }

    // The shared half: the field enums, the data shape, and the outer enum.
    let mut out = banner(
        "The field enums, the data shape, and the outer card enum.",
        "cards.txt",
        &build,
    );
    writeln!(out, "\nuse super::characters::CardPool;").unwrap();
    writeln!(out, "use crate::{{Cost, Field}};\nuse sts2_core::Table;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    for (_, module, _, _) in POOLS {
        writeln!(out, "mod {module};").unwrap();
    }
    writeln!(out).unwrap();
    for (_, module, _, _) in POOLS {
        writeln!(out, "pub use {module}::*;").unwrap();
    }
    writeln!(out).unwrap();

    // Reflected from the game, so a member nothing uses is still present —
    // absent from the table is not absent from the enum. The exceptions are in
    // `DROPPED`, and each carries its reason into the generated file.
    for (name, members) in &enums {
        for (_, member, why) in DROPPED.iter().filter(|&&(e, _, _)| e == name) {
            writeln!(out, "// `{name}::{member}` is not generated: {why}").unwrap();
        }
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            if dropped(name, m) {
                continue;
            }
            let ident = sanitize(m);
            if ident != m {
                writeln!(out, "    #[serde(rename = \"{m}\")]").unwrap();
            }
            // The game's own number, spelled out: a dropped member leaves a
            // hole rather than pulling everything after it down one.
            writeln!(out, "    {ident} = {discriminant},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    // A bitset rather than a slice. The game keeps keywords in a
    // `HashSet<CardKeyword>` and asks `.Contains(…)` almost exclusively, so a
    // set is what it means; a slice was 16 bytes of pointer and length to carry
    // at most four one-byte members, and its order came from the dump's sort
    // rather than from the game. It also has to survive being the per-card
    // runtime state, because `AddKeyword`/`RemoveKeyword` mutate it mid-combat,
    // and a word clones for free where a set does not.
    let keywords: Vec<sets::Member<'_>> = enums["CardKeyword"]
        .iter()
        .filter(|(_, m)| !dropped("CardKeyword", m))
        .map(|(bit, m)| sets::Member {
            name: m,
            bit: *bit as usize,
            doc: "",
        })
        .collect();

    sets::plain(
        &mut out,
        &sets::Set {
            flag: "CardKeyword",
            set: "KeywordSet",
            flag_doc: "",
            set_doc: "A set of [`CardKeyword`]s: one bit per keyword, at its discriminant.\n\
                      \n\
                      Bit 0 is unused, because the game's zero is `None` and that is not\n\
                      a keyword. Using the discriminant anyway keeps a bit meaning the\n\
                      same thing here as it does there.",
            members: &keywords,
            declare: false,
            extras: &[],
        },
    )?;

    // A set over `CardType`, for whoever needs one. Nothing in this domain
    // does — a card has one type, not a set of them — but the enum lives here,
    // so the set over it does too rather than in whichever domain happens to
    // want it first. Enchantments are that domain today.
    //
    // Over every card type, not the three an enchantment can reach: narrowing a
    // type to its first caller is how the second one gets it wrong.
    let types: Vec<sets::Member<'_>> = enums["CardType"]
        .iter()
        .filter(|(_, m)| !dropped("CardType", m))
        .map(|(bit, m)| sets::Member {
            name: m,
            bit: *bit as usize,
            doc: "",
        })
        .collect();
    sets::plain(
        &mut out,
        &sets::Set {
            flag: "CardType",
            set: "CardTypeSet",
            flag_doc: "",
            set_doc: "A set of [`CardType`]s: one bit per type, at its discriminant.\n\
                      \n\
                      Bit 0 is unused, because the game's zero is `None` and that is\n\
                      not a type a card can have. Using the discriminant anyway keeps\n\
                      a bit meaning the same thing here as it does there.",
            members: &types,
            declare: false,
            extras: &[sets::Extra::IsEmpty],
        },
    )?;

    writeln!(out, "/// What is true of a card with no run in progress.").unwrap();
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]").unwrap();
    writeln!(out, "pub struct CardData {{").unwrap();
    writeln!(
        out,
        "    /// `Computed` when the card works it out from combat state."
    )
    .unwrap();
    writeln!(out, "    pub kind: Field<CardType>,").unwrap();
    writeln!(out, "    pub rarity: CardRarity,").unwrap();
    writeln!(out, "    pub target: Field<TargetType>,").unwrap();
    writeln!(out, "    pub cost: Cost,").unwrap();
    writeln!(
        out,
        "    /// How many times the card can be upgraded. 0 or 1 on this build,\n\
         \x20   /// but `CardModel` branches on higher values, so it is not a flag."
    )
    .unwrap();
    writeln!(out, "    pub max_upgrade: u8,").unwrap();
    writeln!(out, "    pub keywords: KeywordSet,").unwrap();
    writeln!(out, "}}\n").unwrap();

    // Untagged, so it round-trips as the flat id the wire already uses: the
    // nesting is ours and the game never sees it.
    writeln!(out, "/// Every card, grouped by the pool it belongs to.").unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[serde(untagged)]").unwrap();
    writeln!(out, "pub enum Cards {{").unwrap();
    for (_, _, enum_name, _) in POOLS {
        writeln!(
            out,
            "    {}({enum_name}),",
            enum_name.trim_end_matches("Card")
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl sts2_core::Table for Cards {{").unwrap();
    writeln!(out, "    type Data = CardData;\n").unwrap();
    writeln!(out, "    fn data(self) -> CardData {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for (_, _, enum_name, _) in POOLS {
        writeln!(
            out,
            "            Cards::{}(c) => c.data(),",
            enum_name.trim_end_matches("Card")
        )
        .unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}\n").unwrap();

    writeln!(out, "    /// Every card in the build, pools flattened.").unwrap();
    writeln!(out, "    const ALL: &'static [Self] = &[").unwrap();
    for (pool_id, _, enum_name, suffix) in POOLS {
        for card in cards.iter().filter(|c| c.pool == pool_id) {
            writeln!(
                out,
                "        Cards::{}({enum_name}::{}),",
                enum_name.trim_end_matches("Card"),
                variant(card, suffix)
            )
            .unwrap();
        }
    }
    writeln!(out, "    ];").unwrap();
    writeln!(out, "}}\n").unwrap();

    // The variant is the pool, and the five that name a character are the
    // five a run can draw from. The rest — colourless, curses, statuses,
    // tokens, quest and event cards — are held by no character, which is not
    // the same as being held by a pool nobody plays.
    writeln!(out, "impl sts2_core::CardTable for Cards {{").unwrap();
    writeln!(out, "    type Pool = CardPool;").unwrap();
    writeln!(out, "    type Rarity = CardRarity;\n").unwrap();
    writeln!(out, "    fn pool(self) -> Option<CardPool> {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for (pool_id, _, enum_name, suffix) in POOLS {
        let name = enum_name.trim_end_matches("Card");
        match suffix {
            Some(_) => writeln!(
                out,
                "            Cards::{name}(_) => Some(CardPool::{}),",
                camel(pool_id.trim_end_matches("_CARD_POOL"))
            )
            .unwrap(),
            None => writeln!(out, "            Cards::{name}(_) => None,").unwrap(),
        }
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}\n").unwrap();

    writeln!(out, "    fn rarity(self) -> CardRarity {{").unwrap();
    writeln!(out, "        self.data().rarity").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    split.finish(
        &out,
        |module| {
            let (pool_id, ..) = POOLS
                .iter()
                .find(|(_, m, _, _)| *m == module)
                .expect("every bucket is a pool");
            format!("Cards in `{pool_id}`.")
        },
        &[
            (
                "super",
                &[
                    "CardData",
                    "CardKeyword",
                    "CardRarity",
                    "CardType",
                    "KeywordSet",
                    "TargetType",
                ],
            ),
            ("crate", &["Cost", "Field"]),
            ("serde", &["Deserialize", "Serialize"]),
        ],
    )?;

    println!(
        "  cards: {total} across {} pools ({stripped} ids had a pool suffix stripped), {} field enums",
        POOLS.len(),
        enums.len()
    );

    Ok(build)
}
