//! What things say: the game's own English text, for a run that has none.
//!
//! The game reports a card's, relic's or potion's title and words as it plays.
//! The simulator has no text, so a view off it carries none, and a card was a
//! box with a name. [`fill`] writes in what the view left out, from the text
//! tables `cargo xtask dump textdump` copies out of each build's pack. A view
//! the game already filled keeps the game's words.
//!
//! A card in a fight takes its numbers from the preview the fight computed.
//! Anything else takes them from the build's dumped numbers. An upgrade's numbers are not dumped. So a card that
//! has been upgraded, read outside a fight, says so rather than showing the
//! numbers it had before.

pub mod format;

use std::collections::HashMap;
use std::sync::OnceLock;

use num_traits::ToPrimitive;
use sts2_core::{CardIdent, CardTable as _, CardVars as _, ContentVersion, View};

use crate::render::spelling;
use format::{Value, Values};

/// The tables a front end reads, by the name the pack files them under.
const TABLES: [&str; 6] = [
    "cards",
    "relics",
    "powers",
    "potions",
    "enchantments",
    "afflictions",
];

/// One build's text and numbers.
struct Build {
    /// Each table's keys, such as `BASH.description`, and their templates.
    text: HashMap<&'static str, HashMap<String, String>>,
    /// Each model's dumped numbers, by table and then id.
    numbers: HashMap<&'static str, HashMap<String, Values>>,
}

macro_rules! build {
    ($dir:literal) => {{
        let text = [
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/text/cards.json"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/text/relics.json"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/text/powers.json"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/text/potions.json"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/text/enchantments.json"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/text/afflictions.json"
            )),
        ];
        let numbers = [
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/cards.txt"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/relics.txt"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/powers.txt"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/potions.txt"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/enchantments.txt"
            )),
            include_str!(concat!(
                "../../../sts2-content/src/",
                $dir,
                "/dumps/afflictions.txt"
            )),
        ];
        Build::read(text, numbers)
    }};
}

impl Build {
    fn read(text: [&str; 6], numbers: [&str; 6]) -> Build {
        let text = (TABLES.iter().zip(text))
            .map(|(table, json)| {
                let read: HashMap<String, String> =
                    serde_json::from_str(json).expect("a dumped text table is JSON");
                (*table, read)
            })
            .collect();
        let numbers = (TABLES.iter().zip(numbers))
            .map(|(table, dump)| (*table, vars(dump)))
            .collect();
        Build { text, numbers }
    }

    fn of<V: ContentVersion>() -> Option<&'static Build> {
        static V0_107_1: OnceLock<Build> = OnceLock::new();
        static V0_111_0: OnceLock<Build> = OnceLock::new();
        match V::VERSION.name {
            "v0.107.1" => Some(V0_107_1.get_or_init(|| build!("v0_107_1"))),
            "v0.111.0" => Some(V0_111_0.get_or_init(|| build!("v0_111_0"))),
            _ => None,
        }
    }

    fn said(&self, table: &str, key: &str) -> Option<&str> {
        self.text.get(table)?.get(key).map(String::as_str)
    }

    fn numbers(&self, table: &str, id: &str) -> Values {
        (self.numbers.get(table))
            .and_then(|by_id| by_id.get(id))
            .cloned()
            .unwrap_or_default()
    }
}

/// The `var <model> <name> <base> ...` lines of a dump, by model.
fn vars(dump: &str) -> HashMap<String, Values> {
    let mut out: HashMap<String, Values> = HashMap::new();
    for line in dump.lines() {
        let mut words = line.split(' ');
        if words.next() != Some("var") {
            continue;
        }
        let (Some(model), Some(name), Some(base)) = (words.next(), words.next(), words.next())
        else {
            continue;
        };
        let Ok(base) = base.parse::<f64>() else {
            continue;
        };
        out.entry(model.to_owned())
            .or_default()
            .insert(name.to_owned(), Value::number(base));
    }
    out
}

/// A card's title, such as *Bash* or *Bash+*, or nothing for a build with no
/// table.
pub fn card_title<V: ContentVersion>(card: &CardIdent<V>) -> Option<String> {
    let build = Build::of::<V>()?;
    let id = spelling(&card.card);
    let title = build.said("cards", &format!("{id}.title"))?;
    Some(match card.upgrade {
        0 => title.to_owned(),
        1 => format!("{title}+"),
        n => format!("{title}+{n}"),
    })
}

/// A card's numbers by name, as a fight's preview keys them.
pub type Numbers<V> =
    sts2_core::Keyed<<V as ContentVersion>::CardVar, <V as ContentVersion>::Number>;

/// What a card says, with the numbers given, or its printed ones.
///
/// `numbers` is a fight's preview: each number now, and as printed. Without
/// one, an upgraded card's numbers are not known, and the words say so.
pub fn card<V: ContentVersion>(
    card: &CardIdent<V>,
    numbers: Option<(&Numbers<V>, &Numbers<V>)>,
) -> Option<String>
where
    V::Number: ToPrimitive,
{
    let build = Build::of::<V>()?;
    let id = spelling(&card.card);
    let template = build.said("cards", &format!("{id}.description"))?;

    let mut values = Values::new();
    for number in card.card.vars() {
        let printed = number.base.to_f64().unwrap_or(0.0);
        values.insert(number.id.name().to_owned(), Value::number(printed));
    }
    if let Some((now, printed)) = numbers {
        for (key, value) in now.iter() {
            let base = printed.get(key).copied().unwrap_or(*value);
            values.insert(
                key.name().to_owned(),
                Value::Number {
                    now: value.to_f64().unwrap_or(0.0),
                    base: base.to_f64().unwrap_or(0.0),
                },
            );
        }
    }
    values.insert("IfUpgraded".into(), Value::Flag(card.upgrade > 0));
    values.insert("energyPrefix".into(), Value::Word(String::new()));
    values.insert("singleStarIcon".into(), Value::Word("1 star".into()));
    values.insert("InCombat".into(), Value::Flag(numbers.is_some()));

    let mut said = format::fill(template, &values);
    if card.upgrade > 0 && numbers.is_none() {
        said.push_str("\n[gold](numbers before the upgrade)[/gold]");
    }
    Some(said)
}

/// A relic's title and words, with the number it shows.
pub fn relic<V: ContentVersion>(relic: &V::Relic, amount: f64) -> (Option<String>, Option<String>) {
    titled::<V>("relics", &spelling(relic), |values| {
        values.insert("Amount".into(), Value::number(amount));
    })
}

/// A potion's title and words.
pub fn potion<V: ContentVersion>(potion: &V::Potion) -> (Option<String>, Option<String>) {
    titled::<V>("potions", &spelling(potion), |_| {})
}

/// A power's title and words, at an amount, on the player or on an enemy.
pub fn power<V: ContentVersion>(
    power: &V::Power,
    amount: f64,
    on_player: bool,
) -> (Option<String>, Option<String>) {
    let id = spelling(power);
    let Some(build) = Build::of::<V>() else {
        return (None, None);
    };
    let title = build
        .said("powers", &format!("{id}.title"))
        .map(str::to_owned);
    let template = build
        .said("powers", &format!("{id}.smartDescription"))
        .or_else(|| build.said("powers", &format!("{id}.description")));
    let mut values = build.numbers("powers", &id);
    values.insert("Amount".into(), Value::number(amount));
    values.insert("OnPlayer".into(), Value::Flag(on_player));
    values.insert("OwnerName".into(), Value::Word("it".into()));
    (
        title,
        template.map(|template| format::fill(template, &values)),
    )
}

/// A model's title, and its description filled from its dumped numbers.
fn titled<V: ContentVersion>(
    table: &str,
    id: &str,
    more: impl FnOnce(&mut Values),
) -> (Option<String>, Option<String>) {
    let Some(build) = Build::of::<V>() else {
        return (None, None);
    };
    let title = build.said(table, &format!("{id}.title")).map(str::to_owned);
    let mut values = build.numbers(table, id);
    more(&mut values);
    let words = (build.said(table, &format!("{id}.description")))
        .map(|template| format::fill(template, &values));
    (title, words)
}

/// Writes in every title and description a view left out.
pub fn fill<V: ContentVersion>(view: &mut View<V>)
where
    V::Number: ToPrimitive,
{
    for carried in &mut view.relics {
        if carried.title.is_none() || carried.text.is_none() {
            let (title, text) = relic::<V>(&carried.relic, carried.amount.to_f64().unwrap_or(0.0));
            carried.title = carried.title.take().or(title);
            carried.text = carried.text.take().or(text);
        }
    }
    for held in &mut view.potions {
        if held.title.is_none() || held.text.is_none() {
            let (title, text) = potion::<V>(&held.potion);
            held.title = held.title.take().or(title);
            held.text = held.text.take().or(text);
        }
    }
    if let Some(fight) = &mut view.combat {
        for playable in &mut fight.legal {
            for preview in &mut playable.effect {
                if preview.text.is_none() {
                    preview.text = card(&playable.card, Some((&preview.vars, &preview.base)));
                }
            }
        }
    }
    if let Some(sts2_core::Room::Shop { entries }) = &mut view.room {
        for entry in entries {
            let (title, text) = match &entry.stock {
                sts2_core::Stock::Card(Some(shop)) => {
                    (card_title(&shop.card), card(&shop.card, None))
                }
                sts2_core::Stock::Relic(Some(id)) => relic::<V>(id, 0.0),
                sts2_core::Stock::Potion(Some(id)) => potion::<V>(id),
                _ => (None, None),
            };
            entry.title = entry.title.take().or(title);
            entry.text = entry.text.take().or(text);
        }
    }
    match &mut view.asking {
        Some(sts2_core::Question::SelectCards { options, .. }) => {
            for option in options {
                option.text = option.text.take().or_else(|| card(&option.card, None));
            }
        }
        Some(sts2_core::Question::Rewards { rewards, .. }) => {
            for detail in rewards {
                if let sts2_core::RewardKind::Card { cards, .. } = &mut detail.reward {
                    for offered in cards {
                        offered.text = offered.text.take().or_else(|| card(&offered.card, None));
                    }
                }
                if let sts2_core::RewardKind::SpecialCard {
                    card: Some(offered),
                } = &mut detail.reward
                {
                    offered.text = offered.text.take().or_else(|| card(&offered.card, None));
                }
            }
        }
        Some(sts2_core::Question::ChooseBundle { bundles }) => {
            for bundle in bundles {
                for offered in &mut bundle.cards {
                    offered.text = offered.text.take().or_else(|| card(&offered.card, None));
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1 as V;

    fn ident(id: &str, upgrade: u8) -> CardIdent<V> {
        CardIdent {
            card: crate::id::parse(id).expect("a card id"),
            upgrade,
            affliction: None,
            enchantment: None,
        }
    }

    #[test]
    fn a_card_says_what_it_does_with_its_printed_numbers() {
        assert_eq!(card_title(&ident("BASH", 0)).as_deref(), Some("Bash"));
        assert_eq!(
            card(&ident("BASH", 0), None).as_deref(),
            Some("Deal 8 damage.\nApply 2 [gold]Vulnerable[/gold].")
        );
    }

    /// In a fight the numbers are the fight's, and one the fight raised is
    /// green, as the game draws it.
    #[test]
    fn a_number_a_fight_raised_is_green() {
        let damage = ident("STRIKE_IRONCLAD", 0).card.vars()[0].id;
        let now = sts2_core::Keyed::from([(damage, 9)]);
        let printed = sts2_core::Keyed::from([(damage, 6)]);
        assert_eq!(
            card(&ident("STRIKE_IRONCLAD", 0), Some((&now, &printed))).as_deref(),
            Some("Deal [green]9[/green] damage.")
        );
    }

    /// Outside a fight, an upgraded card's numbers are not known, and the
    /// words say so rather than print the old ones as if they were.
    #[test]
    fn an_upgraded_card_out_of_a_fight_says_its_numbers_are_old() {
        assert_eq!(card_title(&ident("BASH", 1)).as_deref(), Some("Bash+"));
        let said = card(&ident("BASH", 1), None).expect("Bash has words");
        assert!(
            said.ends_with("(numbers before the upgrade)[/gold]"),
            "{said}"
        );
    }

    #[test]
    fn a_relic_fills_its_dumped_number() {
        let relic = crate::id::parse("BURNING_BLOOD").expect("a relic id");
        let (title, words) = super::relic::<V>(&relic, 0.0);
        assert_eq!(title.as_deref(), Some("Burning Blood"));
        assert_eq!(
            words.as_deref(),
            Some("At the end of combat, heal [green]6[/green] HP.")
        );
    }
}
