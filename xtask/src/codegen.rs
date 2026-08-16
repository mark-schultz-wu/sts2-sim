//! Turns the card dump into Rust.
//!
//! Card identity nests by the game's own twelve card pools, so `Strike` needs
//! no suffix: `IroncladCard::Strike` and `SilentCard::Strike` are different
//! variants of different types. The game disambiguates with a flat namespace
//! instead — `STRIKE_IRONCLAD` — and that id is preserved as the serde rename,
//! so the wire form is unchanged and the generated file stays diffable against
//! the game's own names.
//!
//! Only ten of 578 ids actually carry a pool suffix, all of them `STRIKE_*` and
//! `DEFEND_*`. The stripping rule matters for exactly those, and is a no-op
//! everywhere else.
//!
//! One file per pool rather than one for everything. A content patch usually
//! touches one character, and in a project about noticing when the game and the
//! simulator disagree, that should read as a diff to one file rather than a
//! churn in the middle of eight thousand lines.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const DUMP: &str = "crates/sts2-content/src/v0_107_1/cards.txt";
const OUT_DIR: &str = "crates/sts2-content/src/v0_107_1/generated";

/// Pool id, module name, Rust enum name, and the id suffix to strip inside it.
const POOLS: [(&str, &str, &str, Option<&str>); 12] = [
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

const DERIVES: &str = "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]";

/// Rust identifiers the game's names collide with.
///
/// `Self` is the only one that bites in practice, and it cannot be escaped as
/// `r#Self` either — the language reserves it outright. The serde rename keeps
/// the game's spelling on the wire, so this is a Rust-side name only.
fn sanitize(name: &str) -> &str {
    match name {
        "Self" => "SelfTarget",
        other => other,
    }
}

/// Ids are `SCREAMING_SNAKE`; variants are `CamelCase`.
fn camel(id: &str) -> String {
    id.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect()
}

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

/// The variant name a card gets inside its pool's enum.
fn variant(card: &Card, suffix: Option<&str>) -> String {
    match suffix {
        Some(s) if card.id.ends_with(&format!("_{s}")) => {
            camel(&card.id[..card.id.len() - s.len() - 1])
        }
        _ => camel(&card.id),
    }
}

pub fn run() -> Result<String, String> {
    let dump = fs::read_to_string(DUMP).map_err(|e| format!("reading {DUMP}: {e}"))?;

    let mut cards: Vec<Card> = Vec::new();
    let mut enums: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut content_hash = String::new();

    for line in dump.lines() {
        if let Some(rest) = line.strip_prefix("# card data, contentHash ") {
            content_hash = rest.trim().to_string();
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first() {
            Some(&"card") => cards.push(Card {
                id: f[1].into(),
                pool: f[2].into(),
                kind: f[3].into(),
                rarity: f[4].into(),
                target: f[5].into(),
                cost: f[6].into(),
                costs_x: f[7] == "1",
                max_upgrade: f[8].into(),
                keywords: if f[9] == "-" {
                    Vec::new()
                } else {
                    f[9].split(',').map(String::from).collect()
                },
            }),
            Some(&"enum") => enums.entry(f[1].into()).or_default().push(f[3].into()),
            _ => {}
        }
    }

    if content_hash.is_empty() {
        return Err("the dump has no content hash in its header".into());
    }

    let dir = Path::new(OUT_DIR);
    fs::create_dir_all(dir).map_err(|e| format!("creating {OUT_DIR}: {e}"))?;

    let banner = |what: &str| {
        format!(
            "//! Generated by `cargo xtask codegen`. DO NOT EDIT.\n//!\n//! {what}\n//! Source: `cards.txt`, content hash {content_hash}.\n"
        )
    };

    let mut total = 0usize;
    let mut stripped = 0usize;

    // One file per pool: the enum, its members, and its data table.
    for (pool_id, module, enum_name, suffix) in POOLS {
        let members: Vec<&Card> = cards.iter().filter(|c| c.pool == pool_id).collect();
        total += members.len();

        let mut out = banner(&format!("Cards in `{pool_id}`."));
        writeln!(
            out,
            "\nuse super::{{CardData, CardKeyword, CardRarity, CardType, TargetType}};\n\
             use crate::Field;\n\
             use serde::{{Deserialize, Serialize}};\n"
        )
        .unwrap();

        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u16)]").unwrap();
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
        writeln!(
            out,
            "    pub const ALL: [{enum_name}; {}] = [",
            members.len()
        )
        .unwrap();
        for card in &members {
            writeln!(out, "        {enum_name}::{},", variant(card, suffix)).unwrap();
        }
        writeln!(out, "    ];\n").unwrap();
        writeln!(out, "    pub const fn data(self) -> &'static CardData {{").unwrap();
        writeln!(out, "        &DATA[self as usize]").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        writeln!(out, "static DATA: [CardData; {}] = [", members.len()).unwrap();
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

            writeln!(out, "    CardData {{").unwrap();
            writeln!(out, "        id: \"{}\",", card.id).unwrap();
            writeln!(out, "        kind: {kind},").unwrap();
            writeln!(
                out,
                "        rarity: CardRarity::{},",
                sanitize(&card.rarity)
            )
            .unwrap();
            writeln!(out, "        target: {target},").unwrap();
            writeln!(out, "        cost: {},", card.cost).unwrap();
            writeln!(out, "        costs_x: {},", card.costs_x).unwrap();
            writeln!(out, "        max_upgrade: {},", card.max_upgrade).unwrap();
            writeln!(out, "        keywords: &[{keywords}],").unwrap();
            writeln!(out, "    }},").unwrap();
        }
        writeln!(out, "];").unwrap();

        let path = dir.join(format!("{module}.rs"));
        fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }

    // The shared half: the field enums, the data shape, and the outer enum.
    let mut out = banner("The field enums, the data shape, and the outer card enum.");
    writeln!(out, "\nuse crate::Field;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    for (_, module, _, _) in POOLS {
        writeln!(out, "mod {module};").unwrap();
    }
    writeln!(out).unwrap();
    for (_, module, _, _) in POOLS {
        writeln!(out, "pub use {module}::*;").unwrap();
    }
    writeln!(out).unwrap();

    // Reflected from the game, so a member nothing uses is still present.
    for (name, members) in &enums {
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for m in members {
            let ident = sanitize(m);
            if ident != m {
                writeln!(out, "    #[serde(rename = \"{m}\")]").unwrap();
            }
            writeln!(out, "    {ident},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    writeln!(out, "/// What is true of a card with no run in progress.").unwrap();
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]").unwrap();
    writeln!(out, "pub struct CardData {{").unwrap();
    writeln!(out, "    pub id: &'static str,").unwrap();
    writeln!(
        out,
        "    /// `Computed` when the card works it out from combat state."
    )
    .unwrap();
    writeln!(out, "    pub kind: Field<CardType>,").unwrap();
    writeln!(out, "    pub rarity: CardRarity,").unwrap();
    writeln!(out, "    pub target: Field<TargetType>,").unwrap();
    writeln!(out, "    pub cost: i32,").unwrap();
    writeln!(out, "    pub costs_x: bool,").unwrap();
    writeln!(out, "    pub max_upgrade: i32,").unwrap();
    writeln!(out, "    pub keywords: &'static [CardKeyword],").unwrap();
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

    writeln!(out, "impl Cards {{").unwrap();
    writeln!(out, "    pub const fn data(self) -> &'static CardData {{").unwrap();
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
    writeln!(out, "    pub const ALL: [Cards; {total}] = [").unwrap();
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
    writeln!(out, "    ];\n").unwrap();

    writeln!(out, "    /// The game's own id, and the wire form.").unwrap();
    writeln!(out, "    pub const fn id(self) -> &'static str {{").unwrap();
    writeln!(out, "        self.data().id").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    let path = dir.join("mod.rs");
    fs::write(&path, &out).map_err(|e| format!("writing {}: {e}", path.display()))?;

    Ok(format!(
        "generated {OUT_DIR}/ — mod.rs and {} pool files\n  {total} cards ({stripped} ids had a pool suffix stripped)\n  {} field enums\n  content hash {content_hash}",
        POOLS.len(),
        enums.len()
    ))
}
