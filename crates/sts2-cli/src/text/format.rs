//! The game's text templates, filled in.
//!
//! The game writes a card as `Deal {Damage:diff()} damage.` and fills it with
//! SmartFormat. This is the part of SmartFormat the text tables use, with the
//! game's own formatters from `MegaCrit.Sts2.Core.Localization.Formatters`:
//!
//! - `{Name}` is the value.
//! - `{Name:diff()}` is the value, green when it is above the number printed
//!   on the card and red when below. `inverseDiff()` swaps the two.
//! - `{Name:plural:one|many}` picks by whether the value is one, and `{}`
//!   inside it is the value.
//! - `{Name:cond:>1?yes|no}` and `{Name:yes|no}` pick by a comparison, or by
//!   whether the value is true.
//! - `{Name:choose(A|B):a|b|other}` picks by which word the value is.
//! - `{Name:show:upgraded|not}` picks by whether the card is upgraded.
//! - `{Name:energyIcons()}` and `{Name:starIcons()}` are counted in words,
//!   since a terminal has no icon for either.
//! - `{Name:abs()}`, `percentMore()` and `percentLess()` are arithmetic.
//!
//! A value the caller did not supply reads `?`. The failure is a gap, never
//! an invented number.

use std::collections::HashMap;

/// One value a template can name.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A number, with the number printed on the card for `diff()` to compare
    /// against. The two are equal where nothing has changed it.
    Number {
        now: f64,
        base: f64,
    },
    Flag(bool),
    Word(String),
}

impl Value {
    /// A number nothing has changed.
    pub fn number(n: f64) -> Value {
        Value::Number { now: n, base: n }
    }

    fn now(&self) -> Option<f64> {
        match self {
            Value::Number { now, .. } => Some(*now),
            Value::Flag(flag) => Some(f64::from(u8::from(*flag))),
            Value::Word(_) => None,
        }
    }

    fn truth(&self) -> bool {
        match self {
            Value::Number { now, .. } => *now != 0.0,
            Value::Flag(flag) => *flag,
            Value::Word(word) => !word.is_empty(),
        }
    }

    fn plain(&self) -> String {
        match self {
            Value::Number { now, .. } => number(*now),
            Value::Flag(flag) => flag.to_string(),
            Value::Word(word) => word.clone(),
        }
    }
}

/// What a template names, by name.
pub type Values = HashMap<String, Value>;

/// A number as the game prints one: whole where it is whole.
fn number(n: f64) -> String {
    match n.fract() == 0.0 {
        true => format!("{}", n as i64),
        false => format!("{n}"),
    }
}

/// A template, filled in from `values`.
pub fn fill(template: &str, values: &Values) -> String {
    let mut out = String::new();
    let chars: Vec<char> = template.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        match chars[at] {
            '{' => {
                let end = closing(&chars, at);
                let inside: String = chars[at + 1..end].iter().collect();
                out.push_str(&placeholder(&inside, values, None));
                at = end + 1;
            }
            c => {
                out.push(c);
                at += 1;
            }
        }
    }
    out
}

/// Fills a nested option, where a bare `{}` is the value being formatted.
fn fill_with(template: &str, values: &Values, current: &Value) -> String {
    let mut out = String::new();
    let chars: Vec<char> = template.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        match chars[at] {
            '{' => {
                let end = closing(&chars, at);
                let inside: String = chars[at + 1..end].iter().collect();
                out.push_str(&placeholder(&inside, values, Some(current)));
                at = end + 1;
            }
            c => {
                out.push(c);
                at += 1;
            }
        }
    }
    out
}

/// The `}` that closes the `{` at `open`, or the end for one that never
/// closes.
fn closing(chars: &[char], open: usize) -> usize {
    let mut depth = 0;
    for (at, &c) in chars.iter().enumerate().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return at;
                }
            }
            _ => {}
        }
    }
    chars.len()
}

/// `text` split at each `|` outside braces.
fn options(text: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            '|' if depth == 0 => {
                out.push(String::new());
                continue;
            }
            _ => {}
        }
        out.last_mut().expect("one option at least").push(c);
    }
    out
}

/// What one `{...}` reads as.
fn placeholder(inside: &str, values: &Values, current: Option<&Value>) -> String {
    let (name, rest) = match inside.split_once(':') {
        Some((name, rest)) => (name, Some(rest)),
        None => (inside, None),
    };
    let value = match name {
        "" => current.cloned(),
        name => values.get(name).cloned(),
    };
    let Some(rest) = rest else {
        return value.map_or_else(|| "?".to_owned(), |value| value.plain());
    };

    // The formatter's name, its arguments in brackets, then its options.
    let (formatter, arguments, format) = split_formatter(rest);
    match formatter {
        "diff" | "inverseDiff" => match value {
            Some(Value::Number { now, base }) => {
                let up = match formatter == "diff" {
                    true => now.round() > base.round(),
                    false => now.round() < base.round(),
                };
                let down = now.round() != base.round() && !up;
                let shown = number(now.trunc());
                match (up, down) {
                    (true, _) => format!("[green]{shown}[/green]"),
                    (_, true) => format!("[red]{shown}[/red]"),
                    _ => shown,
                }
            }
            Some(other) => other.plain(),
            None => "?".to_owned(),
        },
        "plural" => {
            let Some(value) = value else {
                return "?".to_owned();
            };
            let picks = options(format);
            let one = value.now().is_some_and(|n| n == 1.0);
            let pick = match (one, picks.len()) {
                (true, _) | (_, 1) => &picks[0],
                (false, _) => &picks[picks.len() - 1],
            };
            fill_with(pick, values, &value)
        }
        "cond" => {
            let Some(value) = value else {
                return "?".to_owned();
            };
            let picks = options(format);
            // `>1?yes|no`: a comparison on the first option.
            let (holds, first) = match picks[0].split_once('?') {
                Some((test, yes)) => (compare(test, &value), yes.to_owned()),
                None => (value.truth(), picks[0].clone()),
            };
            let pick = match holds {
                true => first,
                false => picks.get(1).cloned().unwrap_or_default(),
            };
            fill_with(&pick, values, &value)
        }
        "choose" => {
            let Some(value) = value else {
                return "?".to_owned();
            };
            let words: Vec<&str> = arguments.split('|').collect();
            let picks = options(format);
            let at = words.iter().position(|word| *word == value.plain());
            let pick = match at {
                Some(at) => picks.get(at),
                None => picks.get(words.len()),
            };
            pick.map(|pick| fill_with(pick, values, &value))
                .unwrap_or_default()
        }
        "show" => {
            let picks = options(format);
            let upgraded = value.is_some_and(|value| value.truth());
            let pick = match upgraded {
                true => picks.first(),
                false => picks.get(1),
            };
            pick.map(|pick| fill(pick, values)).unwrap_or_default()
        }
        "energyIcons" => counted(value, arguments, "energy", "energy"),
        "starIcons" => counted(value, arguments, "star", "stars"),
        "abs" => value
            .and_then(|value| value.now())
            .map_or_else(|| "?".to_owned(), |n| number(n.abs())),
        "percentMore" => value
            .and_then(|value| value.now())
            .map_or_else(|| "?".to_owned(), |n| number(((n - 1.0) * 100.0).round())),
        "percentLess" => value
            .and_then(|value| value.now())
            .map_or_else(|| "?".to_owned(), |n| number(((1.0 - n) * 100.0).round())),
        // A bare `{Name:yes|no}` is SmartFormat's conditional on a value.
        _ => {
            let Some(value) = value else {
                return "?".to_owned();
            };
            let picks = options(rest);
            let pick = match value.truth() {
                true => picks[0].clone(),
                false => picks.get(1).cloned().unwrap_or_default(),
            };
            fill_with(&pick, values, &value)
        }
    }
}

/// `diff()`, `choose(A|B):a|b`, `plural:a|b`: the name, what is in the
/// brackets, and what follows the next `:`.
fn split_formatter(rest: &str) -> (&str, &str, &str) {
    let name_end = rest.find(['(', ':']).unwrap_or(rest.len());
    let name = &rest[..name_end];
    let after = &rest[name_end..];
    let (arguments, after) = match after.strip_prefix('(') {
        Some(inner) => match inner.find(')') {
            Some(close) => (&inner[..close], &inner[close + 1..]),
            None => (inner, ""),
        },
        None => ("", after),
    };
    (name, arguments, after.strip_prefix(':').unwrap_or(after))
}

/// A comparison such as `>1` or `<0`, against a value.
fn compare(test: &str, value: &Value) -> bool {
    let Some(n) = value.now() else {
        return false;
    };
    let (op, rhs) = test.split_at(
        test.find(|c: char| c.is_ascii_digit() || c == '-')
            .unwrap_or(0),
    );
    let Ok(rhs) = rhs.parse::<f64>() else {
        return value.truth();
    };
    match op {
        ">" => n > rhs,
        ">=" => n >= rhs,
        "<" => n < rhs,
        "<=" => n <= rhs,
        "=" | "==" => n == rhs,
        "!=" => n != rhs,
        _ => value.truth(),
    }
}

/// An icon count in words. The argument, where there is one, is the count:
/// `energyIcons(1)` is one icon whatever the value.
fn counted(value: Option<Value>, argument: &str, one: &str, many: &str) -> String {
    let count = match argument.parse::<f64>() {
        Ok(count) => Some(count),
        Err(_) => value.and_then(|value| value.now()),
    };
    match count {
        Some(1.0) => format!("1 {one}"),
        Some(n) => format!("{} {many}", number(n)),
        None => format!("? {many}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, Value)]) -> Values {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect()
    }

    #[test]
    fn a_number_fills_in_and_a_changed_one_is_coloured() {
        let vars = values(&[(
            "Damage",
            Value::Number {
                now: 9.0,
                base: 6.0,
            },
        )]);
        assert_eq!(
            fill("Deal {Damage:diff()} damage.", &vars),
            "Deal [green]9[/green] damage."
        );
        let vars = values(&[("Damage", Value::number(6.0))]);
        assert_eq!(
            fill("Deal {Damage:diff()} damage.", &vars),
            "Deal 6 damage."
        );
    }

    #[test]
    fn plural_picks_by_one() {
        let one = values(&[("Cards", Value::number(1.0))]);
        let two = values(&[("Cards", Value::number(2.0))]);
        let template = "Draw {Cards:diff()} {Cards:plural:card|cards}.";
        assert_eq!(fill(template, &one), "Draw 1 card.");
        assert_eq!(fill(template, &two), "Draw 2 cards.");
        assert_eq!(
            fill(
                "{Repeat:plural:| {} times}.",
                &two.into_values()
                    .map(|v| ("Repeat".to_owned(), v))
                    .collect()
            ),
            " 2 times."
        );
    }

    #[test]
    fn conditions_and_choices_pick_their_branch() {
        let negative = values(&[("Amount", Value::number(-2.0))]);
        assert_eq!(
            fill(
                "{Amount:cond:<0?Decreases|Increases} damage by {Amount:abs()}.",
                &negative
            ),
            "Decreases damage by 2."
        );
        let player = values(&[("OnPlayer", Value::Flag(true))]);
        assert_eq!(fill("{OnPlayer:your turn|its turn}", &player), "your turn");
        let attack = values(&[("CardType", Value::Word("Skill".into()))]);
        assert_eq!(
            fill("{CardType:choose(Attack|Skill):a|s|other}", &attack),
            "s"
        );
    }

    #[test]
    fn upgrades_icons_and_percentages() {
        let upgraded = values(&[
            ("IfUpgraded", Value::Flag(true)),
            ("Energy", Value::number(2.0)),
        ]);
        assert_eq!(
            fill("{IfUpgraded:show:ALL cards|a card}", &upgraded),
            "ALL cards"
        );
        assert_eq!(
            fill("Gain {Energy:energyIcons()}.", &upgraded),
            "Gain 2 energy."
        );
        let vulnerable = values(&[("DamageIncrease", Value::number(1.5))]);
        assert_eq!(fill("{DamageIncrease:percentMore()}%", &vulnerable), "50%");
    }

    /// A value nobody supplied is a gap, never a number.
    #[test]
    fn a_missing_value_reads_as_a_question_mark() {
        assert_eq!(
            fill("Deal {Damage:diff()} damage.", &Values::new()),
            "Deal ? damage."
        );
    }
}
