//! One verb for "pick the numbered thing", whichever mechanism the game wants.
//!
//! The protocol has three ways to take a numbered option — `choose_option` for a
//! room's own list, `answer` with cards for a selection, `answer` with take for a
//! reward screen — and which applies is a property of the game's state, not of
//! what the user meant. Asking someone to work that out is asking them to learn
//! the protocol to press a button.
//!
//! The dispatch is unambiguous because the protocol makes it so: a pending
//! question blocks every command except `answer`, `observe` and `quit`, so there
//! is never a moment when both a pending and a room option could be meant.
//!
//! The explicit commands stay. This is the convenience for a person reading
//! output and typing the next thing; a script should say `answer cards 0` and
//! fail loudly if the game was not asking that, rather than silently doing
//! whatever was in front of it.

use super::Taking;
use anyhow::{Result, bail};
use sts2_content::ContentVersion;
use sts2_headless::{Answer as GameAnswer, Command, Question, Response};

/// What the game is currently waiting to be told.
pub enum Awaiting {
    Cards,
    Rewards,
    RoomOption,
}

/// Reads one observation and says which mechanism a choice would go through.
pub fn awaiting(response: &Response) -> Result<Awaiting> {
    match response {
        Response::Pending { question, .. } => Ok(match question {
            Question::SelectCards { .. } => Awaiting::Cards,
            Question::Rewards { .. } => Awaiting::Rewards,
        }),
        Response::Observation { state, .. } => {
            let choosable = state
                .get("room")
                .and_then(|r| r.get("options"))
                .and_then(|o| o.as_array())
                .is_some_and(|options| {
                    options.iter().any(|o| {
                        o.get("chosen") != Some(&serde_json::Value::Bool(true))
                            && o.get("locked") != Some(&serde_json::Value::Bool(true))
                    })
                });

            match choosable {
                true => Ok(Awaiting::RoomOption),
                false => {
                    bail!("nothing is waiting to be chosen. `sts2 observe` shows where the run is")
                }
            }
        }
        Response::Bye => bail!("the game is shutting down"),
    }
}

/// The command that takes those indices, given what the game is waiting for.
///
/// Indices arrive as [`Taking`] because a reward screen needs the two-level
/// `N.M` form and the other two do not. A card selection or a room option given
/// a dotted index is a misunderstanding worth naming: nothing there has an
/// inside.
pub fn command<V: ContentVersion>(awaiting: &Awaiting, indices: &[Taking]) -> Result<Command<V>> {
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
        Awaiting::Cards => Command::Answer(GameAnswer::Cards(flat("a card selection")?)),
        Awaiting::Rewards => {
            Command::Answer(GameAnswer::Take(indices.iter().map(Taking::take).collect()))
        }
        Awaiting::RoomOption => match flat("a room option")?.as_slice() {
            [index] => Command::ChooseOption {
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
    })
}

/// What this dispatched to, for saying so afterwards.
pub fn describe(awaiting: &Awaiting) -> &'static str {
    match awaiting {
        Awaiting::Cards => "answer cards",
        Awaiting::Rewards => "answer take",
        Awaiting::RoomOption => "choose-option",
    }
}
