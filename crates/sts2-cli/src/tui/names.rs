//! Ids, made fit to read.
//!
//! Generated content tables contain ids but no localized display names. This
//! module makes ids readable by splitting words and removing redundant category
//! suffixes. Commands, recordings, and detail views retain the original id.
//!
//! Relics and potions use localized titles from observations, falling back to
//! [`pretty`] when a key is absent. Other content can follow that pattern when
//! its localization table becomes available.

use sts2_core::Undrinkable;

/// A model id or content id, title-cased.
///
/// `STRIKE_IRONCLAD` reads *Strike Ironclad*. `MONSTER/BOWLBUG_ROCK` reads
/// *Bowlbug Rock*, because the category is already the column it sits in.
pub fn pretty(id: &str) -> String {
    let entry = id.rsplit('/').next().unwrap_or(id);
    words(entry)
}

/// A power's name, without the `_POWER` every one of them ends in.
pub fn power(id: &str) -> String {
    let entry = id.rsplit('/').next().unwrap_or(id);
    words(entry.strip_suffix("_POWER").unwrap_or(entry))
}

/// An id the wire spells in PascalCase: `EnergyCostTooHigh`, `RestSite`.
///
/// A separate function from [`pretty`] rather than a cleverer one, because the
/// two spellings cannot be told apart by looking. `RestSite` splits into two
/// words and `McGuffin` must not. Nothing in the string says which is which.
/// Only the field it came out of does, and the caller knows that.
pub fn label(id: &str) -> String {
    let entry = id.rsplit('/').next().unwrap_or(id);
    let mut out = String::with_capacity(entry.len() + 4);
    for (i, ch) in entry.char_indices() {
        if ch == '_' {
            out.push(' ');
            continue;
        }
        let after_lower = entry[..i]
            .chars()
            .next_back()
            .is_some_and(char::is_lowercase);
        if ch.is_uppercase() && after_lower {
            out.push(' ');
        }
        out.push(ch);
    }
    words(&out.replace("  ", " "))
}

/// Why a potion cannot be drunk, as a few words rather than a variant name.
///
/// Written out rather than run through [`label`]: `ItsOwnCheck` de-cases to
/// *Its Own Check*, which reads like a proper noun and says nothing. These are
/// the seven reasons and each gets a sentence fragment a player can act on.
pub fn undrinkable(why: Undrinkable) -> &'static str {
    match why {
        Undrinkable::Dead => "you are dead",
        Undrinkable::Queued => "already drinking",
        Undrinkable::Automatic => "drinks itself",
        Undrinkable::OutOfCombat => "in combat only",
        Undrinkable::NotYourTurn => "not your turn",
        Undrinkable::Stuck => "cannot be removed",
        Undrinkable::ItsOwnCheck => "not right now",
    }
}

/// `SPOILS_OF_BATTLE` to *Spoils of Battle*.
///
/// The small words stay small, which is what makes the result read as a name
/// rather than as a header. A word that is already mixed case came from
/// somewhere other than an id and is left exactly as it is.
fn words(id: &str) -> String {
    const SMALL: [&str; 7] = ["of", "the", "and", "a", "an", "to", "in"];

    let mut out = String::with_capacity(id.len());
    for (i, word) in id.split(['_', ' ']).filter(|w| !w.is_empty()).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        if word.chars().any(char::is_lowercase) {
            out.push_str(word);
            continue;
        }

        let lower = word.to_lowercase();
        if i > 0 && SMALL.contains(&lower.as_str()) {
            out.push_str(&lower);
            continue;
        }
        let mut chars = lower.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_becomes_a_name() {
        assert_eq!(pretty("STRIKE_IRONCLAD"), "Strike Ironclad");
        assert_eq!(pretty("BASH"), "Bash");
    }

    #[test]
    fn a_category_is_the_column_and_not_the_name() {
        assert_eq!(pretty("MONSTER/BOWLBUG_ROCK"), "Bowlbug Rock");
    }

    #[test]
    fn small_words_stay_small_but_never_lead() {
        assert_eq!(pretty("SPOILS_OF_BATTLE"), "Spoils of Battle");
        assert_eq!(pretty("OF_COURSE"), "Of Course");
    }

    #[test]
    fn a_suffix_the_column_already_says_goes() {
        assert_eq!(power("VULNERABLE_POWER"), "Vulnerable");
        // Only as a suffix: a power actually called that keeps its word.
        assert_eq!(power("POWER_THROUGH"), "Power Through");
    }

    /// Anything already spelled for a reader is left alone. A name arriving
    /// from somewhere other than an id is not mangled on the way past.
    #[test]
    fn mixed_case_is_left_alone() {
        assert_eq!(pretty("Crystal Sphere"), "Crystal Sphere");
        assert_eq!(pretty("McGuffin"), "McGuffin");
    }

    #[test]
    fn a_pascal_case_id_splits_where_the_case_changes() {
        assert_eq!(label("EnergyCostTooHigh"), "Energy Cost Too High");
        assert_eq!(label("RestSite"), "Rest Site");
        assert_eq!(label("NoPotionSlot"), "No Potion Slot");
    }

    /// The two spellings meet in the middle: an id that is already one word
    /// reads the same either way.
    #[test]
    fn one_word_is_one_word_in_either_spelling() {
        assert_eq!(label("Monster"), "Monster");
        assert_eq!(pretty("MONSTER"), "Monster");
    }

    #[test]
    fn nothing_is_nothing() {
        assert_eq!(pretty(""), "");
        assert_eq!(power("_POWER"), "");
    }
}
