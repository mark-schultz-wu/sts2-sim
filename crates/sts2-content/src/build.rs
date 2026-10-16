//! What a rule shared between builds needs of the build it runs in.
//!
//! The behaviour is written once and is generic over the build. It names
//! content through the consts in [`crate::ids`], and reads a row through
//! `sts2_core::Table`. What every build spells alike is one type, in
//! [`crate::generated`]. Each bound here is one a shared rule reads.

use sts2_core::{CardTable, ContentVersion, PotionTable, Rules, Table};

use crate::generated::ascension::AscensionLevel;
use crate::generated::cards::{CardKeyword, CardRarity, CardTag, KeywordSet, TargetType};
use crate::generated::encounters::RoomType;
use crate::generated::monsters::IntentKind;
use crate::generated::orbs::OrbData;
use crate::generated::potions::PotionPool;
use crate::generated::powers::PowerData;
use crate::generated::relics::RelicData;
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
    Lists
    + Tables
    + ContentVersion<
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
        Card: CardIds + CardTable<Rarity = CardRarity, Tag = CardTag>,
        CardVar: CardVarIds,
        Character: CharacterIds,
        Enchantment: EnchantmentIds,
        Encounter: EncounterIds,
        Event: EventIds,
        Modifier: ModifierIds,
        Monster: MonsterIds,
        MoveId: MoveIds,
        Orb: OrbIds + Table<Data = OrbData>,
        Potion: PotionIds + PotionTable<Pool = PotionPool>,
        Power: PowerIds + Table<Data = PowerData>,
        Relic: RelicIds + Table<Data = RelicData>,
        RestSiteOption: RestSiteOptionIds,
    >
{
}

impl BuildContent for crate::v0_107_1::V0_107_1 {}

impl BuildContent for crate::v0_111_0::V0_111_0 {}

/// The lists a shared rule hands out whole.
///
/// A list a rule hands out lives as long as the program, and a list of a
/// generic type cannot. So each build holds its own, of its own types. The
/// `lists!` macro beside each rule writes them for a build.
pub trait Lists: ContentVersion {
    const ALL_CHARACTERS: &'static [Self::Character];
    const ARCHAIC_TOOTH_TRANSCENDENCE: &'static [(Self::Card, Self::Card)];
    const CALLING_BELL_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const CAULDRON_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const CHOICES_PARADOX_RETAIN: &'static [(sts2_core::Change<Self>, sts2_core::Lasting)];
    const CROSSBOW_FREE_THIS_TURN: &'static [(sts2_core::Change<Self>, sts2_core::Lasting)];
    const DREAM_CATCHER_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const GLASS_EYE_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const LEAFY_POULTICE_PICKS: &'static [sts2_core::DeckPick<Self>];
    const LOST_COFFER_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const RELICS_MODIFIER_CURSES: &'static [Self::Card];
    const MUSIC_BOX_ETHEREAL: &'static [(sts2_core::Change<Self>, sts2_core::Lasting)];
    const NEOWS_TALISMAN_PICKS: &'static [sts2_core::DeckPick<Self>];
    const ORRERY_REWARD: sts2_core::CustomReward<Self>;
    const ORRERY_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const TINY_MAILBOX_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const TOUCH_OF_OROBAS_REFINEMENTS: &'static [(Self::Relic, Self::Relic)];
    const TOY_BOX_WAX_RELICS: &'static [sts2_core::CustomReward<Self>];
    const VEXING_PUZZLEBOX_FREE_THIS_TURN: &'static [(
        sts2_core::Change<Self>,
        sts2_core::Lasting,
    )];
    fn powers_stunned() -> &'static sts2_core::NextMove<Self>;
    fn powers_revive() -> &'static sts2_core::NextMove<Self>;
    fn powers_spawned() -> &'static sts2_core::NextMove<Self>;
    const POWERS_VALID_ORBS: &'static [Self::Orb];
}

/// The lookups into a build's tables a shared rule makes, where the row they
/// read names content and so is each build's own.
pub trait Tables: ContentVersion {
    /// The cards a character's pool holds, in the order its model generates
    /// them, which is `CardPool.AllCards`.
    fn pool_cards(character: Self::Character) -> &'static [Self::Card];

    /// Where `wanted` stands among the monster's states, as a move-state
    /// index. `at` is the index the caller read, for the error.
    fn monster_state_of(
        monster: Self::Monster,
        wanted: Self::MoveId,
        at: u8,
    ) -> Result<u8, sts2_core::ContentError>;
}

/// [`Tables`] for one build, off its own generated tables.
macro_rules! tables {
    ($build:ty, $module:ident) => {
        impl Tables for $build {
            fn pool_cards(character: Self::Character) -> &'static [Self::Card] {
                crate::$module::cards::pool_cards(Table::data(character).card_pool)
            }

            fn monster_state_of(
                monster: Self::Monster,
                wanted: Self::MoveId,
                at: u8,
            ) -> Result<u8, sts2_core::ContentError> {
                Table::data(monster)
                    .states
                    .iter()
                    .position(|state| {
                        matches!(state, crate::$module::monsters::State::Move(held) if held.id == wanted)
                    })
                    .and_then(|found| u8::try_from(found).ok())
                    .ok_or(sts2_core::ContentError::InvalidMonsterState { index: at })
            }
        }
    };
}

tables!(crate::v0_107_1::V0_107_1, v0_107_1);
tables!(crate::v0_111_0::V0_111_0, v0_111_0);

/// Every rule's lists, for one build.
macro_rules! lists {
    ($build:ty) => {
        impl Lists for $build {
            crate::behaviour::lists!($build);
            crate::behaviour::relics::archaic_tooth::lists!($build);
            crate::behaviour::relics::calling_bell::lists!($build);
            crate::behaviour::relics::cauldron::lists!($build);
            crate::behaviour::relics::choices_paradox::lists!($build);
            crate::behaviour::relics::crossbow::lists!($build);
            crate::behaviour::relics::dream_catcher::lists!($build);
            crate::behaviour::relics::glass_eye::lists!($build);
            crate::behaviour::relics::leafy_poultice::lists!($build);
            crate::behaviour::relics::lost_coffer::lists!($build);
            crate::behaviour::relics::lists!($build);
            crate::behaviour::relics::music_box::lists!($build);
            crate::behaviour::relics::neows_talisman::lists!($build);
            crate::behaviour::relics::orrery::lists!($build);
            crate::behaviour::relics::tiny_mailbox::lists!($build);
            crate::behaviour::relics::touch_of_orobas::lists!($build);
            crate::behaviour::relics::toy_box::lists!($build);
            crate::behaviour::relics::vexing_puzzlebox::lists!($build);
            crate::behaviour::powers::lists!($build);
        }
    };
}

lists!(crate::v0_107_1::V0_107_1);
lists!(crate::v0_111_0::V0_111_0);

/// The rules a build ports for itself, where the game changed one, which a
/// shared dispatcher hands over.
pub trait OwnRules: ContentVersion + Sized {
    /// A relic's `EventHook`, for a relic whose rule is the build's own.
    fn own_relic_event_hook(
        state: &mut crate::behaviour::relics::State<Self>,
        hook: sts2_core::EventHook<Self>,
        asked: &mut sts2_core::HookEffects<Self>,
    ) where
        Self: Build;

    /// Whether a power's rule is the build's own.
    fn own_power(power: Self::Power) -> bool;

    /// The state a power of the build's own starts with, or nothing for a
    /// shared one.
    fn own_power_state(power: Self::Power) -> Option<crate::behaviour::powers::State<Self>>
    where
        Self: Build;

    /// `ModifyHook` for a power of the build's own, or nothing for a hook the
    /// shared powers answer for it.
    fn own_power_modify_hook(
        power: Self::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, Self>,
        state: &crate::behaviour::powers::State<Self>,
        hook: sts2_core::ModifyHook<Self>,
    ) -> Option<sts2_core::Modify<i32>>
    where
        Self: Build;

    /// `EventHook` for a power of the build's own, or nothing for a hook the
    /// shared powers answer for it.
    fn own_power_event_hook(
        power: Self::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, Self>,
        state: &mut crate::behaviour::powers::State<Self>,
        hook: sts2_core::EventHook<Self>,
    ) -> Option<Option<sts2_core::HookEffect<Self>>>
    where
        Self: Build;
}

/// A build the shared behaviour runs in: its content, and its rules, which
/// a shared rule calls back into.
///
/// A shared rule that waits on a question names where it resumes as its own
/// module's `Resume`, which the build's wraps.
pub trait Build:
    BuildContent
    + Rules<
        PowerState = crate::behaviour::powers::State<Self>,
        EncounterState = crate::behaviour::encounters::State,
    > + OwnRules
    + ContentVersion<Resume: From<crate::behaviour::potions::Resume>>
{
}

impl<V> Build for V where
    V: BuildContent
        + Rules<
            PowerState = crate::behaviour::powers::State<V>,
            EncounterState = crate::behaviour::encounters::State,
        > + OwnRules
        + ContentVersion<Resume: From<crate::behaviour::potions::Resume>>
{
}
