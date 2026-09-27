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
mod biiig_hug;
mod bing_bong;
mod black_blood;
mod black_star;
mod blood_vial;
mod bone_tea;
mod booming_conch;
mod bound_phylactery;
mod bowler_hat;
mod bronze_scales;
mod burning_blood;
mod byrdpip;
mod calling_bell;
mod centennial_puzzle;
mod chosen_cheese;
mod cracked_core;
mod cursed_pearl;
mod daughter_of_the_wind;
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
mod fishing_rod;
mod glass_eye;
mod golden_pearl;
mod gorget;
mod gremlin_horn;
mod hefty_tablet;
mod horn_cleat;
mod infused_core;
mod intimidating_helmet;
mod juzu_bracelet;
mod lantern;
mod lava_rock;
mod lead_paperweight;
mod leafy_poultice;
mod lost_coffer;
mod lost_wisp;
mod lucky_fysh;
mod meal_ticket;
mod mercury_hourglass;
mod mr_struggles;
mod neows_talisman;
mod neows_torment;
mod new_leaf;
mod nutritious_oyster;
mod orichalcum;
mod paels_blood;
mod paels_flesh;
mod paels_horn;
mod paels_tears;
mod pandoras_box;
mod pantograph;
mod pen_nib;
mod permafrost;
mod phial_holster;
mod phylactery_unbound;
mod pocketwatch;
mod pollinous_core;
mod pomander;
mod precarious_shears;
mod precise_scissors;
mod radiant_pearl;
mod red_mask;
mod ring_of_the_drake;
mod ring_of_the_snake;
mod runic_pyramid;
mod sand_castle;
mod seal_of_gold;
mod sozu;
mod sparkling_rouge;
mod stone_humidifier;
mod storybook;
mod strawberry;
mod strike_dummy;
mod sword_of_stone;
mod toasty_mittens;
mod touch_of_orobas;
mod tungsten_rod;
mod vajra;
mod very_hot_cocoa;
mod war_paint;
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
}

/// `RelicModel.IsTradable`: not used up, nothing on pickup, not melted, no
/// pet, and neither a starter, an event relic nor an ancient's. Nothing here
/// melts a relic.
///
/// # Panics
///
/// A relic whose model decides for itself whether it is used up, or whether
/// it does something on pickup or brings a pet.
pub(crate) fn tradable(relic: Relic) -> bool {
    use super::super::generated::relics::{RelicFlag, RelicRarity};

    let data = relic.data();
    if matches!(
        data.rarity,
        RelicRarity::Starter | RelicRarity::Event | RelicRarity::Ancient
    ) {
        return false;
    }
    [
        RelicFlag::UsedUp,
        RelicFlag::UponPickup,
        RelicFlag::SpawnsPets,
    ]
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
        State::SwordOfStone(state) => sword_of_stone::event_hook(state, hook),
        State::VeryHotCocoa => very_hot_cocoa::event_hook(hook, asked),
        State::BagOfMarbles => bag_of_marbles::event_hook(hook, asked),
        State::Akabeko => akabeko::event_hook(hook, asked),
        State::FestivePopper => festive_popper::event_hook(hook, asked),
        State::BronzeScales => bronze_scales::event_hook(hook, asked),
        State::Vajra => vajra::event_hook(hook, asked),
        State::MealTicket => meal_ticket::event_hook(hook, asked),
        State::Strawberry => strawberry::event_hook(hook, asked),
        State::WarPaint => war_paint::event_hook(hook, asked),
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
        | State::CentennialPuzzle(_) => unimplemented!("what {state:?} does for {hook:?}"),
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
            relic => unimplemented!("what {relic:?} does in a run that carries it"),
        }
    }

    /// `DisplayAmount`. The table answers for a relic that prints a constant,
    /// and the rest work it out from what they saved.
    ///
    /// # Panics
    ///
    /// A relic that works its number out and nobody has ported.
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
                _ => unimplemented!("the number {relic:?} shows"),
            },
        }
    }
}
