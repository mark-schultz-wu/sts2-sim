//! The palette, in one place.
//!
//! Named for what a colour *means* rather than for the colour. A screen never
//! writes `Color::Red`, and the meaning of red can be changed once.
//!
//! The set is deliberately small. A terminal game that colours everything reads
//! as noise. The eye belongs on what the enemy is about to do, and on what a
//! card does about it.
//!
//! Where `COLORTERM` says the terminal draws 24-bit colour, each meaning gets a
//! colour of its own. Anywhere else it is the terminal's own sixteen. The
//! README says how to override the guess.
//!
//! The accent is the run's own colour, shared by headings, labels and the
//! cursor. It follows the character: an Ironclad run reads red.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use ratatui::style::{Color, Modifier, Style};

/// How many colours the terminal draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// The sixteen the terminal's own palette names.
    Ansi,
    /// Any colour, as red, green and blue.
    #[cfg_attr(test, allow(dead_code, reason = "tests draw in sixteen colours"))]
    True,
}

static DEPTH: OnceLock<Depth> = OnceLock::new();
/// Which of [`ACCENTS`] is the run's, by its place there.
static ACCENT: AtomicU8 = AtomicU8::new(0);

/// How many colours this terminal draws, worked out once.
pub fn depth() -> Depth {
    *DEPTH.get_or_init(detect)
}

/// Tests draw in sixteen colours, whatever terminal runs them, so a claim
/// about a colour holds everywhere.
#[cfg(test)]
fn detect() -> Depth {
    Depth::Ansi
}

#[cfg(not(test))]
fn detect() -> Depth {
    let said = |name| std::env::var(name).unwrap_or_default().to_ascii_lowercase();
    match said("STS2_COLOR").as_str() {
        "16" | "ansi" => return Depth::Ansi,
        "truecolor" | "24bit" => return Depth::True,
        _ => {}
    }
    match said("COLORTERM").as_str() {
        "truecolor" | "24bit" => Depth::True,
        _ => Depth::Ansi,
    }
}

/// One colour at each depth.
fn pick(ansi: Color, (r, g, b): (u8, u8, u8)) -> Color {
    match depth() {
        Depth::Ansi => ansi,
        Depth::True => Color::Rgb(r, g, b),
    }
}

/// The run's own colour, by the character it plays as the build spells it.
/// The first is the one for no character, or one this list does not name.
const ACCENTS: [(&str, Color, (u8, u8, u8)); 6] = [
    ("", Color::Cyan, (96, 196, 208)),
    ("IRONCLAD", Color::Red, (226, 92, 72)),
    ("SILENT", Color::Green, (112, 196, 120)),
    ("DEFECT", Color::Blue, (96, 168, 240)),
    ("NECROBINDER", Color::Magenta, (186, 128, 230)),
    ("REGENT", Color::Yellow, (240, 178, 72)),
];

/// Sets the accent to the character a run plays: `IRONCLAD`, `SILENT`, or
/// `CHARACTER/SILENT` as a creature's model spells it. Called as each run
/// starts, since one process can play several.
pub fn accent_for(character: &str) {
    let name = character.rsplit('/').next().unwrap_or(character);
    let at = ACCENTS
        .iter()
        .position(|(who, ..)| *who == name)
        .unwrap_or(0);
    ACCENT.store(at as u8, Ordering::Relaxed);
}

fn accent() -> Color {
    let (_, ansi, rgb) = ACCENTS[usize::from(ACCENT.load(Ordering::Relaxed))];
    pick(ansi, rgb)
}

/// Ordinary text.
pub const TEXT: Style = Style::new();

/// Something present but not currently actionable: an unplayable card, a
/// visited room, a pile that is merely a number.
pub fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

/// Quieter than [`dim`]: scenery for the eye to pass over, such as a path on
/// the map that leads nowhere the run can go yet.
pub fn faint() -> Style {
    Style::new().fg(pick(Color::DarkGray, (92, 96, 108)))
}

/// The thing the cursor is on: a solid block of the accent, with the text cut
/// out of it, which reads on any background.
pub fn cursor() -> Style {
    Style::new()
        .fg(Color::Black)
        .bg(accent())
        .add_modifier(Modifier::BOLD)
}

/// A pane's title, and the labels down its left.
pub fn label() -> Style {
    Style::new().fg(accent())
}

/// A heading a reader is meant to land on first.
pub fn heading() -> Style {
    Style::new().fg(accent()).add_modifier(Modifier::BOLD)
}

/// The rule under a pane's title, and a box nobody is pointing at.
pub fn border() -> Style {
    Style::new().fg(pick(Color::DarkGray, (78, 82, 94)))
}

/// Hit points, and damage about to be taken.
pub fn hurt() -> Style {
    Style::new().fg(pick(Color::Red, (232, 88, 82)))
}

/// Block, and anything that protects.
pub fn guard() -> Style {
    Style::new().fg(pick(Color::Blue, (100, 156, 236)))
}

/// Energy, gold, and the game's own `[gold]`.
pub fn gold() -> Style {
    Style::new().fg(pick(Color::Yellow, (242, 196, 84)))
}

/// A number the game recomputed upward, and anything going the player's way.
pub fn better() -> Style {
    Style::new().fg(pick(Color::Green, (116, 204, 124)))
}

/// A refusal, and anything that will not work.
pub fn refused() -> Style {
    hurt().add_modifier(Modifier::BOLD)
}

/// The creature a card is currently pointed at.
pub fn aimed() -> Style {
    Style::new()
        .fg(Color::Black)
        .bg(pick(Color::Yellow, (242, 196, 84)))
}

/// Hit points, coloured by how many are left: green when whole, through
/// yellow, to red near nothing. Sixteen colours have no ramp, so there it is
/// red throughout, as [`hurt`] is.
pub fn health(left: f64) -> Style {
    if depth() == Depth::Ansi {
        return hurt();
    }
    const LOW: (f64, f64, f64) = (232.0, 88.0, 82.0);
    const MID: (f64, f64, f64) = (236.0, 196.0, 84.0);
    const HIGH: (f64, f64, f64) = (116.0, 204.0, 124.0);
    let left = left.clamp(0.0, 1.0);
    let (from, to, at) = match left < 0.5 {
        true => (LOW, MID, left * 2.0),
        false => (MID, HIGH, (left - 0.5) * 2.0),
    };
    let mix = |a: f64, b: f64| (a + (b - a) * at).round() as u8;
    Style::new().fg(Color::Rgb(
        mix(from.0, to.0),
        mix(from.1, to.1),
        mix(from.2, to.2),
    ))
}

/// A card's name, coloured by its rarity as the build spells it. The common
/// cards keep ordinary text, so the rare ones stand out against them.
pub fn rarity(spelled: &str) -> Style {
    let colour = match spelled {
        "Uncommon" => pick(Color::Cyan, (110, 186, 236)),
        "Rare" => pick(Color::Yellow, (242, 196, 84)),
        "Ancient" => pick(Color::Magenta, (206, 140, 236)),
        "Curse" => pick(Color::Magenta, (170, 100, 190)),
        "Status" => pick(Color::DarkGray, (140, 140, 150)),
        _ => return Style::new().add_modifier(Modifier::BOLD),
    };
    Style::new().fg(colour).add_modifier(Modifier::BOLD)
}

/// A card's frame, coloured by what kind of card it is: attacks warm, skills
/// cool, powers gold.
pub fn kind(kind: sts2_core::CardKind) -> Style {
    use sts2_core::CardKind;
    let colour = match kind {
        CardKind::Attack => pick(Color::Red, (196, 96, 84)),
        CardKind::Skill => pick(Color::Blue, (92, 140, 196)),
        CardKind::Power => pick(Color::Yellow, (214, 176, 84)),
        CardKind::Curse => pick(Color::Magenta, (150, 92, 170)),
        CardKind::Status | CardKind::Quest => pick(Color::DarkGray, (120, 120, 132)),
    };
    Style::new().fg(colour)
}

/// The colour a kind of room is drawn in on the map.
///
/// Uses the build's room name and falls back to ordinary text. Warm colours
/// mark costly rooms, cool colours mark rewards, and ordinary fights stay
/// neutral.
pub fn room(kind: &str) -> Style {
    let red = pick(Color::Red, (232, 88, 82));
    match kind {
        "Elite" => Style::new().fg(red),
        "Boss" => Style::new().fg(red).add_modifier(Modifier::BOLD),
        "RestSite" => Style::new().fg(pick(Color::Green, (116, 204, 124))),
        "Shop" => Style::new().fg(pick(Color::Magenta, (206, 140, 236))),
        "Treasure" => Style::new().fg(pick(Color::Yellow, (242, 196, 84))),
        "Unknown" => Style::new().fg(pick(Color::Cyan, (110, 186, 236))),
        "Ancient" => Style::new().fg(pick(Color::Blue, (100, 156, 236))),
        // `Monster` and anything unfamiliar.
        _ => Style::new(),
    }
}

/// A creature that is dead but still on the board.
pub fn gone() -> Style {
    Style::new()
        .fg(pick(Color::DarkGray, (92, 96, 108)))
        .add_modifier(Modifier::CROSSED_OUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests draw in sixteen colours, so every claim about a colour elsewhere
    /// holds on any terminal.
    #[test]
    fn tests_draw_in_sixteen_colours() {
        assert_eq!(depth(), Depth::Ansi);
        assert_eq!(hurt().fg, Some(Color::Red));
    }

    /// Sixteen colours have no ramp, so hit points stay the one red.
    #[test]
    fn health_has_no_ramp_in_sixteen_colours() {
        assert_eq!(health(1.0), hurt());
        assert_eq!(health(0.1), hurt());
    }
}
