//! Before there is a run: who to play, and which run to play.
//!
//! The one screen not drawn from an observation, because there is none. The
//! game has refused to say where it is standing, on the grounds that it is not
//! standing anywhere. Everything here is what the player has typed so far.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use sts2_core::{ContentVersion, Play};

use crate::render::spelling;
use crate::tui::app::App;
use crate::tui::names;
use crate::tui::state::NewRun;
use crate::tui::theme;

use super::{centred, widget};

pub fn draw<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, setting_up: &NewRun)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let roster = app.roster();

    // Tall enough for the roster, the seed field and the note under it, and no
    // taller: a panel with room for eight characters when there are five is a
    // box of air.
    let panel = centred(
        area,
        52.min(area.width),
        (roster.len() as u16 + 8).min(area.height),
    );
    let block = Block::bordered()
        .border_style(theme::label())
        .title(Span::styled(" a new run ", theme::heading()));
    let inner = block.inner(panel);
    frame.render_widget(block, panel);
    if inner.height == 0 {
        return;
    }

    let [who, gap, seed, note] = Layout::vertical([
        Constraint::Length(roster.len() as u16 + 1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .areas(inner);

    characters(frame, who, &roster, setting_up.at);
    let _ = gap;
    field(frame, seed, setting_up);
    why(frame, note, setting_up);
}

fn characters<C: serde::Serialize + std::fmt::Debug + Copy>(
    frame: &mut Frame,
    area: Rect,
    roster: &[C],
    at: usize,
) {
    let mut lines = vec![Line::styled("who", theme::label())];
    lines.extend(roster.iter().enumerate().map(|(n, character)| {
        let picked = n == at;
        Line::from(vec![
            Span::styled(
                match picked {
                    true => " > ",
                    false => "   ",
                },
                theme::gold(),
            ),
            Span::styled(
                names::pretty(&spelling(character)),
                match picked {
                    true => theme::cursor(),
                    false => theme::TEXT,
                },
            ),
        ])
    }));
    frame.render_widget(Paragraph::new(lines), area);
}

/// The seed, with a block for a cursor so an empty field still looks typed-in.
fn field(frame: &mut Frame, area: Rect, setting_up: &NewRun) {
    let typed = Line::from(vec![
        Span::raw("   "),
        Span::styled(setting_up.seed.clone(), theme::TEXT),
        Span::styled("_", theme::cursor()),
        Span::styled(
            match setting_up.seed.is_empty() {
                // Said rather than left blank: an empty field looks like a
                // field that has to be filled in, and this one does not.
                true => "  (blank picks one)",
                false => "",
            },
            theme::dim(),
        ),
    ]);

    frame.render_widget(
        Paragraph::new(vec![Line::styled("seed", theme::label()), typed]),
        area,
    );
}

fn why(frame: &mut Frame, area: Rect, setting_up: &NewRun) {
    if area.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    if let Some(why) = &setting_up.why {
        lines.push(Line::styled(format!(" {why}"), theme::refused()));
    }
    lines.push(Line::from(
        [
            widget::hint("↑↓", "who"),
            widget::hint("enter", "play"),
            widget::hint("esc", "leave"),
        ]
        .concat(),
    ));
    frame.render_widget(Paragraph::new(lines), area);
}
