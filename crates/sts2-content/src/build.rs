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
        Enchantment: EnchantmentIds + crate::vars::EnchantmentVars,
        Encounter: EncounterIds,
        Event: EventIds,
        EventOption: EventOptionIds,
        EventOptionId = crate::behaviour::EventOptionId<Self>,
        Modifier: ModifierIds,
        Monster: MonsterIds + Table<Data = crate::generated::monsters::MonsterData<Self::MoveId>>,
        MoveId: MoveIds,
        Orb: OrbIds + Table<Data = OrbData>,
        Potion: PotionIds
                    + crate::vars::PotionVars
                    + PotionTable<Pool = PotionPool>
                    + Table<Data = PotionData>,
        Power: PowerIds + crate::vars::PowerVars + Table<Data = PowerData>,
        Relic: RelicIds
                   + crate::vars::RelicVars
                   + Table<Data = RelicData>
                   + RelicTable<Rarity = RelicRarity>,
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
    const DARV_SETS: &'static [(crate::behaviour::events::darv::Acts, &[Self::Relic])];
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

    /// A character pool's unlocked cards, in the pool's own order.
    fn merchant_pool_cards(
        pool: <Self::Card as sts2_core::CardTable>::Pool,
    ) -> &'static [Self::Card];

    /// The cards of a character pool a fight can generate.
    fn combat_pool_cards(pool: <Self::Card as sts2_core::CardTable>::Pool)
    -> &'static [Self::Card];

    /// The unlocked curses.
    fn unlocked_curse_cards() -> &'static [Self::Card];

    /// The curses a fight can generate.
    fn combat_curse_cards() -> &'static [Self::Card];

    /// The unlocked statuses.
    fn unlocked_status_cards() -> &'static [Self::Card];

    /// The statuses a fight can generate.
    fn combat_status_cards() -> &'static [Self::Card];
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

            fn merchant_pool_cards(
                pool: <Self::Card as sts2_core::CardTable>::Pool,
            ) -> &'static [Self::Card] {
                crate::$module::cards::merchant_cards(pool)
            }

            fn combat_pool_cards(
                pool: <Self::Card as sts2_core::CardTable>::Pool,
            ) -> &'static [Self::Card] {
                crate::$module::cards::combat_cards(pool)
            }

            fn unlocked_curse_cards() -> &'static [Self::Card] {
                crate::$module::cards::unlocked_curse_cards()
            }

            fn combat_curse_cards() -> &'static [Self::Card] {
                crate::$module::cards::combat_curse_cards()
            }

            fn unlocked_status_cards() -> &'static [Self::Card] {
                crate::$module::cards::unlocked_status_cards()
            }

            fn combat_status_cards() -> &'static [Self::Card] {
                crate::$module::cards::combat_status_cards()
            }
        }
    };
}

tables!(crate::v0_107_1::V0_107_1, v0_107_1);
tables!(crate::v0_111_0::V0_111_0, v0_111_0);

/// Every rule's lists, for one build.
macro_rules! lists {
    ($build:ty, $module:ident) => {
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
            crate::behaviour::events::darv::lists!($build, $module);
            crate::behaviour::events::doll_room::lists!($build);
            crate::behaviour::events::drowning_beacon::lists!($build);
            crate::behaviour::events::endless_conveyor::lists!($build);
            crate::behaviour::events::fake_merchant::lists!($build);
            crate::behaviour::events::hungry_for_mushrooms::lists!($build);
            crate::behaviour::events::nonupeipe::lists!($build);
            crate::behaviour::events::orobas::lists!($build, $module);
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

lists!(crate::v0_107_1::V0_107_1, v0_107_1);
lists!(crate::v0_111_0::V0_111_0, v0_111_0);

/// The rules a build ports for itself, where the game changed one, which a
/// shared dispatcher hands over.
pub trait OwnRules: ContentVersion + Sized {
    /// The deck cards whose rules the build ports for itself, as what each
    /// saves, which the shared cards hold as `State::Own`.
    type OwnCard: Copy + std::fmt::Debug + Eq + std::hash::Hash;

    /// What a deck card of the build's own saves as it is made, or nothing
    /// for a shared one.
    fn own_card_state(card: Self::Card) -> Option<Self::OwnCard>;

    /// What a deck card of the build's own writes on its fight copy, from
    /// what it saved.
    fn own_card_written(own: Self::OwnCard) -> Option<sts2_core::Change<Self>>;

    /// A deck card's `EventHook`, for a card whose rule is the build's own.
    fn own_deck_card_event_hook(
        card: Self::Card,
        own: &mut Self::OwnCard,
        hook: sts2_core::EventHook<Self>,
        asked: &mut sts2_core::HookEffects<Self>,
    );

    /// The relics whose rules the build ports for itself, as the state each
    /// holds, which the shared relics hold as `State::Own`.
    type OwnRelic: Clone + std::fmt::Debug + Eq;

    /// The state a relic of the build's own starts with, or nothing for a
    /// shared one.
    fn own_relic_state(relic: Self::Relic) -> Option<Self::OwnRelic>;

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

    /// A move whose rule is the build's own, performed from `step`, or
    /// nothing for a shared move. It holds the step to go on from where a
    /// command asked the player something.
    fn own_move<F: sts2_core::Fight<Self>>(
        monster: Self::Monster,
        move_id: Self::MoveId,
        step: u8,
        fight: &mut F,
    ) -> Option<Option<u8>>
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

    /// A power's `AfterDeath` commands, for a power whose death the build
    /// ports for itself, or nothing for a shared one.
    fn own_power_death<F: sts2_core::Fight<Self>>(
        power: Self::Power,
        holder: sts2_core::CombatId,
        fight: &mut F,
    ) -> Option<()>
    where
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

    /// A yes-or-no hook for a power of the build's own, or nothing for a
    /// hook the shared powers answer for it.
    fn own_power_decide_hook(
        power: Self::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, Self>,
        state: &crate::behaviour::powers::State<Self>,
        hook: sts2_core::DecisionHook<Self>,
    ) -> Option<sts2_core::HookResponse<bool>>
    where
        Self: Build;

    /// What a potion of the build's own does when it is drunk in a fight, or
    /// nothing for a shared one.
    fn own_potion_use<F: sts2_core::Fight<Self>>(
        potion: Self::Potion,
        fight: &mut F,
    ) -> Option<sts2_core::Played<Self>>
    where
        Self: Build;

    /// What a potion of the build's own asks of the run, or nothing for a
    /// shared one.
    fn own_potion_use_on_run(
        potion: Self::Potion,
        run: &impl sts2_core::EventView<Self>,
        in_fight: bool,
    ) -> Option<sts2_core::HookEffects<Self>>;
}

/// A build the shared behaviour runs in: its content, and its rules, which
/// a shared rule calls back into.
///
/// A shared rule that waits on a question names where it resumes as its own
/// module's `Resume`, which [`crate::behaviour::Resume`] wraps.
pub trait Build:
    BuildContent
    + Rules<
        PowerState = crate::behaviour::powers::State<Self>,
        EncounterState = crate::behaviour::encounters::State,
        MonsterState = crate::behaviour::monsters::Model<Self>,
        RelicState = crate::behaviour::relics::State<Self>,
        ModifierState = crate::behaviour::modifiers::State,
        EventState = crate::behaviour::events::State<Self>,
        CardState = crate::behaviour::cards::State<Self>,
        CardFields = crate::behaviour::cards::Fields,
    > + OwnRules
    + ContentVersion<
        Resume = crate::behaviour::Resume<Self>,
        EnchantmentState = crate::behaviour::enchantments::State,
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
            RelicState = crate::behaviour::relics::State<V>,
            ModifierState = crate::behaviour::modifiers::State,
            EventState = crate::behaviour::events::State<V>,
            CardState = crate::behaviour::cards::State<V>,
            CardFields = crate::behaviour::cards::Fields,
        > + OwnRules
        + ContentVersion<
            Resume = crate::behaviour::Resume<V>,
            EnchantmentState = crate::behaviour::enchantments::State,
            CardWrite = crate::behaviour::cards::CardWrite,
        >
{
}

/// Each build's `Hooks` and `Rules`, as forwards to the functions in
/// `crate::behaviour::rules`, which do the work for every build.
///
/// The impls sit in an anonymous const, whose `use` names the core types the
/// traits spell their methods with.
macro_rules! rules {
    ($build:ty, $module:ident) => {
        const _: () = {
            use sts2_core::*;

            impl sts2_core::Hooks for $build {
                type RelicState = crate::behaviour::relics::State<$build>;
                type ModifierState = crate::behaviour::modifiers::State;
                type PowerState = crate::behaviour::powers::State<$build>;
                type MonsterState = crate::behaviour::monsters::Model<$build>;
                type EncounterState = crate::behaviour::encounters::State;
                type CardState = crate::behaviour::cards::State<$build>;
                type CardFields = crate::behaviour::cards::Fields;

                fn power_noted(state: &mut Self::PowerState, note: Self::Number) {
                    crate::behaviour::rules::power_noted::<Self>(state, note)
                }

                fn power_kept_card(state: &mut Self::PowerState, kept: sts2_core::CardId<Self>) {
                    crate::behaviour::rules::power_kept_card::<Self>(state, kept)
                }

                fn modify_hook<P: Printed<Self> + ?Sized>(
                    listener: Listener<'_, Self, P>,
                    hook: ModifyHook<Self>,
                ) -> Modify<Self::Number> {
                    crate::behaviour::rules::modify_hook::<Self, P>(listener, hook)
                }

                fn decide_hook<P: Printed<Self> + ?Sized>(
                    listener: Listener<'_, Self, P>,
                    hook: DecisionHook<Self>,
                ) -> sts2_core::HookResponse<bool> {
                    crate::behaviour::rules::decide_hook::<Self, P>(listener, hook)
                }

                fn power_event_hook(
                    power: EventPower<'_, Self>,
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::power_event_hook::<Self>(power, hook, asked)
                }

                fn keyword_while_afflicted(
                    affliction: Self::Affliction,
                ) -> Option<Self::CardKeyword> {
                    crate::behaviour::rules::keyword_while_afflicted::<Self>(affliction)
                }

                fn card_eternal(card: &sts2_core::CardIdent<Self>) -> bool {
                    crate::behaviour::rules::card_eternal::<Self>(card)
                }

                fn enchanted_changes(enchantment: Self::Enchantment) -> &'static [Change<Self>] {
                    crate::behaviour::rules::enchanted_changes::<Self>(enchantment)
                }

                fn card_result_pile(
                    card: Self::Card,
                    base: Option<sts2_core::Pile>,
                ) -> sts2_core::HookResponse<Option<sts2_core::Pile>> {
                    crate::behaviour::rules::card_result_pile::<Self>(card, base)
                }

                fn write_deck_card(card: &mut DeckCard<Self>, write: Self::DeckWrite) {
                    crate::behaviour::rules::write_deck_card::<Self>(card, write)
                }

                fn card_is_playable(
                    card: Self::Card,
                    fight: CardLogic,
                ) -> sts2_core::HookResponse<bool> {
                    crate::behaviour::rules::card_is_playable::<Self>(card, fight)
                }

                fn result_pile<P: Printed<Self> + ?Sized>(
                    listener: Listener<'_, Self, P>,
                    hook: ResultPile<Self>,
                ) -> sts2_core::HookResponse<(Option<sts2_core::Pile>, PilePosition)> {
                    crate::behaviour::rules::result_pile::<Self, P>(listener, hook)
                }

                fn deck_card_event_hook(
                    card: Self::Card,
                    state: &mut Self::CardState,
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::deck_card_event_hook::<Self>(card, state, hook, asked)
                }

                fn modifier_event_hook(
                    modifier: sts2_core::RunModifier<Self>,
                    state: &mut Self::ModifierState,
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::modifier_event_hook::<Self>(
                        modifier, state, hook, asked,
                    )
                }

                fn modifier_ported(modifier: Self::Modifier) -> bool {
                    crate::behaviour::rules::modifier_ported::<Self>(modifier)
                }

                fn relic_event_hook(
                    state: &mut Self::RelicState,
                    belt: &[Self::Relic],
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::relic_event_hook::<Self>(state, belt, hook, asked)
                }

                fn potion_event_hook(
                    potion: Self::Potion,
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::potion_event_hook::<Self>(potion, hook, asked)
                }

                fn monster_event_hook(
                    monster: Self::Monster,
                    model: &mut Self::MonsterState,
                    creature: sts2_core::CombatId,
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::monster_event_hook::<Self>(
                        monster, model, creature, hook, asked,
                    )
                }

                fn enchant_modify(
                    enchanted: sts2_core::Enchanted<Self>,
                    state: &Self::EnchantmentState,
                    hook: EnchantHook,
                ) -> Modify<Self::Number> {
                    crate::behaviour::rules::enchant_modify::<Self>(enchanted, state, hook)
                }

                fn enchantment_event_hook(
                    enchanted: sts2_core::Enchanted<Self>,
                    id: sts2_core::CardId<Self>,
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::enchantment_event_hook::<Self>(
                        enchanted, id, hook, asked,
                    )
                }

                fn card_event_hook<P: Printed<Self> + ?Sized>(
                    card: Self::Card,
                    id: sts2_core::CardId<Self>,
                    pile: Option<sts2_core::Pile>,
                    printed: &P,
                    history: History<'_, Self>,
                    hook: EventHook<Self>,
                    asked: &mut HookEffects<Self>,
                ) {
                    crate::behaviour::rules::card_event_hook::<Self, P>(
                        card, id, pile, printed, history, hook, asked,
                    )
                }
            }

            impl sts2_core::Rules for $build {
                type EventState = crate::behaviour::events::State<$build>;

                fn relic_pool(character: Self::Character) -> <Self::Relic as RelicTable>::Pool {
                    Table::data(character).relic_pool
                }

                fn act_layout(act: Self::Act) -> Layout {
                    crate::behaviour::rules::act_layout::<Self>(act)
                }

                fn starting_deck(character: Self::Character) -> Vec<Self::Card> {
                    Table::data(character).deck.iter().collect()
                }

                fn ascension_deck(ascension: Self::AscensionLevel) -> Option<Self::Card> {
                    crate::behaviour::rules::ascension_deck::<Self>(ascension)
                }

                fn starting_relics(character: Self::Character) -> Vec<Self::Relic> {
                    Table::data(character).starting_relics.iter().collect()
                }

                fn starting_hp(character: Self::Character) -> Self::Health {
                    Table::data(character).starting_hp.into()
                }

                fn starting_gold(character: Self::Character) -> u16 {
                    Table::data(character).starting_gold
                }

                fn max_energy(character: Self::Character) -> u8 {
                    Table::data(character).max_energy
                }

                fn orb_slots(character: Self::Character) -> u8 {
                    Table::data(character).orb_slots
                }

                fn channelled(orb: Self::Orb) -> sts2_core::Channelled<Self> {
                    crate::behaviour::rules::channelled::<Self>(orb)
                }

                fn orb_turn_end<F: Fight<Self>>(
                    orb: &mut sts2_core::Channelled<Self>,
                    fight: &mut F,
                ) {
                    crate::behaviour::rules::orb_turn_end::<Self, F>(orb, fight)
                }

                fn orb_turn_start<F: Fight<Self>>(
                    orb: &mut sts2_core::Channelled<Self>,
                    fight: &mut F,
                ) {
                    crate::behaviour::rules::orb_turn_start::<Self, F>(orb, fight)
                }

                fn enchantment_on_play<F: Fight<Self>>(
                    enchanted: sts2_core::Enchanted<Self>,
                    state: &mut Self::EnchantmentState,
                    fight: &mut F,
                ) {
                    crate::behaviour::rules::enchantment_on_play::<Self, F>(enchanted, state, fight)
                }

                fn orb_evoke<F: Fight<Self>>(
                    orb: &sts2_core::Channelled<Self>,
                    fight: &mut F,
                ) -> Opponents {
                    crate::behaviour::rules::orb_evoke::<Self, F>(orb, fight)
                }

                fn orb_passive<F: Fight<Self>>(
                    orb: &mut sts2_core::Channelled<Self>,
                    fight: &mut F,
                ) {
                    crate::behaviour::rules::orb_passive::<Self, F>(orb, fight)
                }

                fn orb_passive_val(
                    orb: &sts2_core::Channelled<Self>,
                    modify: impl FnOnce(Self::Number) -> Self::Number,
                ) -> Self::Number {
                    crate::behaviour::rules::orb_passive_val::<Self>(orb, modify)
                }

                fn orb_evoke_val(
                    orb: &sts2_core::Channelled<Self>,
                    modify: impl FnOnce(Self::Number) -> Self::Number,
                ) -> Self::Number {
                    crate::behaviour::rules::orb_evoke_val::<Self>(orb, modify)
                }

                fn starting_potion_slots() -> usize {
                    crate::behaviour::rules::starting_potion_slots()
                }

                fn neow_event() -> Self::Event {
                    crate::behaviour::rules::neow_event::<Self>()
                }

                fn colorless_cards() -> Drafted<'static, Self::Card> {
                    crate::$module::cards::colorless_cards()
                }

                fn reward_rarity(card: Self::Card) -> RewardRarity {
                    crate::behaviour::rules::reward_rarity::<Self>(card)
                }

                fn lead_paperweight() -> Self::Relic {
                    crate::behaviour::rules::lead_paperweight::<Self>()
                }

                fn card_upgrade(card: Self::Card) -> &'static [Change<Self>] {
                    crate::behaviour::rules::card_upgrade::<Self>(card)
                }

                fn on_use<F: Fight<Self>>(potion: Self::Potion, fight: &mut F) -> Played<Self> {
                    crate::behaviour::rules::on_use::<Self, F>(potion, fight)
                }

                fn on_use_on_run(
                    potion: Self::Potion,
                    run: &impl sts2_core::EventView<Self>,
                    in_fight: bool,
                ) -> HookEffects<Self> {
                    crate::behaviour::rules::on_use_on_run::<Self>(potion, run, in_fight)
                }

                fn on_play<F: Fight<Self>>(card: Self::Card, fight: &mut F) -> Played<Self> {
                    crate::behaviour::rules::on_play::<Self, F>(card, fight)
                }

                fn on_card_turn_end<F: Fight<Self>>(card: Self::Card, fight: &mut F) {
                    crate::behaviour::rules::on_card_turn_end::<Self, F>(card, fight)
                }

                fn resume_play<F: Fight<Self>>(
                    fight: &mut F,
                    at: Self::Resume,
                    chosen: Taken<Self>,
                ) -> Played<Self> {
                    crate::behaviour::rules::resume_play::<Self, F>(fight, at, chosen)
                }

                fn reward_cards(character: Self::Character) -> Drafted<'static, Self::Card> {
                    crate::$module::cards::reward_cards(character)
                }

                fn combat_cards(character: Self::Character) -> &'static [Self::Card] {
                    crate::$module::cards::combat_cards(Table::data(character).card_pool)
                }

                fn unlocked_cards(character: Self::Character) -> &'static [Self::Card] {
                    crate::$module::cards::merchant_cards(Table::data(character).card_pool)
                }

                fn unlocked_colorless_cards() -> &'static [Self::Card] {
                    crate::$module::cards::merchant_colorless_cards()
                }

                fn combat_colorless_cards() -> &'static [Self::Card] {
                    crate::$module::cards::combat_colorless_cards()
                }

                fn merchant_cards(
                    character: Self::Character,
                    kind: CardKind,
                    rarity: RewardRarity,
                ) -> Vec<Self::Card> {
                    crate::behaviour::rules::merchant_cards::<Self>(character, kind, rarity)
                }

                fn card_kind(card: Self::Card) -> Option<CardKind> {
                    use crate::generated::cards::CardType;

                    let crate::Field::Constant(kind) = Table::data(card).kind else {
                        return None;
                    };
                    Some(match kind {
                        CardType::Attack => CardKind::Attack,
                        CardType::Skill => CardKind::Skill,
                        CardType::Power => CardKind::Power,
                        CardType::Status => CardKind::Status,
                        CardType::Curse => CardKind::Curse,
                        CardType::Quest => CardKind::Quest,
                    })
                }

                fn merchant_colorless_cards(rarity: RewardRarity) -> Vec<Self::Card> {
                    crate::behaviour::rules::merchant_colorless_cards::<Self>(rarity)
                }

                fn transform_options(card: Self::Card, in_combat: bool) -> Vec<Self::Card> {
                    crate::behaviour::rules::transform_options::<Self>(card, in_combat)
                }

                fn reward_potions(character: Self::Character) -> Drafted<'static, Self::Potion> {
                    crate::$module::potions::reward_potions(character)
                }

                fn unlocked_potions(character: Self::Character) -> &'static [Self::Potion] {
                    crate::$module::potions::unlocked_potions(character)
                }

                fn can_enchant(
                    enchantment: Self::Enchantment,
                    card: &sts2_core::CardIdent<Self>,
                ) -> bool {
                    crate::behaviour::rules::can_enchant::<Self>(enchantment, card)
                }

                fn enchant(
                    enchantment: Self::Enchantment,
                    amount: u8,
                    card: &mut sts2_core::CardIdent<Self>,
                ) {
                    crate::behaviour::rules::enchant::<Self>(enchantment, amount, card)
                }

                fn should_start_at_bottom_of_draw_pile(
                    enchantment: Self::Enchantment,
                ) -> sts2_core::HookResponse<bool> {
                    crate::behaviour::rules::should_start_at_bottom_of_draw_pile::<Self>(
                        enchantment,
                    )
                }

                fn encounter_gold(
                    encounter: Self::Encounter,
                    ascension: Self::AscensionLevel,
                ) -> sts2_core::rng::Interval<u16> {
                    let gold = Table::data(encounter).gold;
                    sts2_core::rng::Interval::new(
                        gold.min.at(ascension as u8),
                        gold.max.at(ascension as u8),
                    )
                }

                fn monster_health_after_added(
                    monster: Self::Monster,
                    ascension: Self::AscensionLevel,
                    rolled: Self::Health,
                    teammates: &[Self::Health],
                ) -> Option<Self::Health> {
                    crate::behaviour::rules::monster_health_after_added::<Self>(
                        monster, ascension, rolled, teammates,
                    )
                }

                fn encounter_gold_proportion(
                    encounter: Self::Encounter,
                    state: &Self::EncounterState,
                    escaped: &[Self::Monster],
                ) -> Option<f32> {
                    crate::behaviour::rules::encounter_gold_proportion::<Self>(
                        encounter, state, escaped,
                    )
                }

                fn perform_power_death<F: Fight<Self>>(
                    power: Self::Power,
                    holder: sts2_core::CombatId,
                    fight: &mut F,
                ) {
                    crate::behaviour::rules::perform_power_death::<Self, F>(power, holder, fight)
                }

                fn monster_hp_after_added(
                    monster: Self::Monster,
                    model: &Self::MonsterState,
                    hp: Self::Health,
                ) -> Option<Self::Health> {
                    crate::behaviour::rules::monster_hp_after_added::<Self>(monster, model, hp)
                }

                fn monster_hp_bounds(
                    monster: Self::Monster,
                    ascension: Self::AscensionLevel,
                ) -> sts2_core::rng::Interval<Self::Health> {
                    crate::behaviour::rules::monster_hp_bounds::<Self>(monster, ascension)
                }

                fn roll_monster_move<
                    'a,
                    R: sts2_core::rng::GameRng,
                    P: Printed<Self> + ?Sized + 'a,
                >(
                    monster: Self::Monster,
                    ascension: Self::AscensionLevel,
                    spawn: MonsterSpawn<Self::Encounter>,
                    powers: impl IntoIterator<Item = sts2_core::Listener<'a, Self, P>>,
                    advertised: impl Fn(Self::MoveId) -> bool,
                    state: &Self::MonsterState,
                    machine: &mut MoveMachine,
                    rng: &mut R,
                ) -> Result<Option<sts2_core::NextMove<Self>>, ContentError> {
                    crate::behaviour::rules::roll_monster_move::<Self, R, P>(
                        monster, ascension, spawn, powers, advertised, state, machine, rng,
                    )
                }

                fn follow_up(monster: Self::Monster, machine: &MoveMachine) -> u8 {
                    crate::behaviour::rules::follow_up::<Self>(monster, machine)
                }

                fn can_leave_current_move(monster: Self::Monster, machine: &MoveMachine) -> bool {
                    crate::behaviour::rules::can_leave_current_move::<Self>(monster, machine)
                }

                fn set_move_immediate(
                    monster: Self::Monster,
                    ascension: Self::AscensionLevel,
                    move_id: Self::MoveId,
                    force: bool,
                    machine: &mut MoveMachine,
                ) -> Result<sts2_core::NextMove<Self>, MovePinned> {
                    crate::behaviour::rules::set_move_immediate::<Self>(
                        monster, ascension, move_id, force, machine,
                    )
                }

                fn opening_move_machine(
                    monster: Self::Monster,
                    opens_on: Option<Self::MoveId>,
                ) -> sts2_core::MoveMachine {
                    crate::behaviour::rules::opening_move_machine::<Self>(monster, opens_on)
                }

                fn monster_starting_powers(
                    monster: Self::Monster,
                    ascension: Self::AscensionLevel,
                    side: Self::CombatSide,
                    model: &Self::MonsterState,
                ) -> impl Iterator<Item = (Self::Power, PowerAmount<Self::Number>)> {
                    crate::behaviour::rules::monster_starting_powers::<Self>(
                        monster, ascension, side, model,
                    )
                }

                fn monster_starting_opponent_powers(
                    monster: Self::Monster,
                    ascension: Self::AscensionLevel,
                ) -> impl Iterator<Item = (Self::Power, PowerAmount<Self::Number>)> {
                    crate::behaviour::rules::monster_starting_opponent_powers::<Self>(
                        monster, ascension,
                    )
                }

                fn computed_card_target(
                    card: Self::Card,
                    powers: &[sts2_core::Stack<Self>],
                ) -> Self::TargetType {
                    crate::behaviour::rules::computed_card_target::<Self>(card, powers)
                }

                fn deck_card_narrows_unknown_rooms(card: Self::Card, act: usize) -> bool {
                    crate::behaviour::rules::deck_card_narrows_unknown_rooms::<Self>(card, act)
                }

                fn deck_card_modify_generated_map(
                    card: Self::Card,
                    act: usize,
                ) -> Option<ReplacedMap> {
                    crate::behaviour::rules::deck_card_modify_generated_map::<Self>(card, act)
                }

                fn deck_card_modify_generated_map_late(
                    card: Self::Card,
                    act: usize,
                ) -> Option<Self::Number> {
                    crate::behaviour::rules::deck_card_modify_generated_map_late::<Self>(card, act)
                }

                fn deck_card_after_map_generated(card: Self::Card, act: usize) {
                    crate::behaviour::rules::deck_card_after_map_generated::<Self>(card, act)
                }

                fn deck_card_next_event(
                    card: Self::Card,
                    act: usize,
                    event: Self::Event,
                ) -> Self::Event {
                    crate::behaviour::rules::deck_card_next_event::<Self>(card, act, event)
                }

                fn computed_potion_target(
                    potion: Self::Potion,
                    in_combat: bool,
                ) -> Self::TargetType {
                    crate::behaviour::rules::computed_potion_target::<Self>(potion, in_combat)
                }

                fn computed_potion_check(potion: Self::Potion, in_combat: bool) -> bool {
                    crate::behaviour::rules::computed_potion_check::<Self>(potion, in_combat)
                }

                fn calculated_card_var<P: Printed<Self> + ?Sized>(
                    card: Self::Card,
                    key: Self::CardVar,
                    printed: &P,
                    owner: CardOwner<'_, Self>,
                ) -> Self::Number {
                    crate::behaviour::rules::calculated_card_var::<Self, P>(
                        card, key, printed, owner,
                    )
                }

                fn perform_move<F: Fight<Self>>(
                    monster: Self::Monster,
                    move_id: Self::MoveId,
                    fight: &mut F,
                ) -> Played<Self> {
                    crate::behaviour::rules::perform_move::<Self, F>(monster, move_id, fight)
                }

                fn intent_repeats_now(
                    monster: Self::Monster,
                    move_id: Self::MoveId,
                    model: &Self::MonsterState,
                ) -> Option<Self::Count> {
                    crate::behaviour::rules::intent_repeats_now::<Self>(monster, move_id, model)
                }

                fn resume_move<F: Fight<Self>>(
                    monster: Self::Monster,
                    move_id: Self::MoveId,
                    at: Self::Resume,
                    fight: &mut F,
                ) -> Played<Self> {
                    crate::behaviour::rules::resume_move::<Self, F>(monster, move_id, at, fight)
                }

                fn random_encounter_monsters<R: sts2_core::rng::GameRng>(
                    encounter: Self::Encounter,
                    rng: &mut R,
                ) -> Result<Vec<sts2_core::Spawned<Self>>, ContentError> {
                    crate::behaviour::rules::random_encounter_monsters::<Self, R>(encounter, rng)
                }

                fn encounters(act: Self::Act) -> &'static [Self::Encounter] {
                    crate::$module::encounters::encounters_of(act)
                }

                fn events(act: Self::Act) -> &'static [Self::Event] {
                    crate::$module::events::events_of(act)
                }

                fn ancients(act: Self::Act) -> &'static [Self::Event] {
                    crate::$module::events::ancients_of(act)
                }

                fn shared_events() -> &'static [Self::Event] {
                    &crate::$module::events::SHARED_EVENTS
                }

                fn shared_ancients() -> &'static [Self::Event] {
                    &crate::$module::events::SHARED_ANCIENTS
                }

                fn the_architect() -> Self::Event {
                    crate::behaviour::rules::the_architect::<Self>()
                }

                fn event_allowed(
                    event: Self::Event,
                    run: &impl sts2_core::EventView<Self>,
                ) -> bool {
                    crate::behaviour::rules::event_allowed::<Self>(event, run)
                }

                fn enter_event<R: sts2_core::rng::GameRng>(
                    event: Self::Event,
                    run: &impl sts2_core::EventView<Self>,
                    rng: &mut R,
                ) -> Self::EventState {
                    crate::behaviour::rules::enter_event::<Self, R>(event, run, rng)
                }

                fn set_up_event<R: sts2_core::rng::GameRng>(
                    state: &mut Self::EventState,
                    run: &impl sts2_core::EventView<Self>,
                    rewards: &mut R,
                ) {
                    crate::behaviour::rules::set_up_event::<Self, R>(state, run, rewards)
                }

                fn event_relics_pulled(_state: &Self::EventState) -> &'static [RelicPull<Self>] {
                    crate::behaviour::rules::event_relics_pulled::<Self>(_state)
                }

                fn hand_event_pulled_relics(
                    _state: &mut Self::EventState,
                    _relics: &[Self::Relic],
                ) {
                    crate::behaviour::rules::hand_event_pulled_relics::<Self>(_state, _relics)
                }

                fn event_holds_potions(state: &Self::EventState) -> bool {
                    crate::behaviour::rules::event_holds_potions::<Self>(state)
                }

                fn observe_event(
                    state: &Self::EventState,
                    run: &impl sts2_core::EventView<Self>,
                ) -> sts2_core::Room<Self> {
                    crate::behaviour::rules::observe_event::<Self>(state, run)
                }

                fn choose_event<R: sts2_core::rng::GameRng>(
                    state: &mut Self::EventState,
                    option: u8,
                    run: &impl sts2_core::EventView<Self>,
                    rng: &mut R,
                ) -> sts2_core::HookEffects<Self> {
                    crate::behaviour::rules::choose_event::<Self, R>(state, option, run, rng)
                }

                fn stock_event<R: sts2_core::rng::GameRng>(
                    _state: &mut Self::EventState,
                    _shops: &mut R,
                ) {
                    crate::behaviour::rules::stock_event::<Self, R>(_state, _shops)
                }

                fn buy_in_event(
                    _state: &mut Self::EventState,
                    index: u8,
                ) -> sts2_core::HookEffects<Self> {
                    crate::behaviour::rules::buy_in_event::<Self>(_state, index)
                }

                fn event_question(_state: &Self::EventState) -> Option<sts2_core::Question<Self>> {
                    crate::behaviour::rules::event_question::<Self>(_state)
                }

                fn answer_event(
                    _state: &mut Self::EventState,
                    answer: &sts2_core::Answer,
                ) -> sts2_core::HookEffects<Self> {
                    crate::behaviour::rules::answer_event::<Self>(_state, answer)
                }

                fn resume_event<R: sts2_core::rng::GameRng>(
                    _state: &mut Self::EventState,
                    _run: &impl sts2_core::EventView<Self>,
                    _rng: &mut R,
                ) -> sts2_core::HookEffects<Self> {
                    crate::behaviour::rules::resume_event::<Self, R>(_state, _run, _rng)
                }

                fn resume_event_from_fight(
                    _state: &mut Self::EventState,
                    _escaped: bool,
                ) -> sts2_core::HookEffects<Self> {
                    crate::behaviour::rules::resume_event_from_fight::<Self>(_state, _escaped)
                }

                fn event_rewards(_state: &Self::EventState) -> Vec<CustomReward<Self>> {
                    crate::behaviour::rules::event_rewards::<Self>(_state)
                }

                fn neow_curses() -> &'static [Self::Relic] {
                    &crate::$module::events::NEOW_CURSES
                }

                fn neow_positives() -> &'static [Self::Relic] {
                    &crate::$module::events::NEOW_POSITIVES
                }

                fn neow_flips() -> &'static [(Self::Relic, Self::Relic)] {
                    &crate::$module::events::NEOW_FLIPS
                }

                fn neow_options() -> impl Iterator<Item = Self::Relic> {
                    crate::behaviour::rules::neow_options::<Self>()
                }

                fn neow_pairs() -> &'static [(Self::Relic, Self::Relic)] {
                    &crate::$module::events::NEOW_PAIRS
                }

                fn neow_suppresses_flip() -> Self::Relic {
                    crate::$module::events::NEOW_SUPPRESSES_FLIP
                }

                fn allowed_at_neow(relic: Self::Relic, players: usize) -> bool {
                    crate::behaviour::rules::allowed_at_neow::<Self>(relic, players)
                }

                fn relic_allowed(relic: Self::Relic, floor: usize, players: usize) -> bool {
                    crate::behaviour::rules::relic_allowed::<Self>(relic, floor, players)
                }

                fn run_rng_types() -> &'static [Self::RunRngType] {
                    crate::behaviour::rules::run_rng_types()
                }

                fn player_rng_types() -> &'static [Self::PlayerRngType] {
                    crate::behaviour::rules::player_rng_types()
                }

                fn enemy_side() -> Self::CombatSide {
                    crate::behaviour::rules::enemy_side()
                }
            }
        };
    };
}

pub(crate) use rules;
