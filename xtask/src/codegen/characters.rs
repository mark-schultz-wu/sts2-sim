//! Every character: what a run starts with, and what it draws from.
//!
//! The smallest domain and the one that ties the others together. A character
//! is three pool references, a starting deck, a starting relic and four numbers.
//! Most of it points at content another dump already printed.
//!
//! The xtask README says which pool references are typed, and why `CardPool`
//! is the one declared here.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::cards::{POOLS as CARD_POOLS, card_path};
use super::{Build, DERIVES, Dump, banner, camel, repr};

/// The most cards a starting deck holds, and the most starting relics.
///
/// Caps rather than guesses. The build's largest are the Silent's twelve cards
/// and one relic apiece, and generation fails rather than truncating if either
/// grows. They are what lets both be inline instead of a pointer. See
/// `sts2_content::Few`.
const DECK_CAP: usize = 12;
const RELIC_CAP: usize = 2;

/// A pool column, and the type its values generate as.
///
/// `RelicPool` is the relics domain's, and the other two are declared here. See
/// the module docs for why they differ.
const POOLS: [(&str, &str, usize, &str, bool); 3] = [
    ("card_pool", "CardPool", 9, "_CARD_POOL", true),
    ("relic_pool", "RelicPool", 10, "_RELIC_POOL", false),
    ("potion_pool", "PotionPool", 11, "_POTION_POOL", false),
];

pub fn generate(root: &Path) -> Result<Build, String> {
    let dump = Dump::read("characters.txt")?;
    let build = dump.build.clone();

    let characters: Vec<&Vec<String>> = dump.of("character").collect();
    if characters.is_empty() {
        return Err("the dump declares no characters".into());
    }
    for row in &characters {
        if row.len() != 13 {
            return Err(format!("{}: {} columns, not 12", row[1], row.len() - 1));
        }
    }

    // Starting HP and gold were read at all eleven ascension levels and neither
    // moved, so both are plain numbers. A build that starts scaling one would
    // generate a table describing ascension 0 only, so it fails here instead.
    if dump.of("scalestart").next().is_some() {
        return Err(
            "a character's starting HP or gold now scales with ascension, which this table \
             cannot spell"
                .into(),
        );
    }

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

    // The card and the pool it is from. A starting deck is not always the
    // character's own pool: `RANDOM_CHARACTER` opens with a Strike and a Defend
    // from each of the five, while its own `CardPool` is the Ironclad's.
    let mut deck: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for row in dump.of("deck") {
        deck.entry(&row[1]).or_default().push((&row[3], &row[4]));
    }

    let mut relics: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in dump.of("startingrelic") {
        relics.entry(&row[1]).or_default().push(&row[3]);
    }

    for row in &characters {
        let cards = deck.get(row[1].as_str()).map_or(0, Vec::len);
        if cards > DECK_CAP {
            return Err(format!(
                "{} starts with {cards} cards, past the {DECK_CAP} a Deck can hold",
                row[1]
            ));
        }
        let held = relics.get(row[1].as_str()).map_or(0, Vec::len);
        if held > RELIC_CAP {
            return Err(format!(
                "{} starts with {held} relics, past the {RELIC_CAP} a StartingRelics can hold",
                row[1]
            ));
        }
    }

    let mut out = banner(
        "Every character's starting kit and pools.",
        "characters.txt",
        &build,
    );
    // The pool enums the decks actually name, so the import list is what the
    // file uses and not every pool the game has.
    let mut named: Vec<&str> = vec!["Cards"];
    for cards in deck.values() {
        for (_, pool) in cards {
            let (_, _, enum_name, _) = CARD_POOLS
                .iter()
                .find(|(id, ..)| id == pool)
                .ok_or_else(|| format!("{pool} is not a card pool"))?;
            if !named.contains(enum_name) {
                named.push(enum_name);
            }
        }
    }
    named[1..].sort_unstable();

    writeln!(out, "\nuse super::cards::{{{}}};", named.join(", ")).unwrap();
    writeln!(out, "use super::potions::PotionPool;").unwrap();
    writeln!(out, "use super::relics::{{Relic, RelicPool}};").unwrap();
    writeln!(out, "use crate::Few;\nuse sts2_core::Table;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};\n").unwrap();

    for (name, members) in &enums {
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr(u8)]").unwrap();
        writeln!(out, "pub enum {name} {{").unwrap();
        for (discriminant, m) in members {
            writeln!(out, "    {m} = {discriminant},").unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    // The two pool enums this domain declares, from the values it actually
    // sees. Reflected from the dump rather than from the pool models, because
    // what matters here is which pools a character can point at.
    for (field, ty, column, suffix, local) in POOLS {
        if !local {
            continue;
        }

        let mut seen: Vec<&str> = Vec::new();
        for row in &characters {
            if !seen.contains(&row[column].as_str()) {
                seen.push(&row[column]);
            }
        }
        seen.sort_unstable();

        writeln!(out, "/// The pools a character's `{field}` can name.").unwrap();
        writeln!(out, "{DERIVES}").unwrap();
        writeln!(out, "#[repr({})]", repr(seen.len())?).unwrap();
        writeln!(out, "pub enum {ty} {{").unwrap();
        for (i, pool) in seen.iter().enumerate() {
            writeln!(out, "    #[serde(rename = \"{pool}\")]").unwrap();
            writeln!(
                out,
                "    {} = {i},",
                camel(pool.strip_suffix(suffix).unwrap_or(pool))
            )
            .unwrap();
        }
        writeln!(out, "}}\n").unwrap();
    }

    writeln!(
        out,
        "/// The cards a run opens with, in the order they are added.\n\
         ///\n\
         /// A list and not a set: the same card appears several times, and the\n\
         /// order is what the first shuffle sees.\n\
         pub type Deck = Few<Cards, {DECK_CAP}>;\n\
         \n\
         /// The relics a run opens with. One apiece in this build.\n\
         pub type StartingRelics = Few<Relic, {RELIC_CAP}>;\n\
         \n\
         /// What a run starts with, and what it draws from.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct CharacterData {{\n\
         \x20   /// Whether the character select screen offers it. Three of the\n\
         \x20   /// eight are not: the deprecated placeholder, the Deprived, and\n\
         \x20   /// the `random` pseudo-character.\n\
         \x20   pub playable: bool,\n\
         \x20   pub gender: CharacterGender,\n\
         \x20   /// Flat: read at all eleven ascension levels and unmoved, which\n\
         \x20   /// is worth knowing because the first game scaled it.\n\
         \x20   pub starting_hp: u16,\n\
         \x20   pub starting_gold: u16,\n\
         \x20   pub max_energy: u8,\n\
         \x20   /// Orb slots at the start. Three for the Defect, none for\n\
         \x20   /// anyone else.\n\
         \x20   pub orb_slots: u8,\n\
         \x20   pub card_pool: CardPool,\n\
         \x20   pub relic_pool: RelicPool,\n\
         \x20   pub potion_pool: PotionPool,\n\
         \x20   pub deck: Deck,\n\
         \x20   pub starting_relics: StartingRelics,\n\
         \x20   /// The character a run must be finished as before this one is\n\
         \x20   /// offered, or `None` for one available from the start.\n\
         \x20   pub unlocks_after: Option<Character>,\n\
         }}\n"
    )
    .unwrap();

    emit_characters(&mut out, &characters, &deck, &relics)?;

    let dir = root.join("characters.rs");
    fs::write(&dir, &out).map_err(|e| format!("writing {}: {e}", dir.display()))?;

    let playable = characters.iter().filter(|r| r[2] == "1").count();
    println!("  characters: {} ({playable} playable)", characters.len());

    Ok(build)
}

fn emit_characters(
    out: &mut String,
    characters: &[&Vec<String>],
    deck: &BTreeMap<&str, Vec<(&str, &str)>>,
    relics: &BTreeMap<&str, Vec<&str>>,
) -> Result<(), String> {
    writeln!(
        out,
        "/// Every character in the build, playable or not.\n\
         ///\n\
         /// The unplayable three are dumped rather than left out, on the same\n\
         /// reasoning the monster dump reflects instead of walking encounter\n\
         /// tables: a roster gathered from what uses it is a roster with holes.\n\
         ///\n\
         /// The id is the serde rename, and the only place it is spelled."
    )
    .unwrap();
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr({})]", repr(characters.len())?).unwrap();
    writeln!(out, "pub enum Character {{").unwrap();
    for row in characters {
        writeln!(out, "    #[serde(rename = \"{}\")]", row[1]).unwrap();
        writeln!(out, "    {},", camel(&row[1])).unwrap();
    }
    writeln!(out, "}}\n").unwrap();

    writeln!(out, "impl sts2_core::Table for Character {{").unwrap();
    writeln!(out, "    type Data = CharacterData;\n").unwrap();
    writeln!(
        out,
        "    /// Every character, so callers can sweep the table without a way\n\
         \x20   /// to enumerate an enum."
    )
    .unwrap();
    writeln!(out, "    const ALL: &'static [Self] = &[").unwrap();
    for row in characters {
        writeln!(out, "        Character::{},", camel(&row[1])).unwrap();
    }
    writeln!(out, "    ];\n").unwrap();

    let mut table: Vec<String> = Vec::new();
    for row in characters {
        table.push(literal(row, deck, relics)?);
    }
    writeln!(out, "    /// What a run as this character starts with.").unwrap();
    writeln!(out, "    fn data(self) -> &'static CharacterData {{").unwrap();
    writeln!(out, "        &ROWS[self as usize]").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}\n").unwrap();
    super::rows(out, "CharacterData", &table);

    // The one fact a caller that does not know what a character is still has
    // to know: whether a run can be started as one.
    writeln!(out, "impl sts2_core::CharacterTable for Character {{").unwrap();
    writeln!(out, "    fn playable(self) -> bool {{").unwrap();
    writeln!(out, "        self.data().playable").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    Ok(())
}

fn literal(
    row: &[String],
    deck: &BTreeMap<&str, Vec<(&str, &str)>>,
    relics: &BTreeMap<&str, Vec<&str>>,
) -> Result<String, String> {
    let id = &row[1];
    let number = |what: &str, v: &str| {
        v.parse::<u16>()
            .map_err(|e| format!("{id}: {what} {v}: {e}"))
    };

    let mut out = String::new();
    writeln!(out, "CharacterData {{").unwrap();
    writeln!(out, "                playable: {},", row[2] == "1").unwrap();
    writeln!(out, "                gender: CharacterGender::{},", row[3]).unwrap();
    writeln!(
        out,
        "                starting_hp: {},",
        number("starting hp", &row[4])?
    )
    .unwrap();
    writeln!(
        out,
        "                starting_gold: {},",
        number("starting gold", &row[5])?
    )
    .unwrap();
    writeln!(
        out,
        "                max_energy: {},",
        number("max energy", &row[6])?
    )
    .unwrap();
    writeln!(
        out,
        "                orb_slots: {},",
        number("orb slots", &row[7])?
    )
    .unwrap();

    for (field, ty, column, suffix, _) in POOLS {
        let pool = &row[column];
        let variant = camel(pool.strip_suffix(suffix).unwrap_or(pool));
        writeln!(out, "                {field}: {ty}::{variant},").unwrap();
    }

    let cards = match deck.get(id.as_str()) {
        None => String::new(),
        Some(cs) => cs
            .iter()
            .map(|(card, pool)| card_path(pool, card))
            .collect::<Result<Vec<_>, _>>()?
            .join(", "),
    };
    writeln!(out, "                deck: Deck::new(&[{cards}]),").unwrap();

    let held = relics
        .get(id.as_str())
        .map(|rs| {
            rs.iter()
                .map(|r| format!("Relic::{}", camel(r)))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    writeln!(
        out,
        "                starting_relics: StartingRelics::new(&[{held}]),"
    )
    .unwrap();

    let unlocks = match row[12].as_str() {
        "-" => "None".to_string(),
        after => format!("Some(Character::{})", camel(after)),
    };
    writeln!(out, "                unlocks_after: {unlocks},").unwrap();

    write!(out, "            }}").unwrap();
    Ok(out)
}
