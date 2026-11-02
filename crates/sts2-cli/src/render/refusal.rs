//! A refused move, in words.
//!
//! `sts2-core` carries the refusal as data and stops there. Comparing two
//! implementations is [`PartialEq`] on a [`Reason`]. Prose there means that
//! crate knowing what a build calls a card, which it does not know and has no
//! business knowing.
//!
//! So the English is here, where the naming already is. It is written from the
//! fields rather than parsed out of a message, which is the property the typed
//! reason bought. A simulator refusing the same move for the same reason gets
//! the same sentence without having to write one.

use super::{entry, flags, ident, spelling};
use std::fmt;
use sts2_core::{Blocked, Reason, Refusal};

/// What was refused, and why.
pub fn refusal<V: sts2_content_wire::WireContentVersion>(refused: &Refusal<V>) -> String {
    format!("refused {}: {}", refused.what, reason(&refused.reason))
}

/// Why, on its own.
pub fn reason<V: sts2_content_wire::WireContentVersion>(why: &Reason<V>) -> String {
    Prose(why).to_string()
}

/// A wrapper because both `Reason` and `Display` belong to somebody else.
struct Prose<'a, V: sts2_content_wire::WireContentVersion>(&'a Reason<V>);

impl<V: sts2_content_wire::WireContentVersion> fmt::Display for Prose<'_, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /// A room by the name its build gives it, or `nowhere`.
        fn room<T: serde::Serialize + fmt::Debug>(kind: &Option<T>) -> String {
            match kind {
                Some(kind) => spelling(kind),
                None => "nowhere".to_string(),
            }
        }

        match self.0 {
            Reason::NoRun => f.write_str("no run is in progress; start one first"),
            Reason::RunOver { victory } => write!(
                f,
                "the run is over ({}); start a new run",
                if *victory { "victory" } else { "defeat" }
            ),
            Reason::NotInCombat => f.write_str("not in combat"),
            Reason::InCombat => f.write_str("cannot be done during combat"),
            Reason::WrongRoom { wanted, found } => write!(
                f,
                "this needs a {} and the run is in {}",
                spelling(wanted),
                room(found)
            ),
            Reason::NothingToChoose { found } => {
                write!(f, "{} offers nothing to choose", room(found))
            }
            Reason::Pending { kind } => {
                write!(f, "a {kind} decision is pending; answer it first")
            }
            Reason::NothingPending => f.write_str("nothing is pending"),

            Reason::OutOfRange { index, count } => {
                write!(f, "index {index} out of range; {count} offered")
            }
            Reason::NoSuchCreature { id } => write!(f, "no creature with id {id}"),

            Reason::Unplayable {
                card,
                why,
                preventer,
            } => {
                write!(f, "{} is not playable: {}", ident(card), flags(*why))?;
                match preventer {
                    Some(model) => write!(f, " (blocked by {})", entry(model)),
                    None => Ok(()),
                }
            }
            Reason::WrongTarget {
                card,
                target,
                takes,
            } => {
                let at = match target {
                    Some(id) => format!("creature {id}"),
                    None => "nothing".to_string(),
                };
                write!(
                    f,
                    "{} cannot target {at}; it targets {}",
                    ident(card),
                    spelling(takes)
                )
            }
            Reason::Undrinkable { potion, why } => {
                write!(f, "{} cannot be drunk: {}", spelling(potion), spelling(why))
            }
            Reason::WrongPotionTarget {
                potion,
                target,
                takes,
            } => {
                let at = match target {
                    Some(id) => format!("creature {id}"),
                    None => "nothing".to_string(),
                };
                write!(
                    f,
                    "{} cannot target {at}; it targets {}",
                    spelling(potion),
                    spelling(takes)
                )
            }
            Reason::NotTravelable { at } => write!(f, "{at} is not travelable from here"),
            Reason::CannotProceed => {
                f.write_str("nothing to proceed from; this is for leaving a finished boss room")
            }
            Reason::OptionDisabled => f.write_str("that option is disabled"),
            Reason::WrongOptionTarget { target } => match target {
                Some(seat) => write!(f, "that option cannot be aimed at seat {}", seat.0),
                None => f.write_str("that option needs another player to aim at"),
            },
            Reason::OptionLocked => f.write_str("that option is locked"),
            Reason::OptionWouldKill => f.write_str("that option would kill you"),
            Reason::AlreadyAnswered => f.write_str("this chest has already been answered"),
            Reason::Unpurchasable {
                index,
                cost,
                gold,
                stocked,
                blocked,
            } => {
                write!(f, "entry {index} will not sell: cost {cost}, gold {gold}")?;
                if !stocked {
                    f.write_str(", out of stock")?;
                }
                match blocked {
                    Some(Blocked::NoPotionSlot) => f.write_str(", no potion slot"),
                    None => Ok(()),
                }
            }

            Reason::Duplicate { index } => write!(f, "index {index} given twice"),
            Reason::WrongCount { chosen, min, max } => {
                write!(f, "chose {chosen}, but this decision takes {min}..{max}")
            }
            Reason::NothingPicked => f.write_str("no card is picked to take back"),
            Reason::NotSkippable => f.write_str("this reward set cannot be skipped"),
            Reason::AlreadyTaken { index } => {
                write!(f, "reward {index} has already been taken")
            }
            Reason::NeedsCardChoice { index, count } => write!(
                f,
                "reward {index} is a card reward and needs a card in 0..{}",
                count.saturating_sub(1)
            ),
            Reason::EmptyTake => f.write_str("took nothing; skip to leave the rest"),
            Reason::OffGrid {
                x,
                y,
                width,
                height,
            } => write!(f, "({x},{y}) is off a {width}x{height} grid"),
            Reason::AlreadyUncovered { x, y } => write!(f, "({x},{y}) is already uncovered"),

            Reason::Console { message } => f.write_str(message),
            Reason::Malformed { message } => f.write_str(message),
        }
    }
}
