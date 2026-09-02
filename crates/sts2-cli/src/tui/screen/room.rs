//! Everything that is a choice rather than a fight.
//!
//! Rest sites, chests, events, shops, card selections and reward screens are
//! six mechanisms in the protocol and one question to a player: *which of
//! these*. So they are drawn as one shape: a numbered list down the left, and
//! the thing under the cursor spelled out. The card-shaped ones get card boxes,
//! because a card being chosen looks like the card it will become.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use sts2_core::{ContentVersion, Play, Question, RewardKind, Room, Stock, Take, TakeReward};

use crate::render::spelling;
use crate::tui::app::App;
use crate::tui::names;
use crate::tui::theme;

use super::{card, titled, widget};

/// One row of a chooser: the number that takes it, what it is, and a note.
struct Choice {
    /// The digit the player presses. `None` for a row that cannot be taken.
    key: Option<usize>,
    what: String,
    note: Option<(String, ratatui::style::Style)>,
    /// The game's own words about it, if it has any.
    words: Vec<Line<'static>>,
    faded: bool,
}

// ---------------------------------------------------------------------------
// Rooms.
// ---------------------------------------------------------------------------

pub fn draw<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, room: &Room<V>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    match room {
        Room::Shop { entries } => {
            let moves = app.moves();
            let offered = |index: u8| {
                moves
                    .iter()
                    .position(|m| *m == crate::tui::app::Move::Buy(index))
            };
            let choices = entries
                .iter()
                .map(|entry| {
                    let what = match &entry.stock {
                        Stock::Card(Some(c)) => card::ident(c),
                        Stock::Card(None) => "a card".into(),
                        Stock::Relic(Some(r)) => names::pretty(&spelling(r)),
                        Stock::Relic(None) => "a relic".into(),
                        Stock::Potion(Some(p)) => names::pretty(&spelling(p)),
                        Stock::Potion(None) => "a potion".into(),
                        Stock::CardRemoval => "remove a card".into(),
                    };
                    let note = if !entry.stocked {
                        Some(("sold".to_string(), theme::dim()))
                    } else if let Some(blocked) = &entry.blocked {
                        Some((names::label(&format!("{blocked:?}")), theme::refused()))
                    } else if !entry.affordable {
                        Some((format!("{} gold — short", entry.cost), theme::refused()))
                    } else {
                        Some((format!("{} gold", entry.cost), theme::gold()))
                    };
                    Choice {
                        key: offered(entry.index),
                        what,
                        note,
                        words: Vec::new(),
                        faded: offered(entry.index).is_none(),
                    }
                })
                .collect::<Vec<_>>();
            let mut choices = choices;
            choices.extend(leaving(app, &moves));
            chooser(frame, area, "the shop", None, choices, app.ui.at);
        }

        Room::RestSite { options } => {
            let moves = app.moves();
            let choices = options
                .iter()
                .map(|option| Choice {
                    key: chose(&moves, option.index),
                    what: names::pretty(&spelling(&option.id)),
                    note: (!option.enabled).then(|| ("not offered here".to_string(), theme::dim())),
                    words: Vec::new(),
                    faded: !option.enabled,
                })
                .collect();
            chooser(frame, area, "a fire", None, choices, app.ui.at);
        }

        Room::Treasure { finished, options } => {
            let moves = app.moves();
            let choices = options
                .iter()
                .map(|option| Choice {
                    key: chose(&moves, option.index),
                    what: match &option.take {
                        Take::Relic(relic) => names::pretty(&spelling(relic)),
                        Take::Skip => "leave it".into(),
                    },
                    note: option.chosen.then(|| ("taken".to_string(), theme::dim())),
                    words: Vec::new(),
                    faded: option.chosen,
                })
                .collect();
            let title = match finished {
                true => "an opened chest",
                false => "a chest",
            };
            chooser(frame, area, title, None, choices, app.ui.at);
        }

        Room::Event {
            event,
            title,
            text,
            options,
            ..
        } => {
            let moves = app.moves();
            let choices = options
                .iter()
                .map(|option| Choice {
                    key: chose(&moves, option.index),
                    what: option
                        .title
                        .clone()
                        .map(|t| crate::render::prose::plain(&t))
                        .unwrap_or_else(|| names::pretty(&option.key)),
                    // The relic an option hands over, *unless* the option's
                    // own title already names it — which at Neow every one of
                    // them does, and "Golden Pearl   Golden Pearl" is not a
                    // second fact.
                    note: option
                        .relic
                        .as_ref()
                        .map(|r| names::pretty(&spelling(r)))
                        .filter(|named| {
                            option
                                .title
                                .as_deref()
                                .map(crate::render::prose::plain)
                                .as_deref()
                                != Some(named.as_str())
                        })
                        .map(|named| (named, theme::better())),
                    words: option
                        .text
                        .as_deref()
                        .map(widget::words)
                        .unwrap_or_default(),
                    faded: option.locked || option.chosen,
                })
                .collect();

            let heading = title
                .clone()
                .map(|t| crate::render::prose::plain(&t))
                .unwrap_or_else(|| names::pretty(&spelling(event)));
            let prose = text.as_deref().map(widget::words);
            chooser(frame, area, &heading, prose, choices, app.ui.at);
        }
    }
}

/// Where a room's option sits in the list the keys act on.
fn chose(moves: &[crate::tui::app::Move], index: u8) -> Option<usize> {
    moves
        .iter()
        .position(|m| *m == crate::tui::app::Move::Choose(index))
}

/// The ways out, as rows at the end of a room's own list.
///
/// A shop is the room that has both: stock to buy and a door to leave by, at
/// the same time.
fn leaving<V, G>(app: &App<V, G>, moves: &[crate::tui::app::Move]) -> Vec<Choice>
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let travelable = app
        .observation()
        .map(|o| o.map.travelable.clone())
        .unwrap_or_default();

    moves
        .iter()
        .enumerate()
        .filter_map(|(at, one)| match one {
            crate::tui::app::Move::Proceed => Some(Choice {
                key: Some(at),
                what: "leave the act".into(),
                note: None,
                words: Vec::new(),
                faded: false,
            }),
            crate::tui::app::Move::Travel(to) => {
                let kind = travelable
                    .iter()
                    .find(|r| r.at == *to)
                    .map(|r| names::label(&spelling(&r.kind)))
                    .unwrap_or_else(|| "somewhere".into());
                Some(Choice {
                    key: Some(at),
                    what: format!("leave for {kind}"),
                    note: Some((format!("{},{}", to.col, to.row), theme::dim())),
                    words: Vec::new(),
                    faded: false,
                })
            }
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Questions.
// ---------------------------------------------------------------------------

pub fn asking<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, question: &Question<V>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    match question {
        Question::SelectCards {
            min,
            max,
            asker,
            options,
        } => {
            let asked = match (min, max) {
                (0, 1) => "choose a card, or none".to_string(),
                (a, b) if a == b => format!("choose {a}"),
                (a, b) => format!("choose {a} to {b}"),
            };
            let by = asker
                .as_deref()
                .map(|raw| format!("  ({})", crate::render::prose::asker(raw)))
                .unwrap_or_default();

            let faces: Vec<card::Face> = options
                .iter()
                .enumerate()
                .map(|(at, option)| {
                    let mut face = card::Face::plain(&option.card);
                    face.key = Some(at);
                    face.picked = at == app.ui.at;
                    face.marked = app.marked().contains(&at);
                    face.rules = option
                        .text
                        .as_deref()
                        .map(widget::words)
                        .unwrap_or_default();
                    face
                })
                .collect();
            card_wall(frame, area, &format!("{asked}{by}"), faces, app.ui.at);
        }

        Question::Rewards { rewards, .. } => {
            // The cursor walks individual things, so a card set is as many rows
            // as it has cards: taking "the card reward" is not a move. The
            // number under each one is its place in `App::takeable`, which is
            // the list the keys act on — so a reward already taken keeps its
            // box, loses its digit, and does not renumber the ones after it.
            let takeable = app.takeable();
            let mut faces = Vec::new();
            for (index, detail) in rewards.iter().enumerate() {
                match &detail.reward {
                    RewardKind::Card { cards } => {
                        for (card_at, offered) in cards.iter().enumerate() {
                            let mut face = card::Face::plain(&offered.card);
                            face.rules = offered
                                .text
                                .as_deref()
                                .map(widget::words)
                                .unwrap_or_default();
                            face.faded = detail.taken;
                            face.key = takeable.iter().position(|t| {
                                *t == TakeReward::Card {
                                    index,
                                    card: card_at,
                                }
                            });
                            faces.push(face);
                        }
                    }
                    other => {
                        let mut face = card::Face::named(reward_name(other));
                        face.faded = detail.taken;
                        face.key = takeable
                            .iter()
                            .position(|t| *t == TakeReward::NonCard { index });
                        faces.push(face);
                    }
                }
            }
            for face in faces.iter_mut() {
                face.picked = face.key == Some(app.ui.at);
            }
            card_wall(frame, area, "rewards", faces, app.ui.at);
        }

        Question::Divine { left, .. } => {
            let lines = vec![
                Line::styled("the Crystal Sphere", theme::heading()),
                Line::raw(""),
                Line::from(format!("{left} divinations left")),
                Line::raw(""),
                Line::styled(
                    "its grid is not drawn here yet — answer it with",
                    theme::dim(),
                ),
                Line::styled("`sts2 act answer divine <x> <y>`", theme::gold()),
            ];
            frame.render_widget(
                Paragraph::new(lines).centered(),
                super::centred(area, 52, 8),
            );
        }
    }
}

fn reward_name<V: ContentVersion>(reward: &RewardKind<V>) -> String {
    match reward {
        RewardKind::Gold { amount } => format!("{amount} gold"),
        RewardKind::Card { .. } => "a card".into(),
        RewardKind::Relic { relic: Some(r), .. } => names::pretty(&spelling(r)),
        RewardKind::Relic { relic: None, .. } => "a relic".into(),
        RewardKind::Potion {
            potion: Some(potion),
        } => names::pretty(&spelling(potion)),
        RewardKind::Potion { potion: None } => "a potion".into(),
        RewardKind::Other { kind } => names::pretty(kind),
    }
}

// ---------------------------------------------------------------------------
// The two shapes.
// ---------------------------------------------------------------------------

/// A numbered list, with the game's prose over it.
fn chooser(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    prose: Option<Vec<Line<'static>>>,
    choices: Vec<Choice>,
    at: usize,
) {
    let block = titled(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let told = prose.unwrap_or_default();
    // No prose, no gap: a room that says nothing should not push its options
    // down a row for the silence.
    let words = match told.is_empty() {
        true => 0,
        false => (told.len() as u16 + 1).min(inner.height / 2),
    };
    let [said, list] =
        Layout::vertical([Constraint::Length(words), Constraint::Min(1)]).areas(inner);

    if !told.is_empty() {
        frame.render_widget(Paragraph::new(told).wrap(Wrap { trim: true }), said);
    }

    let mut lines = Vec::new();
    for choice in &choices {
        let selected = choice.key == Some(at);
        let number = match choice.key {
            Some(key) => format!(" {} ", key + 1),
            None => "   ".to_string(),
        };
        let style = match (selected, choice.faded) {
            (true, _) => theme::cursor(),
            (false, true) => theme::dim(),
            (false, false) => theme::TEXT,
        };

        let mut spans = vec![
            Span::styled(number, theme::label()),
            Span::styled(format!(" {} ", choice.what), style),
        ];
        if let Some((note, note_style)) = &choice.note {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(note.clone(), *note_style));
        }
        lines.push(Line::from(spans));

        // The option's own words, under it, and only for the one being looked
        // at — a screen of six paragraphs is a screen nobody reads.
        if selected {
            for line in &choice.words {
                let mut indented = vec![Span::raw("     ")];
                indented.extend(line.spans.iter().cloned());
                lines.push(Line::from(indented));
            }
        }
    }

    // Not trimmed: the indent under a chosen option is what ties its words to
    // it, and `trim` throws leading space away.
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), list);
}

/// Card boxes, wrapped across as many rows as they need.
fn card_wall(frame: &mut Frame, area: Rect, title: &str, faces: Vec<card::Face>, at: usize) {
    let block = titled(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if faces.is_empty() || inner.height < 3 || inner.width < 12 {
        return;
    }

    let each = card::width(inner.width, faces.len().min(5));
    let per_row = (inner.width / each).max(1) as usize;
    let rows = faces.len().div_ceil(per_row);
    // One row of cards gets room to say what it does — a reward screen is read,
    // not scanned, and "Deals 2 additional damage for ALL your" is a sentence
    // cut where it matters. Several rows share the space and stay compact.
    let most = match rows {
        1 => 12,
        _ => 6,
    };
    let tall = (inner.height / rows as u16).clamp(3, most);

    // The row the cursor is on is always drawn, however many there are.
    let first_row = match rows as u16 * tall <= inner.height {
        true => 0,
        false => (at / per_row).saturating_sub((inner.height / tall) as usize - 1),
    };

    for (n, face) in faces.iter().enumerate() {
        let row = n / per_row;
        if row < first_row {
            continue;
        }
        let y = inner.y + ((row - first_row) as u16) * tall;
        if y + tall > inner.y + inner.height {
            break;
        }
        let slot = Rect {
            x: inner.x + ((n % per_row) as u16) * each,
            y,
            width: each,
            height: tall,
        };
        card::draw(frame, slot, face);
    }
}
