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
pub mod event;
mod hook_response;
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
    ActTable, AscensionKinds, CardTable, CardVars, CharacterTable, ContentError, ContentVersion,
    Cost, DecisionHook, Drafted, Effect, EffectResolution, EncounterRoster, EncounterTable,
    EventHook, EventTable, Fight, Fixed, FlagSet, Holding, HookEffect, Hooks, Layout, Listener,
    MapPointKinds, Modify, ModifyHook, MonsterSpawn, MonsterTable, MoveMachine, PotionTable,
    PotionUsage, PowerAmount, PowerTable, Previewed, Printed, RarityKinds, RelicModel,
    RelicPoolKinds, RelicTable, Rests, RewardRarity, RoomKinds, Rules, SideKinds, SideTurnEnd,
    StreamKinds, Table, TargetKinds, Temporary, TurnPhaseKinds, UnplayableReasonKinds, Version,
};
pub use event::{EventBehavior, EventContext, EventFlow, EventView};
pub use hook_response::HookResponse;
pub use ident::{CardIdent, Coord, ModelId, OutOfRange};
pub use seed::Seed;

pub use report::digest::{Digest, Section};
pub use report::observation::{
    ActMap, Attack, Blocked, Carried, Channelled, ChestOption, Combat, CombatId, Counters,
    Creature, Entry, EventOption, EventOptionKey, EventOptionKinds, Held, Intent, Map, NextMove,
    NoNode, Node, Observation, Orbs, Playable, Preview, Reachable, RestOption, Room, ShopCard,
    Stack, Stock, Take, Undrinkable,
};
pub use report::question::{
    CardOption, Cell, FoundItem, Question, QuestionKind, RewardCard, RewardDetail, RewardKind,
};

#[cfg(feature = "fixture")]
pub mod fixture;

pub use step::Step;
pub use step::answer::{Answer, TakeReward, Tool};
pub use step::decision::{BeltIndex, Decision, HandIndex};
pub use step::setup::{Gift, Setup};
pub use step::start::Start;

pub use play::recorder::{Recorder, Tape};
pub use play::recording::Recording;
pub use play::refusal::{Reason, Refusal};
pub use play::{Answered, Applied, Construct, Decided, Play, Snapshot};
