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
    ActTable, AfflictionTable, Amount, AscensionKinds, AttackId, AttackResults, BlockGained,
    CHOICES, CardDraw, CardKind, CardModel, CardNumber, CardNumberModifier, CardOwner,
    CardRarityKinds, CardSource, CardTable, CardTagKinds, CardVars, Change, CharacterTable,
    ContentError, ContentVersion, Cost, CostPool, CreatureView, CustomReward, DamageResult,
    DecisionHook, DeckAct, DeckCard, DeckPick, Drafted, Effect, EffectResolution, EnchantHook,
    EnchantmentStatus, EnchantmentTable, EncounterRoster, EncounterTable, EventHook, EventPower,
    EventTable, Fight, FightView, Fixed, FlagSet, HeldInstance, History, HistoryEntry, HookEffect,
    HookEffects, Hooks, Kept, KeywordKinds, Lasting, Layout, Listener, MAX_CARDS_IN_HAND,
    MapPointKinds, ModelHook, Modify, ModifyHook, MonsterKinds, MonsterModel, MonsterSpawn,
    MonsterTable, MonsterVirtual, MoveKinds, MoveLog, MoveMachine, MovePinned, Offered, Opponents,
    Overrides, Picked, PileAct, PilePosition, PlayAt, PlayFinished, PlayStarted, Played,
    PlayerHealth, PlayerStreamKinds, PoolFilter, PotionTable, PotionUsage, PowerAmount, PowerKinds,
    PowerModel, PowerTable, PowerVirtual, Printed, RarityKinds, RelicModel, RelicPoolKinds,
    RelicPull, RelicTable, RestKinds, Rests, ResultPile, RewardRarity, RoomKinds, RosterEntry,
    Rules, RunStreamKinds, Scaling, SelectedCards, Selection, SideKinds, SideTurnEnd, Spawned,
    Stream, StreamKinds, StunResume, Table, Taken, TargetKinds, Temporary, TurnHistory,
    TurnPhaseKinds, UnplayableReasonKinds, Version, Writing,
};
pub use event::{CarriedRelic, EventBehavior, EventView};
pub use hook_response::HookResponse;
pub use ident::{CardAt, CardId, CardIdent, Coord, Enchanted, ModelId, OutOfRange, Pile};
pub use seed::Seed;

pub use report::digest::{Digest, Digested, Section};
pub use report::observation::{
    ActMap, Attack, Blocked, Carried, Channelled, ChestOption, Combat, CombatId, Creature, Entry,
    EventOption, EventOptionKey, EventOptionKinds, Held, Intent, Keyed, Map, NextMove, NoNode,
    Node, Observation, Orbs, Playable, Position, Positions, Preview, Reachable, RestOption, Room,
    ShopCard, Stack, Stock, Take, Targets, Undrinkable,
};
pub use report::observed::{Observed, ObservedCreature, ObservedFight, ObservedPlayable};
pub use report::question::{
    CardOption, Cell, FoundItem, Question, QuestionKind, RewardCard, RewardDetail, RewardKind,
};

#[cfg(feature = "fixture")]
pub mod fixture;

pub use step::Step;
pub use step::answer::{Answer, Picks, TakeReward, Tool, Unpickable};
pub use step::decision::{BeltIndex, Decision, HandIndex};
pub use step::setup::{Gift, Setup};
pub use step::start::Start;

pub use play::recorder::{Recorder, Tape};
pub use play::recording::Recording;
pub use play::refusal::{Reason, Refusal};
pub use play::{Answered, Applied, Construct, Decided, Play, Snapshot};
