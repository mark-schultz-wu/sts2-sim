//! The command surface, and how it becomes a game command.
//!
//! Ids are named the way the game names them: `IRONCLAD`, `BOWLBUGS_WEAK`,
//! `DeadlyEnemies`. That is the serde rename on every generated type, so
//! parsing is one `serde_json::from_value` rather than a table of aliases that
//! drifts.

pub mod choose;
pub mod moves;

use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};
use sts2_core::Seed;
use sts2_core::{
    Answer as GameAnswer, Coord, Decision, Gift, Picks, Setup, Start, Step, TakeReward, Tool,
};

/// A command line that names nothing the game has, or asks what it cannot.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{spelling:?} is not {article} {what} this build has")]
    NoSuch {
        what: &'static str,
        article: &'static str,
        spelling: String,
        source: serde::de::value::Error,
    },
    #[error("that is not a seed: {0}")]
    Seed(#[from] sts2_core::seed::Error),
    #[error("{0} is no creature in a fight")]
    NoCreature(u32),
    #[error("{0} is no card in a hand")]
    NoCard(u8),
    #[error("{0} is no potion in a belt")]
    NoPotion(u8),
    #[error("no reward screen offers {0}")]
    NoReward(usize),
    #[error("no room offers option {0}")]
    NoOption(usize),
    #[error("as a card selection: {0}")]
    Unpickable(#[from] sts2_core::Unpickable),
    #[error("making {}: {source}", path.display())]
    MakingDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{stem} already has 999 recordings in {}", dir.display())]
    NoFreeName { stem: String, dir: PathBuf },
    #[error("grant needs a --card, a --relic or a --potion")]
    NothingGranted,
    #[error("nothing is waiting to be chosen. `sts2 show observe` shows where the run is")]
    NothingWaiting,
    #[error("{0} is a plain index; `N.M` is only for a card reward")]
    Dotted(&'static str),
    #[error("a reward screen takes one reward per answer, and asks again")]
    OneReward,
    #[error("a divination names a cell and a tool: `sts2 act answer divine X Y [--big]`")]
    Divination,
    #[error("a room option is taken one at a time, but {0} were given")]
    OneOption(usize),
    #[error("a shop entry is bought one at a time, but {0} were given")]
    OneEntry(usize),
    #[error("a confirmation takes one answer: 1 for yes, 0 for no")]
    OneConfirmation,
}

/// An ascension level by the game's own name for it, such as `None`.
///
/// Public because the front end has to name one for a run started anywhere but
/// the command line. `V::AscensionLevel` is an associated type with no default,
/// and there is no way to spell "the lowest one" except by naming it.
pub fn ascension<V: sts2_content_wire::WireContentVersion>(
    spelling: &str,
) -> Result<V::AscensionLevel, Error> {
    id("ascension level", spelling)
}

/// Parses an id from the game's own spelling of it.
pub fn id<T: serde::de::DeserializeOwned>(what: &'static str, spelling: &str) -> Result<T, Error> {
    let article = match what.starts_with(['a', 'e', 'i', 'o', 'u']) {
        true => "an",
        false => "a",
    };

    crate::id::parse(spelling).map_err(|source| Error::NoSuch {
        what,
        article,
        spelling: spelling.to_owned(),
        source,
    })
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
    /// The runs the profile has won with the character, which picks the
    /// dialogue a won run ends on.
    #[arg(long, default_value_t = 0)]
    pub wins: u32,
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

/// A creature id as a person types one.
fn combat_id(id: u32) -> Result<sts2_core::CombatId, Error> {
    u8::try_from(id)
        .ok()
        .and_then(|id| sts2_core::CombatId::try_new(id).ok())
        .ok_or(Error::NoCreature(id))
}

/// A place in the hand as a person types one.
fn hand_index(index: u8) -> Result<sts2_core::HandIndex, Error> {
    sts2_core::HandIndex::try_new(index).map_err(|_| Error::NoCard(index))
}

/// A slot in the belt as a person types one.
fn belt_index(index: u8) -> Result<sts2_core::BeltIndex, Error> {
    sts2_core::BeltIndex::try_new(index).map_err(|_| Error::NoPotion(index))
}

impl StartArgs {
    /// The typed run this command line asks to start.
    pub fn start<V: sts2_content_wire::WireContentVersion>(&self) -> Result<Start<V>, Error> {
        Ok(Start {
            seed: Seed::fold(&self.seed)?,
            character: id("character", &self.character)?,
            ascension: match &self.ascension {
                Some(level) => id("ascension level", level)?,
                None => id("ascension level", "None")?,
            },
            modifiers: self
                .modifiers
                .iter()
                .map(|modifier| id("modifier", modifier))
                .collect::<Result<_, _>>()?,
            wins: self.wins,
        })
    }

    /// Where this run gets written, if it does.
    ///
    /// Recording is the default because a valuable run is recognized only
    /// afterwards. It can expose a divergence, an interface problem, or a seed
    /// worth replaying. An unrecorded run cannot be shown to anybody. Each file
    /// is only a few kilobytes.
    pub fn recording(&self) -> Result<Option<PathBuf>, Error> {
        if self.no_record {
            return Ok(None);
        }
        if let Some(path) = &self.record {
            return Ok(Some(path.clone()));
        }

        let dir = crate::runs::runs_dir();
        std::fs::create_dir_all(&dir).map_err(|source| Error::MakingDir {
            path: dir.clone(),
            source,
        })?;
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
fn free_name(dir: &Path, seed: &str, character: &str) -> Result<PathBuf, Error> {
    let folded = Seed::fold(seed).map_or_else(|_| seed.to_uppercase(), |s| s.as_str().to_string());
    let stem = format!("{folded}-{}", character.to_uppercase());

    let first = dir.join(format!("{stem}.jsonl"));
    if !first.exists() {
        return Ok(first);
    }

    (2..1000)
        .map(|n| dir.join(format!("{stem}-{n}.jsonl")))
        .find(|path| !path.exists())
        .ok_or_else(|| Error::NoFreeName {
            stem,
            dir: dir.to_path_buf(),
        })
}

/// One reward being taken: which reward, and which card inside it.
///
/// Spelled `N` or `N.M`. The dot is doing real work. A card reward holds
/// several cards and the game will not pick one for you. So `0` and `0.1` are
/// different requests, rather than one with a default. `N.reroll` and
/// `N.sacrifice` name a card reward's alternative.
#[derive(Debug, Clone, Copy)]
pub struct Taking {
    reward: usize,
    card: Option<usize>,
    alternative: Option<sts2_core::RewardAlternative>,
}

impl Taking {
    pub fn reward(&self) -> usize {
        self.reward
    }

    pub fn card(&self) -> Option<usize> {
        self.card
    }

    /// The reward this names, or why no reward screen offers it.
    pub(crate) fn take(&self) -> Result<TakeReward, Error> {
        let position = |at: usize| u8::try_from(at).map_err(|_| Error::NoReward(at));
        if let Some(alternative) = self.alternative {
            return Ok(TakeReward::Alternative {
                index: position(self.reward)?,
                alternative,
            });
        }
        Ok(match self.card {
            Some(card) => TakeReward::Card {
                index: position(self.reward)?,
                card: position(card)?,
            },
            None => TakeReward::NonCard {
                index: position(self.reward)?,
            },
        })
    }
}

impl std::str::FromStr for Taking {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let number = |part: &str, what: &str| {
            part.parse::<usize>()
                .map_err(|_| format!("{part:?} is not {what}, in {s:?}"))
        };

        let alternative = |part: &str| match part {
            "reroll" => Some(sts2_core::RewardAlternative::Reroll),
            "sacrifice" => Some(sts2_core::RewardAlternative::Sacrifice),
            _ => None,
        };
        Ok(match s.split_once('.') {
            Some((reward, part)) if alternative(part).is_some() => Taking {
                reward: number(reward, "a reward index")?,
                card: None,
                alternative: alternative(part),
            },
            Some((reward, card)) => Taking {
                reward: number(reward, "a reward index")?,
                card: Some(number(card, "a card index")?),
                alternative: None,
            },
            None => Taking {
                reward: number(s, "a reward index")?,
                card: None,
                alternative: None,
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
    /// Takes one reward from a reward screen, which then asks again.
    ///
    /// A reward is `N`, and a card inside a card reward is `N.M`. `take 0.1`
    /// is the second card of the first reward. The game refuses a card reward
    /// taken without a card rather than choosing one, so the pair is not
    /// optional there.
    Take { reward: Taking },
    /// Uncovers a cell of the Crystal Sphere's grid.
    ///
    /// Costs one divination whichever tool it uses, so `--big` is a gamble and
    /// not a saving. It uncovers the neighbours too, and one of the things
    /// under them can be a curse.
    Divine {
        x: u8,
        y: u8,
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
    pub fn step<V: sts2_content_wire::WireContentVersion>(&self) -> Result<Step<V>, Error> {
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
                index: hand_index(*index)?,
                target: target.map(combat_id).transpose()?,
            }
            .into(),
            Play::EndTurn => Decision::EndTurn.into(),
            Play::UsePotion { index, target } => Decision::UsePotion {
                index: belt_index(*index)?,
                target: target.map(combat_id).transpose()?,
            }
            .into(),
            Play::Proceed => Decision::Proceed.into(),
            Play::ChooseOption { index } => Decision::ChooseOption {
                index: *index,
                target: None,
            }
            .into(),
            Play::Buy { index } => Decision::Buy { index: *index }.into(),
            Play::Answer(answering) => Decision::Answer(match answering {
                Answering::Cards { cards } => GameAnswer::Cards(Picks::try_from(cards.as_slice())?),
                Answering::Skip => GameAnswer::Skip,
                Answering::Take { reward } => GameAnswer::Take(reward.take()?),
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
                (None, None, None) => return Err(Error::NothingGranted),
            })
            .into(),
        })
    }
}
