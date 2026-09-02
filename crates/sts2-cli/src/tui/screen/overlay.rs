//! Panels over the screen: the keys, a pile, the map, one thing in full.
//!
//! Over the body and never over the run strip. The numbers that decide whether
//! to open a pile at all stay visible while it is open.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use sts2_core::{CardIdent, ContentVersion, Play};

use crate::tui::app::App;
use crate::tui::keys::HELP;
use crate::tui::names;
use crate::tui::state::{Overlay, Pile};
use crate::tui::theme;

use super::{card, centred, map, widget};

pub fn draw<V, G>(frame: &mut Frame, body: Rect, app: &App<V, G>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(overlay) = &app.ui.overlay else {
        return;
    };

    match overlay {
        Overlay::Help => keys(frame, body),
        Overlay::Detail(lines) => detail(frame, body, lines),
        Overlay::Map => {
            // The whole body, not an inset: two columns of the screen beneath
            // showing down each side reads as a torn panel rather than as
            // something lying on top of something else.
            let area = centred(body, body.width, body.height);
            let Some(observed) = app.observation() else {
                return;
            };
            frame.render_widget(Clear, area);
            frame.render_widget(panel("the act"), area);
            map::grid_of(frame, inside(area), &observed.map, usize::MAX);
        }
        Overlay::Pile(which) => pile(frame, body, app, *which),
        Overlay::Belt => belt(frame, body, app),
        Overlay::Relics => relics(frame, body, app),
    }
}

fn panel(title: &str) -> Block<'static> {
    Block::bordered()
        .border_style(theme::label())
        .title(Span::styled(format!(" {title} "), theme::heading()))
        .title_bottom(Line::from(Span::styled(" esc ", theme::dim())).right_aligned())
}

fn inside(area: Rect) -> Rect {
    Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    }
}

fn keys(frame: &mut Frame, body: Rect) {
    let width = 46u16;
    let area = centred(body, width, HELP.len() as u16 + 2);
    frame.render_widget(Clear, area);
    frame.render_widget(panel("keys"), area);

    let lines: Vec<Line> = HELP
        .iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(format!("{key:>14}"), theme::gold().bold()),
                Span::raw("  "),
                Span::styled((*what).to_string(), theme::TEXT),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inside(area));
}

fn detail(frame: &mut Frame, body: Rect, lines: &[String]) {
    let area = centred(body, 52, (lines.len() as u16 + 3).min(body.height));
    frame.render_widget(Clear, area);
    frame.render_widget(panel("in full"), area);

    let drawn: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| match i {
            0 => Line::styled(line.clone(), theme::heading()),
            _ => Line::raw(line.clone()),
        })
        .collect();
    frame.render_widget(
        Paragraph::new(drawn).wrap(Wrap { trim: true }),
        inside(area),
    );
}

/// The belt: what is in it, what each one does, and which can be drunk.
///
/// The only actionable overlay, since potions are otherwise unreachable. A
/// potion the game rejects keeps its row but loses its digit, with the reported
/// reason beside it.
fn belt<V, G>(frame: &mut Frame, body: Rect, app: &App<V, G>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(observed) = app.observation() else {
        return;
    };
    let potions = &observed.potions;

    let title = format!("potions ({}/{})", potions.len(), observed.potion_slots);
    let area = centred(
        body,
        56.min(body.width),
        body.height.min(4 + potions.len() as u16 * 3),
    );
    frame.render_widget(Clear, area);
    frame.render_widget(panel(&title), area);

    let inner = inside(area);
    if potions.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled("the belt is empty", theme::dim())),
            inner,
        );
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (n, potion) in potions.iter().enumerate() {
        let name = potion
            .title
            .clone()
            .unwrap_or_else(|| names::pretty(&crate::render::spelling(&potion.potion)));

        let mut head = vec![
            Span::styled(
                match potion.reason {
                    None => format!("[{}] ", n + 1),
                    Some(_) => "    ".to_string(),
                },
                theme::gold(),
            ),
            Span::styled(
                name,
                match n == app.ui.at {
                    true => theme::cursor(),
                    false => theme::TEXT,
                },
            ),
        ];
        if let Some(why) = potion.reason {
            head.push(Span::raw("   "));
            head.push(Span::styled(names::undrinkable(why), theme::refused()));
        }
        lines.push(Line::from(head));

        for line in widget::words(potion.text.as_deref().unwrap_or("")) {
            let mut indented = vec![Span::raw("    ")];
            indented.extend(line.spans);
            lines.push(Line::from(indented));
        }
        lines.push(Line::raw(""));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// Every relic being carried, and what each one does.
///
/// Read-only: a relic is a passive and there is nothing to do to one. It exists
/// because a run strip listing eight names says nothing about what any of them
/// does, either before it was picked up or after.
fn relics<V, G>(frame: &mut Frame, body: Rect, app: &App<V, G>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(observed) = app.observation() else {
        return;
    };
    let relics = &observed.relics;

    let area = centred(body, 56.min(body.width), body.height);
    frame.render_widget(Clear, area);
    frame.render_widget(panel(&format!("relics ({})", relics.len())), area);

    let inner = inside(area);
    if relics.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::styled("none yet", theme::dim())),
            inner,
        );
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for relic in relics {
        let name = relic
            .title
            .clone()
            .unwrap_or_else(|| names::pretty(&crate::render::spelling(&relic.relic)));
        lines.push(Line::styled(name, theme::better()));
        lines.extend(widget::words(relic.text.as_deref().unwrap_or("")));
        lines.push(Line::raw(""));
    }

    // Scrolled by the cursor, because eight relics with their text is taller
    // than most terminals.
    let from = app.ui.at.min(lines.len().saturating_sub(1));
    let shown: Vec<Line> = lines.into_iter().skip(from).collect();
    frame.render_widget(Paragraph::new(shown).wrap(Wrap { trim: false }), inner);
}

/// A pile, counted and listed.
///
/// The draw pile is listed in the order the game reports it, which is the order
/// it will be drawn in. That is a real advantage the game gives a player who
/// looks, and an alphabetical sort throws it away.
fn pile<V, G>(frame: &mut Frame, body: Rect, app: &App<V, G>, which: Pile)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(observed) = app.observation() else {
        return;
    };
    let combat = observed.fight();

    let cards: &[CardIdent<V>] = match which {
        Pile::Deck => &observed.deck,
        Pile::Draw => combat.map(|c| c.draw.as_slice()).unwrap_or_default(),
        Pile::Discard => combat.map(|c| c.discard.as_slice()).unwrap_or_default(),
        Pile::Exhaust => combat.map(|c| c.exhaust.as_slice()).unwrap_or_default(),
    };

    let area = centred(body, 40.min(body.width), body.height);
    frame.render_widget(Clear, area);
    frame.render_widget(panel(&format!("{} ({})", which.title(), cards.len())), area);

    let inner = inside(area);
    if inner.height == 0 {
        return;
    }

    // The deck is the only pile whose order says nothing, so it is the only one
    // worth counting instead of listing.
    let lines: Vec<Line> = match which {
        Pile::Deck => counted(cards),
        _ => cards
            .iter()
            .enumerate()
            .map(|(n, c)| {
                Line::from(vec![
                    Span::styled(format!("{:>3} ", n + 1), theme::dim()),
                    Span::raw(card::ident(c)),
                ])
            })
            .collect(),
    };

    let from = app.ui.at.min(lines.len().saturating_sub(1));
    let shown: Vec<Line> = lines.into_iter().skip(from).collect();
    frame.render_widget(Paragraph::new(shown), inner);
}

/// The same card several times is one line and a count.
fn counted<V: ContentVersion>(cards: &[CardIdent<V>]) -> Vec<Line<'static>> {
    let mut tally: Vec<(String, usize)> = Vec::new();
    for card in cards {
        let name = card::ident(card);
        match tally.iter_mut().find(|(n, _)| *n == name) {
            Some((_, count)) => *count += 1,
            None => tally.push((name, 1)),
        }
    }
    tally.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    tally
        .into_iter()
        .map(|(name, count)| {
            Line::from(vec![
                Span::styled(format!("{count:>3} "), theme::dim()),
                Span::raw(name),
            ])
        })
        .collect()
}
