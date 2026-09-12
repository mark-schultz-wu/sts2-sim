//! One verb for "pick the numbered thing", whichever mechanism the game wants.
//!
//! The protocol has three mechanisms for numbered options: room choices, card
//! selections, and reward screens. The game state selects the mechanism, not
//! the user.s intent.
//!
//! Dispatch is unambiguous because a pending question permits only `answer`,
//! `observe`, and `quit`. Explicit commands remain available to scripts that
//! need a wrong state to fail rather than choose whatever is present.

use super::Taking;
use anyhow::{Result, bail};
use sts2_core::{Answer as GameAnswer, Decision, Question, Room, Snapshot};

/// What the game is currently waiting to be told.
pub enum Awaiting {
    Cards,
    Rewards,
    Divine,
    RoomOption,
    ShopEntry,
}

/// Every option `choose_option` accepts in this room, in the room's order.
///
/// Uses one game rule per room kind. `Protocol.cs` rejects a disabled rest-site
/// option, an answered chest, and a locked or chosen event option. Typed rooms
/// make those rules independent of absent JSON keys.
///
/// Shops use [`buyable`](self::buyable) and a different command. The renderer
/// shares this function, so its advice cannot drift from the action.
pub(crate) fn choosable<V: sts2_content_wire::WireContentVersion>(room: &Room<V>) -> Vec<u8> {
    match room {
        Room::RestSite { options } => options
            .iter()
            .filter(|o| o.enabled)
            .map(|o| o.index)
            .collect(),

        // `finished` is the chest's answer and every option repeats it, so one
        // answered option means the chest is done.
        Room::Treasure { options, .. } => options
            .iter()
            .filter(|o| !o.chosen)
            .map(|o| o.index)
            .collect(),

        Room::Event { options, .. } => options
            .iter()
            .filter(|o| !o.locked && !o.chosen)
            .map(|o| o.index)
            .collect(),

        Room::Shop { .. } => Vec::new(),
    }
}

/// Every entry `buy` accepts in this room, in the shop's order.
///
/// All three tests matter and none implies the others. A sold-out entry is gone.
/// One you cannot afford is still there to come back for. A potion offered to a
/// full belt is stocked, affordable, and refused. That last one was found by
/// playing at random: the shop said yes twice and the game said no.
pub(crate) fn buyable<V: sts2_content_wire::WireContentVersion>(room: &Room<V>) -> Vec<u8> {
    match room {
        Room::Shop { entries } => entries
            .iter()
            .filter(|e| e.stocked && e.affordable && e.blocked.is_none())
            .map(|e| e.index)
            .collect(),
        _ => Vec::new(),
    }
}

/// Reads one reply and says which mechanism a choice goes through.
pub fn awaiting<V: sts2_content_wire::WireContentVersion>(
    standing: &Snapshot<V>,
) -> Result<Awaiting> {
    if let Some(question) = &standing.asking {
        return Ok(match question {
            Question::SelectCards { .. } => Awaiting::Cards,
            Question::Rewards { .. } => Awaiting::Rewards,
            Question::Divine { .. } => Awaiting::Divine,
        });
    }

    // A room offers one or the other: a list to choose from, or a shop's stock,
    // which is bought rather than chosen — a different command for what reads
    // as the same act.
    let room = standing.observation.room.as_ref();
    if room.is_some_and(|r| !choosable(r).is_empty()) {
        return Ok(Awaiting::RoomOption);
    }
    if room.is_some_and(|r| !buyable(r).is_empty()) {
        return Ok(Awaiting::ShopEntry);
    }

    bail!("nothing is waiting to be chosen. `sts2 show observe` shows where the run is")
}

/// The command that takes those indices, given what the game is waiting for.
///
/// Indices arrive as [`Taking`] because a reward screen needs the two-level
/// `N.M` form and the other two do not. A card selection or a room option given
/// a dotted index is a misunderstanding worth naming: nothing there has an
/// inside.
pub fn command<V: sts2_content_wire::WireContentVersion>(
    awaiting: &Awaiting,
    indices: &[Taking],
) -> Result<Decision<V>> {
    let flat = |what: &str| -> Result<Vec<usize>> {
        indices
            .iter()
            .map(|t| match t.card() {
                Some(_) => bail!("{what} is a plain index; `N.M` is only for a card reward"),
                None => Ok(t.reward()),
            })
            .collect()
    };

    Ok(match awaiting {
        Awaiting::Cards => Decision::Answer(GameAnswer::Cards(flat("a card selection")?)),
        Awaiting::Rewards => {
            Decision::Answer(GameAnswer::Take(indices.iter().map(Taking::take).collect()))
        }
        // A cell is a pair and a tool, which no list of indices spells. Said
        // rather than guessed: taking the first two numbers as `x` and `y` would
        // make `choose 3 4` a divination with a tool nobody named.
        Awaiting::Divine => {
            bail!("a divination names a cell and a tool: `sts2 act answer divine X Y [--big]`")
        }
        Awaiting::RoomOption => match flat("a room option")?.as_slice() {
            [index] => Decision::ChooseOption {
                index: u8::try_from(*index)?,
            },
            // A room takes one option and then stops being a room with options,
            // so more than one is a misunderstanding worth naming rather than
            // silently taking the first.
            _ => bail!(
                "a room option is taken one at a time, but {} were given",
                indices.len()
            ),
        },
        // One at a time for the same reason, and a sharper one: buying changes
        // what you can afford, so a second index chosen before the first was
        // paid for is a decision made against stale gold.
        Awaiting::ShopEntry => match flat("a shop entry")?.as_slice() {
            [index] => Decision::Buy {
                index: u8::try_from(*index)?,
            },
            _ => bail!(
                "a shop entry is bought one at a time, but {} were given",
                indices.len()
            ),
        },
    })
}

/// What this dispatched to, for saying so afterwards.
pub fn describe(awaiting: &Awaiting) -> &'static str {
    match awaiting {
        Awaiting::Cards => "answer cards",
        Awaiting::Rewards => "answer take",
        Awaiting::Divine => "answer divine",
        Awaiting::RoomOption => "choose-option",
        Awaiting::ShopEntry => "buy",
    }
}
