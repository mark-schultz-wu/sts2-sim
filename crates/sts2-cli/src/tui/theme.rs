//! The palette, in one place.
//!
//! Named for what a colour *means* rather than for the colour. A screen never
//! writes `Color::Red`, and the meaning of red can be changed once.
//!
//! The set is deliberately small. A terminal game that colours everything reads
//! as noise. The eye belongs on the two things that decide a turn: what the
//! enemy is about to do, and what a card does about it.
//!
//! Sixteen-colour ANSI, so it inherits whatever palette the terminal is set to
//! and sits inside the user's own theme rather than fighting it.

use ratatui::style::{Color, Modifier, Style};

/// Ordinary text.
pub const TEXT: Style = Style::new();

/// Something present but not currently actionable: an unplayable card, a
/// visited room, a pile that is merely a number.
pub fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

/// The thing the cursor is on.
pub fn cursor() -> Style {
    Style::new()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// A pane's title, and the labels down its left.
pub fn label() -> Style {
    Style::new().fg(Color::Cyan)
}

/// A heading a reader is meant to land on first.
pub fn heading() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

/// Hit points, and damage about to be taken.
pub fn hurt() -> Style {
    Style::new().fg(Color::Red)
}

/// Block, and anything that protects.
pub fn guard() -> Style {
    Style::new().fg(Color::Blue)
}

/// Energy, gold, and the game's own `[gold]`.
pub fn gold() -> Style {
    Style::new().fg(Color::Yellow)
}

/// A number the game recomputed upward, and anything going the player's way.
pub fn better() -> Style {
    Style::new().fg(Color::Green)
}

/// A refusal, and anything that will not work.
pub fn refused() -> Style {
    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
}

/// The creature a card is currently pointed at.
pub fn aimed() -> Style {
    Style::new().fg(Color::Black).bg(Color::Yellow)
}

/// The colour a kind of room is drawn in on the map.
///
/// Uses the build.s room name and falls back to ordinary text. Warm colours mark
/// costly rooms, cool colours mark rewards, and ordinary fights stay neutral.
pub fn room(kind: &str) -> Style {
    match kind {
        "Elite" => Style::new().fg(Color::Red),
        "Boss" => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        "RestSite" => Style::new().fg(Color::Green),
        "Shop" => Style::new().fg(Color::Magenta),
        "Treasure" => Style::new().fg(Color::Yellow),
        "Unknown" => Style::new().fg(Color::Cyan),
        "Ancient" => Style::new().fg(Color::Blue),
        // `Monster` and anything unfamiliar.
        _ => Style::new(),
    }
}

/// A creature that is dead but still on the board.
pub fn gone() -> Style {
    Style::new()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::CROSSED_OUT)
}
