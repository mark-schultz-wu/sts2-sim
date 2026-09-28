//! What a relic does, ported.
//!
//! One module per relic, the way `RelicModel`'s subclasses are one file each,
//! and one function here per hook. A relic is ported when a run can pick it
//! up. The game gives hooks to every relic in the belt.
//!
//! [`new`] is the one place a relic becomes a model, and a relic nobody has
//! ported panics there. The belt a run carries is then the list of relics
//! this tree has read.
//!
//! Every hook dispatches on the state alone. The state names the relic that
//! saved it, so no caller can pair a relic with another relic's model.

mod akabeko;
mod alchemical_coffer;
mod amethyst_aubergine;
mod anchor;
mod arcane_scroll;
mod archaic_tooth;
mod astrolabe;
mod bag_of_marbles;
mod bag_of_preparation;
mod beating_remnant;
mod beautiful_bracelet;
mod big_mushroom;
mod biiig_hug;
mod bing_bong;
mod black_blood;
mod black_star;
mod blessed_antler;
mod blood_soaked_rose;
mod blood_vial;
mod bone_flute;
mod bone_tea;
mod book_of_five_rings;
mod booming_conch;
mod bound_phylactery;
mod bowler_hat;
mod bronze_scales;
mod burning_blood;
mod byrdpip;
mod calling_bell;
mod candelabra;
mod centennial_puzzle;
mod chandelier;
mod charons_ashes;
mod chosen_cheese;
mod claws;
mod cloak_clasp;
mod cracked_core;
mod crossbow;
mod cursed_pearl;
mod data_disk;
mod daughter_of_the_wind;
mod delicate_frond;
mod diamond_diadem;
mod distinguished_cape;
mod divine_destiny;
mod divine_right;
mod dollys_mirror;
mod empty_cage;
mod eternal_feather;
mod fake_anchor;
mod fake_blood_vial;
mod fake_happy_flower;
mod fake_lees_waffle;
mod fake_mango;
mod fake_orichalcum;
mod fake_snecko_eye;
mod fake_strike_dummy;
mod fake_venerable_tea_set;
mod festive_popper;
mod fiddle;
mod fishing_rod;
mod forgotten_soul;
mod fragrant_mushroom;
mod frozen_egg;
mod girya;
mod glass_eye;
mod glitter;
mod golden_pearl;
mod gorget;
mod gremlin_horn;
mod happy_flower;
mod hefty_tablet;
mod horn_cleat;
mod ice_cream;
mod infused_core;
mod intimidating_helmet;
mod iron_club;
mod jeweled_mask;
mod jewelry_box;
mod juzu_bracelet;
mod kunai;
mod kusarigama;
mod lantern;
mod lasting_candy;
mod lava_rock;
mod lead_paperweight;
mod leafy_poultice;
mod letter_opener;
mod lizard_tail;
mod looming_fruit;
mod lost_coffer;
mod lost_wisp;
mod lucky_fysh;
mod mango;
mod meal_ticket;
mod meat_on_the_bone;
mod mercury_hourglass;
mod miniature_cannon;
mod molten_egg;
mod mr_struggles;
mod neows_talisman;
mod neows_torment;
mod new_leaf;
mod nunchaku;
mod nutritious_oyster;
mod oddly_smooth_stone;
mod old_coin;
mod orange_dough;
mod orichalcum;
mod ornamental_fan;
mod paels_blood;
mod paels_flesh;
mod paels_horn;
mod paels_tears;
mod pandoras_box;
mod pantograph;
mod paper_phrog;
mod pear;
mod pen_nib;
mod pendulum;
mod permafrost;
mod petrified_toad;
mod phial_holster;
mod phylactery_unbound;
mod pocketwatch;
mod pollinous_core;
mod pomander;
mod potion_belt;
mod prayer_wheel;
mod precarious_shears;
mod precise_scissors;
mod preserved_fog;
mod radiant_pearl;
mod rainbow_ring;
mod red_mask;
mod red_skull;
mod regal_pillow;
mod reptile_trinket;
mod ring_of_the_drake;
mod ring_of_the_snake;
mod ripple_basin;
mod royal_poison;
mod ruined_helmet;
mod runic_pyramid;
mod sai;
mod sand_castle;
mod seal_of_gold;
mod self_forming_clay;
mod sere_talon;
mod shovel;
mod shuriken;
mod signet_ring;
mod snecko_skull;
mod sozu;
mod sparkling_rouge;
mod spiked_gauntlets;
mod stone_calendar;
mod stone_cracker;
mod stone_humidifier;
mod storybook;
mod strawberry;
mod strike_dummy;
mod sturdy_clamp;
mod sword_of_jade;
mod sword_of_stone;
mod symbiotic_virus;
mod tanxs_whistle;
mod throwing_axe;
mod tiny_mailbox;
mod toasty_mittens;
mod touch_of_orobas;
mod toxic_egg;
mod tri_boomerang;
mod tungsten_rod;
mod tuning_fork;
mod twisted_funnel;
mod vajra;
mod venerable_tea_set;
mod very_hot_cocoa;
mod vexing_puzzlebox;
mod war_hammer;
mod war_paint;
mod whetstone;
mod white_beast_statue;
mod white_star;
mod winged_boots;
mod wongos_mystery_ticket;
mod yummy_cookie;

use sts2_core::{EventHook, Modify, ModifyHook, Table};

use super::super::V0_107_1 as V;
use super::super::generated::relics::Relic;

use crate::Field;

pub(crate) use archaic_tooth::TRANSCENDENCE;

pub(crate) type Number = <V as sts2_core::ContentVersion>::Number;

/// `room is CombatRoom`, which is how a relic tells a fight's room apart.
fn combat_room(room: super::super::generated::encounters::RoomType) -> bool {
    use super::super::generated::encounters::RoomType;

    matches!(room, RoomType::Monster | RoomType::Elite | RoomType::Boss)
}

/// `EggRelicHelper.UpgradeValidCards`, and the egg's own check as a card
/// joins the deck: an upgraded clone of an upgradable card of `kind`. A run
/// here has one player, whose cards are all the owner's.
fn egg(kind: sts2_core::CardKind, hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    use sts2_core::CardTable as _;

    if let EventHook::CardRewardOption { card }
    | EventHook::MerchantCardCreated { card }
    | EventHook::CardBeingAddedToDeck { card } = hook
        && <V as sts2_core::Rules>::card_kind(card.card) == Some(kind)
        && card.upgrade < card.card.max_upgrade()
    {
        asked.push(sts2_core::HookEffect::UpgradeCard);
    }
}

/// One relic's own model, as the run carries it.
///
/// A variant per ported relic, holding what that relic's `[SavedProperty]`
/// fields hold. A relic that saves nothing still has a variant of its own.
/// That variant is what says the port has read the relic and found no state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    AlchemicalCoffer,
    Anchor,
    ArcaneScroll,
    ArchaicTooth,
    BiiigHug,
    BoneTea(bone_tea::State),
    BoomingConch,
    BoundPhylactery,
    BurningBlood,
    Byrdpip,
    ChosenCheese,
    CrackedCore,
    CursedPearl,
    DivineRight,
    DollysMirror,
    DustyTome,
    FishingRod(fishing_rod::State),
    FakeHappyFlower(fake_happy_flower::State),
    FakeOrichalcum(fake_orichalcum::State),
    Orichalcum(orichalcum::State),
    FakeVenerableTeaSet(fake_venerable_tea_set::State),
    WongosMysteryTicket(wongos_mystery_ticket::State),
    GlassEye,
    GoldenPearl,
    Gorget,
    GremlinHorn,
    HeftyTablet,
    LavaRock(lava_rock::State),
    LeadPaperweight,
    LeafyPoultice,
    LostCoffer,
    LostWisp,
    MercuryHourglass,
    DaughterOfTheWind,
    MrStruggles,
    BingBong,
    IntimidatingHelmet,
    RunicPyramid,
    TungstenRod,
    TouchOfOrobas,
    BlackBlood,
    RingOfTheDrake,
    DivineDestiny,
    PhylacteryUnbound,
    InfusedCore,
    Circlet,
    RadiantPearl,
    ToastyMittens,
    EmptyCage,
    Astrolabe,
    PandorasBox,
    CallingBell,
    BlackStar,
    Sozu,
    NeowsTalisman,
    NeowsTorment,
    NewLeaf,
    NutritiousOyster,
    PaelsBlood,
    PaelsFlesh(paels_flesh::State),
    PaelsHorn,
    PaelsTears(paels_tears::State),
    Permafrost(permafrost::State),
    PhialHolster,
    Pocketwatch(pocketwatch::State),
    PollinousCore(pollinous_core::State),
    Pomander,
    PrecariousShears,
    PreciseScissors,
    RingOfTheSnake,
    SandCastle,
    SealOfGold,
    StoneHumidifier,
    Storybook,
    StrikeDummy,
    SwordOfStone(sword_of_stone::State),
    VeryHotCocoa,
    FakeAnchor,
    FakeMango,
    FakeLeesWaffle,
    FakeBloodVial,
    BloodVial,
    HornCleat,
    SparklingRouge,
    BowlerHat,
    FakeStrikeDummy,
    BagOfMarbles,
    Akabeko,
    FestivePopper,
    BronzeScales,
    Vajra,
    MealTicket,
    Strawberry,
    WarPaint,
    BigMushroom,
    ForgottenSoul,
    FragrantMushroom,
    Whetstone,
    JuzuBracelet,
    BagOfPreparation,
    WingedBoots(winged_boots::State),
    YummyCookie,
    RedMask,
    Lantern,
    LuckyFysh,
    AmethystAubergine,
    EternalFeather,
    Pantograph,
    FakeSneckoEye,
    CentennialPuzzle(centennial_puzzle::State),
    PenNib(pen_nib::State),
    Mango,
    MiniatureCannon,
    RegalPillow,
    BookOfFiveRings(book_of_five_rings::State),
    Kusarigama(kusarigama::State),
    BeatingRemnant(beating_remnant::State),
    ReptileTrinket,
    Pear,
    DataDisk,
    BoneFlute,
    HappyFlower(happy_flower::State),
    Chandelier,
    Kunai(kunai::State),
    Nunchaku(nunchaku::State),
    VenerableTeaSet(venerable_tea_set::State),
    TwistedFunnel,
    Pendulum(pendulum::State),
    RippleBasin(ripple_basin::State),
    PrayerWheel,
    WhiteStar,
    RedSkull(red_skull::State),
    TinyMailbox,
    FrozenEgg,
    MoltenEgg,
    ToxicEgg,
    SelfFormingClay,
    OldCoin,
    Claws,
    Crossbow,
    IronClub(iron_club::State),
    MeatCleaver,
    Sai,
    SpikedGauntlets,
    TanxsWhistle,
    ThrowingAxe(throwing_axe::State),
    TriBoomerang,
    WarHammer,
    BeautifulBracelet,
    BlessedAntler,
    DiamondDiadem(diamond_diadem::State),
    JewelryBox,
    LoomingFruit,
    SignetRing,
    DelicateFrond,
    Glitter,
    BloodSoakedRose,
    DistinguishedCape,
    Fiddle,
    JeweledMask,
    PreservedFog,
    SereTalon,
    RoyalPoison,
    PotionBelt,
    OddlySmoothStone,
    CharonsAshes,
    CloakClasp,
    StoneCalendar(stone_calendar::State),
    Shuriken(shuriken::State),
    OrnamentalFan(ornamental_fan::State),
    LetterOpener(letter_opener::State),
    TuningFork(tuning_fork::State),
    RainbowRing(rainbow_ring::State),
    SturdyClamp,
    LizardTail(lizard_tail::State),
    SneckoSkull,
    PaperPhrog,
    MeatOnTheBone,
    IceCream,
    PetrifiedToad,
    StoneCracker,
    VexingPuzzlebox,
    Shovel,
    Girya(girya::State),
    SwordOfJade,
    RuinedHelmet(ruined_helmet::State),
    SymbioticVirus,
    WhiteBeastStatue,
    OrangeDough,
    LastingCandy(lasting_candy::State),
    Candelabra,
}

/// `RelicModel.IsTradable`: not used up, nothing on pickup, not melted, no
/// pet, and neither a starter, an event relic nor an ancient's. Nothing here
/// melts a relic.
///
/// # Panics
///
/// A relic whose model decides for itself whether it does something on
/// pickup or brings a pet.
pub(crate) fn tradable(held: sts2_core::CarriedRelic<V>) -> bool {
    use super::super::generated::relics::{RelicFlag, RelicRarity};

    let relic = held.relic;
    let data = relic.data();
    if matches!(
        data.rarity,
        RelicRarity::Starter | RelicRarity::Event | RelicRarity::Ancient
    ) {
        return false;
    }
    !held.used_up
        && [RelicFlag::UponPickup, RelicFlag::SpawnsPets]
            .into_iter()
            .all(|flag| match data.flags.get(flag) {
                Field::Constant(set) => !set,
                Field::Computed => unimplemented!("whether {relic:?} is {flag:?}"),
            })
}

/// `RelicModel.AddsPet`, which two relics override.
pub(crate) fn adds_pet(relic: Relic) -> bool {
    matches!(relic, Relic::Byrdpip | Relic::PaelsLegion)
}

/// Deliver a notification to one relic.
pub(crate) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match state {
        State::AlchemicalCoffer => alchemical_coffer::event_hook(hook, asked),
        State::Anchor => anchor::event_hook(hook, asked),
        State::ArchaicTooth => archaic_tooth::event_hook(hook, asked),
        State::BiiigHug => biiig_hug::event_hook(hook, asked),
        State::ArcaneScroll => arcane_scroll::event_hook(hook, asked),
        State::BoneTea(state) => bone_tea::event_hook(state, hook, asked),
        State::BoomingConch => booming_conch::event_hook(hook, asked),
        State::BoundPhylactery => bound_phylactery::event_hook(hook, asked),
        State::BurningBlood => burning_blood::event_hook(hook, asked),
        State::Byrdpip => byrdpip::event_hook(hook, asked),
        State::ChosenCheese => chosen_cheese::event_hook(hook, asked),
        State::CrackedCore => cracked_core::event_hook(hook, asked),
        State::CursedPearl => cursed_pearl::event_hook(hook, asked),
        State::DivineRight => divine_right::event_hook(hook, asked),
        State::DollysMirror => dollys_mirror::event_hook(hook, asked),
        // `DustyTome.AfterObtained` adds the ancient card `SetupForPlayer`
        // named, upgraded. The relic's model comes with nothing written on
        // it, so Darv hands that card over beside it.
        State::DustyTome => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what DustyTome does for {hook:?}"),
        },
        State::FishingRod(state) => fishing_rod::event_hook(state, hook, asked),
        State::FakeHappyFlower(state) => fake_happy_flower::event_hook(state, hook, asked),
        State::FakeOrichalcum(state) => fake_orichalcum::event_hook(state, hook, asked),
        State::Orichalcum(state) => orichalcum::event_hook(state, hook, asked),
        State::FakeVenerableTeaSet(state) => fake_venerable_tea_set::event_hook(state, hook, asked),
        State::WongosMysteryTicket(state) => wongos_mystery_ticket::event_hook(state, hook, asked),
        State::GlassEye => glass_eye::event_hook(hook, asked),
        State::GoldenPearl => golden_pearl::event_hook(hook, asked),
        State::Gorget => gorget::event_hook(hook, asked),
        State::GremlinHorn => gremlin_horn::event_hook(hook, asked),
        State::HeftyTablet => hefty_tablet::event_hook(hook, asked),
        State::LavaRock(state) => lava_rock::event_hook(state, hook, asked),
        State::LeadPaperweight => lead_paperweight::event_hook(hook),
        State::LeafyPoultice => leafy_poultice::event_hook(hook, asked),
        State::LostCoffer => lost_coffer::event_hook(hook, asked),
        State::LostWisp => lost_wisp::event_hook(hook, asked),
        State::MercuryHourglass => mercury_hourglass::event_hook(hook, asked),
        State::DaughterOfTheWind => daughter_of_the_wind::event_hook(hook, asked),
        State::MrStruggles => mr_struggles::event_hook(hook, asked),
        State::BingBong => bing_bong::event_hook(hook, asked),
        State::IntimidatingHelmet => intimidating_helmet::event_hook(hook, asked),
        State::EmptyCage => empty_cage::event_hook(hook, asked),
        State::Astrolabe => astrolabe::event_hook(hook, asked),
        State::PandorasBox => pandoras_box::event_hook(hook, asked),
        State::CallingBell => calling_bell::event_hook(hook, asked),
        State::BlackStar => black_star::event_hook(hook, asked),
        // Its hooks are `ShouldProcurePotion` and `ModifyMaxEnergy`, which the
        // run and the fight ask of it.
        State::Sozu => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what Sozu does for {hook:?}"),
        },
        State::TungstenRod => tungsten_rod::event_hook(hook),
        State::RadiantPearl => radiant_pearl::event_hook(hook, asked),
        State::ToastyMittens => toasty_mittens::event_hook(hook, asked),
        State::TouchOfOrobas => touch_of_orobas::event_hook(hook, asked),
        State::BlackBlood => black_blood::event_hook(hook, asked),
        State::DivineDestiny => divine_destiny::event_hook(hook, asked),
        State::PhylacteryUnbound => phylactery_unbound::event_hook(hook, asked),
        State::InfusedCore => infused_core::event_hook(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it.
        State::RingOfTheDrake => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what RingOfTheDrake does for {hook:?}"),
        },
        // It overrides nothing.
        State::Circlet => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what Circlet does for {hook:?}"),
        },
        // Its one hook is `ShouldFlush`, which the fight asks of it.
        State::RunicPyramid => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what RunicPyramid does for {hook:?}"),
        },
        State::NeowsTalisman => neows_talisman::event_hook(hook, asked),
        State::NeowsTorment => neows_torment::event_hook(hook, asked),
        State::NewLeaf => new_leaf::event_hook(hook, asked),
        State::NutritiousOyster => nutritious_oyster::event_hook(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it.
        State::PaelsBlood => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what PaelsBlood does for {hook:?}"),
        },
        State::PaelsFlesh(state) => paels_flesh::event_hook(state, hook),
        State::PaelsHorn => paels_horn::event_hook(hook, asked),
        State::PaelsTears(state) => paels_tears::event_hook(state, hook, asked),
        State::Permafrost(state) => permafrost::event_hook(state, hook, asked),
        State::PhialHolster => phial_holster::event_hook(hook, asked),
        State::Pocketwatch(state) => pocketwatch::event_hook(state, hook),
        State::PollinousCore(state) => pollinous_core::event_hook(state, hook),
        State::Pomander => pomander::event_hook(hook, asked),
        State::PrecariousShears => precarious_shears::event_hook(hook, asked),
        State::PreciseScissors => precise_scissors::event_hook(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it
        // rather than tells it. `AfterObtained` reaches every relic, and its
        // own does nothing.
        State::RingOfTheSnake => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what RingOfTheSnake does for {hook:?}"),
        },
        State::SandCastle => sand_castle::event_hook(hook, asked),
        State::SealOfGold => seal_of_gold::event_hook(hook, asked),
        State::StoneHumidifier => stone_humidifier::event_hook(hook, asked),
        State::Storybook => storybook::event_hook(hook, asked),
        State::StrikeDummy => strike_dummy::event_hook(hook),
        State::FakeStrikeDummy => fake_strike_dummy::event_hook(hook),
        State::FakeAnchor => fake_anchor::event_hook(hook, asked),
        State::FakeMango => fake_mango::event_hook(hook, asked),
        State::FakeLeesWaffle => fake_lees_waffle::event_hook(hook, asked),
        State::FakeBloodVial => fake_blood_vial::event_hook(hook, asked),
        State::BloodVial => blood_vial::event_hook(hook, asked),
        State::HornCleat => horn_cleat::event_hook(hook, asked),
        State::SparklingRouge => sparkling_rouge::event_hook(hook, asked),
        State::BowlerHat => bowler_hat::event_hook(hook),
        State::SwordOfStone(state) => sword_of_stone::event_hook(state, hook, asked),
        State::VeryHotCocoa => very_hot_cocoa::event_hook(hook, asked),
        State::BagOfMarbles => bag_of_marbles::event_hook(hook, asked),
        State::Akabeko => akabeko::event_hook(hook, asked),
        State::FestivePopper => festive_popper::event_hook(hook, asked),
        State::BronzeScales => bronze_scales::event_hook(hook, asked),
        State::Vajra => vajra::event_hook(hook, asked),
        State::MealTicket => meal_ticket::event_hook(hook, asked),
        State::Strawberry => strawberry::event_hook(hook, asked),
        State::WarPaint => war_paint::event_hook(hook, asked),
        State::BigMushroom => big_mushroom::event_hook(hook, asked),
        State::ForgottenSoul => forgotten_soul::event_hook(hook, asked),
        State::FragrantMushroom => fragrant_mushroom::event_hook(hook, asked),
        State::Whetstone => whetstone::event_hook(hook, asked),
        State::JuzuBracelet => juzu_bracelet::event_hook(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it
        // rather than tells it. `AfterObtained` reaches every relic, and its
        // own does nothing.
        State::BagOfPreparation => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what BagOfPreparation does for {hook:?}"),
        },
        State::WingedBoots(state) => winged_boots::event_hook(state, hook),
        State::YummyCookie => yummy_cookie::event_hook(hook, asked),
        State::RedMask => red_mask::event_hook(hook, asked),
        State::Lantern => lantern::event_hook(hook, asked),
        State::LuckyFysh => lucky_fysh::event_hook(hook, asked),
        State::AmethystAubergine => amethyst_aubergine::event_hook(hook, asked),
        State::EternalFeather => eternal_feather::event_hook(hook, asked),
        State::Pantograph => pantograph::event_hook(hook, asked),
        State::FakeSneckoEye => fake_snecko_eye::event_hook(hook, asked),
        State::CentennialPuzzle(state) => centennial_puzzle::event_hook(state, hook, asked),
        State::PenNib(state) => pen_nib::event_hook(state, hook),
        State::Mango => mango::event_hook(hook, asked),
        State::MiniatureCannon => miniature_cannon::event_hook(hook),
        // Its `AfterRoomEntered` and `AfterRestSiteHeal` set only how the
        // relic is drawn.
        State::RegalPillow => {}
        State::BookOfFiveRings(state) => book_of_five_rings::event_hook(state, hook, asked),
        State::Kusarigama(state) => kusarigama::event_hook(state, hook, asked),
        State::BeatingRemnant(state) => beating_remnant::event_hook(state, hook),
        State::ReptileTrinket => reptile_trinket::event_hook(hook, asked),
        State::Pear => pear::event_hook(hook, asked),
        State::DataDisk => data_disk::event_hook(hook, asked),
        State::BoneFlute => bone_flute::event_hook(hook, asked),
        State::HappyFlower(state) => happy_flower::event_hook(state, hook, asked),
        State::Chandelier => chandelier::event_hook(hook, asked),
        State::Kunai(state) => kunai::event_hook(state, hook, asked),
        State::Nunchaku(state) => nunchaku::event_hook(state, hook, asked),
        State::VenerableTeaSet(state) => venerable_tea_set::event_hook(state, hook, asked),
        State::TwistedFunnel => twisted_funnel::event_hook(hook, asked),
        State::Pendulum(state) => pendulum::event_hook(state, hook, asked),
        State::RippleBasin(state) => ripple_basin::event_hook(state, hook, asked),
        State::PrayerWheel => prayer_wheel::event_hook(hook, asked),
        State::WhiteStar => white_star::event_hook(hook, asked),
        State::RedSkull(state) => red_skull::event_hook(state, hook, asked),
        State::TinyMailbox => tiny_mailbox::event_hook(hook, asked),
        State::FrozenEgg => frozen_egg::event_hook(hook, asked),
        State::MoltenEgg => molten_egg::event_hook(hook, asked),
        State::ToxicEgg => toxic_egg::event_hook(hook, asked),
        State::SelfFormingClay => self_forming_clay::event_hook(hook, asked),
        State::OldCoin => old_coin::event_hook(hook, asked),
        State::Claws => claws::event_hook(hook, asked),
        State::Crossbow => crossbow::event_hook(hook, asked),
        State::IronClub(state) => iron_club::event_hook(state, hook, asked),
        State::Sai => sai::event_hook(hook, asked),
        State::TanxsWhistle => tanxs_whistle::event_hook(hook, asked),
        State::ThrowingAxe(state) => throwing_axe::event_hook(state, hook),
        State::TriBoomerang => tri_boomerang::event_hook(hook, asked),
        State::WarHammer => war_hammer::event_hook(hook, asked),
        State::BeautifulBracelet => beautiful_bracelet::event_hook(hook, asked),
        State::BlessedAntler => blessed_antler::event_hook(hook, asked),
        State::DiamondDiadem(state) => diamond_diadem::event_hook(state, hook, asked),
        State::JewelryBox => jewelry_box::event_hook(hook, asked),
        State::LoomingFruit => looming_fruit::event_hook(hook, asked),
        State::SignetRing => signet_ring::event_hook(hook, asked),
        State::DelicateFrond => delicate_frond::event_hook(hook, asked),
        State::Glitter => glitter::event_hook(hook, asked),
        State::BloodSoakedRose => blood_soaked_rose::event_hook(hook, asked),
        State::DistinguishedCape => distinguished_cape::event_hook(hook, asked),
        State::JeweledMask => jeweled_mask::event_hook(hook, asked),
        State::PreservedFog => preserved_fog::event_hook(hook, asked),
        State::SereTalon => sere_talon::event_hook(hook, asked),
        State::RoyalPoison => royal_poison::event_hook(hook, asked),
        State::Candelabra => candelabra::event_hook(hook, asked),
        State::LastingCandy(state) => lasting_candy::event_hook(state, hook, asked),
        State::OrangeDough => orange_dough::event_hook(hook, asked),
        State::WhiteBeastStatue => white_beast_statue::event_hook(hook, asked),
        State::SymbioticVirus => symbiotic_virus::event_hook(hook, asked),
        State::RuinedHelmet(state) => ruined_helmet::event_hook(state, hook, asked),
        State::SwordOfJade => sword_of_jade::event_hook(hook, asked),
        State::Girya(state) => girya::event_hook(state, hook, asked),
        State::Shovel => shovel::event_hook(hook, asked),
        State::VexingPuzzlebox => vexing_puzzlebox::event_hook(hook, asked),
        State::StoneCracker => stone_cracker::event_hook(hook, asked),
        State::PetrifiedToad => petrified_toad::event_hook(hook, asked),
        State::IceCream => ice_cream::event_hook(hook, asked),
        State::MeatOnTheBone => meat_on_the_bone::event_hook(hook, asked),
        State::PaperPhrog => paper_phrog::event_hook(hook, asked),
        State::SneckoSkull => snecko_skull::event_hook(hook, asked),
        State::LizardTail(state) => lizard_tail::event_hook(state, hook, asked),
        State::SturdyClamp => sturdy_clamp::event_hook(hook, asked),
        State::RainbowRing(state) => rainbow_ring::event_hook(state, hook, asked),
        State::TuningFork(state) => tuning_fork::event_hook(state, hook, asked),
        State::LetterOpener(state) => letter_opener::event_hook(state, hook, asked),
        State::OrnamentalFan(state) => ornamental_fan::event_hook(state, hook, asked),
        State::Shuriken(state) => shuriken::event_hook(state, hook, asked),
        State::StoneCalendar(state) => stone_calendar::event_hook(state, hook, asked),
        State::CloakClasp => cloak_clasp::event_hook(hook, asked),
        State::CharonsAshes => charons_ashes::event_hook(hook, asked),
        State::OddlySmoothStone => oddly_smooth_stone::event_hook(hook, asked),
        State::PotionBelt => potion_belt::event_hook(hook, asked),
        // Its hooks are `ShouldDraw` and `ModifyHandDrawLate`, which the fight
        // asks of it.
        // `AfterPreventingDraw` flashes it and does nothing else.
        State::Fiddle => match hook {
            EventHook::Obtained | EventHook::DrawPrevented => {}
            hook => unimplemented!("what Fiddle does for {hook:?}"),
        },
        // `TryModifyRestSiteOptions`: `COOK`.
        State::MeatCleaver => match hook {
            EventHook::Obtained => {}
            EventHook::RestSiteOptions => asked.push(sts2_core::HookEffect::OfferRestOption(
                super::super::generated::restsite::RestSiteOption::Cook,
            )),
            hook => unimplemented!("what {state:?} does for {hook:?}"),
        },
        // Its hooks are ones the fight asks of it: `ModifyMaxEnergy` and
        // `TryModifyEnergyCostInCombat`.
        State::SpikedGauntlets => match hook {
            EventHook::Obtained => {}
            hook => unimplemented!("what {state:?} does for {hook:?}"),
        },
    }
}

/// Ask one relic a yes-or-no question.
///
/// The fight and the run ask only a relic that overrides the hook.
pub(crate) fn decide_hook(
    state: &State,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    match state {
        State::WingedBoots(state) => winged_boots::decide_hook(state, hook),
        State::RunicPyramid => runic_pyramid::decide_hook(hook),
        State::Sozu => sozu::decide_hook(hook),
        State::Fiddle => fiddle::decide_hook(hook),
        State::SturdyClamp => sturdy_clamp::decide_hook(hook),
        State::IceCream => ice_cream::decide_hook(hook),
        State::WhiteBeastStatue => white_beast_statue::decide_hook(hook),
        State::LizardTail(state) => lizard_tail::decide_hook(state, hook),
        _ => match hook.model_hook() {
            Some(_) => unimplemented!("what {state:?} decides for {hook:?}"),
            None => sts2_core::HookResponse::DoesNotImplement,
        },
    }
}

/// Ask one relic to change a value.
pub(crate) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match state {
        State::Pocketwatch(state) => pocketwatch::modify_hook(state, hook),
        State::PollinousCore(state) => pollinous_core::modify_hook(state, hook),
        State::RingOfTheSnake => ring_of_the_snake::modify_hook(hook),
        State::BoomingConch => booming_conch::modify_hook(hook),
        State::BagOfPreparation => bag_of_preparation::modify_hook(hook),
        State::BigMushroom => big_mushroom::modify_hook(hook),
        State::StrikeDummy => strike_dummy::modify_hook(hook),
        State::BowlerHat => bowler_hat::modify_hook(hook),
        State::FakeStrikeDummy => fake_strike_dummy::modify_hook(hook),
        State::PaelsBlood => paels_blood::modify_hook(hook),
        State::PaelsFlesh(_) => paels_flesh::modify_hook(hook),
        State::Sozu => sozu::modify_hook(hook),
        State::TungstenRod => tungsten_rod::modify_hook(hook),
        State::RingOfTheDrake => ring_of_the_drake::modify_hook(hook),
        State::InfusedCore => infused_core::modify_hook(hook),
        State::PenNib(state) => pen_nib::modify_hook(state, hook),
        State::MiniatureCannon => miniature_cannon::modify_hook(hook),
        State::RegalPillow => regal_pillow::modify_hook(hook),
        State::BeatingRemnant(state) => beating_remnant::modify_hook(state, hook),
        State::SpikedGauntlets => spiked_gauntlets::modify_hook(hook),
        State::ThrowingAxe(state) => throwing_axe::modify_hook(state, hook),
        State::BlessedAntler => blessed_antler::modify_hook(hook),
        State::BloodSoakedRose => blood_soaked_rose::modify_hook(hook),
        State::Fiddle => fiddle::modify_hook(hook),
        State::SneckoSkull => snecko_skull::modify_hook(hook),
        State::RuinedHelmet(state) => ruined_helmet::modify_hook(state, hook),
        // The fight asks only a relic that overrides the hook, and none of
        // the rest overrides one of these.
        State::AlchemicalCoffer
        | State::Anchor
        | State::ArcaneScroll
        | State::ArchaicTooth
        | State::BiiigHug
        | State::BoneTea(_)
        | State::BoundPhylactery
        | State::BurningBlood
        | State::Byrdpip
        | State::ChosenCheese
        | State::CrackedCore
        | State::CursedPearl
        | State::DivineRight
        | State::DollysMirror
        | State::DustyTome
        | State::FishingRod(_)
        | State::FakeHappyFlower(_)
        | State::FakeOrichalcum(_)
        | State::Orichalcum(_)
        | State::FakeVenerableTeaSet(_)
        | State::WongosMysteryTicket(_)
        | State::GlassEye
        | State::GoldenPearl
        | State::Gorget
        | State::GremlinHorn
        | State::HeftyTablet
        | State::LavaRock(_)
        | State::LeadPaperweight
        | State::LeafyPoultice
        | State::LostCoffer
        | State::LostWisp
        | State::MercuryHourglass
        | State::DaughterOfTheWind
        | State::MrStruggles
        | State::BingBong
        | State::IntimidatingHelmet
        | State::RunicPyramid
        | State::TouchOfOrobas
        | State::BlackBlood
        | State::DivineDestiny
        | State::PhylacteryUnbound
        | State::Circlet
        | State::RadiantPearl
        | State::ToastyMittens
        | State::EmptyCage
        | State::Astrolabe
        | State::PandorasBox
        | State::CallingBell
        | State::BlackStar
        | State::NeowsTalisman
        | State::NeowsTorment
        | State::NewLeaf
        | State::NutritiousOyster
        | State::PaelsHorn
        | State::PaelsTears(_)
        | State::Permafrost(_)
        | State::PhialHolster
        | State::Pomander
        | State::PrecariousShears
        | State::PreciseScissors
        | State::SandCastle
        | State::SealOfGold
        | State::StoneHumidifier
        | State::Storybook
        | State::SwordOfStone(_)
        | State::VeryHotCocoa
        | State::FakeAnchor
        | State::FakeMango
        | State::FakeLeesWaffle
        | State::FakeBloodVial
        | State::BloodVial
        | State::HornCleat
        | State::SparklingRouge
        | State::BagOfMarbles
        | State::Akabeko
        | State::FestivePopper
        | State::BronzeScales
        | State::Vajra
        | State::MealTicket
        | State::Strawberry
        | State::WarPaint
        | State::ForgottenSoul
        | State::FragrantMushroom
        | State::Whetstone
        | State::JuzuBracelet
        | State::WingedBoots(_)
        | State::YummyCookie
        | State::RedMask
        | State::Lantern
        | State::LuckyFysh
        | State::AmethystAubergine
        | State::EternalFeather
        | State::Pantograph
        | State::FakeSneckoEye
        | State::CentennialPuzzle(_)
        | State::Mango
        | State::BookOfFiveRings(_)
        | State::Kusarigama(_)
        | State::ReptileTrinket
        | State::Pear
        | State::DataDisk
        | State::BoneFlute
        | State::HappyFlower(_)
        | State::Chandelier
        | State::Kunai(_)
        | State::Nunchaku(_)
        | State::VenerableTeaSet(_)
        | State::TwistedFunnel
        | State::Pendulum(_)
        | State::RippleBasin(_)
        | State::PrayerWheel
        | State::WhiteStar
        | State::RedSkull(_)
        | State::TinyMailbox
        | State::FrozenEgg
        | State::MoltenEgg
        | State::ToxicEgg
        | State::SelfFormingClay
        | State::OldCoin
        | State::Claws
        | State::Crossbow
        | State::IronClub(_)
        | State::MeatCleaver
        | State::Sai
        | State::TanxsWhistle
        | State::TriBoomerang
        | State::WarHammer
        | State::BeautifulBracelet
        | State::DiamondDiadem(_)
        | State::JewelryBox
        | State::LoomingFruit
        | State::SignetRing
        | State::DelicateFrond
        | State::Glitter
        | State::DistinguishedCape
        | State::JeweledMask
        | State::PreservedFog
        | State::SereTalon
        | State::RoyalPoison
        | State::PotionBelt
        | State::OddlySmoothStone
        | State::CharonsAshes
        | State::CloakClasp
        | State::StoneCalendar(_)
        | State::Shuriken(_)
        | State::OrnamentalFan(_)
        | State::LetterOpener(_)
        | State::TuningFork(_)
        | State::RainbowRing(_)
        | State::SturdyClamp
        | State::LizardTail(_)
        | State::PaperPhrog
        | State::MeatOnTheBone
        | State::IceCream
        | State::PetrifiedToad
        | State::StoneCracker
        | State::VexingPuzzlebox
        | State::Shovel
        | State::Girya(_)
        | State::SwordOfJade
        | State::SymbioticVirus
        | State::WhiteBeastStatue
        | State::OrangeDough
        | State::LastingCandy(_)
        | State::Candelabra => unimplemented!("what {state:?} does for {hook:?}"),
    }
}

impl sts2_core::RelicModel<V> for State {
    /// One relic's model as the run picks it up.
    ///
    /// # Panics
    ///
    /// A relic nobody has ported.
    fn new(relic: Relic) -> Self {
        match relic {
            Relic::AlchemicalCoffer => State::AlchemicalCoffer,
            Relic::Anchor => State::Anchor,
            Relic::ArchaicTooth => State::ArchaicTooth,
            Relic::BiiigHug => State::BiiigHug,
            Relic::ArcaneScroll => State::ArcaneScroll,
            Relic::BoneTea => State::BoneTea(bone_tea::State::default()),
            Relic::BoomingConch => State::BoomingConch,
            Relic::BoundPhylactery => State::BoundPhylactery,
            Relic::BurningBlood => State::BurningBlood,
            Relic::Byrdpip => State::Byrdpip,
            Relic::ChosenCheese => State::ChosenCheese,
            Relic::CrackedCore => State::CrackedCore,
            Relic::CursedPearl => State::CursedPearl,
            Relic::DivineRight => State::DivineRight,
            Relic::DollysMirror => State::DollysMirror,
            Relic::DustyTome => State::DustyTome,
            Relic::FishingRod => State::FishingRod(fishing_rod::State::default()),
            Relic::WongosMysteryTicket => {
                State::WongosMysteryTicket(wongos_mystery_ticket::State::default())
            }
            Relic::GlassEye => State::GlassEye,
            Relic::GoldenPearl => State::GoldenPearl,
            Relic::Gorget => State::Gorget,
            Relic::GremlinHorn => State::GremlinHorn,
            Relic::HeftyTablet => State::HeftyTablet,
            Relic::LavaRock => State::LavaRock(lava_rock::State::default()),
            Relic::LeadPaperweight => State::LeadPaperweight,
            Relic::LeafyPoultice => State::LeafyPoultice,
            Relic::LostCoffer => State::LostCoffer,
            Relic::LostWisp => State::LostWisp,
            Relic::MercuryHourglass => State::MercuryHourglass,
            Relic::DaughterOfTheWind => State::DaughterOfTheWind,
            Relic::MrStruggles => State::MrStruggles,
            Relic::BingBong => State::BingBong,
            Relic::IntimidatingHelmet => State::IntimidatingHelmet,
            Relic::RunicPyramid => State::RunicPyramid,
            Relic::TungstenRod => State::TungstenRod,
            Relic::TouchOfOrobas => State::TouchOfOrobas,
            Relic::BlackBlood => State::BlackBlood,
            Relic::RingOfTheDrake => State::RingOfTheDrake,
            Relic::DivineDestiny => State::DivineDestiny,
            Relic::PhylacteryUnbound => State::PhylacteryUnbound,
            Relic::InfusedCore => State::InfusedCore,
            Relic::Circlet => State::Circlet,
            Relic::RadiantPearl => State::RadiantPearl,
            Relic::ToastyMittens => State::ToastyMittens,
            Relic::EmptyCage => State::EmptyCage,
            Relic::Astrolabe => State::Astrolabe,
            Relic::PandorasBox => State::PandorasBox,
            Relic::CallingBell => State::CallingBell,
            Relic::BlackStar => State::BlackStar,
            Relic::Sozu => State::Sozu,
            Relic::NeowsTalisman => State::NeowsTalisman,
            Relic::NeowsTorment => State::NeowsTorment,
            Relic::NewLeaf => State::NewLeaf,
            Relic::NutritiousOyster => State::NutritiousOyster,
            Relic::PaelsBlood => State::PaelsBlood,
            Relic::PaelsFlesh => State::PaelsFlesh(paels_flesh::State::default()),
            Relic::PaelsHorn => State::PaelsHorn,
            Relic::PaelsTears => State::PaelsTears(paels_tears::State::default()),
            Relic::Permafrost => State::Permafrost(permafrost::State::default()),
            Relic::PhialHolster => State::PhialHolster,
            Relic::Pocketwatch => State::Pocketwatch(pocketwatch::State::default()),
            Relic::PollinousCore => State::PollinousCore(pollinous_core::State::default()),
            Relic::Pomander => State::Pomander,
            Relic::PrecariousShears => State::PrecariousShears,
            Relic::PreciseScissors => State::PreciseScissors,
            Relic::RingOfTheSnake => State::RingOfTheSnake,
            Relic::SandCastle => State::SandCastle,
            Relic::SealOfGold => State::SealOfGold,
            Relic::StoneHumidifier => State::StoneHumidifier,
            Relic::Storybook => State::Storybook,
            Relic::StrikeDummy => State::StrikeDummy,
            Relic::FakeStrikeDummy => State::FakeStrikeDummy,
            Relic::FakeAnchor => State::FakeAnchor,
            Relic::FakeMango => State::FakeMango,
            Relic::FakeLeesWaffle => State::FakeLeesWaffle,
            Relic::FakeBloodVial => State::FakeBloodVial,
            Relic::BloodVial => State::BloodVial,
            Relic::HornCleat => State::HornCleat,
            Relic::SparklingRouge => State::SparklingRouge,
            Relic::BowlerHat => State::BowlerHat,
            Relic::Orichalcum => State::Orichalcum(orichalcum::State::default()),
            Relic::FakeOrichalcum => State::FakeOrichalcum(fake_orichalcum::State::default()),
            Relic::FakeHappyFlower => State::FakeHappyFlower(fake_happy_flower::State::default()),
            Relic::FakeVenerableTeaSet => {
                State::FakeVenerableTeaSet(fake_venerable_tea_set::State::default())
            }
            Relic::SwordOfStone => State::SwordOfStone(sword_of_stone::State::default()),
            Relic::VeryHotCocoa => State::VeryHotCocoa,
            Relic::BagOfMarbles => State::BagOfMarbles,
            Relic::Akabeko => State::Akabeko,
            Relic::FestivePopper => State::FestivePopper,
            Relic::BronzeScales => State::BronzeScales,
            Relic::Vajra => State::Vajra,
            Relic::MealTicket => State::MealTicket,
            Relic::Strawberry => State::Strawberry,
            Relic::WarPaint => State::WarPaint,
            Relic::BigMushroom => State::BigMushroom,
            Relic::ForgottenSoul => State::ForgottenSoul,
            Relic::FragrantMushroom => State::FragrantMushroom,
            Relic::Whetstone => State::Whetstone,
            Relic::JuzuBracelet => State::JuzuBracelet,
            Relic::BagOfPreparation => State::BagOfPreparation,
            Relic::WingedBoots => State::WingedBoots(winged_boots::State::default()),
            Relic::YummyCookie => State::YummyCookie,
            Relic::RedMask => State::RedMask,
            Relic::Lantern => State::Lantern,
            Relic::LuckyFysh => State::LuckyFysh,
            Relic::AmethystAubergine => State::AmethystAubergine,
            Relic::EternalFeather => State::EternalFeather,
            Relic::Pantograph => State::Pantograph,
            Relic::FakeSneckoEye => State::FakeSneckoEye,
            Relic::CentennialPuzzle => State::CentennialPuzzle(centennial_puzzle::State::default()),
            Relic::PenNib => State::PenNib(pen_nib::State::default()),
            Relic::Mango => State::Mango,
            Relic::MiniatureCannon => State::MiniatureCannon,
            Relic::RegalPillow => State::RegalPillow,
            Relic::BookOfFiveRings => State::BookOfFiveRings(book_of_five_rings::State::default()),
            Relic::Kusarigama => State::Kusarigama(kusarigama::State::default()),
            Relic::BeatingRemnant => State::BeatingRemnant(beating_remnant::State::default()),
            Relic::ReptileTrinket => State::ReptileTrinket,
            Relic::Pear => State::Pear,
            Relic::DataDisk => State::DataDisk,
            Relic::BoneFlute => State::BoneFlute,
            Relic::HappyFlower => State::HappyFlower(happy_flower::State::default()),
            Relic::Chandelier => State::Chandelier,
            Relic::Kunai => State::Kunai(kunai::State::default()),
            Relic::Nunchaku => State::Nunchaku(nunchaku::State::default()),
            Relic::VenerableTeaSet => State::VenerableTeaSet(venerable_tea_set::State::default()),
            Relic::TwistedFunnel => State::TwistedFunnel,
            Relic::Pendulum => State::Pendulum(pendulum::State::default()),
            Relic::RippleBasin => State::RippleBasin(ripple_basin::State::default()),
            Relic::PrayerWheel => State::PrayerWheel,
            Relic::WhiteStar => State::WhiteStar,
            Relic::RedSkull => State::RedSkull(red_skull::State::default()),
            Relic::TinyMailbox => State::TinyMailbox,
            Relic::FrozenEgg => State::FrozenEgg,
            Relic::MoltenEgg => State::MoltenEgg,
            Relic::ToxicEgg => State::ToxicEgg,
            Relic::SelfFormingClay => State::SelfFormingClay,
            Relic::OldCoin => State::OldCoin,
            Relic::Claws => State::Claws,
            Relic::Crossbow => State::Crossbow,
            Relic::IronClub => State::IronClub(iron_club::State::default()),
            Relic::MeatCleaver => State::MeatCleaver,
            Relic::Sai => State::Sai,
            Relic::SpikedGauntlets => State::SpikedGauntlets,
            Relic::TanxsWhistle => State::TanxsWhistle,
            Relic::ThrowingAxe => State::ThrowingAxe(throwing_axe::State::default()),
            Relic::TriBoomerang => State::TriBoomerang,
            Relic::WarHammer => State::WarHammer,
            Relic::BeautifulBracelet => State::BeautifulBracelet,
            Relic::BlessedAntler => State::BlessedAntler,
            Relic::DiamondDiadem => State::DiamondDiadem(diamond_diadem::State::default()),
            Relic::JewelryBox => State::JewelryBox,
            Relic::LoomingFruit => State::LoomingFruit,
            Relic::SignetRing => State::SignetRing,
            Relic::DelicateFrond => State::DelicateFrond,
            Relic::Glitter => State::Glitter,
            Relic::BloodSoakedRose => State::BloodSoakedRose,
            Relic::DistinguishedCape => State::DistinguishedCape,
            Relic::Fiddle => State::Fiddle,
            Relic::JeweledMask => State::JeweledMask,
            Relic::PreservedFog => State::PreservedFog,
            Relic::SereTalon => State::SereTalon,
            Relic::RoyalPoison => State::RoyalPoison,
            Relic::Candelabra => State::Candelabra,
            Relic::LastingCandy => State::LastingCandy(lasting_candy::State::default()),
            Relic::OrangeDough => State::OrangeDough,
            Relic::WhiteBeastStatue => State::WhiteBeastStatue,
            Relic::SymbioticVirus => State::SymbioticVirus,
            Relic::RuinedHelmet => State::RuinedHelmet(ruined_helmet::State::default()),
            Relic::SwordOfJade => State::SwordOfJade,
            Relic::Girya => State::Girya(girya::State::default()),
            Relic::Shovel => State::Shovel,
            Relic::VexingPuzzlebox => State::VexingPuzzlebox,
            Relic::StoneCracker => State::StoneCracker,
            Relic::PetrifiedToad => State::PetrifiedToad,
            Relic::IceCream => State::IceCream,
            Relic::MeatOnTheBone => State::MeatOnTheBone,
            Relic::PaperPhrog => State::PaperPhrog,
            Relic::SneckoSkull => State::SneckoSkull,
            Relic::LizardTail => State::LizardTail(lizard_tail::State::default()),
            Relic::SturdyClamp => State::SturdyClamp,
            Relic::RainbowRing => State::RainbowRing(rainbow_ring::State::default()),
            Relic::TuningFork => State::TuningFork(tuning_fork::State::default()),
            Relic::LetterOpener => State::LetterOpener(letter_opener::State::default()),
            Relic::OrnamentalFan => State::OrnamentalFan(ornamental_fan::State::default()),
            Relic::Shuriken => State::Shuriken(shuriken::State::default()),
            Relic::StoneCalendar => State::StoneCalendar(stone_calendar::State::default()),
            Relic::CloakClasp => State::CloakClasp,
            Relic::CharonsAshes => State::CharonsAshes,
            Relic::OddlySmoothStone => State::OddlySmoothStone,
            Relic::PotionBelt => State::PotionBelt,
            relic => unimplemented!("what {relic:?} does in a run that carries it"),
        }
    }

    /// `DisplayAmount`. The table answers for a relic that prints a constant,
    /// and the rest work it out from what they saved.
    ///
    /// # Panics
    ///
    /// A relic that works its number out and nobody has ported.
    /// `IsUsedUp`. The table answers for a relic that returns a constant,
    /// and the rest work it out from what they saved.
    fn used_up(&self, relic: Relic) -> bool {
        use super::super::generated::relics::RelicFlag;

        match (self, Table::data(relic).flags.get(RelicFlag::UsedUp)) {
            (State::LizardTail(state), _) => lizard_tail::used_up(state),
            (State::BoneTea(state), _) => bone_tea::used_up(state),
            (State::WingedBoots(state), _) => state.used_up(),
            (State::WongosMysteryTicket(state), _) => wongos_mystery_ticket::used_up(state),
            (_, Field::Constant(used_up)) => used_up,
            (_, Field::Computed) => unimplemented!("whether {relic:?} is used up"),
        }
    }

    fn display_amount(&self, relic: Relic) -> Number {
        match Table::data(relic).display_amount {
            // The table holds the column's own width, and a relic counts in
            // the build's game number. Widening here is the only place the two
            // meet.
            Field::Constant(amount) => Number::from(amount),
            Field::Computed => match self {
                State::BoneTea(state) => bone_tea::display_amount(state),
                State::FishingRod(state) => fishing_rod::display_amount(state),
                State::FakeHappyFlower(state) => fake_happy_flower::display_amount(state),
                State::WongosMysteryTicket(state) => wongos_mystery_ticket::display_amount(state),
                State::SwordOfStone(state) => sword_of_stone::display_amount(state),
                State::WingedBoots(state) => winged_boots::display_amount(state),
                State::Pocketwatch(state) => pocketwatch::display_amount(state),
                State::PollinousCore(state) => pollinous_core::display_amount(state),
                State::PaelsFlesh(state) => paels_flesh::display_amount(state),
                State::PenNib(state) => pen_nib::display_amount(state),
                State::BookOfFiveRings(state) => book_of_five_rings::display_amount(state),
                State::Kusarigama(state) => kusarigama::display_amount(state),
                State::HappyFlower(state) => happy_flower::display_amount(state),
                State::Kunai(state) => kunai::display_amount(state),
                State::LastingCandy(state) => lasting_candy::display_amount(state),
                State::Girya(state) => girya::display_amount(state),
                State::TuningFork(state) => tuning_fork::display_amount(state),
                State::LetterOpener(state) => letter_opener::display_amount(state),
                State::OrnamentalFan(state) => ornamental_fan::display_amount(state),
                State::Shuriken(state) => shuriken::display_amount(state),
                State::StoneCalendar(state) => stone_calendar::display_amount(state),
                State::Nunchaku(state) => nunchaku::display_amount(state),
                State::IronClub(state) => iron_club::display_amount(state),
                State::DiamondDiadem(state) => diamond_diadem::display_amount(state),
                State::Pendulum(state) => pendulum::display_amount(state),
                _ => unimplemented!("the number {relic:?} shows"),
            },
        }
    }
}
