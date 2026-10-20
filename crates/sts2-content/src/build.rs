//! What a rule shared between builds needs of the build it runs in.
//!
//! The behaviour is written once and is generic over the build. It names
//! content through the consts in [`crate::ids`], and reads a row through
//! `sts2_core::Table`. What every build spells alike is one type, in
//! [`crate::generated`]. Each bound here is one a shared rule reads.

use sts2_core::{CardTable, ContentVersion, PotionTable, RelicTable, Rules, Table};

use crate::generated::ascension::AscensionLevel;
use crate::generated::cards::{CardKeyword, CardRarity, CardTag, KeywordSet, TargetType};
use crate::generated::encounters::RoomType;
use crate::generated::monsters::IntentKind;
use crate::generated::orbs::OrbData;
use crate::generated::potions::{PotionData, PotionPool};
use crate::generated::powers::PowerData;
use crate::generated::relics::{RelicData, RelicRarity};
use crate::generated::runstate::{
    CombatSide, MapPointType, PlayerRngType, PlayerTurnPhase, RunRngType, UnplayableReason,
    UnplayableReasons,
};
use crate::ids::{
    ActIds, AfflictionIds, CardIds, CardVarIds, CharacterIds, EnchantmentIds, EncounterIds,
    EventIds, EventOptionIds, ModifierIds, MonsterIds, MoveIds, OrbIds, PotionIds, PowerIds,
    RelicIds, RestSiteOptionIds,
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
        EventOption: EventOptionIds,
        EventOptionId = crate::behaviour::EventOptionId<Self>,
        Modifier: ModifierIds,
        Monster: MonsterIds,
        MoveId: MoveIds,
        Orb: OrbIds + Table<Data = OrbData>,
        Potion: PotionIds + PotionTable<Pool = PotionPool> + Table<Data = PotionData>,
        Power: PowerIds + Table<Data = PowerData>,
        Relic: RelicIds + Table<Data = RelicData> + RelicTable<Rarity = RelicRarity>,
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
    const GLORY_DEFENSE_SPAWNS: &'static [Self::Monster];
    const GLORY_AGGRO_SPAWNS: &'static [Self::Monster];
    const COLORFUL_PHILOSOPHERS_COLOR_ORDER: &'static [(Self::Character, Self::EventOption)];
    const COLORFUL_PHILOSOPHERS_REWARDS: &'static [[sts2_core::CustomReward<Self>; 3]];
    const COLOSSAL_FLOWER_PAGES: &'static [[Self::EventOption; 2]];
    const DARV_SETS: &'static [(Option<usize>, &[Self::Relic])];
    const DOLL_ROOM_DOLLS: &'static [Self::Relic];
    const DOLL_ROOM_DOLLS_BY_ID: &'static [Self::Relic];
    const DROWNING_BEACON_BOTTLE: &'static [sts2_core::CustomReward<Self>];
    const ENDLESS_CONVEYOR_CONDIMENT: &'static [sts2_core::CustomReward<Self>];
    const FAKE_MERCHANT_INVENTORY: &'static [Self::Relic];
    const HUNGRY_FOR_MUSHROOMS_OFFERED: &'static [(Self::EventOption, Self::Relic)];
    const NONUPEIPE_POOL: &'static [Self::Relic];
    const OROBAS_POOL_ONE: &'static [Self::Relic];
    const OROBAS_POOL_TWO: &'static [Self::Relic];
    const PAEL_POOL_ONE: &'static [Self::Relic];
    const PAEL_POOL_THREE: &'static [Self::Relic];
    const POTION_COURIER_FOUL: &'static [sts2_core::CustomReward<Self>];
    const RELIC_TRADER_PULLED: &'static [sts2_core::RelicPull<Self>];
    const RELIC_TRADER_OPTIONS: &'static [Self::EventOption];
    const SELF_HELP_BOOK_READINGS: &'static [(
        Self::EventOption,
        Self::Enchantment,
        sts2_core::CardKind,
    )];
    const TANX_POOL: &'static [Self::Relic];
    const TEZCATARA_POOL_ONE: &'static [Self::Relic];
    const TEZCATARA_POOL_TWO: &'static [Self::Relic];
    const TEZCATARA_POOL_THREE: &'static [Self::Relic];
    const THE_FUTURE_OF_POTIONS_REWARDS: &'static [[sts2_core::CustomReward<Self>; 1]];
    const THE_LANTERN_KEY_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const TRASH_HEAP_RELICS: &'static [Self::Relic];
    const TRASH_HEAP_CARDS: &'static [Self::Card];
    const TRIAL_REWARDS: &'static [sts2_core::CustomReward<Self>];
    const VAKUU_POOL_ONE: &'static [Self::Relic];
    const VAKUU_POOL_TWO: &'static [Self::Relic];
    const VAKUU_POOL_THREE: &'static [Self::Relic];
    const WAR_HISTORIAN_REPY_CHEST: &'static [sts2_core::CustomReward<Self>];
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

    /// The orbs a random channel draws from, in the order a roll reads them.
    fn rollable_orbs() -> &'static [Self::Orb];

    /// The pool a card is filed under.
    fn card_filed(card: Self::Card) -> crate::behaviour::cards::CardFiled;

    /// Where a monster's moves are filed.
    fn filed(monster: Self::Monster) -> crate::generated::monster_moves::Filed;

    /// The branches of a monster's conditional state at `at`, or nothing for
    /// any other state.
    fn monster_conditional(
        monster: Self::Monster,
        at: u8,
    ) -> Option<&'static [crate::generated::monsters::StateIndex]>;

    /// A relic's place in the build's table, which is `ModelDb` order.
    fn relic_index(relic: Self::Relic) -> u16;

    /// The bounds a monster's health is rolled between, lowest first.
    fn monster_hp(monster: Self::Monster) -> (crate::Scaled<u16>, crate::Scaled<u16>);
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

            fn rollable_orbs() -> &'static [Self::Orb] {
                &crate::$module::orbs::ROLLABLE
            }

            fn card_filed(card: Self::Card) -> crate::behaviour::cards::CardFiled {
                use crate::$module::cards::Cards;
                use crate::behaviour::cards::CardFiled;

                match card {
                    Cards::Ironclad(_) => CardFiled::Ironclad,
                    Cards::Silent(_) => CardFiled::Silent,
                    Cards::Regent(_) => CardFiled::Regent,
                    Cards::Necrobinder(_) => CardFiled::Necrobinder,
                    Cards::Defect(_) => CardFiled::Defect,
                    Cards::Colorless(_) => CardFiled::Colorless,
                    Cards::Curse(_) => CardFiled::Curse,
                    Cards::Event(_) => CardFiled::Event,
                    Cards::Quest(_) => CardFiled::Quest,
                    Cards::Status(_) => CardFiled::Status,
                    Cards::Token(_) => CardFiled::Token,
                    Cards::Deprecated(_) => CardFiled::Deprecated,
                }
            }

            fn filed(monster: Self::Monster) -> crate::generated::monster_moves::Filed {
                crate::$module::monster_moves::filed(monster)
            }

            fn monster_conditional(
                monster: Self::Monster,
                at: u8,
            ) -> Option<&'static [crate::generated::monsters::StateIndex]> {
                match Table::data(monster).states.get(usize::from(at)) {
                    Some(crate::$module::monsters::State::Conditional(branches)) => Some(branches),
                    _ => None,
                }
            }

            fn relic_index(relic: Self::Relic) -> u16 {
                relic as u16
            }

            fn monster_hp(monster: Self::Monster) -> (crate::Scaled<u16>, crate::Scaled<u16>) {
                let data = Table::data(monster);
                (data.min_hp, data.max_hp)
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
            crate::behaviour::monsters::glory::lists!($build);
            crate::behaviour::events::colorful_philosophers::lists!($build);
            crate::behaviour::events::colossal_flower::lists!($build);
            crate::behaviour::events::darv::lists!($build);
            crate::behaviour::events::doll_room::lists!($build);
            crate::behaviour::events::drowning_beacon::lists!($build);
            crate::behaviour::events::endless_conveyor::lists!($build);
            crate::behaviour::events::fake_merchant::lists!($build);
            crate::behaviour::events::hungry_for_mushrooms::lists!($build);
            crate::behaviour::events::nonupeipe::lists!($build);
            crate::behaviour::events::orobas::lists!($build);
            crate::behaviour::events::pael::lists!($build);
            crate::behaviour::events::potion_courier::lists!($build);
            crate::behaviour::events::relic_trader::lists!($build);
            crate::behaviour::events::self_help_book::lists!($build);
            crate::behaviour::events::tanx::lists!($build);
            crate::behaviour::events::tezcatara::lists!($build);
            crate::behaviour::events::the_future_of_potions::lists!($build);
            crate::behaviour::events::the_lantern_key::lists!($build);
            crate::behaviour::events::trash_heap::lists!($build);
            crate::behaviour::events::trial::lists!($build);
            crate::behaviour::events::vakuu::lists!($build);
            crate::behaviour::events::war_historian_repy::lists!($build);
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

    /// What a card of the build's own does when it is played, or nothing for
    /// a shared one.
    fn own_card_play<F: sts2_core::Fight<Self>>(
        card: Self::Card,
        fight: &mut F,
    ) -> Option<sts2_core::Played<Self>>
    where
        Self: Build;

    /// A number a card of the build's own works out, or nothing for a shared
    /// one.
    fn own_card_calculated<P: sts2_core::Printed<Self> + ?Sized>(
        card: Self::Card,
        key: Self::CardVar,
        printed: &P,
        owner: sts2_core::CardOwner<'_, Self>,
    ) -> Option<i32>
    where
        Self: Build;

    /// What a card's first upgrade writes, as the build's generated scripts
    /// have it.
    fn generated_upgrade(card: Self::Card) -> &'static [sts2_core::Change<Self>];

    /// A card's play from the command at `from` on, as the build's generated
    /// scripts have it, or the command a question stopped it at.
    fn generated_play_from<F: sts2_core::Fight<Self>>(
        card: Self::Card,
        fight: &mut F,
        from: u8,
    ) -> Result<sts2_core::Played<Self>, u8>
    where
        Self: Build;

    /// What a card in the hand does as its owner's turn ends, as the build's
    /// generated scripts have it.
    fn generated_turn_end<F: sts2_core::Fight<Self>>(card: Self::Card, fight: &mut F)
    where
        Self: Build;

    /// A move the build's generated scripts perform, from `step`, and the
    /// step to go on from where a command asked the player something.
    fn generated_move<F: sts2_core::Fight<Self>>(
        monster: Self::Monster,
        move_id: Self::MoveId,
        step: u8,
        fight: &mut F,
    ) -> Option<u8>
    where
        Self: Build;

    /// The events whose rules the build ports for itself, as one
    /// `EventBehavior` over their states.
    type OwnEvents: sts2_core::EventBehavior<Self> + Default + Clone + std::fmt::Debug;

    /// `IsAllowed` for an event whose rule is the build's own, or nothing for
    /// a shared one.
    fn own_event_allowed(event: Self::Event, run: &impl sts2_core::EventView<Self>)
    -> Option<bool>;

    /// An event whose rule is the build's own, entered, or nothing for a
    /// shared one.
    fn own_event_enter<R: sts2_core::rng::GameRng>(
        event: Self::Event,
        run: &impl sts2_core::EventView<Self>,
        rng: &mut R,
    ) -> Option<<Self::OwnEvents as sts2_core::EventBehavior<Self>>::State>;

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
        MonsterState = crate::behaviour::monsters::Model<Self>,
        CardState = crate::behaviour::cards::State,
        CardFields = crate::behaviour::cards::Fields,
    > + OwnRules
    + ContentVersion<
        Resume: From<crate::behaviour::potions::Resume>
                    + From<crate::behaviour::monsters::Resume>
                    + From<crate::behaviour::cards::Resume<Self>>,
        CardWrite = crate::behaviour::cards::CardWrite,
    >
{
}

impl<V> Build for V where
    V: BuildContent
        + Rules<
            PowerState = crate::behaviour::powers::State<V>,
            EncounterState = crate::behaviour::encounters::State,
            MonsterState = crate::behaviour::monsters::Model<V>,
            CardState = crate::behaviour::cards::State,
            CardFields = crate::behaviour::cards::Fields,
        > + OwnRules
        + ContentVersion<
            Resume: From<crate::behaviour::potions::Resume>
                        + From<crate::behaviour::monsters::Resume>
                        + From<crate::behaviour::cards::Resume<V>>,
            CardWrite = crate::behaviour::cards::CardWrite,
        >
{
}
