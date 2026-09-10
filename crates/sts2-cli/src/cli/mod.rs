//! The command surface, and how it becomes a game command.
//!
//! Ids are named the way the game names them: `IRONCLAD`, `BOWLBUGS_WEAK`,
//! `DeadlyEnemies`. That is the serde rename on every generated type, so
//! parsing is one `serde_json::from_value` rather than a table of aliases that
//! drifts.

pub mod choose;
pub mod moves;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::Path;
use sts2_core::ContentVersion;
use sts2_core::{
    Answer as GameAnswer, Coord, Decision, Gift, Setup, Start, Step, TakeReward, Tool,
};
use sts2_rng::Seed;

/// An ascension level by the game's own name for it, such as `None`.
///
/// Public because the front end has to name one for a run started anywhere but
/// the command line. `V::AscensionLevel` is an associated type with no default,
/// and there is no way to spell "the lowest one" except by naming it.
pub fn ascension<V: ContentVersion>(spelling: &str) -> Result<V::AscensionLevel> {
    id("ascension level", spelling)
}

/// Parses an id from the game's own spelling of it.
fn id<T: serde::de::DeserializeOwned>(what: &str, spelling: &str) -> Result<T> {
    let article = match what.starts_with(['a', 'e', 'i', 'o', 'u']) {
        true => "an",
        false => "a",
    };

    crate::id::parse(spelling)
        .with_context(|| format!("{spelling:?} is not {article} {what} this build has"))
}

#[derive(Debug, Args)]
pub struct StartArgs {
    /// The run's seed. Folded the way the game folds it, so `oracle1` and
    /// `ORACLE1` are one run. Both are `0RACLE1`, since the alphabet has no
    /// `O`.
    pub seed: String,
    /// A character id, such as `IRONCLAD`.
    pub character: String,
    /// An ascension level by name, such as `DeadlyEnemies`. Defaults to none.
    #[arg(long)]
    pub ascension: Option<String>,
    /// Modifier ids, such as `DRAFT`.
    #[arg(long, value_delimiter = ',')]
    pub modifiers: Vec<String>,
    /// Write the run to this file as it is played, for `sts2 run replay`.
    ///
    /// On `start` rather than on every command, because the daemon does the
    /// writing and only needs telling once. And because a recording that begins
    /// halfway through a run is not a run.
    ///
    /// Omit it and the run is still recorded, under a name of its own in the
    /// state directory. `--no-record` is how to not keep it.
    #[arg(long, value_name = "FILE")]
    pub record: Option<std::path::PathBuf>,
    /// Do not write this run down at all.
    #[arg(long, conflicts_with = "record")]
    pub no_record: bool,
}

/// A creature id as a person types one, read the way an observation spells one.
///
/// A number off a command line is outside data, like a number off the wire, so
/// it comes in the same way. The player is zero and the monsters count from
/// one, so a fight has few enough ids to fit a byte.
fn combat_id(id: u32) -> anyhow::Result<sts2_core::CombatId> {
    serde_json::from_value(serde_json::Value::from(id))
        .with_context(|| format!("{id} is no creature in a fight"))
}

impl StartArgs {
    /// The typed run this command line asks to start.
    pub fn start<V: ContentVersion>(&self) -> Result<Start<V>> {
        Ok(Start {
            seed: Seed::fold(&self.seed).context("that is not a seed")?,
            character: id("character", &self.character)?,
            ascension: match &self.ascension {
                Some(level) => id("ascension level", level)?,
                None => id("ascension level", "None")?,
            },
            modifiers: self
                .modifiers
                .iter()
                .map(|modifier| id("modifier", modifier))
                .collect::<Result<_>>()?,
        })
    }

    /// Where this run gets written, if it does.
    ///
    /// Recording is the default because a valuable run is recognized only
    /// afterwards. It can expose a divergence, an interface problem, or a seed
    /// worth replaying. An unrecorded run cannot be shown to anybody. Each file
    /// is only a few kilobytes.
    pub fn recording(&self) -> Result<Option<std::path::PathBuf>> {
        if self.no_record {
            return Ok(None);
        }
        if let Some(path) = &self.record {
            return Ok(Some(path.clone()));
        }

        let dir = crate::runs::runs_dir();
        std::fs::create_dir_all(&dir).with_context(|| format!("making {}", dir.display()))?;
        free_name(&dir, &self.seed, &self.character).map(Some)
    }
}

/// The first unused `<seed>-<character>.jsonl` under `dir`.
///
/// Named for the run rather than the clock. The filesystem already records when
/// it was played. The seed is folded first, so the name contains what the game
/// used rather than what was typed.
///
/// A repeated seed and character gets a number. Overwriting the first attempt
/// discards the run being compared.
fn free_name(dir: &Path, seed: &str, character: &str) -> Result<std::path::PathBuf> {
    let folded = Seed::fold(seed).map_or_else(|_| seed.to_uppercase(), |s| s.as_str().to_string());
    let stem = format!("{folded}-{}", character.to_uppercase());

    let first = dir.join(format!("{stem}.jsonl"));
    if !first.exists() {
        return Ok(first);
    }

    (2..1000)
        .map(|n| dir.join(format!("{stem}-{n}.jsonl")))
        .find(|path| !path.exists())
        .with_context(|| format!("{stem} already has 999 recordings in {}", dir.display()))
}

/// One reward being taken: which reward, and which card inside it.
///
/// Spelled `N` or `N.M`. The dot is doing real work. A card reward holds
/// several cards and the game will not pick one for you. So `0` and `0.1` are
/// different requests, rather than one with a default.
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
        match self.card {
            Some(card) => TakeReward::Card {
                index: self.reward,
                card,
            },
            None => TakeReward::NonCard { index: self.reward },
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
    /// A reward is `N`, and a card inside a card reward is `N.M`. `take 0.1`
    /// is the second card of the first reward. The game refuses a card reward
    /// taken without a card rather than choosing one, so the pair is not
    /// optional there.
    Take {
        #[arg(value_delimiter = ' ', num_args = 1..)]
        rewards: Vec<Taking>,
    },
    /// Uncovers a cell of the Crystal Sphere's grid.
    ///
    /// Costs one divination whichever tool it uses, so `--big` is a gamble and
    /// not a saving. It uncovers the neighbours too, and one of the things
    /// under them can be a curse.
    Divine {
        x: usize,
        y: usize,
        /// Uncover the neighbouring cells as well.
        #[arg(long)]
        big: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum Play {
    /// Jumps straight into a named encounter, off the map.
    Fight {
        /// An encounter id, such as `BOWLBUGS_WEAK`.
        encounter: String,
    },
    /// Jumps straight to a named event, off the map.
    Event {
        /// An event id, such as `CRYSTAL_SPHERE`.
        event: String,
    },
    /// Moves to a map point.
    Travel { col: u8, row: u8 },
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
    /// Drinks a potion from the belt.
    UsePotion {
        /// Index into the belt, from the last observation.
        index: u8,
        /// A creature's combat id.
        #[arg(long)]
        target: Option<u32>,
    },
    /// Leaves a finished act for the next one.
    ///
    /// The one move the map cannot express. An act's boss has no onward node,
    /// so once it is beaten there is nowhere to travel, and this is the way
    /// out.
    Proceed,
    /// Takes an offered option by index.
    ChooseOption { index: u8 },
    /// Buys the shop item at an index.
    Buy { index: u8 },
    /// Answers the pending question.
    ///
    /// One command with three shapes, because that is what the protocol has.
    /// Which shape applies depends on which question is pending, and the
    /// observation prints that beside it.
    #[command(subcommand)]
    Answer(Answering),
    /// Puts a card, relic or potion into the run, for constructing a scenario.
    ///
    /// One of the three. For more than one, run it again.
    Grant {
        #[arg(
            long,
            conflicts_with_all = ["relic", "potion"],
            required_unless_present_any = ["relic", "potion"],
        )]
        card: Option<String>,
        #[arg(long, conflicts_with = "potion")]
        relic: Option<String>,
        /// Procured the way the game procures one, so a full belt refuses it.
        #[arg(long)]
        potion: Option<String>,
    },
}

impl Play {
    /// The step this asks for.
    pub fn step<V: ContentVersion>(&self) -> Result<Step<V>> {
        Ok(match self {
            Play::Fight { encounter } => Setup::Fight {
                encounter: id("encounter", encounter)?,
            }
            .into(),
            Play::Event { event } => Setup::Event {
                event: id("event", event)?,
            }
            .into(),
            Play::Travel { col, row } => Decision::Travel {
                to: Coord::new(*col, *row),
            }
            .into(),
            Play::EnterRoom { room } => Decision::EnterRoom {
                room: id("room type", room)?,
            }
            .into(),
            Play::PlayCard { index, target } => Decision::PlayCard {
                index: *index,
                target: target.map(combat_id).transpose()?,
            }
            .into(),
            Play::EndTurn => Decision::EndTurn.into(),
            Play::UsePotion { index, target } => Decision::UsePotion {
                index: *index,
                target: target.map(combat_id).transpose()?,
            }
            .into(),
            Play::Proceed => Decision::Proceed.into(),
            Play::ChooseOption { index } => Decision::ChooseOption { index: *index }.into(),
            Play::Buy { index } => Decision::Buy { index: *index }.into(),
            Play::Answer(answering) => Decision::Answer(match answering {
                Answering::Cards { cards } => GameAnswer::Cards(cards.clone()),
                Answering::Skip => GameAnswer::Skip,
                Answering::Take { rewards } => {
                    GameAnswer::Take(rewards.iter().map(Taking::take).collect())
                }
                Answering::Divine { x, y, big } => GameAnswer::Divine {
                    x: *x,
                    y: *y,
                    tool: if *big { Tool::Big } else { Tool::Small },
                },
            })
            .into(),
            Play::Grant {
                card,
                relic,
                potion,
            } => Setup::Grant(match (card, relic, potion) {
                (Some(card), ..) => Gift::Card(id("card", card)?),
                (_, Some(relic), _) => Gift::Relic(id("relic", relic)?),
                (.., Some(potion)) => Gift::Potion(id("potion", potion)?),
                // clap holds this off: `card` is required unless one of the
                // others is present, and all three conflict.
                (None, None, None) => {
                    anyhow::bail!("grant needs a --card, a --relic or a --potion")
                }
            })
            .into(),
        })
    }
}
