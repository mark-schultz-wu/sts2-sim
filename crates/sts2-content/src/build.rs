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
use crate::ids::{
    ActIds, AfflictionIds, CardIds, CardVarIds, CharacterIds, EnchantmentIds, EncounterIds,
    EventIds, ModifierIds, MonsterIds, MoveIds, OrbIds, PotionIds, PowerIds, RelicIds,
    RestSiteOptionIds,
};

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
        Act: ActIds,
        Affliction: AfflictionIds,
        Card: CardIds + CardLists + CardTable<Rarity = CardRarity, Tag = CardTag>,
        CardVar: CardVarIds,
        Character: CharacterIds,
        Enchantment: EnchantmentIds,
        Encounter: EncounterIds,
        Event: EventIds,
        Modifier: ModifierIds,
        Monster: MonsterIds,
        MoveId: MoveIds,
        Orb: OrbIds + Table<Data = OrbData>,
        Potion: PotionIds,
        Power: PowerIds,
        Relic: RelicIds,
        RestSiteOption: RestSiteOptionIds,
    >
{
}

impl BuildContent for crate::v0_107_1::V0_107_1 {}

impl BuildContent for crate::v0_111_0::V0_111_0 {}

/// The lists of cards a shared rule hands out whole.
///
/// A list a rule hands out lives as long as the program, and a list of a
/// generic type cannot. So each build holds its own, of its own cards, and
/// [`card_lists`] names what is in them once.
pub trait CardLists: Sized + 'static {
    /// `CurseCardPool`'s cards whose `CanBeGeneratedByModifiers` holds,
    /// ordered by id. Ascender's Bane, Bad Luck, Curse of the Bell,
    /// Enthralled, Folly, Greed, Poor Sleep and Spore Mind override it to
    /// refuse.
    const MODIFIER_CURSES: &'static [Self];
}

/// Every [`CardLists`] list, for one build's cards.
macro_rules! card_lists {
    ($card:ty) => {
        impl CardLists for $card {
            const MODIFIER_CURSES: &'static [Self] = &[
                <$card as CardIds>::CLUMSY,
                <$card as CardIds>::DEBT,
                <$card as CardIds>::DECAY,
                <$card as CardIds>::DOUBT,
                <$card as CardIds>::GUILTY,
                <$card as CardIds>::INJURY,
                <$card as CardIds>::NORMALITY,
                <$card as CardIds>::REGRET,
                <$card as CardIds>::SHAME,
                <$card as CardIds>::WRITHE,
            ];
        }
    };
}

card_lists!(crate::v0_107_1::cards::Cards);
card_lists!(crate::v0_111_0::cards::Cards);

/// A build the shared behaviour runs in: its content, and its rules, which
/// a shared rule calls back into.
pub trait Build: BuildContent + Rules {}

impl<V: BuildContent + Rules> Build for V {}
