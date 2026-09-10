//! The pieces every screen is built out of.
//!
//! Two rules, both taken from the reference this front end is modelled on. A
//! widget is *pure*: data in, exact rows out, no reaching for the game. And it
//! *degrades* rather than failing. Every one of these has a floor it falls to
//! when the terminal is too small. A player at 80x24 gets a worse screen, not an
//! empty one.

use num_traits::{PrimInt, ToPrimitive};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::render::prose::{self, Tint};
use crate::tui::theme;

/// A meter, in eighths of a cell.
///
/// Eighths because a hit that takes 3 of 40 hit points has to move the bar. A
/// bar drawn in whole blocks only moves every 1/`width` of the total, which at
/// ten columns is four points. Small hits then look free, which is the opposite
/// of what a health bar is for.
pub fn bar<N: PrimInt + ToPrimitive>(value: N, max: N, width: usize) -> String {
    const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

    if width == 0 {
        return String::new();
    }
    if max <= N::zero() {
        return "─".repeat(width);
    }

    let filled = (value.clamp(N::zero(), max).to_f64().unwrap_or(0.0)
        / max.to_f64().expect("positive game number converts to f64"))
        * width as f64;
    let whole = filled.floor() as usize;
    let part = ((filled - filled.floor()) * 8.0).round() as usize;

    let mut out = String::with_capacity(width * 3);
    for _ in 0..whole.min(width) {
        out.push('█');
    }
    // A creature that is alive shows something, even at one hit point in
    // fifty: an empty bar is what dead looks like.
    if whole < width {
        match (part, value > N::zero() && whole == 0) {
            (0, false) => {}
            (0, true) => out.push(EIGHTHS[0]),
            (p, _) => out.push(EIGHTHS[(p - 1).min(7)]),
        }
    }
    let drawn = out.chars().count();
    for _ in drawn..width {
        out.push(' ');
    }
    out
}

/// `hp 34/72` with a bar after it, and the block beside that.
pub fn vitals<N: PrimInt + ToPrimitive + std::fmt::Display>(
    hp: N,
    max_hp: N,
    block: N,
    width: usize,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(format!("{hp}/{max_hp}"), theme::hurt()),
        Span::raw(" "),
        Span::styled(bar(hp, max_hp, width), theme::hurt()),
    ];
    if block > N::zero() {
        spans.push(Span::styled(format!("  B{block}"), theme::guard()));
    }
    Line::from(spans)
}

/// One power, as a chip: `^ Strength 2`, `v Weak 3`.
///
/// The arrow carries the sign, so the colour is not the only thing saying it. A
/// terminal without colour, or a reader who cannot tell red from green, still
/// gets which way the power points.
pub fn chip(name: &str, amount: impl std::fmt::Display, good: bool) -> Span<'static> {
    let (mark, style) = match good {
        true => ("^", theme::better()),
        false => ("v", theme::hurt()),
    };
    // Every glyph on these screens is one column wide. The obvious symbols —
    // ⚡ for energy, ⛁ for gold, ✗ for a refusal — are East Asian Wide or
    // Ambiguous, which means the width depends on the reader's terminal and a
    // panel drawn to the column silently comes apart in half of them.
    Span::styled(format!("{mark} {name} {amount}"), style)
}

/// A power the tables will not class without a creature to ask.
///
/// No arrow and no colour: the amount alone. Half the build's dynamic powers
/// point either way depending on their own amount, and a guess with a colour on
/// it reads as a fact.
pub fn unknown_chip(name: &str, amount: impl std::fmt::Display) -> Span<'static> {
    Span::styled(format!("· {name} {amount}"), theme::TEXT)
}

/// As many whole chips as fit, then a count of what did not.
///
/// Chips are never cut in half: half a power's name is worse than knowing there
/// are two more, and `i` shows the rest.
pub fn chips(powers: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let mut out: Vec<Span> = Vec::new();
    let mut used = 0usize;

    for (i, power) in powers.iter().enumerate() {
        let left = powers.len() - i;
        let overflow = if left > 1 { 4 } else { 0 };
        let need = power.content.chars().count() + usize::from(i > 0) * 2;
        if used + need + overflow > width {
            out.push(Span::styled(format!("  +{left}"), theme::dim()));
            return Line::from(out);
        }
        if i > 0 {
            out.push(Span::raw("  "));
        }
        out.push(power.clone());
        used += need;
    }
    Line::from(out)
}

/// The game's own words, coloured the way the game coloured them.
///
/// The parse is [`crate::render::prose`], shared with the other front end. The
/// two cannot disagree about what a card says, only about how it looks.
pub fn words(text: &str) -> Vec<Line<'static>> {
    let mut lines = vec![Vec::new()];
    for span in prose::spans(text) {
        let style = match span.tint {
            Some(Tint::Gold) => theme::gold(),
            Some(Tint::Green) => theme::better(),
            Some(Tint::Red) => theme::hurt(),
            None => theme::TEXT,
        };
        // The game breaks a card's face for a card-shaped rectangle. Those
        // breaks are kept — they are how the card is written — and the widget
        // that draws them wraps whatever is still too wide.
        for (i, piece) in span.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Vec::new());
            }
            if !piece.is_empty() {
                let last = lines.len() - 1;
                lines[last].push(Span::styled(piece.to_string(), style));
            }
        }
    }
    lines.into_iter().map(Line::from).collect()
}

/// A key and what it does, for the bar along the bottom.
pub fn hint(key: &str, what: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(key.to_string(), Style::new().bold()),
        Span::raw(" "),
        Span::styled(what.to_string(), theme::dim()),
        Span::raw("   "),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width(line: &Line) -> usize {
        line.spans.iter().map(|s| s.content.chars().count()).sum()
    }

    #[test]
    fn a_full_bar_is_all_blocks_and_an_empty_one_is_none() {
        assert_eq!(bar(10, 10, 5), "█████");
        assert_eq!(bar(0, 10, 5), "     ");
    }

    /// The reason for eighths: three points off forty has to be visible.
    #[test]
    fn a_small_loss_moves_the_bar() {
        assert_ne!(bar(40, 40, 10), bar(37, 40, 10));
    }

    /// One hit point left must not read the same as dead.
    #[test]
    fn barely_alive_is_not_empty() {
        assert_ne!(bar(1, 500, 10), bar(0, 500, 10));
    }

    #[test]
    fn a_bar_is_always_its_width() {
        for value in [0, 1, 7, 39, 40] {
            assert_eq!(bar(value, 40, 12).chars().count(), 12, "at {value}");
        }
        assert_eq!(bar(3, 0, 6).chars().count(), 6);
        assert_eq!(bar(3, 10, 0), "");
    }

    /// A creature past its maximum, which the game allows, must not overflow.
    #[test]
    fn more_than_the_maximum_is_still_full() {
        assert_eq!(bar(80, 40, 4).chars().count(), 4);
        assert_eq!(bar(-5, 40, 4).chars().count(), 4);
    }

    #[test]
    fn chips_that_do_not_fit_are_counted_instead_of_cut() {
        let powers = vec![
            chip("Strength", 2, true),
            chip("Vulnerable", 3, false),
            chip("Weak", 1, false),
        ];
        let line = chips(powers, 20);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "^ Strength 2  +2");
    }

    /// Narrower than one whole chip: the count is still the truth, and half a
    /// power's name is not.
    #[test]
    fn a_panel_too_narrow_for_any_chip_still_counts_them() {
        let line = chips(vec![chip("Strength", 2, true), chip("Weak", 1, false)], 6);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "  +2");
    }

    #[test]
    fn every_chip_shown_when_they_all_fit() {
        let line = chips(vec![chip("Strength", 2, true), chip("Weak", 1, false)], 40);
        assert_eq!(width(&line), "^ Strength 2".len() + 2 + "v Weak 1".len());
    }

    /// The card's own line breaks are the card's, and are kept.
    #[test]
    fn a_face_keeps_the_breaks_the_game_wrote() {
        let lines = words("Deal 6 damage.\nExhaust.");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].spans[0].content, "Exhaust.");
    }

    #[test]
    fn a_recomputed_number_keeps_its_colour() {
        let lines = words("Apply [green]4[/green] Poison.");
        let tinted = lines[0]
            .spans
            .iter()
            .find(|s| s.content == "4")
            .expect("the number is its own span");
        assert_eq!(tinted.style, theme::better());
    }
}
