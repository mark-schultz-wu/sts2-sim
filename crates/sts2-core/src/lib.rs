//! What a game of Slay the Spire 2 is, to anything that drives one.
//!
//! Two implementations are meant to satisfy this. The real game, run headless
//! over a socket (`sts2-headless`), and the Rust engine that does not exist yet
//! (`sts2-sim`). A front end, a fuzzer or a differential test written against
//! these types works against either. The two are comparable because both report
//! the same type.
//!
//! Generic over two things, not one: which build (`V: ContentVersion`, from
//! `sts2-content`) and which implementation (the traits here).
//!
//! This crate owns the *snapshot*, never the state. A simulator's combat state
//! is a machine, mutable, holding RNG streams and an action queue. It derives an
//! observation, exactly as the mod's `Observation.cs` derives one from
//! `RunState`. Snapshots are comparable, and machines are not.
//!
//! See `docs/STS2-CORE.md` for why the split is drawn here.

// Three folders for the three parts of the exchange — what comes back
// (`report`), what a caller sends (`step`), and the traits that join them
// (`play`) — over the vocabulary all three draw on (`content`, `ident`,
// `seed`). Every type is re-exported flat below, and that flat surface is the
// API: the folders are how this crate is read, not how it is used.
pub mod content;
mod ident;
mod play;
mod report;
pub mod rng;
pub mod seed;
mod step;

// `sts2_core::recording` was a path before `Recording` had a folder to live in,
// and it still is one.
pub use play::recording;

pub use content::{
    ActTable, CardTable, CardVars, CharacterTable, ContentError, ContentVersion, Cost, Drafted,
    EncounterRoster, EncounterTable, EventTable, Fixed, FlagSet, Ladder, Layout, MapPoints,
    MonsterTable, PotionTable, PotionUsage, PowerAmount, PowerTable, Printed, Rarities, RelicPools,
    RelicTable, Rests, RewardRarity, Rooms, Rules, Sides, Streams, Table, Targets, Temporary,
    TurnPhases, Unplayable, Version,
};
pub use ident::{CardIdent, Coord, ModelId};
pub use seed::Seed;

pub use report::digest::Digest;
pub use report::observation::{
    Attack, Blocked, Carried, Channelled, ChestOption, Combat, Counters, Creature, Entry,
    EventOption, Held, Intent, Map, NextMove, Node, Observation, Orbs, Playable, Preview,
    Reachable, RestOption, Room, ShopCard, Stack, Stock, Take, Undrinkable,
};
pub use report::question::{
    CardOption, Cell, FoundItem, Question, QuestionKind, RewardCard, RewardDetail, RewardKind,
};

pub use step::Step;
pub use step::answer::{Answer, TakeReward, Tool};
pub use step::decision::Decision;
pub use step::setup::{Gift, Setup};
pub use step::start::Start;

pub use play::recorder::{Recorder, Tape};
pub use play::recording::Recording;
pub use play::refusal::{Reason, Refusal};
pub use play::{Answered, Applied, Construct, Play};
