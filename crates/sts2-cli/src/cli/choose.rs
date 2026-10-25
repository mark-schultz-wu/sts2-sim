//! One verb for "pick the numbered thing", whichever mechanism the game wants.
//!
//! The protocol has three mechanisms for numbered options: room choices, card
//! selections, and reward screens. The game state selects the mechanism, not
//! the user.s intent.
//!
//! Dispatch is unambiguous because a pending question permits only `answer`,
//! `observe`, and `quit`. Explicit commands remain available to scripts that
//! need a wrong state to fail rather than choose whatever is present.

use super::{Error, Taking};
use sts2_core::{Answer as GameAnswer, Decision, Picks, Question, Room, Snapshot};

/// What the game is currently waiting to be told.
pub enum Awaiting {
    Cards,
    Rewards,
    Divine,
    /// A yes or a no, which `choose 1` and `choose 0` give.
    Confirm,
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
) -> Result<Awaiting, Error> {
    if let Some(question) = &standing.asking {
        return Ok(match question {
            Question::SelectCards { .. } => Awaiting::Cards,
            Question::Rewards { .. } => Awaiting::Rewards,
            Question::Divine { .. } => Awaiting::Divine,
            // One bundle, named as a one-card selection names its card.
            Question::ChooseBundle { .. } => Awaiting::Cards,
            Question::Confirm => Awaiting::Confirm,
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

    Err(Error::NothingWaiting)
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
) -> Result<Decision<V>, Error> {
    let flat = |what: &'static str| -> Result<Vec<usize>, Error> {
        indices
            .iter()
            .map(|t| match t.card() {
                Some(_) => Err(Error::Dotted(what)),
                None => Ok(t.reward()),
            })
            .collect()
    };

    Ok(match awaiting {
        Awaiting::Cards => Decision::Answer(GameAnswer::Cards(Picks::try_from(
            flat("a card selection")?.as_slice(),
        )?)),
        Awaiting::Rewards => match indices {
            [one] => Decision::Answer(GameAnswer::Take(one.take()?)),
            _ => return Err(Error::OneReward),
        },
        // A cell is a pair and a tool, which no list of indices spells. Said
        // rather than guessed: taking the first two numbers as `x` and `y` would
        // make `choose 3 4` a divination with a tool nobody named.
        Awaiting::Divine => return Err(Error::Divination),
        Awaiting::Confirm => match flat("a confirmation")?.as_slice() {
            [0] => Decision::Answer(GameAnswer::Confirm(false)),
            [1] => Decision::Answer(GameAnswer::Confirm(true)),
            _ => return Err(Error::OneConfirmation),
        },
        Awaiting::RoomOption => match flat("a room option")?.as_slice() {
            [index] => Decision::ChooseOption {
                index: u8::try_from(*index).map_err(|_| Error::NoOption(*index))?,
                target: None,
            },
            // A room takes one option and then stops being a room with options,
            // so more than one is a misunderstanding worth naming rather than
            // silently taking the first.
            _ => return Err(Error::OneOption(indices.len())),
        },
        // One at a time for the same reason, and a sharper one: buying changes
        // what you can afford, so a second index chosen before the first was
        // paid for is a decision made against stale gold.
        Awaiting::ShopEntry => match flat("a shop entry")?.as_slice() {
            [index] => Decision::Buy {
                index: u8::try_from(*index).map_err(|_| Error::NoOption(*index))?,
            },
            _ => return Err(Error::OneEntry(indices.len())),
        },
    })
}

/// What this dispatched to, for saying so afterwards.
pub fn describe(awaiting: &Awaiting) -> &'static str {
    match awaiting {
        Awaiting::Cards => "answer cards",
        Awaiting::Rewards => "answer take",
        Awaiting::Divine => "answer divine",
        Awaiting::Confirm => "answer confirm",
        Awaiting::RoomOption => "choose-option",
        Awaiting::ShopEntry => "buy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_core::TakeReward;

    type V = sts2_content::v0_107_1::V0_107_1;

    fn typed(indices: &[&str]) -> Vec<Taking> {
        indices
            .iter()
            .map(|index| index.parse().expect("a well-formed index"))
            .collect()
    }

    /// What someone types is refused with a reason, never a crash. The
    /// selection and the reward are narrower than what the parser reads.
    #[test]
    fn a_choice_nothing_can_hold_is_an_error() {
        let repeated = command::<V>(&Awaiting::Cards, &typed(&["1", "1"]));
        assert!(repeated.is_err(), "the same card twice");
        let distant = command::<V>(&Awaiting::Cards, &typed(&["70000"]));
        assert!(distant.is_err(), "an option past any a selection names");
        let far = command::<V>(&Awaiting::Rewards, &typed(&["300"]));
        assert!(far.is_err(), "a reward past any a screen offers");
        let two = command::<V>(&Awaiting::Rewards, &typed(&["0", "1"]));
        assert!(two.is_err(), "two rewards in one answer");
    }

    #[test]
    fn a_choice_in_reach_is_the_answer_it_names() {
        let picked = command::<V>(&Awaiting::Cards, &typed(&["2", "0"])).expect("in reach");
        assert_eq!(picked, Decision::Answer(GameAnswer::Cards([2, 0].into())));
        let taken = command::<V>(&Awaiting::Rewards, &typed(&["1.2"])).expect("in reach");
        assert_eq!(
            taken,
            Decision::Answer(GameAnswer::Take(TakeReward::Card { index: 1, card: 2 }))
        );
    }
}
