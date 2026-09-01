//! The command surface, and how it becomes a game command.
//! Ids are named the way the game names them, `IRONCLAD`, `BOWLBUGS_WEAK`, `DeadlyEnemies`, because that is the serde rename on every generated type. Therefore parsing is one `serde_json::from_value` rather than a table of aliases that will drift.

pub mod choose;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use nonmax::NonMaxU32;
use serde::de::DeserializeOwned;
use sts2_content::ContentVersion;
use sts2_headless::{Answer as GameAnswer, Command, TakeReward};
use sts2_rng::Seed;

/// Parses an id from the game's own spelling of it.
fn id<T: DeserializeOwned>(what: &str, spelling: &str) -> Result<T> {
    let article = match what.starts_with(['a', 'e', 'i', 'o', 'u']) {
        true => "an",
        false => "a",
    };

    serde_json::from_value(serde_json::Value::String(spelling.to_string()))
        .with_context(|| format!("{spelling:?} is not {article} {what} this build has"))
}

#[derive(Debug, Args)]
pub struct StartArgs {
    /// The run's seed. Folded the way the game folds it, so `oracle1` and `ORACLE1` are one run, and both are `0RACLE1`, since the alphabet has no `O`.
    pub seed: String,
    /// A character id, such as `IRONCLAD`.
    pub character: String,
    /// An ascension level by name, such as `DeadlyEnemies`. Defaults to none.
    #[arg(long)]
    pub ascension: Option<String>,
    /// Modifier ids, such as `DRAFT`.
    #[arg(long, value_delimiter = ',')]
    pub modifiers: Vec<String>,
}

/// One reward being taken: which reward, and which card inside it.
///
/// Spelled `N` or `N.M`. The dot is doing real work, a card reward holds several cards and the game will not pick one for you. Therefore `0` and `0.1` are different requests rather than one with a default.
#[derive(Debug, Clone, Copy)]
pub struct Taking {
    reward: usize,
    card: Option<usize>,
}

impl Taking {
    pub fn reward(&self) -> usize {
        self.reward
    }

    pub fn card(&self) -> Option<usize> {
        self.card
    }

    pub(crate) fn take(&self) -> TakeReward {
        TakeReward {
            index: self.reward,
            card: self.card,
        }
    }
}

impl std::str::FromStr for Taking {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let number = |part: &str, what: &str| {
            part.parse::<usize>()
                .map_err(|_| format!("{part:?} is not {what}, in {s:?}"))
        };

        Ok(match s.split_once('.') {
            Some((reward, card)) => Taking {
                reward: number(reward, "a reward index")?,
                card: Some(number(card, "a card index")?),
            },
            None => Taking {
                reward: number(s, "a reward index")?,
                card: None,
            },
        })
    }
}

/// Which shape of answer a pending question wants.
#[derive(Debug, Subcommand)]
pub enum Answering {
    /// Picks options from a card selection, by index.
    Cards {
        #[arg(value_delimiter = ' ', num_args = 0..)]
        cards: Vec<usize>,
    },
    /// Declines a reward screen. Refused if it is not skippable.
    Skip,
    /// Takes rewards from a reward screen.
    ///
    /// A reward is `N`, and a card inside a card reward is `N.M`, `take 0.1` is the second card of the first reward. The game refuses a card reward taken without a card rather than choosing one, so the pair is not optional there.
    Take {
        #[arg(value_delimiter = ' ', num_args = 1..)]
        rewards: Vec<Taking>,
    },
}

#[derive(Debug, Subcommand)]
pub enum Play {
    /// Begins a run, launching the game if it is not already up.
    Start(StartArgs),
    /// Jumps straight into a named encounter, off the map.
    Fight {
        /// An encounter id, such as `BOWLBUGS_WEAK`.
        encounter: String,
    },
    /// Moves to a map point.
    Travel { col: i32, row: i32 },
    /// Enters a room by type, such as `Shop`.
    EnterRoom { room: String },
    /// Plays a card from hand.
    PlayCard {
        /// Index into the hand, from the last observation.
        index: u8,
        /// A creature's combat id.
        #[arg(long)]
        target: Option<u32>,
    },
    /// Ends the player's turn.
    EndTurn,
    /// Takes an offered option by index.
    ChooseOption { index: u8 },
    /// Buys the shop item at an index.
    Buy { index: u8 },
    /// Answers the pending question.
    ///
    /// One command with three shapes, because that is what the protocol has. The pending question selects the shape, which the observation prints beside it.
    #[command(subcommand)]
    Answer(Answering),
    /// Puts a card or relic into the run, for constructing a scenario.
    Grant {
        #[arg(long)]
        card: Option<String>,
        #[arg(long)]
        relic: Option<String>,
    },
    /// Asks for the current state without changing anything.
    Observe,
}

impl Play {
    /// The game command this asks for.
    pub fn command<V: ContentVersion>(&self) -> Result<Command<V>> {
        Ok(match self {
            Play::Start(a) => Command::Start {
                seed: Seed::new(&a.seed).context("that is not a seed")?,
                character: id("character", &a.character)?,
                ascension: match &a.ascension {
                    Some(level) => id("ascension level", level)?,
                    None => id("ascension level", "None")?,
                },
                modifiers: a
                    .modifiers
                    .iter()
                    .map(|m| id("modifier", m))
                    .collect::<Result<_>>()?,
            },
            Play::Fight { encounter } => Command::Fight {
                encounter: id("encounter", encounter)?,
            },
            Play::Travel { col, row } => Command::Travel {
                col: *col,
                row: *row,
            },
            Play::EnterRoom { room } => Command::EnterRoom {
                room: id("room type", room)?,
            },
            Play::PlayCard { index, target } => Command::PlayCard {
                index: *index,
                target: match target {
                    Some(t) => Some(NonMaxU32::new(*t).context("u32::MAX is not a creature")?),
                    None => None,
                },
            },
            Play::EndTurn => Command::EndTurn,
            Play::ChooseOption { index } => Command::ChooseOption { index: *index },
            Play::Buy { index } => Command::Buy { index: *index },
            Play::Answer(answering) => Command::Answer(match answering {
                Answering::Cards { cards } => GameAnswer::Cards(cards.clone()),
                Answering::Skip => GameAnswer::Skip,
                Answering::Take { rewards } => {
                    GameAnswer::Take(rewards.iter().map(Taking::take).collect())
                }
            }),
            Play::Grant { card, relic } => Command::Grant {
                card: card.as_deref().map(|c| id("card", c)).transpose()?,
                relic: relic.as_deref().map(|r| id("relic", r)).transpose()?,
            },
            Play::Observe => Command::Observe,
        })
    }
}
