//! The fight.
//!
//! ```text
//!   ┌──────────────────┐  ┌──────────────────┐
//!   │ /! 11 x2   Chomp │  │ [+8]      Harden │   what they are about to do
//!   │ Bowlbug Rock     │  │ Bowlbug Rock     │
//!   │ 20/20 ███████    │  │ 14/20 █████  ⛨5  │
//!   │ v Vulnerable 3   │  │                  │
//!   └──────────────────┘  └──────────────────┘
//!  ─ you ──────────────────────────────────────
//!   61/80 ████████  ⛨5             ⚡ 2
//!   ^ Strength 2
//!  ─ hand ─────────────────────────────────────
//!   ┌(1)────────┐ ┌(2)────────┐ ┌(1)────────┐
//!   ...
//! ```
//!
//! Intents at the top, because they are what the turn is a response to. The
//! hand at the bottom, because that is where the answer is chosen. The player
//! between them, because both halves are about that row of numbers.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use sts2_core::{Combat, CombatId, ContentVersion, Creature, Play};

use crate::render::spelling;
use crate::tui::app::App;
use crate::tui::names;
use crate::tui::theme;

use super::widget;
use super::{card, titled};

/// Rows the enemy row takes: the rule over it, then a border, an intent, a
/// name, a health bar and the power chips.
const ENEMY_ROWS: u16 = 7;
/// The widest one gets. A name, a bar and two power chips need about thirty
/// columns. Past that the panel is holding air.
const PANEL_MAX: u16 = 32;
/// Rows the player's own line and its powers take, under a rule.
const YOU_ROWS: u16 = 3;
/// The most empty battlefield between the enemies and the player.
///
/// A bounded gap separates both sides of the fight. Extra height belongs below
/// the fight beside the log, not between related combatants.
const AIR_MAX: u16 = 8;

pub fn draw<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, combat: &Combat<V>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let (mine, them) = sides(combat);

    // The ally row exists only when there is an ally. A pet is uncommon, and an
    // always-present empty row would cost every other fight seven rows.
    let ally_rows = match mine.is_empty() {
        true => 0,
        false => ENEMY_ROWS,
    };
    let hand_rows = card::height(
        area.height
            .saturating_sub(ENEMY_ROWS + ally_rows + YOU_ROWS + 2),
    );

    // The enemies take what they need, then a bounded gap, then your side and
    // the hand. A tall terminal should give the fight more air, not put the two
    // halves of the same turn a screen apart — so the gap is capped and the
    // surplus falls to the bottom.
    //
    // Worked out rather than left to the solver. Two flexible constraints get
    // the surplus split between them, which is a gap that still grows with the
    // screen — just half as fast. This is a bound.
    let wanted = ENEMY_ROWS + ally_rows + YOU_ROWS + hand_rows + 2;
    let air = area.height.saturating_sub(wanted).min(AIR_MAX);

    let [enemies, _air, allies, you, hand, bar_row, _rest] = Layout::vertical([
        Constraint::Length(ENEMY_ROWS),
        Constraint::Length(air),
        Constraint::Length(ally_rows),
        Constraint::Length(YOU_ROWS),
        Constraint::Length(hand_rows + 1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    let aimed = app.aimed_at();
    creature_row(frame, enemies, "they", &them, aimed);
    if ally_rows > 0 {
        creature_row(frame, allies, "yours", &mine, aimed);
    }
    you_row(frame, you, combat);
    hand_row(frame, hand, app, combat);
    pile_bar(frame, bar_row, combat);
}

// ---------------------------------------------------------------------------
// Them.
// ---------------------------------------------------------------------------

/// The fight's two sides as a player reads them: what is with you, and what is
/// against you.
///
/// Splits on `side`, since pets and charmed monsters can fight opposite their
/// model category. The player gets a separate row and appears in neither list.
/// If no player exists, every creature is treated as an enemy.
fn sides<V: ContentVersion>(combat: &Combat<V>) -> (Vec<&Creature<V>>, Vec<&Creature<V>>) {
    let Some(you) = combat.creatures.iter().find(|c| is_player(c)) else {
        return (Vec::new(), combat.creatures.iter().collect());
    };

    combat
        .creatures
        .iter()
        .filter(|c| c.id != you.id)
        .partition(|c| c.side == you.side)
}

/// One side of the fight, as a row of panels under a rule.
fn creature_row<V: ContentVersion>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    them: &[&Creature<V>],
    aimed: Option<CombatId>,
) {
    let block = titled(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if them.is_empty() || inner.width == 0 {
        return;
    }

    // Capped rather than shared out: two enemies across a wide terminal should
    // be two panels, not two billboards with a creature's name lost in the
    // middle of one. Centred, so the fight sits where the eye already is.
    let slots = Layout::horizontal(vec![Constraint::Max(PANEL_MAX); them.len()])
        .spacing(1)
        .flex(ratatui::layout::Flex::Center)
        .split(inner);

    for (creature, slot) in them.iter().zip(slots.iter()) {
        enemy(frame, *slot, creature, aimed == Some(creature.id));
    }
}

fn enemy<V: ContentVersion>(frame: &mut Frame, area: Rect, creature: &Creature<V>, aimed: bool) {
    let style = match (aimed, creature.alive) {
        (true, _) => theme::aimed(),
        (false, false) => theme::gone(),
        (false, true) => theme::TEXT,
    };
    let block = Block::bordered().border_style(style);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let width = inner.width as usize;
    let mut lines = vec![intent_line(creature, width)];

    let name = match &creature.model {
        Some(model) => names::pretty(&crate::render::entry(model)),
        None => format!("creature {}", creature.id),
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{} ", creature.id), theme::dim()),
        Span::styled(name, theme::heading()),
    ]));

    match creature.alive {
        true => lines.push(widget::vitals(
            creature.hp,
            creature.max_hp,
            creature.block,
            width.saturating_sub(16).clamp(4, 18),
        )),
        false => lines.push(Line::styled("dead", theme::gone())),
    }
    lines.push(powers(creature, width));

    frame.render_widget(Paragraph::new(lines), inner);
}

/// What the creature has told the player it will do.
///
/// The numbers first, because that is what the turn is planned against. The
/// move's name comes after in grey, so a name nobody recognises is not
/// competing with the damage.
fn intent_line<V: ContentVersion>(creature: &Creature<V>, width: usize) -> Line<'static> {
    let Some(next) = &creature.next_move else {
        return Line::styled("??", theme::dim());
    };

    let mut spans: Vec<Span> = Vec::new();
    for intent in &next.intents {
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        match intent.attack {
            Some(attack) => {
                let repeats = match attack.repeats > 1 {
                    true => format!(" x{}", attack.repeats),
                    false => String::new(),
                };
                spans.push(Span::styled(
                    format!("/! {}{repeats}", attack.damage),
                    theme::hurt().bold(),
                ));
                if attack.repeats > 1 {
                    spans.push(Span::styled(
                        format!(
                            " ({})",
                            attack.damage
                                * num_traits::cast(attack.repeats)
                                    .expect("attack repeats fit the game-number type")
                        ),
                        theme::dim(),
                    ));
                }
            }
            // Everything that is not an attack is a word, and the game's own
            // word for it is the most honest thing to print.
            None => spans.push(Span::styled(kind_of(&intent.kind), theme::guard().bold())),
        }
    }
    if spans.is_empty() {
        spans.push(Span::styled("??", theme::dim()));
    }

    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let move_name = names::move_id(&spelling(&next.id));
    if used + move_name.chars().count() + 2 <= width {
        let gap = width - used - move_name.chars().count();
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::styled(move_name, theme::dim()));
    }
    Line::from(spans)
}

/// `AttackIntent` reads *Attack*: the suffix is the column it sits in.
fn kind_of<T: serde::Serialize + std::fmt::Debug>(kind: &T) -> String {
    let spelled = spelling(kind);
    let stem = spelled
        .strip_suffix("_INTENT")
        .or_else(|| spelled.strip_suffix("Intent"))
        .unwrap_or(&spelled);
    names::label(stem)
}

/// One power, as a chip, coloured the way the game classes it.
///
/// Uses the game.s power table rather than a hardcoded list of debuffs. Powers
/// marked `dynamic` decide at runtime and remain neutral here.
fn powers<V: ContentVersion>(creature: &Creature<V>, width: usize) -> Line<'static> {
    let made = creature
        .powers
        .iter()
        .map(|stack| {
            let name = names::power(&spelling(&stack.power));
            match sts2_core::PowerTable::debuff(stack.power) {
                Some(bad) => widget::chip(&name, stack.amount, !bad),
                None => widget::unknown_chip(&name, stack.amount),
            }
        })
        .collect();
    widget::chips(made, width)
}

// ---------------------------------------------------------------------------
// You.
// ---------------------------------------------------------------------------

fn you_row<V: ContentVersion>(frame: &mut Frame, area: Rect, combat: &Combat<V>) {
    let block = titled("you");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let Some(you) = combat.creatures.iter().find(|c| is_player(c)) else {
        return;
    };

    let mut first = widget::vitals(you.hp, you.max_hp, you.block, 16).spans;
    first.push(Span::raw("    "));
    first.push(Span::styled(
        format!("energy {}", combat.energy),
        theme::gold().bold(),
    ));
    first.push(Span::raw("    "));
    first.push(Span::styled(format!("turn {}", combat.turn), theme::dim()));
    if let Some(orbs) = &combat.orbs {
        first.push(Span::raw("    "));
        first.push(Span::styled(
            orbs.queue
                .iter()
                .map(|o| format!("({}:{})", names::pretty(&spelling(&o.orb)), o.passive))
                .chain(std::iter::repeat_n("( )".to_string(), orbs.free()))
                .collect::<Vec<_>>()
                .join(" "),
            theme::guard(),
        ));
    }

    let lines = vec![Line::from(first), powers(you, inner.width as usize)];
    frame.render_widget(Paragraph::new(lines), inner);
}

// ---------------------------------------------------------------------------
// The hand.
// ---------------------------------------------------------------------------

fn hand_row<V, G>(frame: &mut Frame, area: Rect, app: &App<V, G>, combat: &Combat<V>)
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let block = titled(&format!("hand ({})", combat.legal.len()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if combat.legal.is_empty() || inner.height == 0 {
        if combat.legal.is_empty() && inner.height > 0 {
            frame.render_widget(
                Paragraph::new(Line::styled("no cards — e to end the turn", theme::dim())),
                inner,
            );
        }
        return;
    }

    let each = card::width(inner.width, combat.legal.len());
    let fits = (inner.width / each.max(1)) as usize;
    // A hand wider than the terminal scrolls to keep the cursor in view, rather
    // than shrinking the boxes until nothing in them can be read.
    let first = match combat.legal.len() <= fits {
        true => 0,
        false => app
            .ui
            .at
            .saturating_sub(fits.saturating_sub(1))
            .min(combat.legal.len().saturating_sub(fits)),
    };

    let aimed = app.aimed_at().filter(|_| app.drinking().is_none());
    for slot in 0..fits.min(combat.legal.len() - first) {
        let at = first + slot;
        let area = Rect {
            x: inner.x + (slot as u16) * each,
            y: inner.y,
            width: each,
            height: inner.height,
        };
        // Only the card being aimed is shown against the aimed creature: the
        // others are not pointed at anything.
        let at_it = aimed.filter(|_| at == app.ui.at);
        let mut face = card::Face::playable(&combat.legal[at], at, at_it);
        face.picked = at == app.ui.at;
        // While a creature is being picked, the card being aimed keeps the
        // highlight and nothing else competes for it.
        if aimed.is_some() && at != app.ui.at {
            face.faded = true;
        }
        card::draw(frame, area, &face);
    }
}

fn pile_bar<V: ContentVersion>(frame: &mut Frame, area: Rect, combat: &Combat<V>) {
    let spans = vec![
        Span::raw(" "),
        Span::styled("draw ", theme::dim()),
        Span::raw(combat.draw.len().to_string()),
        Span::styled("   discard ", theme::dim()),
        Span::raw(combat.discard.len().to_string()),
        Span::styled("   exhaust ", theme::dim()),
        Span::raw(combat.exhaust.len().to_string()),
    ];
    frame.render_widget(Line::from(spans), area);
}

/// A creature the player *is*, rather than one on the player's side.
///
/// Side alone is insufficient because a charmed monster can fight for you. A
/// missing model is also insufficient because real players have character model
/// ids. The model category identifies the player.
fn is_player<V: ContentVersion>(creature: &Creature<V>) -> bool {
    match &creature.model {
        Some(model) => matches!(model, sts2_core::ModelId::Character(_)),
        // A creature that is neither a monster nor a player. Nothing in this
        // build produces one, and if one appears it is not the player.
        None => false,
    }
}

#[cfg(test)]
mod tests {
    /// The bar the vitals line draws has to leave room for the numbers beside
    /// it at the narrowest panel a hand can produce.
    #[test]
    fn a_narrow_panel_still_has_a_bar() {
        let width = 18usize.saturating_sub(16).clamp(4, 18);
        assert_eq!(super::widget::bar(3, 10, width).chars().count(), 4);
    }
}
