//! Content for game build `v0.107.1`.
//!
//! `dumps/` holds what the running game printed, under
//! `STS2_HEADLESS_MODE=carddump` and friends. See `mod/sts2_dump/`. Each dump
//! carries the content hash in its header, and the `tests` module requires that
//! to match [`VERSION`]. Regenerating against a different build then fails,
//! rather than quietly replacing the tables. `generated/` mirrors it, one
//! module per kind of content.
//!
//! Static means true with no run in progress. That is what a card costs, how
//! much HP a monster rolls, whether a power stacks, and which monsters an
//! encounter fields. What any of it *does* is code, and gets ported by hand.
//!
//! Where a value is code, the dump says so rather than guessing, and it
//! generates as [`Field::Computed`](crate::Field::Computed). That is the simulator's
//! cue to ask rather than look up. Each domain decides it differently, because
//! each had to. A card is marked from its overrides, a power from its IL, and
//! an encounter by running it and reading the `Rng` counter afterwards. All of
//! them err the same way, towards computed. Marking a constant as computed
//! costs one method, and the reverse costs a divergence nobody can see.

mod behaviour;
mod engine;
mod generated;
mod keywords;
mod rest_site;

/// An event's state, as the shared events keep it.
pub type EventState = crate::behaviour::events::State<V0_107_1>;
/// A relic's state, as the shared relics keep it.
pub type RelicState = crate::behaviour::relics::State<V0_107_1>;
/// An option an event offers, as every build spells it.
pub type EventOptionId = crate::behaviour::EventOptionId<V0_107_1>;
pub use generated::*;

use sts2_core::Version;

/// This build, as a type.
///
/// Zero-sized: it exists to be a type parameter, so that a client or a
/// simulator names the build once and every id follows. See
/// [`ContentVersion`](sts2_core::ContentVersion).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct V0_107_1;

impl sts2_core::ContentVersion for V0_107_1 {
    const VERSION: Version = VERSION;

    /// The two-lane fold over UTF-16 code units, kept to 32 bits.
    ///
    /// `StringHelper.GetDeterministicHashCode` returns `int` here, and every
    /// caller casts it to `uint` before it seeds anything.
    fn name_hash(name: &str) -> u64 {
        sts2_core::rng::name_hash(name) as u32 as u64
    }

    /// `Rng`'s seed is `uint` here, so a wider sum truncates.
    fn seed_from(parts: u64) -> u64 {
        parts as u32 as u64
    }

    type EventOptionId = EventOptionId;
    type EventOption = generated::events::EventOption;

    type Number = i32;
    type Health = i32;
    type Count = u32;
    type Index = u16;
    type Resume = crate::behaviour::Resume<Self>;
    type CardWrite = crate::behaviour::cards::CardWrite;
    type DeckWrite = crate::behaviour::DeckWrite;
    type CardSerial = u16;

    type AscensionLevel = generated::ascension::AscensionLevel;
    type Act = generated::acts::Act;
    type Affliction = generated::afflictions::Affliction;
    type Card = generated::cards::Cards;
    type CardKeyword = generated::cards::CardKeyword;
    type CardKeywordSet = generated::cards::KeywordSet;
    type CardVar = generated::cards::CardVar;
    type CardTag = generated::cards::CardTag;
    type Character = generated::characters::Character;
    type EnchantmentState = crate::behaviour::enchantments::State;
    type Enchantment = generated::enchantments::Enchantment;
    type Encounter = generated::encounters::Encounter;
    type Event = generated::events::Event;
    type Modifier = generated::modifiers::Modifier;
    type Monster = generated::monsters::Monster;
    type Orb = generated::orbs::Orb;
    type Potion = generated::potions::Potion;
    type Power = generated::powers::Power;
    type Relic = generated::relics::Relic;
    type RoomType = generated::encounters::RoomType;

    // The vocabulary a run's position is described in. `runstate` is one dump
    // of its own because none of these is a property of a content item; the two
    // below it belong to the domains whose content has them.
    type MapPointType = generated::runstate::MapPointType;
    type CombatSide = generated::runstate::CombatSide;
    type PlayerTurnPhase = generated::runstate::PlayerTurnPhase;
    type UnplayableReason = generated::runstate::UnplayableReason;
    type UnplayableReasons = generated::runstate::UnplayableReasons;
    type RunRngType = generated::runstate::RunRngType;
    type PlayerRngType = generated::runstate::PlayerRngType;
    type IntentKind = generated::monsters::IntentKind;
    type MoveId = generated::monsters::MoveId;
    type RestSiteOption = generated::restsite::RestSiteOption;
    type TargetType = generated::cards::TargetType;
}

// `Hooks` and `Rules`, as the shared rules every build forwards to.
crate::build::rules!(V0_107_1, v0_107_1);

pub const VERSION: Version = Version {
    name: "v0.107.1",
    commit: "59260271",
    assembly_hash: 1_692_500_715,
    content_hash: 3_954_186_980,
};

/// The act dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ACTS: &str = include_str!("dumps/acts.txt");

/// The card dump, verbatim. Test data, not runtime data.
///
/// `cargo xtask codegen` reads it from disk to build [`cards`], and the `tests`
/// module replays it to check that table. Nothing at runtime parses it. That is
/// what the generated table is for. A `pub const` here both invites it and puts
/// the dump's text into every binary that links this crate.
#[cfg(test)]
const CARDS: &str = include_str!("dumps/cards.txt");

/// The monster dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const MONSTERS: &str = include_str!("dumps/monsters.txt");

/// The power dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const POWERS: &str = include_str!("dumps/powers.txt");

/// The encounter dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ENCOUNTERS: &str = include_str!("dumps/encounters.txt");

/// The relic dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const RELICS: &str = include_str!("dumps/relics.txt");

/// The character dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const CHARACTERS: &str = include_str!("dumps/characters.txt");

/// The event dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const EVENTS: &str = include_str!("dumps/events.txt");

/// The potion dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const POTIONS: &str = include_str!("dumps/potions.txt");

/// The enchantment dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ENCHANTMENTS: &str = include_str!("dumps/enchantments.txt");

/// The affliction dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const AFFLICTIONS: &str = include_str!("dumps/afflictions.txt");

/// The orb dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ORBS: &str = include_str!("dumps/orbs.txt");

/// The modifier dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const MODIFIERS: &str = include_str!("dumps/modifiers.txt");

/// The ascension dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ASCENSION: &str = include_str!("dumps/ascension.txt");

/// The run-state dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const RUNSTATE: &str = include_str!("dumps/runstate.txt");

/// The rest-site dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const RESTSITE: &str = include_str!("dumps/restsite.txt");

/// The hook dump, verbatim. Test data, on the same terms as [`CARDS`].
///
/// Mostly the interface between an engine and the things that react to it,
/// which a simulator implements rather than looks up. Codegen takes one thing
/// from it: which hooks each power's class overrides, in the power table. The
/// rest is a measurement of this build, and the tests are where it gets pinned.
#[cfg(test)]
const HOOKS: &str = include_str!("dumps/hooks.txt");

#[cfg(test)]
mod tests;
