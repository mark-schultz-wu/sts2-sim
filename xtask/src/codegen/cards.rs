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

use super::DumpError;
use super::bridge::{Bridge, emit_bridge};
use super::sets;
use super::split::Split;
use super::{Build, DERIVES, Dump, banner, camel, repr, sanitize};
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
    #[error("{card}: costs X {pool} but canonical cost is {cost}")]
    XCost {
        card: String,
        pool: &'static str,
        cost: String,
    },
    #[error("{card}: negative {pool} cost {cost} is not the -1 sentinel")]
    NegativeCost {
        card: String,
        pool: &'static str,
        cost: String,
    },
    #[error("{card}: {pool} cost is the largest byte")]
    LargestCost { card: String, pool: &'static str },
    #[error("{0} has variables but is not a card")]
    NotACard(String),
    #[error("{at}: index {index}, expected {expected}")]
    PoolGap {
        at: PoolAt,
        index: usize,
        expected: usize,
    },
    #[error("{0}: not a card the dump declares")]
    UnknownCard(PoolAt),
    #[error("{at}: the card declares pool {declared}")]
    OtherPool { at: PoolAt, declared: String },
    #[error("{card}: {var} has too many decimal places")]
    Decimals { card: String, var: String },
    #[error("{card}: {var} is a {class}, which nobody has read the preview of")]
    Unpreviewed {
        card: String,
        var: String,
        class: String,
    },
    #[error("no card prints a CalculationBase, which a calculated number reads")]
    NoCalculationBase,
    #[error("{var} is a PowerVar of {} on one card and of {} on another", powers.0, powers.1)]
    PowerVarPowers {
        var: String,
        powers: (String, String),
    },
    #[error("{pool}: the dump has no {kind} rows to order it")]
    NoOrder { kind: &'static str, pool: String },
}

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

/// The reflected enums a rule sorts by, the way `orderby c.Rarity` does.
const ORDERED: [&str; 1] = ["CardRarity"];

/// The enums here the simulator names members of.
const BRIDGES: [Bridge; 3] = [
    Bridge {
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
    },
    Bridge {
        enum_name: "CardRarity",
        trait_name: "CardRarityKinds",
        named: &[
            ("BASIC", "Basic"),
            ("COMMON", "Common"),
            ("UNCOMMON", "Uncommon"),
            ("RARE", "Rare"),
            ("ANCIENT", "Ancient"),
            ("EVENT", "Event"),
            ("TOKEN", "Token"),
            ("STATUS", "Status"),
            ("CURSE", "Curse"),
            ("QUEST", "Quest"),
        ],
        unnamed: &["None"],
    },
    Bridge {
        enum_name: "CardTag",
        trait_name: "CardTagKinds",
        named: &[
            ("STRIKE", "Strike"),
            ("DEFEND", "Defend"),
            ("MINION", "Minion"),
            ("OSTY_ATTACK", "OstyAttack"),
            ("SHIV", "Shiv"),
        ],
        unnamed: &["None"],
    },
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
    star_cost: String,
    star_costs_x: bool,
    max_upgrade: String,
    keywords: Vec<String>,
    tags: Vec<String>,
    /// `GainsBlock`.
    gains_block: bool,
    /// `VisualCardPool.IsColorless`.
    visually_colorless: bool,
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
        "OstyDamageVar" | "CalculatedDamageVar+FromOsty" => "Some(CardNumberModifier::OstyDamage)",
        "DamageVar+Unpowered" => "Some(CardNumberModifier::UnpoweredDamage)",
        "BlockVar" | "CalculatedBlockVar" => "Some(CardNumberModifier::Block)",
        "BlockVar+Unpowered" => "Some(CardNumberModifier::UnpoweredBlock)",
        class if class.starts_with("PowerVar+") => "Some(CardNumberModifier::PowerGiven)",
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
const DROPPED: [(&str, &str, &str); 4] = [
    (
        "CardTag",
        "None",
        "no card carries it. It is the C# zero, and a card with no tag has an \
         empty set.",
    ),
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
fn cost(card: &Card) -> Result<String, Error> {
    priced(&card.id, "energy", card.costs_x, &card.cost)
}

/// The star pool, read the same way and checked the same way.
///
/// `CanonicalStarCost` defaults to the same `-1` and `HasStarCostX` to false,
/// so the two pools encode a cost identically. Two cards cost X stars.
fn star_cost(card: &Card) -> Result<String, Error> {
    priced(&card.id, "star", card.star_costs_x, &card.star_cost)
}

fn priced(id: &str, pool: &'static str, costs_x: bool, canonical: &str) -> Result<String, Error> {
    match (costs_x, canonical) {
        (true, "0" | "-1") => Ok("Cost::X".into()),
        (true, n) => Err(Error::XCost {
            card: id.to_owned(),
            pool,
            cost: n.to_owned(),
        }),
        (false, "-1") => Ok("Cost::None".into()),
        (false, n) if n.starts_with('-') => Err(Error::NegativeCost {
            card: id.to_owned(),
            pool,
            cost: n.to_owned(),
        }),
        (false, n) => {
            let n: u8 = DumpError::number(format_args!("{id}: {pool} cost"), n)?;
            nonmax::NonMaxU8::new(n)
                .map(|_| format!("Cost::fixed({n})"))
                .ok_or_else(|| Error::LargestCost {
                    card: id.to_owned(),
                    pool,
                })
        }
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

/// A card named as in a pool the build has no enum for.
#[derive(Debug, thiserror::Error)]
#[error("{card} is in {pool}, which is not a card pool")]
pub struct NotAPool {
    card: String,
    pool: String,
}

/// One card, as the path another domain has to spell to name it.
///
/// Card identity is pool-scoped. `Strike` is a variant of five different enums,
/// so naming one from outside takes the pool as well as the id.
///
/// The suffix rule lives here because this is the domain that invented it. A
/// second copy of it elsewhere is a second chance to get `STRIKE_IRONCLAD`
/// wrong.
pub(super) fn card_path(pool: &str, id: &str) -> Result<String, NotAPool> {
    let (_, _, enum_name, suffix) = POOLS
        .iter()
        .find(|(pool_id, ..)| *pool_id == pool)
        .ok_or_else(|| NotAPool {
            card: id.to_owned(),
            pool: pool.to_owned(),
        })?;

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

/// One kind of pool row, as each pool's cards in the order the game lists
/// them.
///
/// Each pool's indices run from zero with no gap, and every card is one the
/// dump declares. With `own`, the key is a pool and each card declares it.
fn listed(
    dump: &Dump,
    kind: &'static str,
    cards: &[Card],
    own: bool,
) -> Result<BTreeMap<String, Vec<(usize, String)>>, Error> {
    let mut pools: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    for f in dump.of(kind) {
        let index = DumpError::number(format_args!("{}: {kind} index", f[1]), &f[2])?;
        pools
            .entry(f[1].clone())
            .or_default()
            .push((index, f[3].clone()));
    }
    for (pool, entries) in &mut pools {
        entries.sort_by_key(|(index, _)| *index);
        for (expected, (index, card)) in entries.iter().enumerate() {
            let at = || PoolAt {
                kind,
                pool: pool.clone(),
                card: card.clone(),
            };
            if *index != expected {
                return Err(Error::PoolGap {
                    at: at(),
                    index: *index,
                    expected,
                });
            }
            let Some(row) = cards.iter().find(|row| row.id == *card) else {
                return Err(Error::UnknownCard(at()));
            };
            if own && row.pool != *pool {
                return Err(Error::OtherPool {
                    at: at(),
                    declared: row.pool.clone(),
                });
            }
        }
    }
    Ok(pools)
}

/// Where in a pool listing a card sits.
#[derive(Debug)]
pub struct PoolAt {
    kind: &'static str,
    pool: String,
    card: String,
}

impl std::fmt::Display for PoolAt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.kind, self.pool, self.card)
    }
}

/// Every card, as one module per pool plus the shared field enums.
///
/// Returns the dump's content hash, which stamps the roll-up too.
pub fn generate(root: &Path) -> Result<Build, Error> {
    let mut dump = Dump::read("cards.txt")?;
    super::mad_science::expand(&mut dump);
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
            tags: if f[12] == "-" {
                Vec::new()
            } else {
                f[12].split(',').map(String::from).collect()
            },
            gains_block: f[13] == "1",
            visually_colorless: f[14] == "1",
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
        return Err(Error::NotACard(id));
    }
    let cards = cards;
    let fixed_numbers = cards
        .iter()
        .flat_map(|card| &card.vars)
        .any(|(_, base, _, _)| base.contains('.'));

    // Runtime order inside each CardPoolModel. Reward generation filters this
    // list by rarity and indexes it with RNG, so the table's id order is not a
    // substitute.
    let pools = listed(&dump, "pool", &cards, true)?;

    // The same pools, already narrowed to what a combat generation effect can
    // draw. The narrowing is the game's own `CardFactory.FilterForCombat`, run
    // in the dump, because the facts it reads -- `CanBeGeneratedInCombat` and
    // `MultiplayerConstraint` -- are on the model and in no table.
    let combat_pools = listed(&dump, "combatpool", &cards, true)?;

    // The same pools again, narrowed the way a merchant sees them. The
    // narrowing is `GetUnlockedCards` with a one-player run's constraint, run
    // in the dump for the same reason `combatpool` is.
    let shop_pools = listed(&dump, "shoppool", &cards, true)?;

    // Keyed by character rather than by pool, so no card declares the key.
    let reward_pools = listed(&dump, "rewardpool", &cards, false)?;

    // Discriminant and name, because a dropped member leaves a hole that the
    // rest must not slide into.
    let mut enums: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    for f in dump.of("enum") {
        let discriminant =
            DumpError::number::<u8>(format_args!("{} {}: discriminant", f[1], f[3]), &f[2])?;
        enums
            .entry(f[1].clone())
            .or_default()
            .push((discriminant, f[3].clone()));
    }

    let mut split = Split::new(root, "cards", "cards.txt", &build)?;

    let mut total = 0usize;
    let mut stripped = 0usize;

    // Every card's place in the build's own id order. `ModelId.CompareTo` is
    // ordinal over the id, and shuffling a pile sorts by it. The id is the
    // game's, which the nine Mad Sciences share.
    let ranks: BTreeMap<&str, u16> = {
        let ids: std::collections::BTreeSet<&str> = cards
            .iter()
            .map(|card| super::mad_science::game_id(&card.id))
            .collect();
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
            writeln!(out, "    {},", ranks[super::mad_science::game_id(&card.id)]).unwrap();
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
            let tags = card
                .tags
                .iter()
                .map(|t| format!("CardTag::{}", sanitize(t)))
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
                    let scale = u8::try_from(fraction.len()).map_err(|_| Error::Decimals {
                        card: card.id.clone(),
                        var: name.clone(),
                    })?;
                    let digits = format!("{whole}{fraction}");
                    let coefficient: i32 =
                        DumpError::number(format_args!("{}: {name}", card.id), &digits)?;
                    format!("super::CardNumber::new({coefficient}, {scale})")
                } else {
                    DumpError::number::<i32>(format_args!("{}: {name}", card.id), base)?.to_string()
                };
                writeln!(
                    row,
                    "            sts2_core::CardNumber {{ id: CardVar::{}, base: {base}, calculated: {calculated}, modifier: {} }},",
                    sanitize(name),
                    previewed(class).ok_or_else(|| Error::Unpreviewed {
                        card: card.id.clone(),
                        var: name.clone(),
                        class: class.clone(),
                    })?
                )
                .unwrap();
            }
            writeln!(row, "        ],").unwrap();
            writeln!(row, "        max_upgrade: {},", card.max_upgrade).unwrap();
            writeln!(row, "        keywords: KeywordSet::new(&[{keywords}]),").unwrap();
            writeln!(row, "        tags: TagSet::new(&[{tags}]),").unwrap();
            writeln!(row, "        gains_block: {},", card.gains_block).unwrap();
            writeln!(
                row,
                "        visually_colorless: {},",
                card.visually_colorless
            )
            .unwrap();
            write!(row, "    }}").unwrap();
            table.push(row);
        }
        super::shared_rows(out, "pub(super) ", "CardData", &table);
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
        if ORDERED.contains(&name.as_str()) {
            // Declared in the order of its values, so the derived order is
            // the game's.
            assert!(
                members.windows(2).all(|pair| pair[0].0 < pair[1].0),
                "{name} is reflected out of the order of its values"
            );
            writeln!(
                out,
                "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]"
            )
            .unwrap();
        } else {
            writeln!(out, "{DERIVES}").unwrap();
        }
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

    // `CalculatedVar.GetBaseVar`, which the simulator reads by this name.
    if !keys.contains(&"CalculationBase") {
        return Err(Error::NoCalculationBase);
    }
    // The power each key's `PowerVar` applies. One key names one power
    // wherever it is filed.
    let mut powers: BTreeMap<&str, &str> = BTreeMap::new();
    for card in &cards {
        for (name, _, _, class) in &card.vars {
            let Some(power) = class.strip_prefix("PowerVar+") else {
                continue;
            };
            if let Some(&was) = powers.get(name.as_str())
                && was != power
            {
                return Err(Error::PowerVarPowers {
                    var: name.clone(),
                    powers: (was.to_owned(), power.to_owned()),
                });
            }
            powers.insert(name, power);
        }
    }
    writeln!(out, "impl sts2_core::CardVars for CardVar {{").unwrap();
    writeln!(out, "    type Power = super::powers::Power;\n").unwrap();
    writeln!(
        out,
        "    const CALCULATION_BASE: CardVar = CardVar::CalculationBase;\n"
    )
    .unwrap();

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
    writeln!(out, "    }}\n").unwrap();
    writeln!(
        out,
        "    fn power(self) -> Option<super::powers::Power> {{\n        Some(match self {{"
    )
    .unwrap();
    for (key, power) in &powers {
        writeln!(
            out,
            "            CardVar::{} => super::powers::Power::{},",
            sanitize(key),
            camel(power)
        )
        .unwrap();
    }
    writeln!(out, "            _ => return None,\n        }})\n    }}").unwrap();
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

    // A card's tags, as keywords are: a `HashSet` the game asks `Contains` of.
    let tags: Vec<sets::Member<'_>> = enums["CardTag"]
        .iter()
        .filter(|(_, m)| !dropped("CardTag", m))
        .map(|(bit, m)| sets::Member {
            name: m,
            bit: *bit as usize,
            doc: "",
        })
        .collect();
    sets::plain(
        &mut out,
        &sets::Set {
            flag: "CardTag",
            set: "TagSet",
            flag_doc: "",
            set_doc: "A set of [`CardTag`]s: one bit per tag, at its discriminant.\n\
                      \n\
                      Bit 0 is unused, because the game's zero is `None`.",
            members: &tags,
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
    writeln!(out, "    /// `CanonicalTags`, which no card computes.").unwrap();
    writeln!(out, "    pub tags: TagSet,").unwrap();
    writeln!(
        out,
        "    /// `GainsBlock`, which a card overrides to say so."
    )
    .unwrap();
    writeln!(out, "    pub gains_block: bool,").unwrap();
    writeln!(
        out,
        "    /// `VisualCardPool.IsColorless`: whether it shows as colorless."
    )
    .unwrap();
    writeln!(out, "    pub visually_colorless: bool,").unwrap();
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
    writeln!(
        out,
        "        // The pool's table and the place in it. Every arm has the same\n\
         \x20       // shape, so the match is a lookup by pool, and one index follows."
    )
    .unwrap();
    writeln!(
        out,
        "        let (rows, at): (&'static [CardData], usize) = match self {{"
    )
    .unwrap();
    for (_, module, enum_name, _) in POOLS {
        writeln!(
            out,
            "            Cards::{}(c) => (&{module}::ROWS, c as usize),",
            enum_name.trim_end_matches("Card")
        )
        .unwrap();
    }
    writeln!(out, "        }};").unwrap();
    writeln!(out, "        &rows[at]").unwrap();
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
        let entries = pools.get(pool_id).ok_or_else(|| Error::NoOrder {
            kind: "pool",
            pool: pool_id.to_string(),
        })?;
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
        "/// A character pool as a combat generation effect sees it.\n\
         ///\n\
         /// [`pool_cards`] narrowed by `CardFactory.FilterForCombat`, in the\n\
         /// same order, which is the order the shuffle it feeds will see. An\n\
         /// effect that wants one card type filters this further, and\n\
         /// filtering keeps the order, so the result is what the game's own\n\
         /// query gives."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn combat_cards(pool: CardPool) -> &'static [Cards] {{"
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
        let entries = combat_pools.get(pool_id).ok_or_else(|| Error::NoOrder {
            kind: "combatpool",
            pool: pool_id.to_string(),
        })?;
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
        "/// A pool as a merchant sees it.\n\
         ///\n\
         /// [`pool_cards`] narrowed by `GetUnlockedCards` for a one-player\n\
         /// run, in the same order. A shelf narrows this further by rarity and\n\
         /// by card type, and filtering keeps the order, so the result is what\n\
         /// the game's own query gives."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn merchant_cards(pool: CardPool) -> &'static [Cards] {{"
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
        let entries = shop_pools.get(pool_id).ok_or_else(|| Error::NoOrder {
            kind: "shoppool",
            pool: pool_id.to_string(),
        })?;
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
        "/// The colourless pool as a merchant sees it.\n\
         ///\n\
         /// The same narrowing [`merchant_cards`] applies, on the one pool no\n\
         /// character holds. Two of a merchant's shelves read it."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn merchant_colorless_cards() -> &'static [Cards] {{"
    )
    .unwrap();
    writeln!(out, "    &[").unwrap();
    {
        let entries = shop_pools
            .get("COLORLESS_CARD_POOL")
            .ok_or_else(|| Error::NoOrder {
                kind: "shoppool",
                pool: COLORLESS.to_string(),
            })?;
        for (_, id) in entries {
            let card = cards
                .iter()
                .find(|card| card.id == *id)
                .expect("validated above");
            writeln!(
                out,
                "        Cards::Colorless(ColorlessCard::{}),",
                variant(card, None)
            )
            .unwrap();
        }
    }
    writeln!(out, "    ]").unwrap();
    writeln!(out, "}}\n").unwrap();

    writeln!(
        out,
        "/// The colourless pool as a combat generation effect sees it.\n\
         ///\n\
         /// The same narrowing [`combat_cards`] applies, on the one pool no\n\
         /// character holds."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn combat_colorless_cards() -> &'static [Cards] {{"
    )
    .unwrap();
    writeln!(out, "    &[").unwrap();
    {
        let entries = combat_pools.get(COLORLESS).ok_or_else(|| Error::NoOrder {
            kind: "combatpool",
            pool: COLORLESS.to_string(),
        })?;
        for (_, id) in entries {
            let card = cards
                .iter()
                .find(|card| card.id == *id)
                .expect("validated above");
            writeln!(
                out,
                "        Cards::Colorless(ColorlessCard::{}),",
                variant(card, None)
            )
            .unwrap();
        }
    }
    writeln!(out, "    ]").unwrap();
    writeln!(out, "}}\n").unwrap();

    // The curse and status pools, which no character holds, each unlocked
    // and as a combat generation effect sees it. A transform of a curse or a
    // status draws from its own.
    for (fname, doc, lists, kind, pool, wrap, card_enum) in [
        (
            "unlocked_curse_cards",
            "The curse pool, unlocked.",
            &shop_pools,
            "shoppool",
            "CURSE_CARD_POOL",
            "Curse",
            "CurseCard",
        ),
        (
            "combat_curse_cards",
            "The curse pool as a combat generation effect sees it.",
            &combat_pools,
            "combatpool",
            "CURSE_CARD_POOL",
            "Curse",
            "CurseCard",
        ),
        (
            "unlocked_status_cards",
            "The status pool, unlocked.",
            &shop_pools,
            "shoppool",
            "STATUS_CARD_POOL",
            "Status",
            "StatusCard",
        ),
        (
            "combat_status_cards",
            "The status pool as a combat generation effect sees it.",
            &combat_pools,
            "combatpool",
            "STATUS_CARD_POOL",
            "Status",
            "StatusCard",
        ),
    ] {
        writeln!(out, "/// {doc}").unwrap();
        writeln!(out, "pub fn {fname}() -> &'static [Cards] {{").unwrap();
        writeln!(out, "    &[").unwrap();
        let entries = lists.get(pool).ok_or_else(|| Error::NoOrder {
            kind,
            pool: pool.to_string(),
        })?;
        for (_, id) in entries {
            let card = cards
                .iter()
                .find(|card| card.id == *id)
                .expect("validated above");
            writeln!(
                out,
                "        Cards::{wrap}({card_enum}::{}),",
                variant(card, None)
            )
            .unwrap();
        }
        writeln!(out, "    ]").unwrap();
        writeln!(out, "}}\n").unwrap();
    }

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
         /// `ColorlessCardPool.GetUnlockedCards` for a run of one player, which\n\
         /// leaves out the cards only a multiplayer run deals. No character\n\
         /// holds it, so no reward pool covers it. The pool's own order is not\n\
         /// the id order: it deals Jack of All Trades before Jackpot, and the\n\
         /// two are the same rarity, so the draw lands differently."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn colorless_cards() -> Drafted<'static, Cards> {{"
    )
    .unwrap();
    let colorless = shop_pools.get(COLORLESS).ok_or_else(|| Error::NoOrder {
        kind: "shoppool",
        pool: COLORLESS.to_string(),
    })?;
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
    writeln!(out, "    type Tag = CardTag;\n").unwrap();
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
    writeln!(out, "    fn gains_block(self) -> bool {{").unwrap();
    writeln!(out, "        self.data().gains_block").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn visually_colorless(self) -> bool {{").unwrap();
    writeln!(out, "        self.data().visually_colorless").unwrap();
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
    writeln!(out, "    fn innate(self) -> bool {{").unwrap();
    writeln!(
        out,
        "        self.data().keywords.contains(CardKeyword::Innate)"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn eternal(self) -> bool {{").unwrap();
    writeln!(
        out,
        "        self.data().keywords.contains(CardKeyword::Eternal)"
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
    writeln!(out, "    fn sly(self) -> bool {{").unwrap();
    writeln!(
        out,
        "        self.data().keywords.contains(CardKeyword::Sly)"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    fn tagged(self, tag: CardTag) -> bool {{").unwrap();
    writeln!(out, "        self.data().tags.contains(tag)").unwrap();
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
    writeln!(out).unwrap();
    writeln!(
        out,
        "    fn overrides(self) -> sts2_core::Overrides {{\n\
         \x20       OVERRIDES[usize::from(<Self as sts2_core::CardTable>::order(self))]\n\
         \x20   }}\n\
         \n\
         \x20   fn heard_by_any() -> sts2_core::Overrides {{\n\
         \x20       HEARD_BY_ANY\n\
         \x20   }}"
    )
    .unwrap();
    writeln!(out, "}}").unwrap();

    // Indexed by rank, which `order` is, so one flat table serves every pool.
    let mut ranked: Vec<(&u16, &&str)> = ranks.iter().map(|(id, rank)| (rank, id)).collect();
    ranked.sort_unstable();
    let overrides = super::overrides(&build)?;
    let ranked: Vec<&str> = ranked.into_iter().map(|(_, id)| *id).collect();
    super::emit_overrides(
        &mut out,
        "indexed by the card's place in the build's id order.",
        ranked.iter().copied(),
        &overrides,
    );
    // Every hook some card's class overrides, so a question no card can hear
    // skips the walk over the deck.
    let mut heard = String::from("sts2_core::Overrides::NONE");
    let mut seen: Vec<sts2_core::ModelHook> = Vec::new();
    for id in &ranked {
        for hook in overrides.get(*id).into_iter().flatten() {
            if !seen.contains(hook) {
                seen.push(*hook);
                write!(heard, ".with(sts2_core::ModelHook::{hook:?})").unwrap();
            }
        }
    }
    writeln!(
        out,
        "\n/// Every hook some card's class overrides.\n\
         static HEARD_BY_ANY: sts2_core::Overrides = {heard};"
    )
    .unwrap();

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
                    "CardTag",
                    "CardType",
                    "CardVar",
                    "KeywordSet",
                    "TagSet",
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
