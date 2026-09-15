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

use super::bridge::{Bridge, emit_bridge};
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

/// The one enum here the simulator names members of.
const BRIDGES: [Bridge; 1] = [Bridge {
    enum_name: "TargetType",
    trait_name: "TargetKinds",
    named: &[
        ("NONE", "None"),
        ("SELF", "Self"),
        ("ANY_ENEMY", "AnyEnemy"),
        ("ALL_ENEMIES", "AllEnemies"),
        ("RANDOM_ENEMY", "RandomEnemy"),
        ("ANY_PLAYER", "AnyPlayer"),
        ("ANY_ALLY", "AnyAlly"),
        ("ALL_ALLIES", "AllAllies"),
        ("TARGETED_NO_CREATURE", "TargetedNoCreature"),
        ("OSTY", "Osty"),
    ],
    unnamed: &[],
}];

#[derive(Default)]
struct Card {
    id: String,
    pool: String,
    kind: String,
    rarity: String,
    target: String,
    cost: String,
    costs_x: bool,
    star_cost: String,
    star_costs_x: bool,
    max_upgrade: String,
    keywords: Vec<String>,
    /// Name, printed value, whether the game recomputes it in a fight, and
    /// the `DynamicVar` subclass it is.
    vars: Vec<(String, String, bool, String)>,
}

/// What a var of this class runs through before a card shows it.
///
/// `DynamicVar.UpdateCardPreview`, read class by class. A damage var runs
/// `Hook.ModifyDamage` and a block var runs `Hook.ModifyBlock`, whatever key
/// either is filed under. The rest preview the number they print.
///
/// `None` for a class nobody has read, which fails generation naming it. A
/// build that adds one is a build whose previews nobody has checked.
fn previewed(class: &str) -> Option<&'static str> {
    Some(match class {
        "DamageVar" | "CalculatedDamageVar" => "Some(CardNumberModifier::Damage)",
        "OstyDamageVar" => "Some(CardNumberModifier::OstyDamage)",
        "BlockVar" | "CalculatedBlockVar" => "Some(CardNumberModifier::Block)",
        "CalculationBaseVar"
        | "CalculationExtraVar"
        | "CalculatedVar"
        | "CardsVar"
        | "DynamicVar"
        | "EnergyVar"
        | "ExtraDamageVar"
        | "ForgeVar"
        | "GoldVar"
        | "HealVar"
        | "HpLossVar"
        | "IntVar"
        | "MaxHpVar"
        | "PowerVar"
        | "RepeatVar"
        | "StarsVar"
        | "SummonVar" => "None",
        _ => return None,
    })
}

/// The pool one relic draws from, which is held by no character.
const COLORLESS: &str = "COLORLESS_CARD_POOL";

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
    priced(&card.id, "energy", card.costs_x, &card.cost)
}

/// The star pool, read the same way and checked the same way.
///
/// `CanonicalStarCost` defaults to the same `-1` and `HasStarCostX` to false,
/// so the two pools encode a cost identically. Two cards cost X stars.
fn star_cost(card: &Card) -> Result<String, String> {
    priced(&card.id, "star", card.star_costs_x, &card.star_cost)
}

fn priced(id: &str, pool: &str, costs_x: bool, canonical: &str) -> Result<String, String> {
    match (costs_x, canonical) {
        (true, "0" | "-1") => Ok("Cost::X".into()),
        (true, n) => Err(format!("{id}: costs X {pool} but canonical cost is {n}")),
        (false, "-1") => Ok("Cost::None".into()),
        (false, n) if n.starts_with('-') => Err(format!(
            "{id}: negative {pool} cost {n} is not the -1 sentinel"
        )),
        (false, n) => n
            .parse::<u8>()
            .map(|n| format!("Cost::Fixed({n})"))
            .map_err(|e| format!("{id}: {pool} cost {n}: {e}")),
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
            star_cost: f[8].clone(),
            star_costs_x: f[9] == "1",
            max_upgrade: f[10].clone(),
            keywords: if f[11] == "-" {
                Vec::new()
            } else {
                f[11].split(',').map(String::from).collect()
            },
            vars: Vec::new(),
        })
        .collect();

    // A card's own lines follow it in the dump, and are attached by id rather
    // than by position, so a reordered dump generates the same table.
    let mut by_id: BTreeMap<String, Vec<(String, String, bool, String)>> = BTreeMap::new();
    for f in dump.of("var") {
        by_id.entry(f[1].clone()).or_default().push((
            f[2].clone(),
            f[3].clone(),
            f[4] == "1",
            f[5].clone(),
        ));
    }
    let mut cards = cards;
    for card in &mut cards {
        card.vars = by_id.remove(&card.id).unwrap_or_default();
    }
    if let Some((id, _)) = by_id.into_iter().next() {
        return Err(format!("{id} has variables but is not a card"));
    }
    let cards = cards;
    let fixed_numbers = cards
        .iter()
        .flat_map(|card| &card.vars)
        .any(|(_, base, _, _)| base.contains('.'));

    // Runtime order inside each CardPoolModel. Reward generation filters this
    // list by rarity and indexes it with RNG, so the table's id order is not a
    // substitute.
    let mut pools: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    for f in dump.of("pool") {
        let index = f[2]
            .parse::<usize>()
            .map_err(|e| format!("{}: pool index {}: {e}", f[1], f[2]))?;
        pools
            .entry(f[1].clone())
            .or_default()
            .push((index, f[3].clone()));
    }
    for (pool, entries) in &mut pools {
        entries.sort_by_key(|(index, _)| *index);
        for (expected, (index, card)) in entries.iter().enumerate() {
            if *index != expected {
                return Err(format!("{pool}: pool index {index}, expected {expected}"));
            }
            let Some(row) = cards.iter().find(|row| row.id == *card) else {
                return Err(format!("{pool}: unknown card {card}"));
            };
            if row.pool != *pool {
                return Err(format!("{pool}: {card} declares pool {}", row.pool));
            }
        }
    }

    let mut reward_pools: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    for f in dump.of("rewardpool") {
        let index = f[2]
            .parse::<usize>()
            .map_err(|e| format!("{}: reward pool index {}: {e}", f[1], f[2]))?;
        reward_pools
            .entry(f[1].clone())
            .or_default()
            .push((index, f[3].clone()));
    }
    for (character, entries) in &mut reward_pools {
        entries.sort_by_key(|(index, _)| *index);
        for (expected, (index, card)) in entries.iter().enumerate() {
            if *index != expected {
                return Err(format!(
                    "{character}: reward pool index {index}, expected {expected}"
                ));
            }
            if !cards.iter().any(|row| row.id == *card) {
                return Err(format!("{character}: unknown reward card {card}"));
            }
        }
    }

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

    // Every card's place in the build's own id order. `ModelId.CompareTo` is
    // ordinal over the id, and shuffling a pile sorts by it.
    let ranks: BTreeMap<&str, u16> = {
        let mut ids: Vec<&str> = cards.iter().map(|card| card.id.as_str()).collect();
        ids.sort_unstable();
        ids.into_iter()
            .enumerate()
            .map(|(rank, id)| {
                (
                    id,
                    u16::try_from(rank).expect("a build has fewer than 65536 cards"),
                )
            })
            .collect()
    };

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

        writeln!(
            out,
            "    /// What is true of the card with no run in progress."
        )
        .unwrap();
        writeln!(out, "    pub fn data(self) -> &'static CardData {{").unwrap();
        writeln!(out, "        &ROWS[self as usize]").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "    /// Where this card sorts against every card in the build."
        )
        .unwrap();
        writeln!(out, "    pub fn order(self) -> u16 {{").unwrap();
        writeln!(out, "        ORDER[self as usize]").unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}\n").unwrap();

        writeln!(
            out,
            "/// Every member's place in the build's own id order, which is\n\
             /// ordinal over the id the wire spells."
        )
        .unwrap();
        writeln!(out, "static ORDER: [u16; {}] = [", members.len()).unwrap();
        for card in &members {
            writeln!(out, "    {},", ranks[card.id.as_str()]).unwrap();
        }
        writeln!(out, "];\n").unwrap();

        let mut table: Vec<String> = Vec::new();
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

            let mut row = String::new();
            writeln!(row, "CardData {{").unwrap();
            writeln!(row, "        kind: {kind},").unwrap();
            writeln!(
                row,
                "        rarity: CardRarity::{},",
                sanitize(&card.rarity)
            )
            .unwrap();
            writeln!(row, "        target: {target},").unwrap();
            writeln!(row, "        cost: {},", cost(card)?).unwrap();
            writeln!(row, "        star_cost: {},", star_cost(card)?).unwrap();
            writeln!(row, "        vars: &[").unwrap();
            for (name, base, calculated, class) in &card.vars {
                let base = if fixed_numbers {
                    let (whole, fraction) = base.split_once('.').unwrap_or((base, ""));
                    let scale = u8::try_from(fraction.len())
                        .map_err(|_| format!("{}: {name} has too many decimal places", card.id))?;
                    let digits = format!("{whole}{fraction}");
                    let coefficient = digits
                        .parse::<i32>()
                        .map_err(|e| format!("{}: {name} is {base}: {e}", card.id))?;
                    format!("super::CardNumber::new({coefficient}, {scale})")
                } else {
                    base.parse::<i32>()
                        .map(|base| base.to_string())
                        .map_err(|e| format!("{}: {name} is {base}: {e}", card.id))?
                };
                writeln!(
                    row,
                    "            sts2_core::CardNumber {{ id: CardVar::{}, base: {base}, calculated: {calculated}, modifier: {} }},",
                    sanitize(name),
                    previewed(class).ok_or_else(|| format!(
                        "{}: {name} is a {class}, which nobody has read the preview of",
                        card.id
                    ))?
                )
                .unwrap();
            }
            writeln!(row, "        ],").unwrap();
            writeln!(row, "        max_upgrade: {},", card.max_upgrade).unwrap();
            writeln!(row, "        keywords: KeywordSet::new(&[{keywords}]),").unwrap();
            write!(row, "    }}").unwrap();
            table.push(row);
        }
        super::rows(out, "CardData", &table);
    }

    // The shared half: the field enums, the data shape, and the outer enum.
    let mut out = banner(
        "The field enums, the data shape, and the outer card enum.",
        "cards.txt",
        &build,
    );
    writeln!(out, "\nuse super::characters::{{CardPool, Character}};").unwrap();
    writeln!(
        out,
        "use crate::{{Cost, Field}};\nuse sts2_core::{{Drafted, Table}};"
    )
    .unwrap();
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

        if let Some(bridge) = BRIDGES.iter().find(|b| b.enum_name == name) {
            emit_bridge(&mut out, bridge, members)?;
        }
    }

    // The printed-number keys, as a set of their own. The game keys these by
    // string, and every card's row and every observation's preview names one.
    // A build prints its own set, so the set is generated rather than shared,
    // and the string stays at the edges that spell it.
    let mut keys: Vec<&str> = cards
        .iter()
        .flat_map(|card| card.vars.iter().map(|(name, _, _, _)| name.as_str()))
        .collect();
    keys.sort_unstable();
    keys.dedup();

    writeln!(out, "/// Every number a card on this build prints.").unwrap();
    writeln!(
        out,
        "///\n/// Ordered by the game's own key, which is the order a card's row prints\n         /// them in. No discriminant is the game's: this is a set the dump gathers\n         /// rather than a C# enum reflected out of it."
    )
    .unwrap();
    // Ordered as well as compared: a preview carries its numbers in a map
    // keyed by one of these, and the keys are sorted above by the game's own
    // spelling, so the derived order is that spelling's.
    writeln!(
        out,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]"
    )
    .unwrap();
    writeln!(out, "#[repr({})]", repr(keys.len())?).unwrap();
    writeln!(out, "pub enum CardVar {{").unwrap();
    for key in &keys {
        let ident = sanitize(key);
        if ident != *key {
            writeln!(out, "    #[serde(rename = \"{key}\")]").unwrap();
        }
        writeln!(out, "    {ident},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl sts2_core::CardVars for CardVar {{").unwrap();

    writeln!(out, "    const ALL: &'static [CardVar] = &[").unwrap();
    for key in &keys {
        writeln!(out, "        CardVar::{},", sanitize(key)).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();
    writeln!(out, "    fn name(self) -> &'static str {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for key in &keys {
        writeln!(out, "            CardVar::{} => \"{key}\",", sanitize(key)).unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

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

    if fixed_numbers {
        writeln!(out, "pub type CardNumber = sts2_core::Fixed;\n").unwrap();
    } else {
        writeln!(out, "pub type CardNumber = i32;\n").unwrap();
    }
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
        "    /// The star pool, which only the Regent's cards are priced in."
    )
    .unwrap();
    writeln!(out, "    pub star_cost: Cost,").unwrap();
    writeln!(
        out,
        "    /// The numbers printed on its face, in name order."
    )
    .unwrap();
    writeln!(
        out,
        "    pub vars: &'static [sts2_core::CardNumber<CardVar, CardNumber>],"
    )
    .unwrap();
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
    writeln!(out, "    fn data(self) -> &'static CardData {{").unwrap();
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

    writeln!(
        out,
        "/// A character pool in the order its model generates it."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn pool_cards(pool: CardPool) -> &'static [Cards] {{"
    )
    .unwrap();
    writeln!(out, "    match pool {{").unwrap();
    for (pool_id, _, enum_name, suffix) in POOLS
        .iter()
        .copied()
        .filter(|(_, _, _, suffix)| suffix.is_some())
    {
        let pool_name = camel(pool_id.trim_end_matches("_CARD_POOL"));
        writeln!(out, "        CardPool::{pool_name} => &[").unwrap();
        let entries = pools
            .get(pool_id)
            .ok_or_else(|| format!("{pool_id}: dump has no runtime pool order"))?;
        for (_, id) in entries {
            let card = cards
                .iter()
                .find(|card| card.id == *id)
                .expect("validated above");
            writeln!(
                out,
                "            Cards::{}({enum_name}::{}),",
                enum_name.trim_end_matches("Card"),
                variant(card, suffix)
            )
            .unwrap();
        }
        writeln!(out, "        ],").unwrap();
    }
    writeln!(out, "        _ => &[],").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// Cards eligible for an ordinary room reward, split by rarity.\n\
         ///\n\
         /// Each slice keeps the game's pool order, so an index into one means\n\
         /// what the same index into the filtered pool meant. A card at no\n\
         /// drafted rarity is in none of them, which is what the game's own\n\
         /// rarity filter does with it."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn reward_cards(character: Character) -> Drafted<'static, Cards> {{"
    )
    .unwrap();
    writeln!(out, "    match character {{").unwrap();
    for (character, entries) in &reward_pools {
        writeln!(out, "        Character::{} => Drafted {{", camel(character)).unwrap();
        for (field, rarity) in [
            ("common", "Common"),
            ("uncommon", "Uncommon"),
            ("rare", "Rare"),
        ] {
            writeln!(out, "            {field}: &[").unwrap();
            for (_, id) in entries {
                let card = cards
                    .iter()
                    .find(|card| card.id == *id)
                    .expect("validated above");
                if sanitize(&card.rarity) != rarity {
                    continue;
                }
                let (_, _, enum_name, suffix) = POOLS
                    .iter()
                    .find(|(pool, ..)| *pool == card.pool)
                    .expect("a card has a generated pool");
                writeln!(
                    out,
                    "                Cards::{}({enum_name}::{}),",
                    enum_name.trim_end_matches("Card"),
                    variant(card, *suffix)
                )
                .unwrap();
            }
            writeln!(out, "            ],").unwrap();
        }
        writeln!(out, "        }},").unwrap();
    }
    writeln!(out, "        _ => EMPTY_DRAFT,").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// The colourless pool, split by rarity.\n\
         ///\n\
         /// `ColorlessCardPool.GenerateAllCards`, which no character holds and\n\
         /// so no reward pool covers. One relic draws from it. The pool's own\n\
         /// order is not the id order: it deals Jack of All Trades before\n\
         /// Jackpot, and the two are the same rarity, so the draw lands\n\
         /// differently."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn colorless_cards() -> Drafted<'static, Cards> {{"
    )
    .unwrap();
    let colorless = pools
        .get(COLORLESS)
        .ok_or_else(|| format!("the dump has no {COLORLESS} rows"))?;
    writeln!(out, "    Drafted {{").unwrap();
    for (field, rarity) in [
        ("common", "Common"),
        ("uncommon", "Uncommon"),
        ("rare", "Rare"),
    ] {
        writeln!(out, "        {field}: &[").unwrap();
        for (_, id) in colorless {
            let card = cards
                .iter()
                .find(|card| card.id == *id)
                .expect("validated above");
            if sanitize(&card.rarity) != rarity {
                continue;
            }
            let (_, _, enum_name, suffix) = POOLS
                .iter()
                .find(|(pool, ..)| *pool == card.pool)
                .expect("a card has a generated pool");
            writeln!(
                out,
                "            Cards::{}({enum_name}::{}),",
                enum_name.trim_end_matches("Card"),
                variant(card, *suffix)
            )
            .unwrap();
        }
        writeln!(out, "        ],").unwrap();
    }
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// A character no run is played as, which drafts from nothing."
    )
    .unwrap();
    writeln!(
        out,
        "const EMPTY_DRAFT: Drafted<'static, Cards> = Drafted {{\n\
         \x20   common: &[],\n\
         \x20   uncommon: &[],\n\
         \x20   rare: &[],\n\
         }};\n"
    )
    .unwrap();

    // The variant is the pool, and the five that name a character are the
    // five a run can draw from. The rest — colourless, curses, statuses,
    // tokens, quest and event cards — are held by no character, which is not
    // the same as being held by a pool nobody plays.
    writeln!(out, "impl sts2_core::CardTable for Cards {{").unwrap();
    writeln!(out, "    type Pool = CardPool;").unwrap();
    writeln!(out, "    type Rarity = CardRarity;").unwrap();
    writeln!(out, "    type Target = TargetType;").unwrap();
    writeln!(out, "    type Var = CardVar;\n").unwrap();
    writeln!(out, "    type Number = CardNumber;").unwrap();
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
    writeln!(out).unwrap();
    writeln!(out, "    fn cost(self) -> Cost {{").unwrap();
    writeln!(out, "        self.data().cost").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn star_cost(self) -> Cost {{").unwrap();
    writeln!(out, "        self.data().star_cost").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn target(self) -> Option<TargetType> {{").unwrap();
    writeln!(out, "        match self.data().target {{").unwrap();
    writeln!(out, "            Field::Constant(target) => Some(target),").unwrap();
    writeln!(out, "            Field::Computed => None,").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn unplayable(self) -> bool {{").unwrap();
    writeln!(
        out,
        "        self.data().keywords.contains(CardKeyword::Unplayable)"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn exhausts(self) -> bool {{").unwrap();
    writeln!(
        out,
        "        self.data().keywords.contains(CardKeyword::Exhaust)"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn ethereal(self) -> bool {{").unwrap();
    writeln!(
        out,
        "        self.data().keywords.contains(CardKeyword::Ethereal)"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn retains(self) -> bool {{").unwrap();
    writeln!(
        out,
        "        self.data().keywords.contains(CardKeyword::Retain)"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn max_upgrade(self) -> u8 {{").unwrap();
    writeln!(out, "        self.data().max_upgrade").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn power(self) -> Option<bool> {{").unwrap();
    writeln!(out, "        match self.data().kind {{").unwrap();
    writeln!(
        out,
        "            Field::Constant(kind) => Some(kind == CardType::Power),"
    )
    .unwrap();
    writeln!(out, "            Field::Computed => None,").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    /// Ordinal over the id, which is what `ModelId.CompareTo` is."
    )
    .unwrap();
    writeln!(out, "    fn order(self) -> u16 {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    for (_, _, enum_name, _) in POOLS {
        writeln!(
            out,
            "            Cards::{}(c) => c.order(),",
            enum_name.trim_end_matches("Card")
        )
        .unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn vars(self) -> &'static [sts2_core::CardNumber<CardVar, CardNumber>] {{"
    )
    .unwrap();
    writeln!(out, "        self.data().vars").unwrap();
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
                    "CardVar",
                    "KeywordSet",
                    "TargetType",
                ],
            ),
            ("crate", &["Cost", "Field"]),
            ("sts2_core", &["CardNumberModifier"]),
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
