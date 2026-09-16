//! Cards, drawn as cards.
//!
//! ```text
//! ┌(1)──────────┐   the cost, in the corner the game puts it in
//! │ Strike      │   the name
//! │ ✗ no energy │   why it will not go, when it will not
//! │ Deal 9      │   what it would do *now* — the game's own numbers,
//! │ damage.     │   recomputed against this board and coloured where
//! └─────[1]─────┘   the game recoloured them
//! ```
//!
//! A hand read as a list of ids is a hand nobody reads. The box is what makes
//! the row scannable. The eye lands on the shape, and the cost is always in the
//! same corner. The number that matters is the one the game just worked out,
//! rather than the one printed on the card.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use sts2_core::CardTable as _;
use sts2_core::Cost;
use sts2_core::{CardIdent, CombatId, Playable};

use crate::render::{self, Pool, spelling};
use crate::tui::names;
use crate::tui::theme;

use super::widget;

/// How wide each of `n` boxes is, across `cols` columns.
///
/// Floored at twelve, because a box narrower than that holds no words. Capped at
/// twenty-two, because a hand of two is not two billboards.
pub fn width(cols: u16, n: usize) -> u16 {
    let each = cols.saturating_sub(1) / (n.max(1) as u16);
    each.clamp(12, 22)
}

/// How tall a box is, given the rows the hand has to itself.
pub fn height(rows: u16) -> u16 {
    match rows {
        0..=6 => rows,
        7..=8 => 7,
        _ => 8,
    }
}

/// A card's identity, spelled the way every screen spells it.
///
/// `Bash+1`, the name and then what has been done to it. Anything attached is
/// shown in full rather than as a mark, because a player who has never seen the
/// affliction cannot look up a symbol.
pub fn ident<V: sts2_content_wire::WireContentVersion>(card: &CardIdent<V>) -> String {
    let mut out = names::pretty(&spelling(&card.card));
    if card.upgrade > 0 {
        out.push_str(&format!("+{}", card.upgrade));
    }
    if let Some(affliction) = &card.affliction {
        out.push_str(&format!(" ({})", names::pretty(&spelling(affliction))));
    }
    if let Some(enchantment) = &card.enchantment {
        out.push_str(&format!(" ({})", names::pretty(&spelling(enchantment))));
    }
    out
}

/// What to draw in one box.
pub struct Face {
    pub cost: Option<String>,
    pub name: String,
    /// The line under the name: a refusal, a price, a count. Empty for none.
    pub note: Option<(String, ratatui::style::Style)>,
    pub rules: Vec<Line<'static>>,
    /// Drawn grey: unplayable, unaffordable, already taken.
    pub faded: bool,
    /// The hotkey in the bottom border.
    pub key: Option<usize>,
    pub picked: bool,
    /// Chosen, in a selection that takes several.
    pub marked: bool,
}

/// The two pools a card is priced in, or nothing when it is priced in neither.
///
/// Two values rather than a sum. Energy and stars are separate pools, spent
/// separately and refused separately, and `1+2★` read as arithmetic on one of
/// them. A card priced in only one shows only that one.
/// No mark on the energy amount. Its position on the box says which pool it is.
fn cost(energy: Cost, stars: Cost) -> Option<String> {
    let spelled: Vec<String> = render::pools(energy, stars)
        .map(|(amount, pool)| match pool {
            Pool::Energy => amount.to_string(),
            Pool::Stars => format!("{amount}★"),
        })
        .collect();

    (!spelled.is_empty()).then(|| spelled.join(" "))
}

impl Face {
    /// One card in hand, with everything the game currently says about it.
    ///
    /// `aimed` is the creature it is currently pointed at, which decides which
    /// of the previews is shown. The game computes one per creature the card
    /// reaches, because that is where the numbers differ.
    pub fn playable<V: sts2_content_wire::WireContentVersion>(
        card: &Playable<V>,
        at: usize,
        aimed: Option<CombatId>,
    ) -> Face {
        let unplayable = card.reason.map(|reason| {
            // A card can fail several rules at once, and the set is never empty
            // when it is present. All of them are shown: "not enough energy"
            // and "something says no" are two different turns.
            let why = crate::render::flags(reason)
                .split(", ")
                .map(names::label)
                .collect::<Vec<_>>()
                .join(", ");
            (format!("x {why}"), theme::refused())
        });

        Face {
            cost: cost(card.cost, card.star_cost),
            name: ident(&card.card),
            note: unplayable,
            // While a creature is being aimed at, that creature's own preview:
            // the whole reason aiming is compulsory for a targeted card is that
            // this is where the real number is.
            //
            // At rest, the card as printed. A targeted attack has no untargeted
            // preview — the game computes one per creature it may hit — so
            // taking the first would put one enemy's number on a card pointed
            // at nobody, which reads as a promise: Strike says 9 because the
            // slime on the left is Vulnerable, and 6 is what the other two
            // would take. The preview whose variables still equal the card's
            // printed ones is the honest one to show instead.
            rules: aimed
                .and_then(|id| card.effect.iter().find(|p| p.target == Some(id)))
                .or_else(|| card.effect.iter().find(|p| p.target.is_none()))
                .or_else(|| card.effect.iter().find(|p| p.vars == p.base))
                .or_else(|| card.effect.first())
                .map(|p| widget::words(p.text.as_deref().unwrap_or_default()))
                .unwrap_or_default(),
            faded: card.reason.is_some(),
            key: Some(at),
            picked: false,
            marked: false,
        }
    }

    /// Something that is not a card, in a list of cards: gold, a relic.
    pub fn named(name: String) -> Face {
        Face {
            cost: None,
            name,
            note: None,
            rules: Vec::new(),
            faded: false,
            key: None,
            picked: false,
            marked: false,
        }
    }

    /// A card with the printed cost shown by a reward screen.
    pub fn plain_with_cost<V: sts2_content_wire::WireContentVersion>(
        card: &CardIdent<V>,
        energy: Cost,
        stars: Cost,
    ) -> Face {
        let mut face = Self::plain(card);
        face.cost = cost(energy, stars);
        face
    }

    /// A card priced at what its own table prints.
    ///
    /// For a reward, where no fight has had a say and the game reports no
    /// price. An upgrade moves a price, and no reward offers an upgraded card.
    pub fn printed<V: sts2_content_wire::WireContentVersion>(card: &CardIdent<V>) -> Face {
        Self::plain_with_cost(card, card.card.cost(), card.card.star_cost())
    }

    /// A card with nothing computed about it: a pile listing, a reward.
    pub fn plain<V: sts2_content_wire::WireContentVersion>(card: &CardIdent<V>) -> Face {
        Face {
            cost: None,
            name: ident(card),
            note: None,
            rules: Vec::new(),
            faded: false,
            key: None,
            picked: false,
            marked: false,
        }
    }
}

/// Draws one box, filling `area` exactly.
pub fn draw(frame: &mut Frame, area: Rect, face: &Face) {
    if area.width < 6 || area.height < 3 {
        return;
    }

    let border = match (face.picked, face.faded) {
        (true, _) => theme::cursor(),
        (false, true) => theme::dim(),
        (false, false) => theme::TEXT,
    };

    let mut block = Block::bordered().border_style(border);
    if let Some(cost) = &face.cost {
        block = block.title(Span::styled(format!("({cost})"), theme::gold()));
    }
    // One-based, so the number in the border is the digit that presses it —
    // and nothing past the ninth carries one, because `Nth` only goes to 9 and
    // a `[10]` nobody can press is a border telling a lie. The arrows still
    // reach them.
    let mark = match (face.key, face.marked) {
        (Some(key), true) if key < 9 => Some(format!("[*{}]", key + 1)),
        (Some(key), false) if key < 9 => Some(format!("[{}]", key + 1)),
        (Some(_), true) => Some("[*]".to_string()),
        _ => None,
    };
    if let Some(mark) = mark {
        let style = match face.marked {
            true => theme::better().bold(),
            false => ratatui::style::Style::new().bold(),
        };
        block = block.title_bottom(Line::from(Span::styled(mark, style)).centered());
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let name = match face.faded {
        true => Span::styled(face.name.clone(), theme::dim()),
        false => Span::styled(face.name.clone(), theme::heading()),
    };

    let mut lines = vec![Line::from(name)];
    if let Some((note, style)) = &face.note {
        lines.push(Line::from(Span::styled(note.clone(), *style)));
    }
    for line in &face.rules {
        lines.push(match face.faded {
            true => Line::from(
                line.spans
                    .iter()
                    .map(|s| Span::styled(s.content.clone(), theme::dim()))
                    .collect::<Vec<_>>(),
            ),
            false => line.clone(),
        });
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A card box says what the card is priced at, in whichever pools it is
    /// priced in.
    #[test]
    fn each_pool_is_shown_only_when_the_card_is_priced_in_it() {
        // The ordinary case: energy alone. Every card outside the Regent's has
        // no star cost at all, so a bare number is what almost every box says.
        assert_eq!(cost(Cost::fixed(1), Cost::None).as_deref(), Some("1"));
        assert_eq!(cost(Cost::fixed(0), Cost::None).as_deref(), Some("0"));

        // Stars alone, and both. Separated by a space rather than a `+`,
        // because they are two pools and not one sum.
        assert_eq!(cost(Cost::None, Cost::fixed(2)).as_deref(), Some("2★"));
        assert_eq!(
            cost(Cost::fixed(1), Cost::fixed(2)).as_deref(),
            Some("1 2★")
        );

        // X spends the pool rather than a number of it, and the number the
        // wire carries for such a card is zero. `(0)` would be a lie.
        assert_eq!(cost(Cost::X, Cost::None).as_deref(), Some("X"));
        assert_eq!(cost(Cost::None, Cost::X).as_deref(), Some("X★"));

        // A curse is priced in neither, and gets no title at all rather than
        // the `-1` the game spells that with.
        assert_eq!(cost(Cost::None, Cost::None), None);
    }

    #[test]
    fn boxes_shrink_to_fit_and_stop() {
        assert_eq!(width(80, 5), 15);
        // Ten cards in eighty columns: as narrow as a box goes, and no
        // narrower — the row scrolls rather than the words vanishing.
        assert_eq!(width(80, 10), 12);
        // Two cards do not become two billboards.
        assert_eq!(width(200, 2), 22);
    }

    #[test]
    fn a_box_is_never_taller_than_the_room_for_it() {
        assert_eq!(height(4), 4);
        assert_eq!(height(40), 8);
    }
}
