//! What a parked game is asking.
//!
//! Both implementations stop mid-action to ask. A card is drafted, a discard is
//! chosen, and a reward screen is answered. The run cannot go on until it is,
//! and the answer is a [`crate::Answer`], which is a [`crate::Decision`] like
//! any other.
//!
//! Shared vocabulary rather than the oracle's, for the reason the observation
//! is shared. A simulator that parks at a different question has diverged, and
//! that is only visible if the two spell a question the same way.
//!
//! Three strings are left, and each names a C# class the build has no table
//! for. None is a content id, and inventing an enum for one without a dump
//! behind it is a guess wearing a type.

use crate::{CardIdent, ContentVersion, RelicTable, Tool};

/// A question the game stopped to ask.
#[derive(Debug, Clone, PartialEq)]
pub enum Question<V: ContentVersion> {
    /// Answer with [`crate::Answer::Cards`].
    SelectCards {
        min: u32,
        max: u32,
        /// The running action, not the game's own prompt text. A selector
        /// never receives the prompt, so this is attribution rather than
        /// wording.
        asker: Option<String>,
        options: Vec<CardOption<V>>,
    },
    /// Answer with [`crate::Answer::Skip`] or [`crate::Answer::Take`].
    Rewards {
        skippable: bool,
        rewards: Vec<RewardDetail<V>>,
    },
    /// Answer with [`crate::Answer::Divine`].
    ///
    /// The Crystal Sphere, and nothing else in the game. It is the one event
    /// with a screen of its own, and the mod diverts that screen here.
    Divine {
        /// Divinations remaining. Three or six, by the option taken.
        left: u32,
        width: u8,
        height: u8,
        /// The tool the next click uses. [`Tool::Big`] at the start.
        ///
        /// Carried because the game keeps it between clicks, so an answer that
        /// omits nothing still changes it. A client need not read it. Every
        /// [`crate::Answer::Divine`] names its own tool.
        tool: Tool,
        /// The cells already uncovered.
        ///
        /// Uncovered rather than hidden, because hidden is the starting state of
        /// all `width * height` of them and this list begins empty. The hidden
        /// cells are the complement.
        cleared: Vec<Cell>,
        /// What the divinations have turned up.
        revealed: Vec<FoundItem>,
    },
    /// Answer with [`crate::Answer::Cards`] naming one bundle.
    ///
    /// Scroll Boxes' choose-a-bundle screen, whose chosen bundle all goes into
    /// the deck.
    ChooseBundle { bundles: Vec<BundleOption<V>> },
}

impl<V: ContentVersion> Question<V> {
    /// Which of the four this is.
    ///
    /// For saying what a parked game is waiting for without holding the
    /// question itself. A refusal names it, and a front end routes on it.
    pub fn kind(&self) -> QuestionKind {
        match self {
            Question::SelectCards { .. } => QuestionKind::SelectCards,
            Question::Rewards { .. } => QuestionKind::Rewards,
            Question::Divine { .. } => QuestionKind::Divine,
            Question::ChooseBundle { .. } => QuestionKind::ChooseBundle,
        }
    }
}

/// Which of the four questions a parked game is asking.
///
/// [`Question`] with its payload removed, for the places that need to say what
/// is pending rather than answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestionKind {
    SelectCards,
    Rewards,
    Divine,
    ChooseBundle,
}

impl std::fmt::Display for QuestionKind {
    /// The oracle's own spelling, which is what the mod logs and what
    /// `PROTOCOL.md` calls each of them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            QuestionKind::SelectCards => "select_cards",
            QuestionKind::Rewards => "rewards",
            QuestionKind::Divine => "divine",
            QuestionKind::ChooseBundle => "choose_bundle",
        })
    }
}

/// One card a selection offers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardOption<V: ContentVersion> {
    pub index: usize,
    pub card: CardIdent<V>,
    /// What the card says on its face, with its own numbers in it.
    ///
    /// The game's own rules text, resolved through its localisation tables. A
    /// client can then show what is being chosen between rather than a set of
    /// ids. It carries the game's markup, which a display strips.
    ///
    /// Combat hooks are not applied. This is what the card does in general,
    /// which is the question a selection prompt asks.
    pub text: Option<String>,
}

/// One card inside a card reward.
///
/// The same pair as [`CardOption`] without the index. A reward's cards are
/// addressed by position within the reward, rather than by a number of their
/// own.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RewardCard<V: ContentVersion> {
    pub card: CardIdent<V>,
    /// See [`CardOption::text`].
    pub text: Option<String>,
}

/// One bundle a choose-a-bundle screen offers, with its cards in order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BundleOption<V: ContentVersion> {
    pub index: usize,
    pub cards: Vec<RewardCard<V>>,
}

/// One square of the Crystal Sphere's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub x: u8,
    pub y: u8,
}

/// Something the Crystal Sphere was hiding, once every cell over it is clear.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FoundItem {
    /// The game's class name, such as `CrystalSphereGold` or
    /// `CrystalSphereCurse`.
    ///
    /// A string because these are not models. They have no id in the model
    /// database. The minigame's payout arrives afterwards as an ordinary reward
    /// screen, where every item in it is typed.
    pub item: String,
    /// False for a curse.
    pub good: bool,
    /// Top-left corner, and how many cells it covers.
    pub x: u8,
    pub y: u8,
    pub width: u8,
    pub height: u8,
}

/// One reward on offer.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardDetail<V: ContentVersion> {
    pub index: usize,
    /// A reward can exist in the set without content, which is not the same as
    /// the reward being absent. Reported for every kind, including
    /// [`RewardKind::Other`].
    pub populated: bool,
    /// Already taken, and so no longer answerable.
    ///
    /// A reward screen is asked once per decision, and the same set comes back
    /// with one more of these true each time. The indices never renumber, which
    /// is why this is a flag rather than the reward leaving the list.
    ///
    /// It is also how a failed selection shows. A potion with no free slot is
    /// neither taken nor gone.
    pub taken: bool,
    pub reward: RewardKind<V>,
}

/// What a reward actually offers.
///
/// A sum rather than a struct of mutually exclusive `Option`s. The game sends
/// exactly one shape per reward, so nothing else is expressible.
#[derive(Debug, Clone, PartialEq)]
pub enum RewardKind<V: ContentVersion> {
    Gold {
        amount: i64,
    },
    /// The cards on offer, in the order they are addressed. Empty is possible,
    /// because an unpopulated card reward still reports the field.
    Card {
        cards: Vec<RewardCard<V>>,
        /// What the reward offers in place of a card, other than skipping it.
        alternatives: Vec<crate::RewardAlternative>,
    },
    Relic {
        /// Absent when the reward exists but `RelicReward.Populate` found
        /// nothing.
        relic: Option<V::Relic>,
        /// Which screen the reward came from. That is a fact about the slot
        /// rather than about the relic. So it is still here when the relic is
        /// not, and is not read off `relic.rarity()`.
        rarity: <V::Relic as RelicTable>::Rarity,
    },
    Potion {
        /// Absent for the same reason a relic can be.
        potion: Option<V::Potion>,
    },
    /// One named card handed back rather than chosen between, as the thief
    /// boss returns the card it stole.
    ///
    /// A [`RewardCard`] rather than a bare ident, so it carries the cost and
    /// the rules text a card reward carries. The screen shows a card. The only
    /// difference is that there is nothing to pick.
    SpecialCard {
        /// Absent for the same reason a relic can be: the reward exists in the
        /// set, but the game populated no card into it.
        card: Option<RewardCard<V>>,
    },
    /// A reward type this crate does not unpack, named rather than anonymous.
    ///
    /// Deliberately not a catch-all meaning "cannot say": a value
    /// indistinguishable from a real answer is worse than no value. This says
    /// only that the game described a reward whose per-type fields are not
    /// modelled here.
    Other {
        kind: String,
    },
}
