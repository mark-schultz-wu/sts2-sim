//! The game's own words, made fit for a terminal.
//!
//! The game returns rich-text strings containing BBCode for colours and images.
//! The oracle preserves those strings. This module adapts them for a terminal.
//!
//! Markup carries no semantic facts here. In particular, red can mark either a
//! recomputed card value or ordinary event prose. Computed values come from the
//! oracle.s separate data instead.

/// A colour the game asked a run of text to be drawn in.
///
/// Three, because three is what the game writes. Anything else it wraps text in
/// leaves the text alone, keeping whatever colour was already in force. That
/// covers emphasis, and a tag this build does not have yet. It is what
/// [`Span::tint`] being `None` means, rather than a fourth variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    /// A number the game recomputed upward, and `[gold]` on a keyword.
    Gold,
    Green,
    Red,
}

/// One run of the game's words, and the colour it asked for them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tint: Option<Tint>,
}

/// The text with its markup resolved, in runs a front end can colour.
///
/// [`plain`] is this with the colours dropped, and the two are the same
/// characters in the same order. The parser is one, so a change to how an icon
/// reads cannot land in one front end and not the other.
pub fn spans(text: &str) -> Vec<Span> {
    let (out, marks) = parse(text);

    let mut spans: Vec<Span> = Vec::new();
    for (i, &(start, tint)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map_or(out.len(), |&(next, _)| next);
        if start >= end {
            continue;
        }
        // Runs that agree about colour are one run: an image inside `[green]`
        // opens and closes the stack without changing what the reader sees.
        match spans.last_mut() {
            Some(last) if last.tint == tint => last.text.push_str(&out[start..end]),
            _ => spans.push(Span {
                text: out[start..end].to_string(),
                tint,
            }),
        }
    }
    spans
}

/// The text with its markup resolved for a monospace terminal.
///
/// Tags are dropped and their contents kept. `[img]` is the exception twice
/// over. Its contents are a resource path rather than words, so they go with
/// the tag. But some of those images *are* words, and dropping them loses part
/// of the sentence. See [`icon`].
pub fn plain(text: &str) -> String {
    parse(text).0
}

/// The resolved text, and where along it the colour changes.
///
/// One pass for both front ends. The marks are offsets into the string rather
/// than owned pieces of it, because [`write_amount`] reads what has already
/// been written. "3 energy" is decided by the digits in front of the icon, so
/// the text has to be built whole and cut afterwards.
fn parse(text: &str) -> (String, Vec<(usize, Option<Tint>)>) {
    let mut out = String::with_capacity(text.len());
    let mut marks: Vec<(usize, Option<Tint>)> = vec![(0, None)];
    let mut open: Vec<Option<Tint>> = Vec::new();
    let mut rest = text;

    // The colour in force is the innermost tag that named one, so a tag that
    // names none — `[b]`, or one this build has not seen — keeps its parent's.
    let mut note = |out: &String, open: &[Option<Tint>]| {
        let tint = open.iter().rev().find_map(|t| *t);
        match marks.last_mut() {
            Some(last) if last.0 == out.len() => last.1 = tint,
            Some(last) if last.1 == tint => {}
            _ => marks.push((out.len(), tint)),
        }
    };

    while let Some(at) = rest.find('[') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];

        if let Some((pictured, after)) = peel_image(rest) {
            rest = after;
            let Some(unit) = pictured else {
                // An image with no word for it. Dropped, as the tag itself is.
                continue;
            };

            // The game spells a small amount as that many icons in a row, so
            // the run is the number.
            let mut count = 1;
            while let Some((Some(next), after)) = peel_image(rest) {
                if next != unit {
                    break;
                }
                count += 1;
                rest = after;
            }

            write_amount(&mut out, unit, count);
            continue;
        }

        // Colour or emphasis. The tag goes, what it wraps stays.
        let Some(close) = rest.find(']') else {
            // An unmatched bracket is text, not markup.
            out.push_str(rest);
            return (out, marks);
        };
        let tag = &rest[1..close];
        rest = &rest[close + 1..];

        match tag.strip_prefix('/') {
            // A stray close tag is markup the game did not open. Dropping it is
            // what the old parser did and is still right; there is nothing to
            // pop.
            Some(_) => {
                open.pop();
            }
            None => open.push(tint_of(tag)),
        }
        note(&out, &open);
    }
    out.push_str(rest);
    (out, marks)
}

/// The colour a tag names, for the tags that name one.
fn tint_of(tag: &str) -> Option<Tint> {
    // An attribute is alignment or a shade the terminal has no answer for, so
    // the name in front of it is what decides.
    match tag.split('=').next().unwrap_or(tag) {
        "gold" => Some(Tint::Gold),
        "green" => Some(Tint::Green),
        "red" => Some(Tint::Red),
        _ => None,
    }
}

/// Peels one `[img]…[/img]` off the front, and says what it pictures.
///
/// `None` when the text does not start with an image at all. `Some((None, …))`
/// for an image [`icon`] has no word for.
fn peel_image(text: &str) -> Option<(Option<&'static str>, &str)> {
    let after = text.strip_prefix("[img")?;
    let open = after.find(']')?;

    // `[img]` and `[img=top]` are both images — the attribute is alignment, and
    // the game writes both. Anything else beginning "img" is some other tag.
    let attribute = &after[..open];
    if !attribute.is_empty() && !attribute.starts_with('=') {
        return None;
    }

    let inner = &after[open + 1..];
    let end = inner.find("[/img]")?;
    Some((icon(&inner[..end]), &inner[end + "[/img]".len()..]))
}

/// The word an icon stands for, where it stands for one.
///
/// Some images represent words. Dropping Bloodletting.s energy icons, for
/// example, leaves its sentence without an object.
///
/// Matching the game.s file-name convention covers every character.s energy
/// icon without enumerating characters.
fn icon(path: &str) -> Option<&'static str> {
    let file = path.rsplit('/').next()?;
    match file {
        "star_icon.png" => Some("star"),
        _ if file.ends_with("_energy_icon.png") => Some("energy"),
        _ => None,
    }
}

/// Writes "3 energy" for a run of three energy icons.
///
/// The game has two spellings and this has to read both. One to three is that
/// many icons in a row. Zero, or four and up, is the number followed by a
/// single icon. So a number already sitting there is the amount, and printing
/// `count` beside it gives "52 energy" for five.
fn write_amount(out: &mut String, unit: &str, count: usize) {
    let written = trailing_number(out);
    let amount = written.unwrap_or(count);

    if written.is_none() {
        out.push_str(&amount.to_string());
    }
    if !out.ends_with(char::is_whitespace) {
        out.push(' ');
    }

    // "energy" is a mass noun and does not inflect; a star is a thing and does.
    match (unit, amount) {
        ("star", 1) => out.push_str("star"),
        ("star", _) => out.push_str("stars"),
        (unit, _) => out.push_str(unit),
    }
}

/// The number the text already ends in, if it ends in one.
fn trailing_number(text: &str) -> Option<usize> {
    let digits: Vec<char> = text
        .trim_end()
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();

    digits.iter().rev().collect::<String>().parse().ok()
}

/// One line, for text the game wrote as several.
///
/// A card's face is line-broken for a card-sized rectangle, as "Deal 6 damage."
/// then "Exhaust.". A terminal listing ten of them wants each on one row.
pub fn one_line(text: &str) -> String {
    plain(text)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The card an action is asking about, out of the action's own `ToString`.
///
/// A selector is never told what asked it, so the mod reports the running
/// action, which stringifies as
/// `PlayCardAction card: CARD.SCULPTING_STRIKE (28884561) index: 7 targetid: 3`.
/// Everything after the id is bookkeeping. The two parts a reader wants are
/// what kind of action it is and which card it was.
pub fn asker(raw: &str) -> String {
    let action = raw.split([' ', ':']).next().unwrap_or(raw);

    match raw
        .split_once("CARD.")
        .map(|(_, tail)| tail.split([' ', ':', ')', '(']).next().unwrap_or(tail))
    {
        Some(card) if !card.is_empty() => format!("{action} ({card})"),
        _ => action.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_tags_leave_their_contents_behind() {
        assert_eq!(plain("Gain 5 [gold]Block[/gold]."), "Gain 5 Block.");
    }

    /// Colour is not information here. `[red]12[/red]` is Neow saying that
    /// losing Max HP is bad, not the game saying 12 is not 12. So nothing is
    /// inferred from it, and whether a number moved is answered elsewhere.
    #[test]
    fn a_highlighted_number_is_just_the_number() {
        assert_eq!(plain("Deal [green]7[/green] damage."), "Deal 7 damage.");
        assert_eq!(plain("Lose [red]12[/red] Max HP."), "Lose 12 Max HP.");
    }

    /// Verbatim from the game, which is where this bug came from: two icons in
    /// a row, and the rendering said "Gain ."
    #[test]
    fn energy_icons_are_a_word_and_a_number() {
        let icon = "[img]res://images/packed/sprite_fonts/ironclad_energy_icon.png[/img]";

        assert_eq!(
            one_line(&format!("Lose 3 HP.\nGain {icon}{icon}.")),
            "Lose 3 HP. Gain 2 energy."
        );
        assert_eq!(plain(&format!("Gain {icon}.")), "Gain 1 energy.");
    }

    /// Four or more is written as the number and one icon, so the number is
    /// already there and must not be written twice.
    #[test]
    fn a_numbered_energy_icon_keeps_its_own_number() {
        let icon = "[img]res://images/packed/sprite_fonts/silent_energy_icon.png[/img]";

        assert_eq!(plain(&format!("Gain 5{icon}.")), "Gain 5 energy.");
        assert_eq!(
            plain(&format!("Gain [green]7[/green]{icon}.")),
            "Gain 7 energy."
        );
    }

    /// A star is a thing rather than a quantity, so it inflects.
    #[test]
    fn stars_are_counted_like_things() {
        let star = "[img]res://images/packed/sprite_fonts/star_icon.png[/img]";

        assert_eq!(plain(&format!("Costs {star}.")), "Costs 1 star.");
        assert_eq!(plain(&format!("Costs {star}{star}.")), "Costs 2 stars.");
    }

    /// The game writes both `[img]` and `[img=top]`, and an icon is an icon.
    #[test]
    fn an_aligned_image_is_still_an_image() {
        assert_eq!(
            plain("Gain [img=top]res://images/packed/sprite_fonts/defect_energy_icon.png[/img]."),
            "Gain 1 energy."
        );
    }

    /// An image nobody has a word for is still not worth printing a path for.
    #[test]
    fn an_image_with_no_word_takes_its_path_with_it() {
        assert_eq!(
            plain("Costs [img]res://images/portrait.png[/img] to play."),
            "Costs  to play."
        );
    }

    #[test]
    fn a_stray_bracket_is_text() {
        assert_eq!(plain("[not a tag"), "[not a tag");
        assert_eq!(plain("100% [green]"), "100% ");
    }

    #[test]
    fn a_multi_line_face_becomes_one_row() {
        assert_eq!(
            one_line("Deal [green]9[/green] damage.\n\nExhaust."),
            "Deal 9 damage. Exhaust."
        );
    }

    /// The old rendering printed the whole thing, id and target and all.
    #[test]
    fn an_asker_is_its_action_and_its_card() {
        assert_eq!(
            asker("PlayCardAction card: CARD.SCULPTING_STRIKE (28884561) index: 7 targetid: 3"),
            "PlayCardAction (SCULPTING_STRIKE)"
        );
        assert_eq!(asker("DiscardAction"), "DiscardAction");
    }
}

#[cfg(test)]
mod span_tests {
    use super::{Tint, plain, spans};

    fn run(text: &str) -> Vec<(Option<Tint>, String)> {
        spans(text).into_iter().map(|s| (s.tint, s.text)).collect()
    }

    /// The two front ends have to agree about the words even when the colours
    /// differ. This is the property that keeps them one parser.
    fn same_words(text: &str) {
        let joined: String = spans(text).into_iter().map(|s| s.text).collect();
        assert_eq!(joined, plain(text), "spans and plain disagree on {text:?}");
    }

    #[test]
    fn a_colour_tag_becomes_a_tinted_run() {
        assert_eq!(
            run("Apply [green]4[/green] Poison."),
            [
                (None, "Apply ".to_string()),
                (Some(Tint::Green), "4".to_string()),
                (None, " Poison.".to_string()),
            ]
        );
        same_words("Apply [green]4[/green] Poison.");
    }

    #[test]
    fn the_innermost_colour_wins() {
        assert_eq!(
            run("[gold]Block[red]!"),
            [
                (Some(Tint::Gold), "Block".to_string()),
                (Some(Tint::Red), "!".to_string()),
            ]
        );
    }

    /// A tag that names no colour still nests, so its contents keep whatever
    /// was in force around it rather than falling back to none.
    #[test]
    fn a_tag_with_no_colour_keeps_its_parents() {
        assert_eq!(
            run("[red]lose [b]12[/b] max hp[/red]"),
            [(Some(Tint::Red), "lose 12 max hp".to_string())]
        );
    }

    #[test]
    fn a_closing_tag_restores_what_was_underneath() {
        assert_eq!(
            run("[gold]a[green]b[/green]c[/gold]d"),
            [
                (Some(Tint::Gold), "a".to_string()),
                (Some(Tint::Green), "b".to_string()),
                (Some(Tint::Gold), "c".to_string()),
                (None, "d".to_string()),
            ]
        );
    }

    /// An image opens and closes nothing the reader can see, so the run it
    /// sits in stays one run.
    #[test]
    fn an_icon_inside_a_colour_does_not_split_it() {
        let text = "[green]Gain [img]res://x/ironclad_energy_icon.png[/img][/green].";
        assert_eq!(
            run(text),
            [
                (Some(Tint::Green), "Gain 1 energy".to_string()),
                (None, ".".to_string()),
            ]
        );
        same_words(text);
    }

    #[test]
    fn text_with_no_markup_is_one_untinted_run() {
        assert_eq!(
            run("Deal 6 damage."),
            [(None, "Deal 6 damage.".to_string())]
        );
    }

    #[test]
    fn nothing_at_all_is_no_runs() {
        assert!(spans("").is_empty());
        same_words("");
    }

    #[test]
    fn a_stray_bracket_is_text_here_too() {
        same_words("Deal [6 damage.");
        same_words("[/green]orphaned");
    }
}
