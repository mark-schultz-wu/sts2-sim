//! The act, and where the run can go next.
//!
//! Drawn bottom-up, the way the game draws it. The row you are standing on is
//! low, the boss is at the top, and the paths lead away from you. Every place
//! that can be reached from here carries the digit that goes there, so the map
//! is both the picture and the menu.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sts2_core::{ContentVersion, Coord, Map, Play};

use crate::render::spelling;
use crate::tui::app::App;
use crate::tui::theme;

use super::titled;

pub fn draw<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, map: &Map<V>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let moves = app.moves();
    let rows = (moves.len() as u16 + 2).clamp(3, 8);
    let [grid, choices] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(rows)]).areas(area);

    let block = titled("the act");
    let inner = block.inner(grid);
    frame.render_widget(block, grid);
    grid_of(frame, inner, map, app.ui.at);

    where_to(frame, choices, map, &moves, app.ui.at);
}

/// The whole act, as a grid.
///
/// Draws the far end of the act at the top. Colour marks room type. Shape marks
/// state. `@` is the current room, a digit is its travel key, and visited rooms
/// are dim. The function adds no heading because overlays supply one.
pub fn grid_of<V: ContentVersion>(frame: &mut Frame, inner: Rect, map: &Map<V>, at: usize) {
    if map.points.is_empty() || inner.height == 0 {
        return;
    }

    let cols = map.points.iter().map(|p| p.at.col).max().unwrap_or(0);
    let rows = map.points.iter().map(|p| p.at.row).max().unwrap_or(0);
    let reachable: Vec<Coord> = map.travelable.iter().map(|r| r.at).collect();

    let mut lines = Vec::new();
    for row in (0..=rows).rev() {
        let mut spans = vec![Span::styled(format!("{row:>3} "), theme::dim())];
        for col in 0..=cols {
            let here = Coord::new(col, row);
            let Some(node) = map.points.iter().find(|p| p.at == here) else {
                spans.push(Span::raw("  "));
                continue;
            };

            let kind = spelling(&node.kind);
            let mark = crate::map_point::mark(&kind).glyph;
            let tint = theme::room(&kind);

            // Where the run is standing. `@` rather than the room's letter,
            // because the question the eye asks first is "where am I".
            if map.at == Some(here) {
                spans.push(Span::styled("@ ", theme::cursor()));
                continue;
            }

            match reachable.iter().position(|r| *r == here) {
                // Somewhere the run can go: the room's own colour, bold, with
                // the digit that goes there. The colour is kept rather than
                // replaced — which room it is matters more than that it is
                // reachable, and the digit already says the latter.
                Some(n) => {
                    let digit = match n == at {
                        true => theme::cursor(),
                        false => theme::label().bold(),
                    };
                    spans.push(Span::styled(mark.to_string(), tint.bold()));
                    spans.push(Span::styled((n + 1).to_string(), digit));
                }
                None => {
                    let walked = map.visited.contains(&here);
                    let style = match walked {
                        true => tint.add_modifier(ratatui::style::Modifier::DIM),
                        false => tint,
                    };
                    spans.push(Span::styled(format!("{mark} "), style));
                }
            }
        }
        lines.push(Line::from(spans));
    }

    // The letters are learnable in one line, and a map nobody can read is a
    // map that gets ignored in favour of the room list under it.
    if (lines.len() as u16) < inner.height {
        lines.push(Line::from(legend()));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// What each letter means, in the colour it is drawn in.
fn legend() -> Vec<Span<'static>> {
    const SHOWN: [(&str, &str); 7] = [
        ("Monster", "fight"),
        ("Elite", "elite"),
        ("RestSite", "fire"),
        ("Shop", "shop"),
        ("Treasure", "chest"),
        ("Unknown", "?"),
        ("Boss", "boss"),
    ];

    let mut spans = vec![Span::raw("    ")];
    for (kind, word) in SHOWN {
        spans.push(Span::styled(
            crate::map_point::mark(kind).glyph.to_string(),
            theme::room(kind).bold(),
        ));
        spans.push(Span::styled(format!(" {word}  "), theme::dim()));
    }
    spans
}

/// The moves, spelled out under the picture.
///
/// Numbered off the same list the keys act on. The digit beside a room is the
/// digit that goes there, whatever else is on the screen.
fn where_to<V: ContentVersion>(
    frame: &mut Frame,
    area: Rect,
    map: &Map<V>,
    moves: &[crate::tui::app::Move],
    at: usize,
) {
    let block = titled("where to");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    if moves.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled(" nowhere from here", theme::dim())),
            inner,
        );
        return;
    }

    let lines: Vec<Line> = moves
        .iter()
        .enumerate()
        .map(|(n, one)| {
            let style = match n == at {
                true => theme::cursor(),
                false => theme::TEXT,
            };
            let (kind, what, where_) = match one {
                crate::tui::app::Move::Proceed => (
                    "Boss".to_string(),
                    " leave the act ".to_string(),
                    String::new(),
                ),
                crate::tui::app::Move::Travel(to) => {
                    let kind = map
                        .travelable
                        .iter()
                        .find(|r| r.at == *to)
                        .map(|r| spelling(&r.kind))
                        .unwrap_or_default();
                    (
                        kind.clone(),
                        format!(" {} ", crate::map_point::mark(&kind).description),
                        format!("  {},{}", to.col, to.row),
                    )
                }
                // A room's own options are drawn by the room, not here.
                _ => (String::new(), " something here ".to_string(), String::new()),
            };
            // The cursor takes the row; otherwise the room keeps the colour it
            // has on the map above, so the two read as one picture.
            let style = match n == at {
                true => style,
                false => theme::room(&kind),
            };
            Line::from(vec![
                Span::styled(format!(" {} ", n + 1), theme::label()),
                Span::styled(
                    crate::map_point::mark(&kind).glyph.to_string(),
                    theme::room(&kind).bold(),
                ),
                Span::styled(what, style),
                Span::styled(where_, theme::dim()),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}
