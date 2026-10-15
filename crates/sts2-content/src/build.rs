//! What a rule shared between builds needs of the build it runs in.
//!
//! The behaviour is written once and is generic over the build. It names
//! content through the consts in [`crate::ids`], and reads a row through
//! `sts2_core::Table`. What every build spells alike is one type, in
//! [`crate::generated`]. Each bound here is one a shared rule reads.

use sts2_core::{CardTable, ContentVersion, Rules, Table};

use crate::generated::ascension::AscensionLevel;
use crate::generated::cards::{CardKeyword, CardRarity, CardTag, KeywordSet, TargetType};
use crate::generated::encounters::RoomType;
use crate::generated::monsters::IntentKind;
use crate::generated::orbs::OrbData;
use crate::generated::runstate::{
    CombatSide, MapPointType, PlayerRngType, PlayerTurnPhase, RunRngType, UnplayableReason,
    UnplayableReasons,
};
use crate::ids::{CardIds, EnchantmentIds, OrbIds, PowerIds};

/// The content a shared rule reads, which every build has.
///
/// Every build holds its numbers alike, so a shared rule does its arithmetic
/// in the same types. What every build spells alike is the shared type.
pub trait BuildContent:
    ContentVersion<
        Number = i32,
        Health = i32,
        Count = u32,
        Index = u16,
        CardSerial = u16,
        DeckWrite = crate::behaviour::DeckWrite,
        AscensionLevel = AscensionLevel,
        CardKeyword = CardKeyword,
        CardKeywordSet = KeywordSet,
        CardTag = CardTag,
        TargetType = TargetType,
        RoomType = RoomType,
        MapPointType = MapPointType,
        CombatSide = CombatSide,
        PlayerTurnPhase = PlayerTurnPhase,
        UnplayableReason = UnplayableReason,
        UnplayableReasons = UnplayableReasons,
        RunRngType = RunRngType,
        PlayerRngType = PlayerRngType,
        IntentKind = IntentKind,
        Card: CardIds + CardTable<Rarity = CardRarity, Tag = CardTag>,
        Enchantment: EnchantmentIds,
        Orb: OrbIds + Table<Data = OrbData>,
        Power: PowerIds,
    >
{
}

impl BuildContent for crate::v0_107_1::V0_107_1 {}

impl BuildContent for crate::v0_111_0::V0_111_0 {}

/// A build the shared behaviour runs in: its content, and its rules, which
/// a shared rule calls back into.
pub trait Build: BuildContent + Rules {}

impl<V: BuildContent + Rules> Build for V {}
