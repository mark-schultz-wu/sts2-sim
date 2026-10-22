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
use sts2_core::{Coord, Decision, Map, Play};

use crate::render::spelling;
use crate::tui::app::App;
use crate::tui::theme;

use super::titled;

pub fn draw<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, map: &Map<V>)
where
    V: crate::tui::Build,
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

/// Columns of screen per column of the act. Two hold the room's letter and
/// digit, and two more give a path to the next column a cell to sit in.
const PITCH: usize = 4;
/// The row number in front of each row of rooms.
const GUTTER: usize = 4;

/// The whole act, as a grid, with the paths between its rooms.
///
/// The far end of the act is at the top. Colour marks room type, and shape
/// marks state. `@` is the current room, a digit is its travel key, and
/// visited rooms are dim. A screen too short for every row of paths draws the
/// ones from the current room up. Overlays supply the heading.
pub fn grid_of<V: crate::tui::Build>(frame: &mut Frame, inner: Rect, map: &Map<V>, at: usize) {
    if map.points.is_empty() || inner.height == 0 {
        return;
    }

    let cols = map.points.iter().map(|p| p.at.col).max().unwrap_or(0) as usize;
    let rows = map.points.iter().map(|p| p.at.row).max().unwrap_or(0);
    let reachable: Vec<Coord> = map.travelable.iter().map(|r| r.at).collect();

    // Every row of rooms always shows, and the legend under them when there
    // is a row for it. The rows left over go to paths, the nearest first:
    // from the room the run stands in, upward, which is the way it can go.
    let height = usize::from(inner.height);
    let legend_rows = usize::from(usize::from(rows) + 2 <= height);
    let spare = height.saturating_sub(usize::from(rows) + 1 + legend_rows);
    let first = match spare >= usize::from(rows) {
        true => 0,
        false => map.at.map_or(0, |here| here.row),
    };
    let drawn = |row: u8| row >= first && usize::from(row - first) < spare;

    let mut lines = Vec::new();
    for row in (0..=rows).rev() {
        if row < rows && drawn(row) {
            lines.push(paths(map, row, cols));
        }
        lines.push(rooms(map, row, cols, &reachable, at));
    }
    // The letters are learnable in one line, and a map nobody can read is a
    // map that gets ignored in favour of the room list under it.
    if legend_rows == 1 {
        lines.push(Line::from(legend()));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// One row of rooms.
fn rooms<V: crate::tui::Build>(
    map: &Map<V>,
    row: u8,
    cols: usize,
    reachable: &[Coord],
    at: usize,
) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("{row:>3} "), theme::dim())];
    for col in 0..=cols {
        let here = Coord::new(col as u8, row);
        let Some(node) = map.points.iter().find(|p| p.at == here) else {
            spans.push(Span::raw(" ".repeat(PITCH)));
            continue;
        };

        let kind = spelling(&node.kind);
        let mark = crate::map_point::mark(&kind).glyph;
        let tint = theme::room(&kind);
        let gap = " ".repeat(PITCH - 2);

        // Where the run is standing. `@` rather than the room's letter,
        // because the question the eye asks first is "where am I".
        if map.at == Some(here) {
            spans.push(Span::styled("@ ", theme::cursor()));
            spans.push(Span::raw(gap));
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
                spans.push(Span::raw(gap));
            }
            None => {
                let style = match map.visited.contains(&here) {
                    true => tint.add_modifier(ratatui::style::Modifier::DIM),
                    false => tint,
                };
                spans.push(Span::styled(format!("{mark} "), style));
                spans.push(Span::raw(gap));
            }
        }
    }
    Line::from(spans)
}

/// How a path between two rooms is drawn.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Trail {
    /// A path no run has reason to look at yet.
    Faint,
    /// A path the run has walked.
    Walked,
    /// A path out of the room the run is standing in.
    Open,
}

/// The paths from one row of rooms to the row above it.
///
/// `│` goes straight on, `╱` and `╲` go to a neighbouring column, and `╳` is
/// two crossing. Paths out of the current room are bright, walked paths are
/// dim, and the rest are faint. A path that skips a column has no cell, so
/// it is left out. Only the first room and the boss have one.
fn paths<V: crate::tui::Build>(map: &Map<V>, row: u8, cols: usize) -> Line<'static> {
    let width = GUTTER + PITCH * (cols + 1);
    let mut cells: Vec<Option<(char, Trail)>> = vec![None; width];
    for node in map.points.iter().filter(|p| p.at.row == row) {
        for to in node.to.iter().filter(|to| to.row == row + 1) {
            let trail = match (map.at == Some(node.at), map.visited.contains(&node.at)) {
                (true, _) => Trail::Open,
                (false, true) if map.visited.contains(to) => Trail::Walked,
                _ => Trail::Faint,
            };
            let from = GUTTER + PITCH * usize::from(node.at.col);
            let (x, glyph) = match i16::from(to.col) - i16::from(node.at.col) {
                0 => (from, '│'),
                1 => (from + PITCH / 2, '╱'),
                -1 => (from - PITCH / 2, '╲'),
                _ => continue,
            };
            let crossed = match (cells[x], glyph) {
                (Some(('╱', _)), '╲') | (Some(('╲', _)), '╱') => '╳',
                _ => glyph,
            };
            let trail = cells[x].map_or(trail, |(_, was)| was.max(trail));
            cells[x] = Some((crossed, trail));
        }
    }

    let spans = cells
        .into_iter()
        .map(|cell| match cell {
            None => Span::raw(" "),
            Some((glyph, trail)) => Span::styled(
                glyph.to_string(),
                match trail {
                    Trail::Open => theme::label().bold(),
                    Trail::Walked => theme::dim(),
                    Trail::Faint => theme::faint(),
                },
            ),
        })
        .collect::<Vec<_>>();
    Line::from(spans)
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
fn where_to<V: crate::tui::Build>(
    frame: &mut Frame,
    area: Rect,
    map: &Map<V>,
    moves: &[Decision<V>],
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
                Decision::Proceed => (
                    "Boss".to_string(),
                    " leave the act ".to_string(),
                    String::new(),
                ),
                Decision::Travel { to } => {
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
