//! Drawing. One frame per key press, whole, with no diffing.
//!
//! The shape is the same on every screen, so the eye learns where to look once:
//! a one-line strip of what the run is carrying, the screen itself, a log of
//! what has just happened, and the keys along the bottom. Overlays are drawn
//! over the middle and never over the strip, so the run's numbers are visible
//! while a pile is being read.
//!
//! Nothing here talks to the game. Every panel is a function of two things: the
//! last `Observation<V>`, and the front end's own cursor. That is what makes the
//! screens testable and what stops a number appearing that nothing reported.

mod card;
mod combat;
mod map;
mod overlay;
mod room;
pub(super) mod start;
pub mod widget;

use num_traits::{One, ToPrimitive};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use sts2_core::{Observation, Play, Question};

use super::app::App;
use super::names;
use super::state::Tone;
use super::theme;
use crate::render::spelling;

/// How many rows the log gets.
///
/// Allocates no log on short screens and gives it surplus rows on tall ones. A
/// fight has a fixed useful height, while log history benefits from extra room.
/// The cap avoids a mostly empty panel dominating the screen.
pub(crate) fn log_rows(height: u16) -> u16 {
    match height {
        0..=23 => 0,
        24..=31 => 3,
        32..=39 => 5,
        _ => (height - 34).min(12),
    }
}

pub fn draw<V, G>(frame: &mut Frame, app: &App<V, G>)
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let whole = frame.area();

    let logs = log_rows(whole.height);
    let [strip, body, log, keys] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(logs),
        Constraint::Length(1),
    ])
    .areas(whole);

    match app.observation() {
        Some(observed) => {
            run_strip(frame, strip, observed, app.options.rng);
            body_of(frame, body, app, observed);
        }
        // Not "there is no run" — that is the setup screen above. This is the
        // one frame drawn before the first look has come back.
        None => {
            frame.render_widget(Paragraph::new("looking…").style(theme::dim()), body);
        }
    }

    if logs > 0 {
        scrollback(frame, log, app);
    }
    key_bar(frame, keys, app);

    overlay::draw(frame, body, app);
}

/// What the run is carrying, on one line, always in the same order.
fn run_strip<V: sts2_content_wire::WireContentVersion>(
    frame: &mut Frame,
    area: Rect,
    observed: &Observation<V>,
    rng: bool,
) {
    let mut spans = vec![
        Span::styled(
            format!(" act {} ", observed.act + V::Count::one()),
            theme::label(),
        ),
        Span::styled(format!("floor {}", observed.floor), theme::TEXT),
        Span::raw("   "),
        Span::styled(
            format!("hp {}/{}", observed.hp, observed.max_hp),
            theme::hurt(),
        ),
        Span::raw("   "),
        Span::styled(format!("{} gold", observed.gold), theme::gold()),
    ];

    if !observed.relics.is_empty() {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            observed
                .relics
                .iter()
                .map(|r| {
                    r.title
                        .clone()
                        .unwrap_or_else(|| names::pretty(&spelling(&r.relic)))
                })
                .collect::<Vec<_>>()
                .join(" · "),
            theme::better(),
        ));
    }

    let potions = observed
        .potions
        .iter()
        .map(|p| {
            p.title
                .clone()
                .unwrap_or_else(|| names::pretty(&spelling(&p.potion)))
        })
        .chain(std::iter::repeat_n(
            "—".to_string(),
            observed
                .potion_slots
                .to_usize()
                .expect("potion slot count fits an addressable list")
                .saturating_sub(observed.potions.len()),
        ))
        .collect::<Vec<_>>();
    if !potions.is_empty() {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(potions.join(" · "), theme::dim()));
    }

    if rng {
        let draws: u32 = observed.rng.run.values().map(|at| at.counter).sum::<u32>()
            + observed
                .rng
                .player
                .values()
                .map(|at| at.counter)
                .sum::<u32>();
        spans.push(Span::raw("   "));
        spans.push(Span::styled(format!("rng {draws}"), theme::dim()));
    }

    frame.render_widget(Line::from(spans), area);
}

fn body_of<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, observed: &Observation<V>)
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    if let Some(question) = app.seen().and_then(|a| a.asking.as_ref()) {
        return room::asking(frame, area, app, question);
    }
    if observed.run_over {
        return over(frame, area, observed);
    }
    if let Some(combat) = observed.fight() {
        return combat::draw(frame, area, app, combat);
    }
    match &observed.room {
        // A room that has been answered is no longer a screen. The event whose
        // page is turned, the chest already opened: what the player wants then
        // is the map, and drawing the spent room instead is the dead end this
        // used to be.
        Some(room) if app.room_has_moves() => room::draw(frame, area, app, room),
        _ => map::draw(frame, area, app, &observed.map),
    }
}

fn over<V: sts2_content_wire::WireContentVersion>(
    frame: &mut Frame,
    area: Rect,
    observed: &Observation<V>,
) {
    let (word, style) = match observed.victory {
        Some(true) => ("the spire is yours", theme::better()),
        _ => ("the run ends here", theme::hurt()),
    };
    let lines = vec![
        Line::from(Span::styled(word, style.bold())),
        Line::raw(""),
        Line::from(format!(
            "act {} · floor {} · {} gold",
            observed.act + V::Count::one(),
            observed.floor,
            observed.gold
        )),
        Line::from(format!("{} cards", observed.deck.len())),
        Line::raw(""),
        Line::styled("q to leave", theme::dim()),
    ];
    frame.render_widget(Paragraph::new(lines).centered(), centred(area, 40, 8));
}

/// What has just happened, oldest at the top.
fn scrollback<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>)
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let block = Block::new()
        .borders(Borders::TOP)
        .border_style(theme::dim());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = inner.height as usize;
    let notes: Vec<&super::state::Note> = app.ui.log.iter().collect();
    // Scrolled back by n lines, and never past the start: a log that scrolls
    // off its own top is a log that looks broken.
    let end = notes.len().saturating_sub(app.ui.scrolled);
    let start = end.saturating_sub(rows);

    let lines: Vec<Line> = notes[start..end]
        .iter()
        .map(|note| {
            let style = match note.tone {
                Tone::Did => theme::dim(),
                Tone::Refused => theme::refused(),
                Tone::Cannot => theme::gold(),
                Tone::Warned => theme::gold(),
            };
            let mark = match note.tone {
                Tone::Did => "-",
                Tone::Refused => "x",
                Tone::Cannot => "!",
                Tone::Warned => "!",
            };
            Line::from(vec![
                Span::styled(format!(" {mark} "), style),
                Span::styled(note.text.clone(), style),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

/// The keys that mean something on the screen in front of the player.
/// The name of the potion being aimed, for the bar to say what it drinks.
fn drinking<V, G>(app: &App<V, G>) -> Option<String>
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let at = app.drinking()?;
    let potion = app.observation()?.potions.get(at)?;
    Some(
        potion
            .title
            .clone()
            .unwrap_or_else(|| names::pretty(&spelling(&potion.potion))),
    )
}

fn key_bar<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>)
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let mut spans = vec![Span::raw(" ")];

    if app.ui.overlay.is_some() {
        spans.extend(widget::hint("esc", "close"));
        frame.render_widget(Line::from(spans), area);
        return;
    }

    let asking = app.seen().and_then(|a| a.asking.as_ref());
    let in_fight = app
        .observation()
        .map(|o| o.fight().is_some())
        .unwrap_or(false);
    let aiming = app.aimed_at().is_some();

    match (asking, in_fight, aiming) {
        (Some(Question::SelectCards { min, .. }), ..) => {
            spans.extend(widget::hint("space", "mark"));
            spans.extend(widget::hint("enter", "confirm"));
            if *min == 0 {
                spans.extend(widget::hint("s", "decline"));
            }
        }
        (Some(Question::Rewards { skippable, .. }), ..) => {
            spans.extend(widget::hint("enter", "take"));
            if *skippable {
                spans.extend(widget::hint("s", "skip"));
            }
        }
        (Some(Question::Divine { .. }), ..) => {
            spans.extend(widget::hint(
                "!",
                "not drawn here yet — use `sts2 act answer divine`",
            ));
        }
        (Some(Question::Confirm), ..) => {
            spans.extend(widget::hint(
                "!",
                "not answered here yet — use `sts2 act choose 1` or `0`",
            ));
        }
        (Some(Question::ChooseBundle { .. }), ..) => {
            spans.extend(widget::hint(
                "!",
                "not picked here yet — use `sts2 act choose <bundle>`",
            ));
        }
        (None, true, true) => {
            spans.extend(widget::hint("←→", "aim"));
            // What confirming sends, which is the whole difference between the
            // two aims and the one thing the bar has to be right about. The
            // potion is named because the belt closed to get out of the way,
            // so this line is the only thing still saying what is in flight —
            // the card does not need it, since it is on the screen and lit.
            spans.extend(match drinking(app) {
                Some(name) => widget::hint("enter", &format!("drink {name}")),
                None => widget::hint("enter", "play"),
            });
            spans.extend(widget::hint("esc", "put it down"));
        }
        (None, true, false) => {
            spans.extend(widget::hint("←→", "card"));
            spans.extend(widget::hint("enter", "play"));
            spans.extend(widget::hint("e", "end turn"));
            spans.extend(widget::hint("p", "potions"));
            spans.extend(widget::hint("i", "look"));
            spans.extend(widget::hint("d/w/f/x", "piles"));
            spans.extend(widget::hint("m", "map"));
        }
        (None, false, _) => {
            spans.extend(widget::hint("↑↓", "choose"));
            spans.extend(widget::hint("enter", "take it"));
            spans.extend(widget::hint("p", "potions"));
            spans.extend(widget::hint("v", "relics"));
            spans.extend(widget::hint("d", "deck"));
        }
    }
    spans.extend(widget::hint("?", "keys"));

    frame.render_widget(Line::from(spans), area);
}

/// A rectangle of at most `w` by `h`, in the middle of `area`.
pub fn centred(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// A heading over a panel: the words, then a rule to the edge.
pub fn titled(text: &str) -> Block<'static> {
    Block::new()
        .borders(Borders::TOP)
        .border_style(theme::dim())
        .title(Span::styled(format!(" {text} "), theme::label()))
}
