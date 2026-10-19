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

use crate::ids::{RelicIds as _, RestSiteOptionIds as _};

mod akabeko;
mod alchemical_coffer;
mod amethyst_aubergine;
mod anchor;
mod arcane_scroll;
pub(crate) mod archaic_tooth;
mod art_of_war;
mod astrolabe;
mod bag_of_marbles;
mod bag_of_preparation;
mod beating_remnant;
mod beautiful_bracelet;
mod bellows;
mod belt_buckle;
mod big_hat;
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
mod book_repair_knife;
mod bookmark;
mod booming_conch;
mod bound_phylactery;
mod bowler_hat;
mod bread;
mod brilliant_scarf;
mod brimstone;
mod bronze_scales;
mod burning_blood;
mod burning_sticks;
mod byrdpip;
pub(crate) mod calling_bell;
mod candelabra;
mod captains_wheel;
pub(crate) mod cauldron;
mod centennial_puzzle;
mod chandelier;
mod charons_ashes;
mod chemical_x;
pub(crate) mod choices_paradox;
mod chosen_cheese;
mod claws;
mod cloak_clasp;
mod cracked_core;
pub(crate) mod crossbow;
mod cursed_pearl;
mod darkstone_periapt;
mod data_disk;
mod daughter_of_the_wind;
mod delicate_frond;
mod demon_tongue;
pub(crate) mod diamond_diadem;
mod dingy_rug;
mod distinguished_cape;
mod divine_destiny;
mod divine_right;
mod dollys_mirror;
mod dragon_fruit;
pub(crate) mod dream_catcher;
mod driftwood;
mod ectoplasm;
mod electric_shrymp;
mod ember_tea;
mod emotion_chip;
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
mod fencing_manual;
mod festive_popper;
mod fiddle;
mod fishing_rod;
mod forgotten_soul;
mod fragrant_mushroom;
mod fresnel_lens;
mod frozen_egg;
mod funerary_mask;
mod fur_coat;
mod galactic_dust;
mod gambling_chip;
mod game_piece;
mod ghost_seed;
mod girya;
pub(crate) mod glass_eye;
mod glitter;
mod gnarled_hammer;
mod gold_plated_cables;
mod golden_compass;
mod golden_pearl;
mod gorget;
mod gremlin_horn;
mod hand_drill;
mod happy_flower;
mod hefty_tablet;
mod helical_dart;
mod history_course;
mod horn_cleat;
mod ice_cream;
mod infused_core;
mod intimidating_helmet;
mod iron_club;
mod ivory_tile;
mod jeweled_mask;
mod jewelry_box;
mod joss_paper;
mod juzu_bracelet;
mod kaleidoscope;
mod kifuda;
mod kunai;
mod kusarigama;
mod lantern;
mod large_capsule;
mod lasting_candy;
mod lava_lamp;
mod lava_rock;
mod lead_paperweight;
pub(crate) mod leafy_poultice;
mod lees_waffle;
mod letter_opener;
mod lizard_tail;
mod looming_fruit;
mod lords_parasol;
pub(crate) mod lost_coffer;
mod lost_wisp;
mod lucky_fysh;
mod lunar_pastry;
mod mango;
mod maw_bank;
mod meal_ticket;
mod meat_on_the_bone;
mod membership_card;
mod mercury_hourglass;
mod metronome;
mod mini_regent;
mod miniature_cannon;
mod miniature_tent;
mod molten_egg;
mod mr_struggles;
mod mummified_hand;
pub(crate) mod music_box;
mod mystic_lighter;
mod neows_bones;
pub(crate) mod neows_talisman;
mod neows_torment;
mod new_leaf;
mod ninja_scroll;
mod nunchaku;
mod nutritious_oyster;
mod nutritious_soup;
mod oddly_smooth_stone;
mod old_coin;
mod orange_dough;
mod orichalcum;
mod ornamental_fan;
pub(crate) mod orrery;
mod paels_blood;
mod paels_claw;
mod paels_eye;
mod paels_flesh;
mod paels_growth;
mod paels_horn;
mod paels_legion;
mod paels_tears;
mod paels_tooth;
mod paels_wing;
mod pandoras_box;
mod pantograph;
mod paper_krane;
mod paper_phrog;
mod parrying_shield;
mod pear;
mod pen_nib;
mod pendulum;
mod permafrost;
mod petrified_toad;
mod phial_holster;
mod philosophers_stone;
mod phylactery_unbound;
mod planisphere;
mod pocketwatch;
mod pollinous_core;
mod pomander;
mod potion_belt;
mod power_cell;
mod prayer_wheel;
mod precarious_shears;
mod precise_scissors;
mod preserved_fog;
mod prismatic_gem;
mod pumpkin_candle;
mod punch_dagger;
mod radiant_pearl;
mod rainbow_ring;
mod razor_tooth;
mod red_mask;
mod red_skull;
mod regal_pillow;
mod regalite;
mod reptile_trinket;
mod ring_of_the_drake;
mod ring_of_the_snake;
mod ringing_triangle;
mod ripple_basin;
mod royal_poison;
mod royal_stamp;
mod ruined_helmet;
mod runic_capacitor;
mod runic_pyramid;
mod sai;
mod sand_castle;
mod screaming_flagon;
mod scroll_boxes;
mod sea_glass;
mod seal_of_gold;
mod self_forming_clay;
mod sere_talon;
mod shovel;
mod shuriken;
mod signet_ring;
mod silken_tress;
mod silver_crucible;
mod sling_of_courage;
mod small_capsule;
mod snecko_eye;
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
mod tea_of_discourtesy;
mod the_abacus;
mod the_boot;
mod the_courier;
mod throwing_axe;
mod tingsha;
pub(crate) mod tiny_mailbox;
mod toasty_mittens;
mod toolbox;
pub(crate) mod touch_of_orobas;
mod tough_bandages;
mod toxic_egg;
pub(crate) mod toy_box;
mod tri_boomerang;
mod tungsten_rod;
mod tuning_fork;
mod twisted_funnel;
mod unceasing_top;
mod undying_sigil;
mod unsettling_lamp;
mod vajra;
mod vambrace;
mod velvet_choker;
mod venerable_tea_set;
mod very_hot_cocoa;
pub(crate) mod vexing_puzzlebox;
mod vitruvian_minion;
mod war_hammer;
mod war_paint;
mod whetstone;
mod whispering_earring;
mod white_beast_statue;
mod white_star;
mod wing_charm;
mod winged_boots;
mod wongos_mystery_ticket;
mod yummy_cookie;

use sts2_core::{EventHook, Modify, ModifyHook, Table};

use crate::Field;

pub(crate) type Number = i32;

/// `room is CombatRoom`, which is how a relic tells a fight's room apart.
fn combat_room(room: crate::generated::encounters::RoomType) -> bool {
    use crate::generated::encounters::RoomType;

    matches!(room, RoomType::Monster | RoomType::Elite | RoomType::Boss)
}

/// `EggRelicHelper.UpgradeValidCards`, and the egg's own check as a card
/// joins the deck: an upgraded clone of an upgradable card of `kind`. A run
/// here has one player, whose cards are all the owner's.
fn egg<V: crate::Build>(
    kind: sts2_core::CardKind,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use sts2_core::CardTable as _;

    if let EventHook::CardRewardOption { card, .. }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State<V: crate::Build> {
    /// A relic whose rule is the build's own.
    Own(V::OwnRelic),
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
    BingBong(bing_bong::State),
    IntimidatingHelmet,
    RunicPyramid,
    TungstenRod,
    TheBoot,
    UndyingSigil,
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
    PaelsFlesh,
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
    MawBank(maw_bank::State),
    DreamCatcher,
    FakeMerchantsRug,
    WongoCustomerAppreciationBadge,
    DarkstonePeriapt,
    HandDrill,
    AmethystAubergine,
    EternalFeather,
    Pantograph,
    FakeSneckoEye,
    CentennialPuzzle(centennial_puzzle::State),
    PenNib(pen_nib::State<V>),
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
    FresnelLens,
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
    PaperKrane,
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
    GamePiece,
    Regalite,
    Bellows,
    JossPaper(joss_paper::State),
    MummifiedHand,
    IvoryTile,
    Bookmark,
    ParryingShield,
    Metronome(metronome::State),
    BookRepairKnife,
    Vambrace(vambrace::State<V>),
    UnsettlingLamp(unsettling_lamp::State<V>),
    EmotionChip(emotion_chip::State),
    TheCourier,
    VitruvianMinion,
    MembershipCard,
    Tingsha,
    ToughBandages,
    FencingManual,
    SmallCapsule,
    LargeCapsule,
    NeowsBones,
    SilverCrucible(silver_crucible::State),
    SilkenTress(silken_tress::State),
    ScrollBoxes,
    Kaleidoscope,
    Planisphere,
    GhostSeed,
    LeesWaffle,
    LavaLamp(lava_lamp::State),
    EmberTea(ember_tea::State),
    Kifuda,
    PunchDagger,
    TeaOfDiscourtesy(tea_of_discourtesy::State),
    HelicalDart,
    GamblingChip,
    BurningSticks(burning_sticks::State),
    ArtOfWar(art_of_war::State),
    CaptainsWheel,
    FuneraryMask,
    UnceasingTop,
    GalacticDust(galactic_dust::State),
    MiniRegent(mini_regent::State),
    RazorTooth,
    SlingOfCourage,
    PowerCell,
    Orrery,
    ChemicalX,
    MysticLighter,
    GoldPlatedCables,
    Cauldron,
    LunarPastry,
    WingCharm,
    Toolbox,
    MiniatureTent,
    NutritiousSoup,
    PumpkinCandle(pumpkin_candle::State),
    PaelsClaw,
    PaelsGrowth,
    Driftwood,
    PaelsWing(paels_wing::State),
    ToyBox(toy_box::State),
    ElectricShrymp,
    SneckoEye,
    Ectoplasm,
    PaelsEye(paels_eye::State),
    PaelsLegion(paels_legion::State<V>),
    GoldenCompass(golden_compass::State),
    SeaGlass(sea_glass::State<V>),
    DemonTongue(demon_tongue::State),
    PrismaticGem,
    PaelsTooth(paels_tooth::State<V>),
    NinjaScroll,
    Brimstone,
    ScreamingFlagon,
    TheAbacus,
    DingyRug,
    BeltBuckle(belt_buckle::State),
    Bread,
    RingingTriangle,
    BigHat,
    RoyalStamp,
    RunicCapacitor,
    MusicBox(music_box::State<V>),
    ChoicesParadox,
    WhisperingEarring,
    BrilliantScarf(brilliant_scarf::State),
    VelvetChoker(velvet_choker::State),
    LordsParasol,
    HistoryCourse,
    FurCoat(fur_coat::State),
    PhilosophersStone,
    GnarledHammer,
    DragonFruit,
}

/// `RelicModel.IsTradable`: not used up, nothing on pickup, not melted, no
/// pet, and neither a starter, an event relic nor an ancient's.
///
/// # Panics
///
/// A relic whose model decides for itself whether it does something on
/// pickup or brings a pet.
pub(crate) fn tradable<V: crate::Build>(held: sts2_core::CarriedRelic<V>) -> bool {
    use crate::generated::relics::{RelicFlag, RelicRarity};

    let relic = held.relic;
    let data = relic.data();
    if matches!(
        data.rarity,
        RelicRarity::Starter | RelicRarity::Event | RelicRarity::Ancient
    ) {
        return false;
    }
    !held.used_up
        && !held.melted
        && [RelicFlag::UponPickup, RelicFlag::SpawnsPets]
            .into_iter()
            .all(|flag| match data.flags.get(flag) {
                Field::Constant(set) => !set,
                Field::Computed => unimplemented!("whether {relic:?} is {flag:?}"),
            })
}

/// `RelicModel.AddsPet`, which two relics override.
pub(crate) fn adds_pet<V: crate::Build>(relic: V::Relic) -> bool {
    relic == V::Relic::BYRDPIP || relic == V::Relic::PAELS_LEGION
}

/// Deliver a notification to one relic, which sits on `belt`.
/// Out of line: inlined into the walk, it slows every hook the walk asks.
#[inline(never)]
pub(crate) fn event_hook<V: crate::Build>(
    state: &mut State<V>,
    belt: &[V::Relic],
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // A shared relic whose rule is the build's own, for a hook it answers
    // itself.
    let owned = match state {
        State::ToyBox(_) => V::own_relic(V::Relic::TOY_BOX),
        State::DistinguishedCape => V::own_relic(V::Relic::DISTINGUISHED_CAPE),
        State::BeautifulBracelet => V::own_relic(V::Relic::BEAUTIFUL_BRACELET),
        State::HandDrill => V::own_relic(V::Relic::HAND_DRILL),
        State::PollinousCore(_) => V::own_relic(V::Relic::POLLINOUS_CORE),
        State::SereTalon => V::own_relic(V::Relic::SERE_TALON),
        State::LastingCandy(_) => V::own_relic(V::Relic::LASTING_CANDY),
        _ => false,
    };
    if owned && V::own_shared_relic_event_hook(state, hook, asked).is_some() {
        return;
    }
    match state {
        State::AlchemicalCoffer => alchemical_coffer::event_hook::<V>(hook, asked),
        State::Anchor => anchor::event_hook::<V>(hook, asked),
        State::ArchaicTooth => archaic_tooth::event_hook::<V>(hook, asked),
        State::BiiigHug => biiig_hug::event_hook::<V>(hook, asked),
        State::ArcaneScroll => arcane_scroll::event_hook::<V>(hook, asked),
        State::BoneTea(state) => bone_tea::event_hook::<V>(state, hook, asked),
        State::BoomingConch => booming_conch::event_hook::<V>(hook, asked),
        State::BoundPhylactery => bound_phylactery::event_hook::<V>(hook, asked),
        State::BurningBlood => burning_blood::event_hook::<V>(hook, asked),
        State::Byrdpip => byrdpip::event_hook::<V>(hook, asked),
        State::ChosenCheese => chosen_cheese::event_hook::<V>(hook, asked),
        State::CrackedCore => cracked_core::event_hook::<V>(hook, asked),
        State::CursedPearl => cursed_pearl::event_hook::<V>(hook, asked),
        State::DivineRight => divine_right::event_hook::<V>(hook, asked),
        State::DollysMirror => dollys_mirror::event_hook::<V>(hook, asked),
        // `DustyTome.AfterObtained` adds the ancient card `SetupForPlayer`
        // named, upgraded. The relic's model comes with nothing written on
        // it, so Darv hands that card over beside it.
        State::DustyTome => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what DustyTome does for {hook:?}"),
        },
        State::FishingRod(state) => fishing_rod::event_hook::<V>(state, hook, asked),
        State::FakeHappyFlower(state) => fake_happy_flower::event_hook::<V>(state, hook, asked),
        State::FakeOrichalcum(state) => fake_orichalcum::event_hook::<V>(state, hook, asked),
        State::Orichalcum(state) => orichalcum::event_hook::<V>(state, hook, asked),
        State::FakeVenerableTeaSet(state) => {
            fake_venerable_tea_set::event_hook::<V>(state, hook, asked)
        }
        State::WongosMysteryTicket(state) => {
            wongos_mystery_ticket::event_hook::<V>(state, hook, asked)
        }
        State::GlassEye => glass_eye::event_hook::<V>(hook, asked),
        State::GoldenPearl => golden_pearl::event_hook::<V>(hook, asked),
        State::Gorget => gorget::event_hook::<V>(hook, asked),
        State::GremlinHorn => gremlin_horn::event_hook::<V>(hook, asked),
        State::HeftyTablet => hefty_tablet::event_hook::<V>(hook, asked),
        State::LavaRock(state) => lava_rock::event_hook::<V>(state, hook, asked),
        State::LeadPaperweight => lead_paperweight::event_hook::<V>(hook),
        State::LeafyPoultice => leafy_poultice::event_hook::<V>(hook, asked),
        State::LostCoffer => lost_coffer::event_hook::<V>(hook, asked),
        State::LostWisp => lost_wisp::event_hook::<V>(hook, asked),
        State::MercuryHourglass => mercury_hourglass::event_hook::<V>(hook, asked),
        State::DaughterOfTheWind => daughter_of_the_wind::event_hook::<V>(hook, asked),
        State::MrStruggles => mr_struggles::event_hook::<V>(hook, asked),
        State::BingBong(state) => bing_bong::event_hook::<V>(state, hook, asked),
        State::IntimidatingHelmet => intimidating_helmet::event_hook::<V>(hook, asked),
        State::EmptyCage => empty_cage::event_hook::<V>(hook, asked),
        State::Astrolabe => astrolabe::event_hook::<V>(hook, asked),
        State::PandorasBox => pandoras_box::event_hook::<V>(hook, asked),
        State::CallingBell => calling_bell::event_hook::<V>(hook, asked),
        State::BlackStar => black_star::event_hook::<V>(hook, asked),
        // Its hooks are `ShouldProcurePotion` and `ModifyMaxEnergy`, which the
        // run and the fight ask of it.
        State::Sozu => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what Sozu does for {hook:?}"),
        },
        State::TungstenRod => tungsten_rod::event_hook::<V>(hook),
        State::TheBoot => the_boot::event_hook::<V>(hook),
        State::UndyingSigil => undying_sigil::event_hook::<V>(hook),
        State::RadiantPearl => radiant_pearl::event_hook::<V>(hook, asked),
        State::ToastyMittens => toasty_mittens::event_hook::<V>(hook, asked),
        State::TouchOfOrobas => touch_of_orobas::event_hook::<V>(hook, asked),
        State::BlackBlood => black_blood::event_hook::<V>(hook, asked),
        State::DivineDestiny => divine_destiny::event_hook::<V>(hook, asked),
        State::PhylacteryUnbound => phylactery_unbound::event_hook::<V>(hook, asked),
        State::InfusedCore => infused_core::event_hook::<V>(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it.
        State::RingOfTheDrake => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what RingOfTheDrake does for {hook:?}"),
        },
        State::DreamCatcher => dream_catcher::event_hook::<V>(hook, asked),
        // Each overrides nothing.
        State::FakeMerchantsRug | State::WongoCustomerAppreciationBadge => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what a relic that overrides nothing does for {hook:?}"),
        },
        // It overrides nothing.
        State::Circlet => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what Circlet does for {hook:?}"),
        },
        // Its one hook is `ShouldFlush`, which the fight asks of it.
        State::RunicPyramid => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what RunicPyramid does for {hook:?}"),
        },
        State::NeowsTalisman => neows_talisman::event_hook::<V>(hook, asked),
        State::NeowsTorment => neows_torment::event_hook::<V>(hook, asked),
        State::NewLeaf => new_leaf::event_hook::<V>(hook, asked),
        State::NutritiousOyster => nutritious_oyster::event_hook::<V>(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it.
        State::PaelsBlood => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what PaelsBlood does for {hook:?}"),
        },
        State::PaelsFlesh => paels_flesh::event_hook::<V>(hook),
        State::PaelsHorn => paels_horn::event_hook::<V>(hook, asked),
        State::PaelsTears(state) => paels_tears::event_hook::<V>(state, hook, asked),
        State::Permafrost(state) => permafrost::event_hook::<V>(state, hook, asked),
        State::PhialHolster => phial_holster::event_hook::<V>(hook, asked),
        State::Pocketwatch(state) => pocketwatch::event_hook::<V>(state, hook),
        State::PollinousCore(state) => pollinous_core::event_hook::<V>(state, hook),
        State::Pomander => pomander::event_hook::<V>(hook, asked),
        State::PrecariousShears => precarious_shears::event_hook::<V>(hook, asked),
        State::PreciseScissors => precise_scissors::event_hook::<V>(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it
        // rather than tells it. `AfterObtained` reaches every relic, and its
        // own does nothing.
        State::RingOfTheSnake => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what RingOfTheSnake does for {hook:?}"),
        },
        State::SandCastle => sand_castle::event_hook::<V>(hook, asked),
        State::SealOfGold => seal_of_gold::event_hook::<V>(hook, asked),
        State::StoneHumidifier => stone_humidifier::event_hook::<V>(hook, asked),
        State::Storybook => storybook::event_hook::<V>(hook, asked),
        State::StrikeDummy => strike_dummy::event_hook::<V>(hook),
        State::FakeStrikeDummy => fake_strike_dummy::event_hook::<V>(hook),
        State::FakeAnchor => fake_anchor::event_hook::<V>(hook, asked),
        State::FakeMango => fake_mango::event_hook::<V>(hook, asked),
        State::FakeLeesWaffle => fake_lees_waffle::event_hook::<V>(hook, asked),
        State::FakeBloodVial => fake_blood_vial::event_hook::<V>(hook, asked),
        State::BloodVial => blood_vial::event_hook::<V>(hook, asked),
        State::HornCleat => horn_cleat::event_hook::<V>(hook, asked),
        State::SparklingRouge => sparkling_rouge::event_hook::<V>(hook, asked),
        State::BowlerHat => bowler_hat::event_hook::<V>(hook),
        State::SwordOfStone(state) => sword_of_stone::event_hook::<V>(state, hook, asked),
        State::VeryHotCocoa => very_hot_cocoa::event_hook::<V>(hook, asked),
        State::BagOfMarbles => bag_of_marbles::event_hook::<V>(hook, asked),
        State::Akabeko => akabeko::event_hook::<V>(hook, asked),
        State::FestivePopper => festive_popper::event_hook::<V>(hook, asked),
        State::BronzeScales => bronze_scales::event_hook::<V>(hook, asked),
        State::Vajra => vajra::event_hook::<V>(hook, asked),
        State::MealTicket => meal_ticket::event_hook::<V>(hook, asked),
        State::Strawberry => strawberry::event_hook::<V>(hook, asked),
        State::WarPaint => war_paint::event_hook::<V>(hook, asked),
        State::BigMushroom => big_mushroom::event_hook::<V>(hook, asked),
        State::ForgottenSoul => forgotten_soul::event_hook::<V>(hook, asked),
        State::FragrantMushroom => fragrant_mushroom::event_hook::<V>(hook, asked),
        State::Whetstone => whetstone::event_hook::<V>(hook, asked),
        State::JuzuBracelet => juzu_bracelet::event_hook::<V>(hook, asked),
        // Its one hook is `ModifyHandDraw`, which the fight asks of it
        // rather than tells it. `AfterObtained` reaches every relic, and its
        // own does nothing.
        State::BagOfPreparation => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what BagOfPreparation does for {hook:?}"),
        },
        State::WingedBoots(state) => winged_boots::event_hook::<V>(state, hook),
        State::YummyCookie => yummy_cookie::event_hook::<V>(hook, asked),
        State::RedMask => red_mask::event_hook::<V>(hook, asked),
        State::Lantern => lantern::event_hook::<V>(hook, asked),
        State::LuckyFysh => lucky_fysh::event_hook::<V>(hook, asked),
        State::MawBank(state) => maw_bank::event_hook::<V>(state, hook, asked),
        State::DarkstonePeriapt => darkstone_periapt::event_hook::<V>(hook, asked),
        State::HandDrill => hand_drill::event_hook::<V>(hook, asked),
        State::AmethystAubergine => amethyst_aubergine::event_hook::<V>(hook, asked),
        State::EternalFeather => eternal_feather::event_hook::<V>(hook, asked),
        State::Pantograph => pantograph::event_hook::<V>(hook, asked),
        State::FakeSneckoEye => fake_snecko_eye::event_hook::<V>(hook, asked),
        State::CentennialPuzzle(state) => centennial_puzzle::event_hook::<V>(state, hook, asked),
        State::PenNib(state) => pen_nib::event_hook::<V>(state, hook),
        State::Mango => mango::event_hook::<V>(hook, asked),
        State::MiniatureCannon => miniature_cannon::event_hook::<V>(hook),
        // Its `AfterRoomEntered` and `AfterRestSiteHeal` set only how the
        // relic is drawn.
        State::RegalPillow => {}
        State::BookOfFiveRings(state) => book_of_five_rings::event_hook::<V>(state, hook, asked),
        State::Kusarigama(state) => kusarigama::event_hook::<V>(state, hook, asked),
        State::BeatingRemnant(state) => beating_remnant::event_hook::<V>(state, hook),
        State::ReptileTrinket => reptile_trinket::event_hook::<V>(hook, asked),
        State::Pear => pear::event_hook::<V>(hook, asked),
        State::DataDisk => data_disk::event_hook::<V>(hook, asked),
        State::BoneFlute => bone_flute::event_hook::<V>(hook, asked),
        State::HappyFlower(state) => happy_flower::event_hook::<V>(state, hook, asked),
        State::Chandelier => chandelier::event_hook::<V>(hook, asked),
        State::Kunai(state) => kunai::event_hook::<V>(state, hook, asked),
        State::Nunchaku(state) => nunchaku::event_hook::<V>(state, hook, asked),
        State::VenerableTeaSet(state) => venerable_tea_set::event_hook::<V>(state, hook, asked),
        State::TwistedFunnel => twisted_funnel::event_hook::<V>(hook, asked),
        State::Pendulum(state) => pendulum::event_hook::<V>(state, hook, asked),
        State::RippleBasin(state) => ripple_basin::event_hook::<V>(state, hook, asked),
        State::PrayerWheel => prayer_wheel::event_hook::<V>(hook, asked),
        State::WhiteStar => white_star::event_hook::<V>(hook, asked),
        State::RedSkull(state) => red_skull::event_hook::<V>(state, hook, asked),
        State::TinyMailbox => tiny_mailbox::event_hook::<V>(hook, asked),
        State::FrozenEgg => frozen_egg::event_hook::<V>(hook, asked),
        State::MoltenEgg => molten_egg::event_hook::<V>(hook, asked),
        State::FresnelLens => fresnel_lens::event_hook::<V>(hook, asked),
        State::ToxicEgg => toxic_egg::event_hook::<V>(hook, asked),
        State::SelfFormingClay => self_forming_clay::event_hook::<V>(hook, asked),
        State::OldCoin => old_coin::event_hook::<V>(hook, asked),
        State::Claws => claws::event_hook::<V>(hook, asked),
        State::Crossbow => crossbow::event_hook::<V>(hook, asked),
        State::IronClub(state) => iron_club::event_hook::<V>(state, hook, asked),
        State::Sai => sai::event_hook::<V>(hook, asked),
        State::TanxsWhistle => tanxs_whistle::event_hook::<V>(hook, asked),
        State::ThrowingAxe(state) => throwing_axe::event_hook::<V>(state, hook),
        State::TriBoomerang => tri_boomerang::event_hook::<V>(hook, asked),
        State::WarHammer => war_hammer::event_hook::<V>(hook, asked),
        State::BeautifulBracelet => beautiful_bracelet::event_hook::<V>(hook, asked),
        State::BlessedAntler => blessed_antler::event_hook::<V>(hook, asked),
        state @ (State::DiamondDiadem(_) | State::Own(_)) => {
            V::own_relic_event_hook(state, hook, asked)
        }
        State::JewelryBox => jewelry_box::event_hook::<V>(hook, asked),
        State::LoomingFruit => looming_fruit::event_hook::<V>(hook, asked),
        State::SignetRing => signet_ring::event_hook::<V>(hook, asked),
        State::DelicateFrond => delicate_frond::event_hook::<V>(hook, asked),
        State::Glitter => glitter::event_hook::<V>(hook, asked),
        State::BloodSoakedRose => blood_soaked_rose::event_hook::<V>(hook, asked),
        State::DistinguishedCape => distinguished_cape::event_hook::<V>(hook, asked),
        State::JeweledMask => jeweled_mask::event_hook::<V>(hook, asked),
        State::PreservedFog => preserved_fog::event_hook::<V>(hook, asked),
        State::SereTalon => sere_talon::event_hook::<V>(hook, asked),
        State::RoyalPoison => royal_poison::event_hook::<V>(hook, asked),
        // Its one hook is `ShouldFlush`, which the fight asks of it.
        State::RingingTriangle => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what RingingTriangle does for {hook:?}"),
        },
        State::Bread => bread::event_hook::<V>(hook, asked),
        State::BigHat => big_hat::event_hook::<V>(hook, asked),
        State::RoyalStamp => royal_stamp::event_hook::<V>(hook, asked),
        State::RunicCapacitor => runic_capacitor::event_hook::<V>(hook, asked),
        State::MusicBox(state) => music_box::event_hook::<V>(state, hook, asked),
        State::ChoicesParadox => choices_paradox::event_hook::<V>(hook, asked),
        State::WhisperingEarring => whispering_earring::event_hook::<V>(hook, asked),
        State::BrilliantScarf(state) => brilliant_scarf::event_hook::<V>(state, hook),
        State::VelvetChoker(state) => velvet_choker::event_hook::<V>(state, hook),
        State::LordsParasol => lords_parasol::event_hook::<V>(hook, asked),
        State::HistoryCourse => history_course::event_hook::<V>(hook, asked),
        State::FurCoat(state) => fur_coat::event_hook::<V>(state, hook, asked),
        State::PhilosophersStone => philosophers_stone::event_hook::<V>(hook, asked),
        State::GnarledHammer => gnarled_hammer::event_hook::<V>(hook, asked),
        State::DragonFruit => dragon_fruit::event_hook::<V>(hook, asked),
        State::BeltBuckle(state) => belt_buckle::event_hook::<V>(state, hook, asked),
        State::DingyRug => dingy_rug::event_hook::<V>(hook, asked),
        State::TheAbacus => the_abacus::event_hook::<V>(hook, asked),
        State::ScreamingFlagon => screaming_flagon::event_hook::<V>(hook, asked),
        State::Brimstone => brimstone::event_hook::<V>(hook, asked),
        State::NinjaScroll => ninja_scroll::event_hook::<V>(hook, asked),
        State::PaelsTooth(state) => paels_tooth::event_hook::<V>(state, hook, asked),
        State::PrismaticGem => prismatic_gem::event_hook::<V>(hook, asked),
        State::DemonTongue(state) => demon_tongue::event_hook::<V>(state, hook, asked),
        State::SeaGlass(state) => sea_glass::event_hook::<V>(state, hook, asked),
        State::GoldenCompass(state) => golden_compass::event_hook::<V>(state, hook, asked),
        State::PaelsLegion(state) => paels_legion::event_hook::<V>(state, hook, asked),
        State::PaelsEye(state) => paels_eye::event_hook::<V>(state, belt, hook, asked),
        State::Ectoplasm => ectoplasm::event_hook::<V>(hook),
        State::SneckoEye => snecko_eye::event_hook::<V>(hook, asked),
        State::ElectricShrymp => electric_shrymp::event_hook::<V>(hook, asked),
        State::ToyBox(state) => toy_box::event_hook::<V>(state, hook, asked),
        State::PaelsWing(state) => paels_wing::event_hook::<V>(state, hook, asked),
        State::Driftwood => driftwood::event_hook::<V>(hook, asked),
        State::PaelsGrowth => paels_growth::event_hook::<V>(hook, asked),
        State::PaelsClaw => paels_claw::event_hook::<V>(hook, asked),
        State::PumpkinCandle(state) => pumpkin_candle::event_hook::<V>(state, hook, asked),
        State::NutritiousSoup => nutritious_soup::event_hook::<V>(hook, asked),
        State::MiniatureTent => miniature_tent::event_hook::<V>(hook, asked),
        State::Toolbox => toolbox::event_hook::<V>(hook, asked),
        State::WingCharm => wing_charm::event_hook::<V>(hook, asked),
        State::LunarPastry => lunar_pastry::event_hook::<V>(hook, asked),
        State::Cauldron => cauldron::event_hook::<V>(hook, asked),
        State::GoldPlatedCables => gold_plated_cables::event_hook::<V>(hook, asked),
        State::MysticLighter => mystic_lighter::event_hook::<V>(hook, asked),
        State::ChemicalX => chemical_x::event_hook::<V>(hook, asked),
        State::Orrery => orrery::event_hook::<V>(hook, asked),
        State::PowerCell => power_cell::event_hook::<V>(hook, asked),
        State::SlingOfCourage => sling_of_courage::event_hook::<V>(hook, asked),
        State::RazorTooth => razor_tooth::event_hook::<V>(hook, asked),
        State::MiniRegent(state) => mini_regent::event_hook::<V>(state, hook, asked),
        State::GalacticDust(state) => galactic_dust::event_hook::<V>(state, hook, asked),
        State::UnceasingTop => unceasing_top::event_hook::<V>(hook, asked),
        State::FuneraryMask => funerary_mask::event_hook::<V>(hook, asked),
        State::CaptainsWheel => captains_wheel::event_hook::<V>(hook, asked),
        State::ArtOfWar(state) => art_of_war::event_hook::<V>(state, hook, asked),
        State::BurningSticks(state) => burning_sticks::event_hook::<V>(state, hook, asked),
        State::GamblingChip => gambling_chip::event_hook::<V>(hook, asked),
        State::HelicalDart => helical_dart::event_hook::<V>(hook, asked),
        State::TeaOfDiscourtesy(state) => tea_of_discourtesy::event_hook::<V>(state, hook, asked),
        State::PunchDagger => punch_dagger::event_hook::<V>(hook, asked),
        State::Kifuda => kifuda::event_hook::<V>(hook, asked),
        State::EmberTea(state) => ember_tea::event_hook::<V>(state, hook, asked),
        State::LavaLamp(state) => lava_lamp::event_hook::<V>(state, hook, asked),
        State::LeesWaffle => lees_waffle::event_hook::<V>(hook, asked),
        State::GhostSeed => ghost_seed::event_hook::<V>(hook, asked),
        State::Planisphere => planisphere::event_hook::<V>(hook, asked),
        State::Kaleidoscope => kaleidoscope::event_hook::<V>(hook, asked),
        State::ScrollBoxes => scroll_boxes::event_hook::<V>(hook, asked),
        State::SilkenTress(state) => silken_tress::event_hook::<V>(state, hook, asked),
        State::SilverCrucible(state) => silver_crucible::event_hook::<V>(state, hook, asked),
        State::NeowsBones => neows_bones::event_hook::<V>(hook, asked),
        State::LargeCapsule => large_capsule::event_hook::<V>(hook, asked),
        State::SmallCapsule => small_capsule::event_hook::<V>(hook, asked),
        State::FencingManual => fencing_manual::event_hook::<V>(hook, asked),
        State::ToughBandages => tough_bandages::event_hook::<V>(hook, asked),
        State::Tingsha => tingsha::event_hook::<V>(hook, asked),
        State::MembershipCard => membership_card::event_hook::<V>(hook),
        State::TheCourier => the_courier::event_hook::<V>(hook),
        State::VitruvianMinion => vitruvian_minion::event_hook::<V>(hook),
        State::EmotionChip(state) => emotion_chip::event_hook::<V>(state, hook, asked),
        State::UnsettlingLamp(state) => unsettling_lamp::event_hook::<V>(state, hook),
        State::Vambrace(state) => vambrace::event_hook::<V>(state, hook),
        State::BookRepairKnife => book_repair_knife::event_hook::<V>(hook, asked),
        State::Metronome(state) => metronome::event_hook::<V>(state, hook, asked),
        State::ParryingShield => parrying_shield::event_hook::<V>(hook, asked),
        State::Bookmark => bookmark::event_hook::<V>(hook, asked),
        State::IvoryTile => ivory_tile::event_hook::<V>(hook, asked),
        State::MummifiedHand => mummified_hand::event_hook::<V>(hook, asked),
        State::JossPaper(state) => joss_paper::event_hook::<V>(state, hook, asked),
        State::Bellows => bellows::event_hook::<V>(hook, asked),
        State::Regalite => regalite::event_hook::<V>(hook, asked),
        State::GamePiece => game_piece::event_hook::<V>(hook, asked),
        State::Candelabra => candelabra::event_hook::<V>(hook, asked),
        State::LastingCandy(state) => lasting_candy::event_hook::<V>(state, hook, asked),
        State::OrangeDough => orange_dough::event_hook::<V>(hook, asked),
        State::WhiteBeastStatue => white_beast_statue::event_hook::<V>(hook, asked),
        State::SymbioticVirus => symbiotic_virus::event_hook::<V>(hook, asked),
        State::RuinedHelmet(state) => ruined_helmet::event_hook::<V>(state, hook, asked),
        State::SwordOfJade => sword_of_jade::event_hook::<V>(hook, asked),
        State::Girya(state) => girya::event_hook::<V>(state, hook, asked),
        State::Shovel => shovel::event_hook::<V>(hook, asked),
        State::VexingPuzzlebox => vexing_puzzlebox::event_hook::<V>(hook, asked),
        State::StoneCracker => stone_cracker::event_hook::<V>(hook, asked),
        State::PetrifiedToad => petrified_toad::event_hook::<V>(hook, asked),
        State::IceCream => ice_cream::event_hook::<V>(hook, asked),
        State::MeatOnTheBone => meat_on_the_bone::event_hook::<V>(hook, asked),
        State::PaperKrane => paper_krane::event_hook::<V>(hook, asked),
        State::PaperPhrog => paper_phrog::event_hook::<V>(hook, asked),
        State::SneckoSkull => snecko_skull::event_hook::<V>(hook, asked),
        State::LizardTail(state) => lizard_tail::event_hook::<V>(state, hook, asked),
        State::SturdyClamp => sturdy_clamp::event_hook::<V>(hook, asked),
        State::RainbowRing(state) => rainbow_ring::event_hook::<V>(state, hook, asked),
        State::TuningFork(state) => tuning_fork::event_hook::<V>(state, hook, asked),
        State::LetterOpener(state) => letter_opener::event_hook::<V>(state, hook, asked),
        State::OrnamentalFan(state) => ornamental_fan::event_hook::<V>(state, hook, asked),
        State::Shuriken(state) => shuriken::event_hook::<V>(state, hook, asked),
        State::StoneCalendar(state) => stone_calendar::event_hook::<V>(state, hook, asked),
        State::CloakClasp => cloak_clasp::event_hook::<V>(hook, asked),
        State::CharonsAshes => charons_ashes::event_hook::<V>(hook, asked),
        State::OddlySmoothStone => oddly_smooth_stone::event_hook::<V>(hook, asked),
        State::PotionBelt => potion_belt::event_hook::<V>(hook, asked),
        // Its hooks are `ShouldDraw` and `ModifyHandDrawLate`, which the fight
        // asks of it.
        // `AfterPreventingDraw` flashes it and does nothing else.
        State::Fiddle => match hook {
            EventHook::Obtained { .. } | EventHook::DrawPrevented => {}
            hook => unimplemented!("what Fiddle does for {hook:?}"),
        },
        // `TryModifyRestSiteOptions`: `COOK`.
        State::MeatCleaver => match hook {
            EventHook::Obtained { .. } => {}
            EventHook::RestSiteOptions => asked.push(sts2_core::HookEffect::OfferRestOption(
                V::RestSiteOption::COOK,
            )),
            hook => unimplemented!("what {state:?} does for {hook:?}"),
        },
        // Its hooks are ones the fight asks of it: `ModifyMaxEnergy` and
        // `TryModifyEnergyCostInCombat`.
        State::SpikedGauntlets => match hook {
            EventHook::Obtained { .. } => {}
            hook => unimplemented!("what {state:?} does for {hook:?}"),
        },
    }
}

/// Ask one relic a yes-or-no question.
///
/// The fight and the run ask only a relic that overrides the hook. It sits on
/// `belt`.
/// Out of line: inlined into the walk, it slows every hook the walk asks.
#[inline(never)]
pub(crate) fn decide_hook<V: crate::Build>(
    state: &State<V>,
    belt: &[V::Relic],
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    match state {
        State::WingedBoots(state) => winged_boots::decide_hook::<V>(state, hook),
        State::VelvetChoker(state) => velvet_choker::decide_hook::<V>(state, hook),
        State::RunicPyramid => runic_pyramid::decide_hook::<V>(hook),
        State::RingingTriangle => ringing_triangle::decide_hook::<V>(hook),
        State::Sozu => sozu::decide_hook::<V>(hook),
        State::Fiddle => fiddle::decide_hook::<V>(hook),
        State::SturdyClamp => sturdy_clamp::decide_hook::<V>(hook),
        State::IceCream => ice_cream::decide_hook::<V>(hook),
        State::WhiteBeastStatue => white_beast_statue::decide_hook::<V>(hook),
        State::LizardTail(state) => lizard_tail::decide_hook::<V>(state, hook),
        State::TheCourier => the_courier::decide_hook::<V>(hook),
        State::MiniatureTent => miniature_tent::decide_hook::<V>(hook),
        State::SilverCrucible(state) => silver_crucible::decide_hook::<V>(state, hook),
        State::PaelsEye(state) => paels_eye::decide_hook::<V>(state, belt, hook),
        _ => match hook.model_hook() {
            Some(_) => unimplemented!("what {state:?} decides for {hook:?}"),
            None => sts2_core::HookResponse::DoesNotImplement,
        },
    }
}

/// Ask one relic to change a value.
/// Out of line: inlined into the walk, it slows every hook the walk asks.
#[inline(never)]
pub(crate) fn modify_hook<V: crate::Build>(
    state: &State<V>,
    fight: Option<&dyn sts2_core::FightView<V>>,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    // A shared relic whose rule is the build's own, for a hook it answers
    // itself.
    let owned = match state {
        State::PollinousCore(_) => V::own_relic(V::Relic::POLLINOUS_CORE),
        _ => false,
    };
    if owned && let Some(modify) = V::own_shared_relic_modify_hook(state, hook) {
        return modify;
    }
    match state {
        State::UndyingSigil => undying_sigil::modify_hook::<V>(fight, hook),
        State::Pocketwatch(state) => pocketwatch::modify_hook::<V>(state, hook),
        State::BrilliantScarf(state) => brilliant_scarf::modify_hook::<V>(state, hook),
        State::VelvetChoker(_) => velvet_choker::modify_hook::<V>(hook),
        State::Vambrace(state) => vambrace::modify_hook::<V>(state, hook),
        State::PaelsLegion(state) => paels_legion::modify_hook::<V>(state, hook),
        State::PumpkinCandle(state) => pumpkin_candle::modify_hook::<V>(state, hook),
        State::TheCourier => the_courier::modify_hook::<V>(hook),
        State::VitruvianMinion => vitruvian_minion::modify_hook::<V>(hook),
        State::MembershipCard => membership_card::modify_hook::<V>(hook),
        State::UnsettlingLamp(state) => unsettling_lamp::modify_hook::<V>(state, hook),
        State::PollinousCore(state) => pollinous_core::modify_hook::<V>(state, hook),
        State::RingOfTheSnake => ring_of_the_snake::modify_hook::<V>(hook),
        State::BoomingConch => booming_conch::modify_hook::<V>(hook),
        State::Pendulum(state) => pendulum::modify_hook::<V>(state, hook),
        State::BagOfPreparation => bag_of_preparation::modify_hook::<V>(hook),
        State::BigMushroom => big_mushroom::modify_hook::<V>(hook),
        State::StrikeDummy => strike_dummy::modify_hook::<V>(hook),
        State::BowlerHat => bowler_hat::modify_hook::<V>(hook),
        State::Ectoplasm => ectoplasm::modify_hook::<V>(hook),
        State::Bread => bread::modify_hook::<V>(hook),
        State::WhisperingEarring => whispering_earring::modify_hook::<V>(hook),
        State::PhilosophersStone => philosophers_stone::modify_hook::<V>(hook),
        State::PrismaticGem => prismatic_gem::modify_hook::<V>(hook),
        State::SneckoEye => snecko_eye::modify_hook::<V>(hook),
        State::FakeStrikeDummy => fake_strike_dummy::modify_hook::<V>(hook),
        State::ChemicalX => chemical_x::modify_hook::<V>(hook),
        State::MysticLighter => mystic_lighter::modify_hook::<V>(hook),
        State::GoldPlatedCables => gold_plated_cables::modify_hook::<V>(hook),
        State::PaelsBlood => paels_blood::modify_hook::<V>(hook),
        State::PaelsFlesh => paels_flesh::modify_hook::<V>(hook),
        State::Sozu => sozu::modify_hook::<V>(hook),
        State::TungstenRod => tungsten_rod::modify_hook::<V>(hook),
        State::TheBoot => the_boot::modify_hook::<V>(hook),
        State::RingOfTheDrake => ring_of_the_drake::modify_hook::<V>(hook),
        State::InfusedCore => infused_core::modify_hook::<V>(hook),
        State::PenNib(state) => pen_nib::modify_hook::<V>(state, hook),
        State::MiniatureCannon => miniature_cannon::modify_hook::<V>(hook),
        State::RegalPillow => regal_pillow::modify_hook::<V>(hook),
        State::BeatingRemnant(state) => beating_remnant::modify_hook::<V>(state, hook),
        State::SpikedGauntlets => spiked_gauntlets::modify_hook::<V>(hook),
        State::ThrowingAxe(state) => throwing_axe::modify_hook::<V>(state, hook),
        State::BlessedAntler => blessed_antler::modify_hook::<V>(hook),
        State::BloodSoakedRose => blood_soaked_rose::modify_hook::<V>(hook),
        State::Fiddle => fiddle::modify_hook::<V>(hook),
        State::SneckoSkull => snecko_skull::modify_hook::<V>(hook),
        State::RuinedHelmet(state) => ruined_helmet::modify_hook::<V>(state, hook),
        // The fight asks only a relic that overrides the hook, and none of
        // the rest overrides one of these.
        State::Own(_)
        | State::AlchemicalCoffer
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
        | State::BingBong(_)
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
        | State::MawBank(_)
        | State::DreamCatcher
        | State::FakeMerchantsRug
        | State::WongoCustomerAppreciationBadge
        | State::DarkstonePeriapt
        | State::HandDrill
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
        | State::RippleBasin(_)
        | State::PrayerWheel
        | State::WhiteStar
        | State::RedSkull(_)
        | State::TinyMailbox
        | State::FrozenEgg
        | State::MoltenEgg
        | State::FresnelLens
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
        | State::PaperKrane
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
        | State::Candelabra
        | State::GamePiece
        | State::Regalite
        | State::Bellows
        | State::JossPaper(_)
        | State::MummifiedHand
        | State::IvoryTile
        | State::Bookmark
        | State::ParryingShield
        | State::Metronome(_)
        | State::BookRepairKnife
        | State::EmotionChip(_)
        | State::Tingsha
        | State::ToughBandages
        | State::FencingManual
        | State::SmallCapsule
        | State::LargeCapsule
        | State::NeowsBones
        | State::SilverCrucible(_)
        | State::SilkenTress(_)
        | State::ScrollBoxes
        | State::Kaleidoscope
        | State::Planisphere
        | State::GhostSeed
        | State::LeesWaffle
        | State::LavaLamp(_)
        | State::EmberTea(_)
        | State::Kifuda
        | State::PunchDagger
        | State::TeaOfDiscourtesy(_)
        | State::HelicalDart
        | State::GamblingChip
        | State::BurningSticks(_)
        | State::ArtOfWar(_)
        | State::CaptainsWheel
        | State::FuneraryMask
        | State::UnceasingTop
        | State::GalacticDust(_)
        | State::MiniRegent(_)
        | State::RazorTooth
        | State::SlingOfCourage
        | State::PowerCell
        | State::Orrery
        | State::Cauldron
        | State::LunarPastry
        | State::WingCharm
        | State::Toolbox
        | State::MiniatureTent
        | State::NutritiousSoup
        | State::PaelsClaw
        | State::PaelsGrowth
        | State::Driftwood
        | State::PaelsWing(_)
        | State::ToyBox(_)
        | State::ElectricShrymp
        | State::PaelsEye(_)
        | State::GoldenCompass(_)
        | State::SeaGlass(_)
        | State::DemonTongue(_)
        | State::PaelsTooth(_)
        | State::NinjaScroll
        | State::Brimstone
        | State::ScreamingFlagon
        | State::TheAbacus
        | State::DingyRug
        | State::BeltBuckle(_)
        | State::RingingTriangle
        | State::BigHat
        | State::RoyalStamp
        | State::RunicCapacitor
        | State::MusicBox(_)
        | State::ChoicesParadox
        | State::FurCoat(_)
        | State::GnarledHammer
        | State::DragonFruit
        | State::LordsParasol
        | State::HistoryCourse => unimplemented!("what {state:?} does for {hook:?}"),
    }
}

impl<V: crate::Build> sts2_core::RelicModel<V> for State<V> {
    /// One relic's model as the run picks it up.
    ///
    /// # Panics
    ///
    /// A relic nobody has ported.
    fn new(relic: V::Relic) -> Self {
        match relic {
            __c0 if __c0 == V::Relic::ALCHEMICAL_COFFER => State::AlchemicalCoffer,
            __c0 if __c0 == V::Relic::ANCHOR => State::Anchor,
            __c0 if __c0 == V::Relic::ARCHAIC_TOOTH => State::ArchaicTooth,
            __c0 if __c0 == V::Relic::BIIIG_HUG => State::BiiigHug,
            __c0 if __c0 == V::Relic::ARCANE_SCROLL => State::ArcaneScroll,
            __c0 if __c0 == V::Relic::BONE_TEA => State::BoneTea(bone_tea::State::default()),
            __c0 if __c0 == V::Relic::BOOMING_CONCH => State::BoomingConch,
            __c0 if __c0 == V::Relic::BOUND_PHYLACTERY => State::BoundPhylactery,
            __c0 if __c0 == V::Relic::BURNING_BLOOD => State::BurningBlood,
            __c0 if __c0 == V::Relic::BYRDPIP => State::Byrdpip,
            __c0 if __c0 == V::Relic::CHOSEN_CHEESE => State::ChosenCheese,
            __c0 if __c0 == V::Relic::CRACKED_CORE => State::CrackedCore,
            __c0 if __c0 == V::Relic::CURSED_PEARL => State::CursedPearl,
            __c0 if __c0 == V::Relic::DIVINE_RIGHT => State::DivineRight,
            __c0 if __c0 == V::Relic::DOLLYS_MIRROR => State::DollysMirror,
            __c0 if __c0 == V::Relic::DUSTY_TOME => State::DustyTome,
            __c0 if __c0 == V::Relic::FISHING_ROD => {
                State::FishingRod(fishing_rod::State::default())
            }
            __c0 if __c0 == V::Relic::WONGOS_MYSTERY_TICKET => {
                State::WongosMysteryTicket(wongos_mystery_ticket::State::default())
            }
            __c0 if __c0 == V::Relic::GLASS_EYE => State::GlassEye,
            __c0 if __c0 == V::Relic::GOLDEN_PEARL => State::GoldenPearl,
            __c0 if __c0 == V::Relic::GORGET => State::Gorget,
            __c0 if __c0 == V::Relic::GREMLIN_HORN => State::GremlinHorn,
            __c0 if __c0 == V::Relic::HEFTY_TABLET => State::HeftyTablet,
            __c0 if __c0 == V::Relic::LAVA_ROCK => State::LavaRock(lava_rock::State::default()),
            __c0 if __c0 == V::Relic::LEAD_PAPERWEIGHT => State::LeadPaperweight,
            __c0 if __c0 == V::Relic::LEAFY_POULTICE => State::LeafyPoultice,
            __c0 if __c0 == V::Relic::LOST_COFFER => State::LostCoffer,
            __c0 if __c0 == V::Relic::LOST_WISP => State::LostWisp,
            __c0 if __c0 == V::Relic::MERCURY_HOURGLASS => State::MercuryHourglass,
            __c0 if __c0 == V::Relic::DAUGHTER_OF_THE_WIND => State::DaughterOfTheWind,
            __c0 if __c0 == V::Relic::MR_STRUGGLES => State::MrStruggles,
            __c0 if __c0 == V::Relic::BING_BONG => State::BingBong(bing_bong::State::default()),
            __c0 if __c0 == V::Relic::INTIMIDATING_HELMET => State::IntimidatingHelmet,
            __c0 if __c0 == V::Relic::RUNIC_PYRAMID => State::RunicPyramid,
            __c0 if __c0 == V::Relic::TUNGSTEN_ROD => State::TungstenRod,
            __c0 if __c0 == V::Relic::THE_BOOT => State::TheBoot,
            __c0 if __c0 == V::Relic::UNDYING_SIGIL => State::UndyingSigil,
            __c0 if __c0 == V::Relic::TOUCH_OF_OROBAS => State::TouchOfOrobas,
            __c0 if __c0 == V::Relic::BLACK_BLOOD => State::BlackBlood,
            __c0 if __c0 == V::Relic::RING_OF_THE_DRAKE => State::RingOfTheDrake,
            __c0 if __c0 == V::Relic::DIVINE_DESTINY => State::DivineDestiny,
            __c0 if __c0 == V::Relic::PHYLACTERY_UNBOUND => State::PhylacteryUnbound,
            __c0 if __c0 == V::Relic::INFUSED_CORE => State::InfusedCore,
            __c0 if __c0 == V::Relic::CIRCLET => State::Circlet,
            __c0 if __c0 == V::Relic::RADIANT_PEARL => State::RadiantPearl,
            __c0 if __c0 == V::Relic::TOASTY_MITTENS => State::ToastyMittens,
            __c0 if __c0 == V::Relic::EMPTY_CAGE => State::EmptyCage,
            __c0 if __c0 == V::Relic::ASTROLABE => State::Astrolabe,
            __c0 if __c0 == V::Relic::PANDORAS_BOX => State::PandorasBox,
            __c0 if __c0 == V::Relic::CALLING_BELL => State::CallingBell,
            __c0 if __c0 == V::Relic::BLACK_STAR => State::BlackStar,
            __c0 if __c0 == V::Relic::SOZU => State::Sozu,
            __c0 if __c0 == V::Relic::NEOWS_TALISMAN => State::NeowsTalisman,
            __c0 if __c0 == V::Relic::NEOWS_TORMENT => State::NeowsTorment,
            __c0 if __c0 == V::Relic::NEW_LEAF => State::NewLeaf,
            __c0 if __c0 == V::Relic::NUTRITIOUS_OYSTER => State::NutritiousOyster,
            __c0 if __c0 == V::Relic::PAELS_BLOOD => State::PaelsBlood,
            __c0 if __c0 == V::Relic::PAELS_FLESH => State::PaelsFlesh,
            __c0 if __c0 == V::Relic::PAELS_HORN => State::PaelsHorn,
            __c0 if __c0 == V::Relic::PAELS_TEARS => {
                State::PaelsTears(paels_tears::State::default())
            }
            __c0 if __c0 == V::Relic::PERMAFROST => State::Permafrost(permafrost::State::default()),
            __c0 if __c0 == V::Relic::PHIAL_HOLSTER => State::PhialHolster,
            __c0 if __c0 == V::Relic::POCKETWATCH => {
                State::Pocketwatch(pocketwatch::State::default())
            }
            __c0 if __c0 == V::Relic::POLLINOUS_CORE => {
                State::PollinousCore(pollinous_core::State::default())
            }
            __c0 if __c0 == V::Relic::POMANDER => State::Pomander,
            __c0 if __c0 == V::Relic::PRECARIOUS_SHEARS => State::PrecariousShears,
            __c0 if __c0 == V::Relic::PRECISE_SCISSORS => State::PreciseScissors,
            __c0 if __c0 == V::Relic::RING_OF_THE_SNAKE => State::RingOfTheSnake,
            __c0 if __c0 == V::Relic::SAND_CASTLE => State::SandCastle,
            __c0 if __c0 == V::Relic::SEAL_OF_GOLD => State::SealOfGold,
            __c0 if __c0 == V::Relic::STONE_HUMIDIFIER => State::StoneHumidifier,
            __c0 if __c0 == V::Relic::STORYBOOK => State::Storybook,
            __c0 if __c0 == V::Relic::STRIKE_DUMMY => State::StrikeDummy,
            __c0 if __c0 == V::Relic::FAKE_STRIKE_DUMMY => State::FakeStrikeDummy,
            __c0 if __c0 == V::Relic::FAKE_ANCHOR => State::FakeAnchor,
            __c0 if __c0 == V::Relic::FAKE_MANGO => State::FakeMango,
            __c0 if __c0 == V::Relic::FAKE_LEES_WAFFLE => State::FakeLeesWaffle,
            __c0 if __c0 == V::Relic::FAKE_BLOOD_VIAL => State::FakeBloodVial,
            __c0 if __c0 == V::Relic::BLOOD_VIAL => State::BloodVial,
            __c0 if __c0 == V::Relic::HORN_CLEAT => State::HornCleat,
            __c0 if __c0 == V::Relic::SPARKLING_ROUGE => State::SparklingRouge,
            __c0 if __c0 == V::Relic::BOWLER_HAT => State::BowlerHat,
            __c0 if __c0 == V::Relic::ORICHALCUM => State::Orichalcum(orichalcum::State::default()),
            __c0 if __c0 == V::Relic::FAKE_ORICHALCUM => {
                State::FakeOrichalcum(fake_orichalcum::State::default())
            }
            __c0 if __c0 == V::Relic::FAKE_HAPPY_FLOWER => {
                State::FakeHappyFlower(fake_happy_flower::State::default())
            }
            __c0 if __c0 == V::Relic::FAKE_VENERABLE_TEA_SET => {
                State::FakeVenerableTeaSet(fake_venerable_tea_set::State::default())
            }
            __c0 if __c0 == V::Relic::SWORD_OF_STONE => {
                State::SwordOfStone(sword_of_stone::State::default())
            }
            __c0 if __c0 == V::Relic::VERY_HOT_COCOA => State::VeryHotCocoa,
            __c0 if __c0 == V::Relic::BAG_OF_MARBLES => State::BagOfMarbles,
            __c0 if __c0 == V::Relic::AKABEKO => State::Akabeko,
            __c0 if __c0 == V::Relic::FESTIVE_POPPER => State::FestivePopper,
            __c0 if __c0 == V::Relic::BRONZE_SCALES => State::BronzeScales,
            __c0 if __c0 == V::Relic::VAJRA => State::Vajra,
            __c0 if __c0 == V::Relic::MEAL_TICKET => State::MealTicket,
            __c0 if __c0 == V::Relic::STRAWBERRY => State::Strawberry,
            __c0 if __c0 == V::Relic::WAR_PAINT => State::WarPaint,
            __c0 if __c0 == V::Relic::BIG_MUSHROOM => State::BigMushroom,
            __c0 if __c0 == V::Relic::FORGOTTEN_SOUL => State::ForgottenSoul,
            __c0 if __c0 == V::Relic::FRAGRANT_MUSHROOM => State::FragrantMushroom,
            __c0 if __c0 == V::Relic::WHETSTONE => State::Whetstone,
            __c0 if __c0 == V::Relic::JUZU_BRACELET => State::JuzuBracelet,
            __c0 if __c0 == V::Relic::BAG_OF_PREPARATION => State::BagOfPreparation,
            __c0 if __c0 == V::Relic::WINGED_BOOTS => {
                State::WingedBoots(winged_boots::State::default())
            }
            __c0 if __c0 == V::Relic::YUMMY_COOKIE => State::YummyCookie,
            __c0 if __c0 == V::Relic::RED_MASK => State::RedMask,
            __c0 if __c0 == V::Relic::LANTERN => State::Lantern,
            __c0 if __c0 == V::Relic::LUCKY_FYSH => State::LuckyFysh,
            __c0 if __c0 == V::Relic::MAW_BANK => State::MawBank(maw_bank::State::default()),
            __c0 if __c0 == V::Relic::DREAM_CATCHER => State::DreamCatcher,
            __c0 if __c0 == V::Relic::FAKE_MERCHANTS_RUG => State::FakeMerchantsRug,
            __c0 if __c0 == V::Relic::WONGO_CUSTOMER_APPRECIATION_BADGE => {
                State::WongoCustomerAppreciationBadge
            }
            __c0 if __c0 == V::Relic::DARKSTONE_PERIAPT => State::DarkstonePeriapt,
            __c0 if __c0 == V::Relic::HAND_DRILL => State::HandDrill,
            __c0 if __c0 == V::Relic::AMETHYST_AUBERGINE => State::AmethystAubergine,
            __c0 if __c0 == V::Relic::ETERNAL_FEATHER => State::EternalFeather,
            __c0 if __c0 == V::Relic::PANTOGRAPH => State::Pantograph,
            __c0 if __c0 == V::Relic::FAKE_SNECKO_EYE => State::FakeSneckoEye,
            __c0 if __c0 == V::Relic::CENTENNIAL_PUZZLE => {
                State::CentennialPuzzle(centennial_puzzle::State::default())
            }
            __c0 if __c0 == V::Relic::PEN_NIB => State::PenNib(pen_nib::State::default()),
            __c0 if __c0 == V::Relic::MANGO => State::Mango,
            __c0 if __c0 == V::Relic::MINIATURE_CANNON => State::MiniatureCannon,
            __c0 if __c0 == V::Relic::REGAL_PILLOW => State::RegalPillow,
            __c0 if __c0 == V::Relic::BOOK_OF_FIVE_RINGS => {
                State::BookOfFiveRings(book_of_five_rings::State::default())
            }
            __c0 if __c0 == V::Relic::KUSARIGAMA => State::Kusarigama(kusarigama::State::default()),
            __c0 if __c0 == V::Relic::BEATING_REMNANT => {
                State::BeatingRemnant(beating_remnant::State::default())
            }
            __c0 if __c0 == V::Relic::REPTILE_TRINKET => State::ReptileTrinket,
            __c0 if __c0 == V::Relic::PEAR => State::Pear,
            __c0 if __c0 == V::Relic::DATA_DISK => State::DataDisk,
            __c0 if __c0 == V::Relic::BONE_FLUTE => State::BoneFlute,
            __c0 if __c0 == V::Relic::HAPPY_FLOWER => {
                State::HappyFlower(happy_flower::State::default())
            }
            __c0 if __c0 == V::Relic::CHANDELIER => State::Chandelier,
            __c0 if __c0 == V::Relic::KUNAI => State::Kunai(kunai::State::default()),
            __c0 if __c0 == V::Relic::NUNCHAKU => State::Nunchaku(nunchaku::State::default()),
            __c0 if __c0 == V::Relic::VENERABLE_TEA_SET => {
                State::VenerableTeaSet(venerable_tea_set::State::default())
            }
            __c0 if __c0 == V::Relic::TWISTED_FUNNEL => State::TwistedFunnel,
            __c0 if __c0 == V::Relic::PENDULUM => State::Pendulum(pendulum::State::default()),
            __c0 if __c0 == V::Relic::RIPPLE_BASIN => {
                State::RippleBasin(ripple_basin::State::default())
            }
            __c0 if __c0 == V::Relic::PRAYER_WHEEL => State::PrayerWheel,
            __c0 if __c0 == V::Relic::WHITE_STAR => State::WhiteStar,
            __c0 if __c0 == V::Relic::RED_SKULL => State::RedSkull(red_skull::State::default()),
            __c0 if __c0 == V::Relic::TINY_MAILBOX => State::TinyMailbox,
            __c0 if __c0 == V::Relic::FROZEN_EGG => State::FrozenEgg,
            __c0 if __c0 == V::Relic::MOLTEN_EGG => State::MoltenEgg,
            __c0 if __c0 == V::Relic::FRESNEL_LENS => State::FresnelLens,
            __c0 if __c0 == V::Relic::TOXIC_EGG => State::ToxicEgg,
            __c0 if __c0 == V::Relic::SELF_FORMING_CLAY => State::SelfFormingClay,
            __c0 if __c0 == V::Relic::OLD_COIN => State::OldCoin,
            __c0 if __c0 == V::Relic::CLAWS => State::Claws,
            __c0 if __c0 == V::Relic::CROSSBOW => State::Crossbow,
            __c0 if __c0 == V::Relic::IRON_CLUB => State::IronClub(iron_club::State::default()),
            __c0 if __c0 == V::Relic::MEAT_CLEAVER => State::MeatCleaver,
            __c0 if __c0 == V::Relic::SAI => State::Sai,
            __c0 if __c0 == V::Relic::SPIKED_GAUNTLETS => State::SpikedGauntlets,
            __c0 if __c0 == V::Relic::TANXS_WHISTLE => State::TanxsWhistle,
            __c0 if __c0 == V::Relic::THROWING_AXE => {
                State::ThrowingAxe(throwing_axe::State::default())
            }
            __c0 if __c0 == V::Relic::TRI_BOOMERANG => State::TriBoomerang,
            __c0 if __c0 == V::Relic::WAR_HAMMER => State::WarHammer,
            __c0 if __c0 == V::Relic::BEAUTIFUL_BRACELET => State::BeautifulBracelet,
            __c0 if __c0 == V::Relic::BLESSED_ANTLER => State::BlessedAntler,
            __c0 if __c0 == V::Relic::DIAMOND_DIADEM => {
                State::DiamondDiadem(diamond_diadem::State::default())
            }
            __c0 if __c0 == V::Relic::JEWELRY_BOX => State::JewelryBox,
            __c0 if __c0 == V::Relic::LOOMING_FRUIT => State::LoomingFruit,
            __c0 if __c0 == V::Relic::SIGNET_RING => State::SignetRing,
            __c0 if __c0 == V::Relic::DELICATE_FROND => State::DelicateFrond,
            __c0 if __c0 == V::Relic::GLITTER => State::Glitter,
            __c0 if __c0 == V::Relic::BLOOD_SOAKED_ROSE => State::BloodSoakedRose,
            __c0 if __c0 == V::Relic::DISTINGUISHED_CAPE => State::DistinguishedCape,
            __c0 if __c0 == V::Relic::FIDDLE => State::Fiddle,
            __c0 if __c0 == V::Relic::JEWELED_MASK => State::JeweledMask,
            __c0 if __c0 == V::Relic::PRESERVED_FOG => State::PreservedFog,
            __c0 if __c0 == V::Relic::SERE_TALON => State::SereTalon,
            __c0 if __c0 == V::Relic::ROYAL_POISON => State::RoyalPoison,
            __c0 if __c0 == V::Relic::RINGING_TRIANGLE => State::RingingTriangle,
            __c0 if __c0 == V::Relic::BREAD => State::Bread,
            __c0 if __c0 == V::Relic::BIG_HAT => State::BigHat,
            __c0 if __c0 == V::Relic::ROYAL_STAMP => State::RoyalStamp,
            __c0 if __c0 == V::Relic::RUNIC_CAPACITOR => State::RunicCapacitor,
            __c0 if __c0 == V::Relic::MUSIC_BOX => State::MusicBox(music_box::State::default()),
            __c0 if __c0 == V::Relic::CHOICES_PARADOX => State::ChoicesParadox,
            __c0 if __c0 == V::Relic::WHISPERING_EARRING => State::WhisperingEarring,
            __c0 if __c0 == V::Relic::BRILLIANT_SCARF => {
                State::BrilliantScarf(brilliant_scarf::State::default())
            }
            __c0 if __c0 == V::Relic::VELVET_CHOKER => {
                State::VelvetChoker(velvet_choker::State::default())
            }
            __c0 if __c0 == V::Relic::LORDS_PARASOL => State::LordsParasol,
            __c0 if __c0 == V::Relic::HISTORY_COURSE => State::HistoryCourse,
            __c0 if __c0 == V::Relic::FUR_COAT => State::FurCoat(fur_coat::State::default()),
            __c0 if __c0 == V::Relic::PHILOSOPHERS_STONE => State::PhilosophersStone,
            __c0 if __c0 == V::Relic::GNARLED_HAMMER => State::GnarledHammer,
            __c0 if __c0 == V::Relic::DRAGON_FRUIT => State::DragonFruit,
            __c0 if __c0 == V::Relic::BELT_BUCKLE => {
                State::BeltBuckle(belt_buckle::State::default())
            }
            __c0 if __c0 == V::Relic::DINGY_RUG => State::DingyRug,
            __c0 if __c0 == V::Relic::THE_ABACUS => State::TheAbacus,
            __c0 if __c0 == V::Relic::SCREAMING_FLAGON => State::ScreamingFlagon,
            __c0 if __c0 == V::Relic::BRIMSTONE => State::Brimstone,
            __c0 if __c0 == V::Relic::NINJA_SCROLL => State::NinjaScroll,
            __c0 if __c0 == V::Relic::PAELS_TOOTH => {
                State::PaelsTooth(paels_tooth::State::default())
            }
            __c0 if __c0 == V::Relic::PRISMATIC_GEM => State::PrismaticGem,
            __c0 if __c0 == V::Relic::DEMON_TONGUE => {
                State::DemonTongue(demon_tongue::State::default())
            }
            __c0 if __c0 == V::Relic::SEA_GLASS => State::SeaGlass(sea_glass::State::default()),
            __c0 if __c0 == V::Relic::GOLDEN_COMPASS => {
                State::GoldenCompass(golden_compass::State::default())
            }
            __c0 if __c0 == V::Relic::PAELS_LEGION => {
                State::PaelsLegion(paels_legion::State::default())
            }
            __c0 if __c0 == V::Relic::PAELS_EYE => State::PaelsEye(paels_eye::State::default()),
            __c0 if __c0 == V::Relic::ECTOPLASM => State::Ectoplasm,
            __c0 if __c0 == V::Relic::SNECKO_EYE => State::SneckoEye,
            __c0 if __c0 == V::Relic::ELECTRIC_SHRYMP => State::ElectricShrymp,
            __c0 if __c0 == V::Relic::TOY_BOX => State::ToyBox(toy_box::State::default()),
            __c0 if __c0 == V::Relic::PAELS_WING => State::PaelsWing(paels_wing::State::default()),
            __c0 if __c0 == V::Relic::DRIFTWOOD => State::Driftwood,
            __c0 if __c0 == V::Relic::PAELS_GROWTH => State::PaelsGrowth,
            __c0 if __c0 == V::Relic::PAELS_CLAW => State::PaelsClaw,
            __c0 if __c0 == V::Relic::PUMPKIN_CANDLE => {
                State::PumpkinCandle(pumpkin_candle::State::default())
            }
            __c0 if __c0 == V::Relic::NUTRITIOUS_SOUP => State::NutritiousSoup,
            __c0 if __c0 == V::Relic::MINIATURE_TENT => State::MiniatureTent,
            __c0 if __c0 == V::Relic::TOOLBOX => State::Toolbox,
            __c0 if __c0 == V::Relic::WING_CHARM => State::WingCharm,
            __c0 if __c0 == V::Relic::LUNAR_PASTRY => State::LunarPastry,
            __c0 if __c0 == V::Relic::CAULDRON => State::Cauldron,
            __c0 if __c0 == V::Relic::GOLD_PLATED_CABLES => State::GoldPlatedCables,
            __c0 if __c0 == V::Relic::MYSTIC_LIGHTER => State::MysticLighter,
            __c0 if __c0 == V::Relic::CHEMICAL_X => State::ChemicalX,
            __c0 if __c0 == V::Relic::ORRERY => State::Orrery,
            __c0 if __c0 == V::Relic::POWER_CELL => State::PowerCell,
            __c0 if __c0 == V::Relic::SLING_OF_COURAGE => State::SlingOfCourage,
            __c0 if __c0 == V::Relic::RAZOR_TOOTH => State::RazorTooth,
            __c0 if __c0 == V::Relic::MINI_REGENT => {
                State::MiniRegent(mini_regent::State::default())
            }
            __c0 if __c0 == V::Relic::GALACTIC_DUST => {
                State::GalacticDust(galactic_dust::State::default())
            }
            __c0 if __c0 == V::Relic::UNCEASING_TOP => State::UnceasingTop,
            __c0 if __c0 == V::Relic::FUNERARY_MASK => State::FuneraryMask,
            __c0 if __c0 == V::Relic::CAPTAINS_WHEEL => State::CaptainsWheel,
            __c0 if __c0 == V::Relic::ART_OF_WAR => State::ArtOfWar(art_of_war::State::default()),
            __c0 if __c0 == V::Relic::BURNING_STICKS => {
                State::BurningSticks(burning_sticks::State::default())
            }
            __c0 if __c0 == V::Relic::GAMBLING_CHIP => State::GamblingChip,
            __c0 if __c0 == V::Relic::HELICAL_DART => State::HelicalDart,
            __c0 if __c0 == V::Relic::TEA_OF_DISCOURTESY => {
                State::TeaOfDiscourtesy(tea_of_discourtesy::State::default())
            }
            __c0 if __c0 == V::Relic::PUNCH_DAGGER => State::PunchDagger,
            __c0 if __c0 == V::Relic::KIFUDA => State::Kifuda,
            __c0 if __c0 == V::Relic::EMBER_TEA => State::EmberTea(ember_tea::State::default()),
            __c0 if __c0 == V::Relic::LAVA_LAMP => State::LavaLamp(lava_lamp::State::default()),
            __c0 if __c0 == V::Relic::LEES_WAFFLE => State::LeesWaffle,
            __c0 if __c0 == V::Relic::GHOST_SEED => State::GhostSeed,
            __c0 if __c0 == V::Relic::PLANISPHERE => State::Planisphere,
            __c0 if __c0 == V::Relic::KALEIDOSCOPE => State::Kaleidoscope,
            __c0 if __c0 == V::Relic::SCROLL_BOXES => State::ScrollBoxes,
            __c0 if __c0 == V::Relic::SILKEN_TRESS => {
                State::SilkenTress(silken_tress::State::default())
            }
            __c0 if __c0 == V::Relic::SILVER_CRUCIBLE => {
                State::SilverCrucible(silver_crucible::State::default())
            }
            __c0 if __c0 == V::Relic::NEOWS_BONES => State::NeowsBones,
            __c0 if __c0 == V::Relic::LARGE_CAPSULE => State::LargeCapsule,
            __c0 if __c0 == V::Relic::SMALL_CAPSULE => State::SmallCapsule,
            __c0 if __c0 == V::Relic::FENCING_MANUAL => State::FencingManual,
            __c0 if __c0 == V::Relic::TOUGH_BANDAGES => State::ToughBandages,
            __c0 if __c0 == V::Relic::TINGSHA => State::Tingsha,
            __c0 if __c0 == V::Relic::MEMBERSHIP_CARD => State::MembershipCard,
            __c0 if __c0 == V::Relic::THE_COURIER => State::TheCourier,
            __c0 if __c0 == V::Relic::VITRUVIAN_MINION => State::VitruvianMinion,
            __c0 if __c0 == V::Relic::EMOTION_CHIP => {
                State::EmotionChip(emotion_chip::State::default())
            }
            __c0 if __c0 == V::Relic::UNSETTLING_LAMP => {
                State::UnsettlingLamp(unsettling_lamp::State::default())
            }
            __c0 if __c0 == V::Relic::VAMBRACE => State::Vambrace(vambrace::State::default()),
            __c0 if __c0 == V::Relic::BOOK_REPAIR_KNIFE => State::BookRepairKnife,
            __c0 if __c0 == V::Relic::METRONOME => State::Metronome(metronome::State::default()),
            __c0 if __c0 == V::Relic::PARRYING_SHIELD => State::ParryingShield,
            __c0 if __c0 == V::Relic::BOOKMARK => State::Bookmark,
            __c0 if __c0 == V::Relic::IVORY_TILE => State::IvoryTile,
            __c0 if __c0 == V::Relic::MUMMIFIED_HAND => State::MummifiedHand,
            __c0 if __c0 == V::Relic::JOSS_PAPER => State::JossPaper(joss_paper::State::default()),
            __c0 if __c0 == V::Relic::BELLOWS => State::Bellows,
            __c0 if __c0 == V::Relic::REGALITE => State::Regalite,
            __c0 if __c0 == V::Relic::GAME_PIECE => State::GamePiece,
            __c0 if __c0 == V::Relic::CANDELABRA => State::Candelabra,
            __c0 if __c0 == V::Relic::LASTING_CANDY => {
                State::LastingCandy(lasting_candy::State::default())
            }
            __c0 if __c0 == V::Relic::ORANGE_DOUGH => State::OrangeDough,
            __c0 if __c0 == V::Relic::WHITE_BEAST_STATUE => State::WhiteBeastStatue,
            __c0 if __c0 == V::Relic::SYMBIOTIC_VIRUS => State::SymbioticVirus,
            __c0 if __c0 == V::Relic::RUINED_HELMET => {
                State::RuinedHelmet(ruined_helmet::State::default())
            }
            __c0 if __c0 == V::Relic::SWORD_OF_JADE => State::SwordOfJade,
            __c0 if __c0 == V::Relic::GIRYA => State::Girya(girya::State::default()),
            __c0 if __c0 == V::Relic::SHOVEL => State::Shovel,
            __c0 if __c0 == V::Relic::VEXING_PUZZLEBOX => State::VexingPuzzlebox,
            __c0 if __c0 == V::Relic::STONE_CRACKER => State::StoneCracker,
            __c0 if __c0 == V::Relic::PETRIFIED_TOAD => State::PetrifiedToad,
            __c0 if __c0 == V::Relic::ICE_CREAM => State::IceCream,
            __c0 if __c0 == V::Relic::MEAT_ON_THE_BONE => State::MeatOnTheBone,
            __c0 if __c0 == V::Relic::PAPER_KRANE => State::PaperKrane,
            __c0 if __c0 == V::Relic::PAPER_PHROG => State::PaperPhrog,
            __c0 if __c0 == V::Relic::SNECKO_SKULL => State::SneckoSkull,
            __c0 if __c0 == V::Relic::LIZARD_TAIL => {
                State::LizardTail(lizard_tail::State::default())
            }
            __c0 if __c0 == V::Relic::STURDY_CLAMP => State::SturdyClamp,
            __c0 if __c0 == V::Relic::RAINBOW_RING => {
                State::RainbowRing(rainbow_ring::State::default())
            }
            __c0 if __c0 == V::Relic::TUNING_FORK => {
                State::TuningFork(tuning_fork::State::default())
            }
            __c0 if __c0 == V::Relic::LETTER_OPENER => {
                State::LetterOpener(letter_opener::State::default())
            }
            __c0 if __c0 == V::Relic::ORNAMENTAL_FAN => {
                State::OrnamentalFan(ornamental_fan::State::default())
            }
            __c0 if __c0 == V::Relic::SHURIKEN => State::Shuriken(shuriken::State::default()),
            __c0 if __c0 == V::Relic::STONE_CALENDAR => {
                State::StoneCalendar(stone_calendar::State::default())
            }
            __c0 if __c0 == V::Relic::CLOAK_CLASP => State::CloakClasp,
            __c0 if __c0 == V::Relic::CHARONS_ASHES => State::CharonsAshes,
            __c0 if __c0 == V::Relic::ODDLY_SMOOTH_STONE => State::OddlySmoothStone,
            __c0 if __c0 == V::Relic::POTION_BELT => State::PotionBelt,
            relic => match V::own_relic_state(relic) {
                Some(own) => State::Own(own),
                None => unimplemented!("what {relic:?} does in a run that carries it"),
            },
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
    fn used_up(&self, relic: V::Relic) -> bool {
        use crate::generated::relics::RelicFlag;

        match (self, Table::data(relic).flags.get(RelicFlag::UsedUp)) {
            (State::LizardTail(state), _) => lizard_tail::used_up(state),
            (State::MawBank(state), _) => maw_bank::used_up(state),
            (State::BoneTea(state), _) => bone_tea::used_up(state),
            (State::WingedBoots(state), _) => state.used_up(),
            (State::WongosMysteryTicket(state), _) => wongos_mystery_ticket::used_up(state),
            (State::SilverCrucible(state), _) => silver_crucible::used_up::<V>(state),
            (State::SilkenTress(state), _) => silken_tress::used_up(state),
            (State::EmberTea(state), _) => state.used_up(),
            (State::ToyBox(state), _) => toy_box::used_up::<V>(state),
            (State::TeaOfDiscourtesy(state), _) => state.used_up(),
            (_, Field::Constant(used_up)) => used_up,
            (_, Field::Computed) => unimplemented!("whether {relic:?} is used up"),
        }
    }

    fn display_amount(&self, relic: V::Relic, fighting: bool, turn: Option<u32>) -> Number {
        match Table::data(relic).display_amount {
            // The table holds the column's own width, and a relic counts in
            // the build's game number. Widening here is the only place the two
            // meet.
            Field::Constant(amount) => Number::from(amount),
            Field::Computed => match self {
                State::BoneTea(state) => bone_tea::display_amount(state),
                State::FishingRod(state) => fishing_rod::display_amount::<V>(state),
                State::FakeHappyFlower(state) => fake_happy_flower::display_amount(state),
                State::WongosMysteryTicket(state) => wongos_mystery_ticket::display_amount(state),
                State::SwordOfStone(state) => sword_of_stone::display_amount(state),
                State::WingedBoots(state) => winged_boots::display_amount(state),
                State::Pocketwatch(state) => pocketwatch::display_amount(state),
                State::PollinousCore(state) => pollinous_core::display_amount(state),
                State::PaelsFlesh => paels_flesh::display_amount(turn),
                State::PenNib(state) => pen_nib::display_amount::<V>(state),
                State::BookOfFiveRings(state) => book_of_five_rings::display_amount::<V>(state),
                State::Kusarigama(state) => kusarigama::display_amount::<V>(state),
                State::HappyFlower(state) => happy_flower::display_amount(state),
                State::Kunai(state) => kunai::display_amount::<V>(state),
                State::PaelsTooth(state) => paels_tooth::display_amount::<V>(state),
                State::PaelsLegion(state) => paels_legion::display_amount::<V>(state, fighting),
                State::ToyBox(state) => toy_box::display_amount::<V>(state),
                State::PaelsWing(state) => paels_wing::display_amount::<V>(state),
                State::PumpkinCandle(state) => pumpkin_candle::display_amount(state),
                State::GalacticDust(state) => galactic_dust::display_amount::<V>(state),
                State::EmberTea(state) => ember_tea::display_amount(state),
                State::SilverCrucible(state) => silver_crucible::display_amount::<V>(state),
                State::Metronome(state) => metronome::display_amount::<V>(state),
                State::JossPaper(state) => joss_paper::display_amount(state),
                State::LastingCandy(state) => lasting_candy::display_amount(state),
                State::Girya(state) => girya::display_amount(state),
                State::TuningFork(state) => tuning_fork::display_amount(state),
                State::LetterOpener(state) => letter_opener::display_amount::<V>(state),
                State::OrnamentalFan(state) => ornamental_fan::display_amount::<V>(state),
                State::Shuriken(state) => shuriken::display_amount::<V>(state),
                State::StoneCalendar(_) => {
                    stone_calendar::display_amount::<V>(turn.filter(|_| fighting))
                }
                State::Nunchaku(state) => nunchaku::display_amount::<V>(state),
                State::IronClub(state) => iron_club::display_amount::<V>(state),
                State::DiamondDiadem(state) => diamond_diadem::display_amount(state),
                State::Pendulum(state) => pendulum::display_amount(state),
                State::BrilliantScarf(state) => brilliant_scarf::display_amount(state),
                State::VelvetChoker(state) => velvet_choker::display_amount(state),
                _ => unimplemented!("the number {relic:?} shows"),
            },
        }
    }

    fn held_cards(&self) -> &[sts2_core::DeckCard<V>] {
        match self {
            State::PaelsTooth(state) => state.held(),
            _ => &[],
        }
    }

    fn marked_points(&self) -> &[sts2_core::Coord] {
        match self {
            State::FurCoat(state) => state.marked(),
            _ => &[],
        }
    }

    fn mark_points(&mut self, points: &[sts2_core::Coord]) {
        match self {
            State::FurCoat(state) => state.mark(points),
            state => unimplemented!("{state:?} marking map points"),
        }
    }

    fn hold_cards(&mut self, cards: &[sts2_core::DeckCard<V>]) {
        match self {
            State::PaelsTooth(state) => state.hold(cards),
            state => unimplemented!("{state:?} keeping deck cards"),
        }
    }

    fn release_card(&mut self, at: usize) -> sts2_core::DeckCard<V> {
        match self {
            State::PaelsTooth(state) => state.release(at),
            state => unimplemented!("{state:?} handing back a deck card"),
        }
    }
}

/// `MODIFIER_CURSES`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `CurseCardPool`'s cards whose `CanBeGeneratedByModifiers` holds, ordered
        /// by id. Ascender's Bane, Bad Luck, Curse of the Bell, Enthralled, Folly,
        /// Greed, Poor Sleep and Spore Mind override it to refuse.
        const RELICS_MODIFIER_CURSES: &'static [<$build as sts2_core::ContentVersion>::Card] = &[
            <$build as sts2_core::ContentVersion>::Card::CLUMSY,
            <$build as sts2_core::ContentVersion>::Card::DEBT,
            <$build as sts2_core::ContentVersion>::Card::DECAY,
            <$build as sts2_core::ContentVersion>::Card::DOUBT,
            <$build as sts2_core::ContentVersion>::Card::GUILTY,
            <$build as sts2_core::ContentVersion>::Card::INJURY,
            <$build as sts2_core::ContentVersion>::Card::NORMALITY,
            <$build as sts2_core::ContentVersion>::Card::REGRET,
            <$build as sts2_core::ContentVersion>::Card::SHAME,
            <$build as sts2_core::ContentVersion>::Card::WRITHE,
        ];
    };
}

pub(crate) use lists;
