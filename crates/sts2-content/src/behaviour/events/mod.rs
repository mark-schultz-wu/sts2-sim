//! Event behavior dispatch for this build.

use crate::ids::{CardIds as _, EnchantmentIds as _, EventIds as _, PotionIds as _};

mod abyssal_baths;
mod ancient;
mod aroma_of_chaos;
mod brain_leech;
mod bugslayer;
mod byrdonis_nest;
pub(crate) mod colorful_philosophers;
pub(crate) mod colossal_flower;
mod crystal_sphere;
pub(crate) mod darv;
mod dense_vegetation;
pub(crate) mod doll_room;
mod doors_of_light_and_dark;
pub(crate) mod drowning_beacon;
pub(crate) mod endless_conveyor;
pub(crate) mod fake_merchant;
mod field_of_man_sized_holes;
mod grave_of_the_forgotten;
pub(crate) mod hungry_for_mushrooms;
mod infested_automaton;
mod jungle_maze_adventure;
mod lost_wisp;
mod luminous_choir;
mod morphic_grove;
pub(crate) mod nonupeipe;
pub(crate) mod orobas;
pub(crate) mod pael;
pub(crate) mod potion_courier;
mod punch_off;
mod ranwid_the_elder;
mod reflections;
pub(crate) mod relic_trader;
mod room_full_of_cheese;
mod round_tea_party;
mod sapphire_seed;
pub(crate) mod self_help_book;
mod slippery_bridge;
mod spiraling_whirlpool;
mod spirit_grafter;
mod stone_of_all_time;
mod sunken_statue;
mod sunken_treasury;
mod symbiote;
mod tablet_of_truth;
pub(crate) mod tanx;
mod tea_master;
pub(crate) mod tezcatara;
pub(crate) mod the_architect;
pub(crate) mod the_future_of_potions;
pub(crate) mod the_lantern_key;
mod the_legends_were_true;
mod this_or_that;
pub(crate) mod trash_heap;
pub(crate) mod trial;
mod unrest_site;
pub(crate) mod vakuu;
pub(crate) mod war_historian_repy;
mod waterlogged_scriptorium;
mod welcome_to_wongos;
mod wellspring;
mod whispering_hollow;
mod wood_carvings;
mod zen_weaver;

use nonmax::NonMaxU8;
use sts2_core::{
    CardTable as _, EventBehavior, EventOption, EventOptionKey, EventView, HookEffects, Room,
};

use crate::behaviour::EventOptionId;
use crate::behaviour::enchantments::can_enchant;
use crate::behaviour::relics::adds_pet;
use crate::behaviour::relics::tradable;
use crate::generated::cards::{CardRarity, CardTag};

#[derive(Debug, Clone)]
pub enum State<V: crate::Build> {
    /// An event whose rule is the build's own.
    Own(<V::OwnEvents as sts2_core::EventBehavior<V>>::State),
    InfestedAutomaton(infested_automaton::State),
    Symbiote(symbiote::State),
    StoneOfAllTime(stone_of_all_time::State),
    TheArchitect(the_architect::State),
    TheLanternKey(the_lantern_key::State),
    ZenWeaver(zen_weaver::State),
    RanwidTheElder(ranwid_the_elder::State<V>),
    RelicTrader(relic_trader::State<V>),
    DollRoom(doll_room::State<V>),
    FakeMerchant(fake_merchant::State<V>),
    SpiritGrafter(spirit_grafter::State),
    ColossalFlower(colossal_flower::State),
    CrystalSphere(crystal_sphere::State),
    Bugslayer(bugslayer::State),
    ColorfulPhilosophers(colorful_philosophers::State),
    PotionCourier(potion_courier::State),
    AromaOfChaos(aroma_of_chaos::State),
    BrainLeech(brain_leech::State),
    ByrdonisNest(byrdonis_nest::State),
    Darv(darv::State<V>),
    DenseVegetation(dense_vegetation::State),
    PunchOff(punch_off::State),
    FieldOfManSizedHoles(field_of_man_sized_holes::State),
    JungleMazeAdventure(jungle_maze_adventure::State),
    LostWisp(lost_wisp::State),
    LuminousChoir(luminous_choir::State),
    MorphicGrove(morphic_grove::State),
    Orobas(orobas::State<V>),
    Nonupeipe(ancient::Offered<V>),
    Pael(ancient::Offered<V>),
    RoomFullOfCheese(room_full_of_cheese::State),
    GraveOfTheForgotten(grave_of_the_forgotten::State),
    HungryForMushrooms(hungry_for_mushrooms::State),
    Reflections(reflections::State),
    RoundTeaParty(round_tea_party::State),
    Trial(trial::State),
    WarHistorianRepy(war_historian_repy::State),
    SapphireSeed(sapphire_seed::State),
    SelfHelpBook(self_help_book::State),
    SlipperyBridge(slippery_bridge::State),
    SunkenStatue(sunken_statue::State),
    TrashHeap(trash_heap::State),
    EndlessConveyor(endless_conveyor::State),
    DrowningBeacon(drowning_beacon::State),
    SpiralingWhirlpool(spiraling_whirlpool::State),
    DoorsOfLightAndDark(doors_of_light_and_dark::State),
    WaterloggedScriptorium(waterlogged_scriptorium::State),
    AbyssalBaths(abyssal_baths::State),
    SunkenTreasury(sunken_treasury::State),
    UnrestSite(unrest_site::State),
    TabletOfTruth(tablet_of_truth::State),
    TeaMaster(tea_master::State),
    Tanx(ancient::Offered<V>),
    Tezcatara(ancient::Offered<V>),
    Vakuu(ancient::Offered<V>),
    TheFutureOfPotions(the_future_of_potions::State<V>),
    TheLegendsWereTrue(the_legends_were_true::State),
    ThisOrThat(this_or_that::State),
    WelcomeToWongos(welcome_to_wongos::State<V>),
    Wellspring(wellspring::State),
    WhisperingHollow(whispering_hollow::State),
    WoodCarvings(wood_carvings::State),
}

/// A page of the options its keys name, each with whether it is locked, and
/// the one taken marked.
///
/// An option that asks a question leaves the page up, with its choice
/// marked, until the question is answered.
pub(crate) fn page<V: crate::Build>(
    event: V::Event,
    options: &[(V::EventOption, bool)],
    chosen: Option<NonMaxU8>,
) -> Room<V> {
    Room::Event {
        event,
        finished: false,
        title: None,
        text: None,
        options: options
            .iter()
            .enumerate()
            .map(|(index, &(option, locked))| {
                let index = u8::try_from(index).expect("a page offers few options");
                EventOption {
                    index,
                    key: EventOptionKey {
                        event,
                        option: EventOptionId::Said(option),
                    },
                    title: None,
                    text: None,
                    locked,
                    proceed: false,
                    chosen: chosen == NonMaxU8::new(index),
                    relic: None,
                }
            })
            .collect(),
    }
}

/// `EventModel.IsAllowed`, which is true unless an event overrides it.
///
/// A run has one player, so "every player" is the run's own player.
pub(crate) fn allowed<V: crate::Build>(event: V::Event, run: &impl EventView<V>) -> bool {
    // An event whose rule is the build's own.
    if let Some(allowed) = V::own_event_allowed(event, run) {
        return allowed;
    }
    let act = run.act();
    let gold = run.gold();
    let removable =
        |card: &sts2_core::CardIdent<V>| !crate::behaviour::enchantments::card_eternal(card);
    let basic = |card: &sts2_core::CardIdent<V>| card.card.rarity() == CardRarity::Basic;
    match event {
        __c0 if __c0 == V::Event::SELF_HELP_BOOK => self_help_book::SelfHelpBook.allowed(run),
        __c0 if __c0 == V::Event::JUNGLE_MAZE_ADVENTURE => {
            jungle_maze_adventure::JungleMazeAdventure.allowed(run)
        }
        __c0 if __c0 == V::Event::TABLET_OF_TRUTH => tablet_of_truth::TabletOfTruth.allowed(run),
        __c0 if __c0 == V::Event::DARV => darv::Darv.allowed(run),
        __c0 if __c0 == V::Event::DENSE_VEGETATION => {
            dense_vegetation::DenseVegetation.allowed(run)
        }
        __c0 if __c0 == V::Event::BRAIN_LEECH => brain_leech::BrainLeech.allowed(run),
        __c0 if __c0 == V::Event::SAPPHIRE_SEED => sapphire_seed::SapphireSeed.allowed(run),
        __c0 if __c0 == V::Event::WOOD_CARVINGS => wood_carvings::WoodCarvings.allowed(run),
        __c0 if __c0 == V::Event::AROMA_OF_CHAOS => aroma_of_chaos::AromaOfChaos.allowed(run),
        __c0 if __c0 == V::Event::BYRDONIS_NEST => byrdonis_nest::ByrdonisNest.allowed(run),
        __c0 if __c0 == V::Event::MORPHIC_GROVE => morphic_grove::MorphicGrove.allowed(run),
        __c0 if __c0 == V::Event::OROBAS => orobas::Orobas.allowed(run),
        __c0 if __c0 == V::Event::NONUPEIPE => nonupeipe::Nonupeipe.allowed(run),
        __c0 if __c0 == V::Event::PAEL => pael::Pael.allowed(run),
        __c0 if __c0 == V::Event::ROOM_FULL_OF_CHEESE => {
            room_full_of_cheese::RoomFullOfCheese.allowed(run)
        }
        __c0 if __c0 == V::Event::GRAVE_OF_THE_FORGOTTEN => {
            grave_of_the_forgotten::GraveOfTheForgotten.allowed(run)
        }
        __c0 if __c0 == V::Event::HUNGRY_FOR_MUSHROOMS => {
            hungry_for_mushrooms::HungryForMushrooms.allowed(run)
        }
        __c0 if __c0 == V::Event::REFLECTIONS => reflections::Reflections.allowed(run),
        __c0 if __c0 == V::Event::ROUND_TEA_PARTY => round_tea_party::RoundTeaParty.allowed(run),
        __c0 if __c0 == V::Event::TRIAL => trial::Trial.allowed(run),
        __c0 if __c0 == V::Event::WAR_HISTORIAN_REPY => {
            war_historian_repy::WarHistorianRepy.allowed(run)
        }
        __c0 if __c0 == V::Event::SUNKEN_STATUE => sunken_statue::SunkenStatue.allowed(run),
        __c0 if __c0 == V::Event::TEA_MASTER => tea_master::TeaMaster.allowed(run),
        __c0 if __c0 == V::Event::TANX => tanx::Tanx.allowed(run),
        __c0 if __c0 == V::Event::TEZCATARA => tezcatara::Tezcatara.allowed(run),
        __c0 if __c0 == V::Event::VAKUU => vakuu::Vakuu.allowed(run),
        __c0 if __c0 == V::Event::THE_LEGENDS_WERE_TRUE => {
            the_legends_were_true::TheLegendsWereTrue.allowed(run)
        }
        __c0 if __c0 == V::Event::THIS_OR_THAT => this_or_that::ThisOrThat.allowed(run),
        __c0 if __c0 == V::Event::WELLSPRING => wellspring::Wellspring.allowed(run),
        __c0 if __c0 == V::Event::WHISPERING_HOLLOW => {
            whispering_hollow::WhisperingHollow.allowed(run)
        }

        __c0 if (__c0 == V::Event::DOLL_ROOM || __c0 == V::Event::STONE_OF_ALL_TIME)
            && (act != 1) =>
        {
            false
        }
        __c0 if __c0 == V::Event::DOLL_ROOM => true,
        __c0 if __c0 == V::Event::STONE_OF_ALL_TIME => run.potions().next().is_some(),
        __c0 if __c0 == V::Event::POTION_COURIER || __c0 == V::Event::SYMBIOTE => act > 0,
        __c0 if __c0 == V::Event::WELCOME_TO_WONGOS => act == 1 && gold >= 100,
        __c0 if __c0 == V::Event::CRYSTAL_SPHERE => gold >= 100 && act > 0,

        __c0 if __c0 == V::Event::ENDLESS_CONVEYOR => gold >= 120,
        __c0 if __c0 == V::Event::WATERLOGGED_SCRIPTORIUM => gold >= 55,
        __c0 if __c0 == V::Event::ZEN_WEAVER => gold >= 125,
        // `GoldVar(149)`, before `CalculateVars` takes up to 50 off it.
        __c0 if __c0 == V::Event::LUMINOUS_CHOIR => gold >= 149 && run.relics_left(),

        __c0 if __c0 == V::Event::COLOSSAL_FLOWER => run.hp() >= 19,
        __c0 if __c0 == V::Event::TRASH_HEAP => run.hp() > 5,
        // `CurrentHp <= MaxHp * 0.70m`, in whole numbers.
        __c0 if __c0 == V::Event::UNREST_SITE => run.hp() * 100 <= run.max_hp() * 70,

        __c0 if __c0 == V::Event::PUNCH_OFF => punch_off::PunchOff.allowed(run),
        __c0 if __c0 == V::Event::SLIPPERY_BRIDGE => {
            run.floor() > 6 && run.deck().iter().any(removable)
        }
        __c0 if __c0 == V::Event::THE_FUTURE_OF_POTIONS => run.potions().count() >= 2,
        // A run has every character's card pool unlocked.
        __c0 if __c0 == V::Event::COLORFUL_PHILOSOPHERS => true,
        __c0 if __c0 == V::Event::FAKE_MERCHANT => {
            act >= 1
                && (gold >= 100 || run.potions().any(|potion| potion == V::Potion::FOUL_POTION))
        }

        __c0 if __c0 == V::Event::AMALGAMATOR => {
            [CardTag::Strike, CardTag::Defend].into_iter().all(|tag| {
                let valid = |card: &&sts2_core::CardIdent<V>| {
                    card.card.tagged(tag) && basic(card) && removable(card)
                };
                run.deck().iter().filter(valid).count() >= 2
            })
        }

        __c0 if __c0 == V::Event::FIELD_OF_MAN_SIZED_HOLES => {
            enchantable::<V>(run, V::Enchantment::PERFECT_FIT)
        }
        __c0 if __c0 == V::Event::SPIRALING_WHIRLPOOL => {
            enchantable::<V>(run, V::Enchantment::SPIRAL)
        }

        __c0 if (__c0 == V::Event::RELIC_TRADER || __c0 == V::Event::RANWID_THE_ELDER)
            && (act == 0) =>
        {
            false
        }
        __c0 if __c0 == V::Event::RELIC_TRADER => {
            run.relics().filter(|&held| tradable(held)).count() >= 5
        }
        __c0 if __c0 == V::Event::RANWID_THE_ELDER => {
            run.relics().any(tradable) && gold >= 100 && run.potions().next().is_some()
        }

        _ => true,
    }
}

/// `Player.HasEventPet`: a relic that adds a pet, or a Byrdonis Egg in the
/// deck.
fn has_event_pet<V: crate::Build>(run: &impl EventView<V>) -> bool {
    run.relics().any(|held| adds_pet::<V>(held.relic))
        || run
            .deck()
            .iter()
            .any(|card| card.card == V::Card::BYRDONIS_EGG)
}

/// Whether any card in the deck takes `enchantment`.
fn enchantable<V: crate::Build>(run: &impl EventView<V>, enchantment: V::Enchantment) -> bool {
    run.deck().iter().any(|card| can_enchant(enchantment, card))
}

pub(crate) fn enter<V: crate::Build, R: sts2_core::rng::GameRng>(
    event: V::Event,
    run: &impl EventView<V>,
    rng: &mut R,
) -> State<V> {
    // An event whose rule is the build's own.
    if let Some(state) = V::own_event_enter(event, run, rng) {
        return State::Own(state);
    }
    match event {
        __c0 if __c0 == V::Event::POTION_COURIER => {
            State::PotionCourier(potion_courier::PotionCourier.enter(run, rng))
        }
        __c0 if __c0 == V::Event::COLORFUL_PHILOSOPHERS => {
            State::ColorfulPhilosophers(colorful_philosophers::ColorfulPhilosophers.enter(run, rng))
        }
        __c0 if __c0 == V::Event::INFESTED_AUTOMATON => {
            State::InfestedAutomaton(infested_automaton::InfestedAutomaton.enter(run, rng))
        }
        __c0 if __c0 == V::Event::SYMBIOTE => State::Symbiote(symbiote::Symbiote.enter(run, rng)),
        __c0 if __c0 == V::Event::STONE_OF_ALL_TIME => {
            State::StoneOfAllTime(stone_of_all_time::StoneOfAllTime.enter(run, rng))
        }
        __c0 if __c0 == V::Event::THE_ARCHITECT => {
            State::TheArchitect(the_architect::TheArchitect.enter(run, rng))
        }
        __c0 if __c0 == V::Event::THE_LANTERN_KEY => {
            State::TheLanternKey(the_lantern_key::TheLanternKey.enter(run, rng))
        }
        __c0 if __c0 == V::Event::ZEN_WEAVER => {
            State::ZenWeaver(zen_weaver::ZenWeaver.enter(run, rng))
        }
        __c0 if __c0 == V::Event::RANWID_THE_ELDER => {
            State::RanwidTheElder(ranwid_the_elder::RanwidTheElder.enter(run, rng))
        }
        __c0 if __c0 == V::Event::RELIC_TRADER => {
            State::RelicTrader(relic_trader::RelicTrader.enter(run, rng))
        }
        __c0 if __c0 == V::Event::DOLL_ROOM => State::DollRoom(doll_room::DollRoom.enter(run, rng)),
        __c0 if __c0 == V::Event::SPIRIT_GRAFTER => {
            State::SpiritGrafter(spirit_grafter::SpiritGrafter.enter(run, rng))
        }
        __c0 if __c0 == V::Event::COLOSSAL_FLOWER => {
            State::ColossalFlower(colossal_flower::ColossalFlower.enter(run, rng))
        }
        __c0 if __c0 == V::Event::BUGSLAYER => {
            State::Bugslayer(bugslayer::Bugslayer.enter(run, rng))
        }
        __c0 if __c0 == V::Event::AROMA_OF_CHAOS => {
            State::AromaOfChaos(aroma_of_chaos::AromaOfChaos.enter(run, rng))
        }
        __c0 if __c0 == V::Event::FIELD_OF_MAN_SIZED_HOLES => State::FieldOfManSizedHoles(
            field_of_man_sized_holes::FieldOfManSizedHoles.enter(run, rng),
        ),
        __c0 if __c0 == V::Event::BRAIN_LEECH => {
            State::BrainLeech(brain_leech::BrainLeech.enter(run, rng))
        }
        __c0 if __c0 == V::Event::BYRDONIS_NEST => {
            State::ByrdonisNest(byrdonis_nest::ByrdonisNest.enter(run, rng))
        }
        __c0 if __c0 == V::Event::DARV => State::Darv(darv::Darv.enter(run, rng)),
        __c0 if __c0 == V::Event::PUNCH_OFF => State::PunchOff(punch_off::PunchOff.enter(run, rng)),
        __c0 if __c0 == V::Event::DENSE_VEGETATION => {
            State::DenseVegetation(dense_vegetation::DenseVegetation.enter(run, rng))
        }
        __c0 if __c0 == V::Event::JUNGLE_MAZE_ADVENTURE => {
            State::JungleMazeAdventure(jungle_maze_adventure::JungleMazeAdventure.enter(run, rng))
        }
        __c0 if __c0 == V::Event::LOST_WISP => State::LostWisp(lost_wisp::LostWisp.enter(run, rng)),
        __c0 if __c0 == V::Event::LUMINOUS_CHOIR => {
            State::LuminousChoir(luminous_choir::LuminousChoir.enter(run, rng))
        }
        __c0 if __c0 == V::Event::MORPHIC_GROVE => {
            State::MorphicGrove(morphic_grove::MorphicGrove.enter(run, rng))
        }
        __c0 if __c0 == V::Event::OROBAS => State::Orobas(orobas::Orobas.enter(run, rng)),
        __c0 if __c0 == V::Event::NONUPEIPE => {
            State::Nonupeipe(nonupeipe::Nonupeipe.enter(run, rng))
        }
        __c0 if __c0 == V::Event::PAEL => State::Pael(pael::Pael.enter(run, rng)),
        __c0 if __c0 == V::Event::ROOM_FULL_OF_CHEESE => {
            State::RoomFullOfCheese(room_full_of_cheese::RoomFullOfCheese.enter(run, rng))
        }
        __c0 if __c0 == V::Event::GRAVE_OF_THE_FORGOTTEN => {
            State::GraveOfTheForgotten(grave_of_the_forgotten::GraveOfTheForgotten.enter(run, rng))
        }
        __c0 if __c0 == V::Event::HUNGRY_FOR_MUSHROOMS => {
            State::HungryForMushrooms(hungry_for_mushrooms::HungryForMushrooms.enter(run, rng))
        }
        __c0 if __c0 == V::Event::REFLECTIONS => {
            State::Reflections(reflections::Reflections.enter(run, rng))
        }
        __c0 if __c0 == V::Event::ROUND_TEA_PARTY => {
            State::RoundTeaParty(round_tea_party::RoundTeaParty.enter(run, rng))
        }
        __c0 if __c0 == V::Event::TRIAL => State::Trial(trial::Trial.enter(run, rng)),
        __c0 if __c0 == V::Event::WAR_HISTORIAN_REPY => {
            State::WarHistorianRepy(war_historian_repy::WarHistorianRepy.enter(run, rng))
        }
        __c0 if __c0 == V::Event::SAPPHIRE_SEED => {
            State::SapphireSeed(sapphire_seed::SapphireSeed.enter(run, rng))
        }
        __c0 if __c0 == V::Event::SELF_HELP_BOOK => {
            State::SelfHelpBook(self_help_book::SelfHelpBook.enter(run, rng))
        }
        __c0 if __c0 == V::Event::SLIPPERY_BRIDGE => {
            State::SlipperyBridge(slippery_bridge::SlipperyBridge.enter(run, rng))
        }
        __c0 if __c0 == V::Event::SUNKEN_STATUE => {
            State::SunkenStatue(sunken_statue::SunkenStatue.enter(run, rng))
        }
        __c0 if __c0 == V::Event::TRASH_HEAP => {
            State::TrashHeap(trash_heap::TrashHeap.enter(run, rng))
        }
        __c0 if __c0 == V::Event::ENDLESS_CONVEYOR => {
            State::EndlessConveyor(endless_conveyor::EndlessConveyor.enter(run, rng))
        }
        __c0 if __c0 == V::Event::DROWNING_BEACON => {
            State::DrowningBeacon(drowning_beacon::DrowningBeacon.enter(run, rng))
        }
        __c0 if __c0 == V::Event::SPIRALING_WHIRLPOOL => {
            State::SpiralingWhirlpool(spiraling_whirlpool::SpiralingWhirlpool.enter(run, rng))
        }
        __c0 if __c0 == V::Event::DOORS_OF_LIGHT_AND_DARK => {
            State::DoorsOfLightAndDark(doors_of_light_and_dark::DoorsOfLightAndDark.enter(run, rng))
        }
        __c0 if __c0 == V::Event::WATERLOGGED_SCRIPTORIUM => State::WaterloggedScriptorium(
            waterlogged_scriptorium::WaterloggedScriptorium.enter(run, rng),
        ),
        __c0 if __c0 == V::Event::ABYSSAL_BATHS => {
            State::AbyssalBaths(abyssal_baths::AbyssalBaths.enter(run, rng))
        }
        __c0 if __c0 == V::Event::SUNKEN_TREASURY => {
            State::SunkenTreasury(sunken_treasury::SunkenTreasury.enter(run, rng))
        }
        __c0 if __c0 == V::Event::UNREST_SITE => {
            State::UnrestSite(unrest_site::UnrestSite.enter(run, rng))
        }
        __c0 if __c0 == V::Event::TABLET_OF_TRUTH => {
            State::TabletOfTruth(tablet_of_truth::TabletOfTruth.enter(run, rng))
        }
        __c0 if __c0 == V::Event::TEA_MASTER => {
            State::TeaMaster(tea_master::TeaMaster.enter(run, rng))
        }
        __c0 if __c0 == V::Event::TANX => State::Tanx(tanx::Tanx.enter(run, rng)),
        __c0 if __c0 == V::Event::TEZCATARA => {
            State::Tezcatara(tezcatara::Tezcatara.enter(run, rng))
        }
        __c0 if __c0 == V::Event::VAKUU => State::Vakuu(vakuu::Vakuu.enter(run, rng)),
        __c0 if __c0 == V::Event::THE_FUTURE_OF_POTIONS => {
            State::TheFutureOfPotions(the_future_of_potions::TheFutureOfPotions.enter(run, rng))
        }
        __c0 if __c0 == V::Event::THE_LEGENDS_WERE_TRUE => {
            State::TheLegendsWereTrue(the_legends_were_true::TheLegendsWereTrue.enter(run, rng))
        }
        __c0 if __c0 == V::Event::THIS_OR_THAT => {
            State::ThisOrThat(this_or_that::ThisOrThat.enter(run, rng))
        }
        __c0 if __c0 == V::Event::WELLSPRING => {
            State::Wellspring(wellspring::Wellspring.enter(run, rng))
        }
        __c0 if __c0 == V::Event::FAKE_MERCHANT => {
            State::FakeMerchant(fake_merchant::FakeMerchant.enter(run, rng))
        }
        __c0 if __c0 == V::Event::WELCOME_TO_WONGOS => {
            State::WelcomeToWongos(welcome_to_wongos::WelcomeToWongos.enter(run, rng))
        }
        __c0 if __c0 == V::Event::CRYSTAL_SPHERE => {
            State::CrystalSphere(crystal_sphere::CrystalSphere.enter(run, rng))
        }
        __c0 if __c0 == V::Event::WHISPERING_HOLLOW => {
            State::WhisperingHollow(whispering_hollow::WhisperingHollow.enter(run, rng))
        }
        __c0 if __c0 == V::Event::WOOD_CARVINGS => {
            State::WoodCarvings(wood_carvings::WoodCarvings.enter(run, rng))
        }
        _ => unimplemented!("entering event {event:?}"),
    }
}

/// `SetupForPlayer` on what the options offer, off the player's rewards
/// stream. Darv's Dusty Tome is the one option that sets itself up.
pub(crate) fn set_up<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut State<V>,
    run: &impl EventView<V>,
    rewards: &mut R,
) {
    if let State::Darv(state) = state {
        darv::Darv.set_up(state, run, rewards);
    }
}

/// [`EventBehavior::relics_pulled`] for the event entered.
pub(crate) fn relics_pulled<V: crate::Build>(
    state: &State<V>,
) -> &'static [sts2_core::RelicPull<V>] {
    match state {
        State::RelicTrader(state) => relic_trader::RelicTrader.relics_pulled(state),
        State::WelcomeToWongos(state) => welcome_to_wongos::WelcomeToWongos.relics_pulled(state),
        _ => &[],
    }
}

/// [`EventBehavior::hand_pulled_relics`] for the event entered.
pub(crate) fn hand_pulled_relics<V: crate::Build>(state: &mut State<V>, relics: &[V::Relic]) {
    match state {
        State::RelicTrader(state) => relic_trader::RelicTrader.hand_pulled_relics(state, relics),
        State::WelcomeToWongos(state) => {
            welcome_to_wongos::WelcomeToWongos.hand_pulled_relics(state, relics)
        }
        state => unreachable!("{state:?} pulls no relics"),
    }
}

/// `BeforeEventStarted` setting `CanRemovePotions` off, as The Future of
/// Potions does.
pub(crate) fn holds_potions<V: crate::Build>(state: &State<V>) -> bool {
    matches!(
        state,
        State::TheFutureOfPotions(_) | State::RanwidTheElder(_) | State::StoneOfAllTime(_)
    )
}

pub(crate) fn observe<V: crate::Build>(state: &State<V>, run: &impl EventView<V>) -> Room<V> {
    match state {
        State::Own(state) => V::OwnEvents::default().observe(state, run),
        State::PotionCourier(state) => potion_courier::PotionCourier.observe(state, run),
        State::ColorfulPhilosophers(state) => {
            colorful_philosophers::ColorfulPhilosophers.observe(state, run)
        }
        State::InfestedAutomaton(state) => {
            infested_automaton::InfestedAutomaton.observe(state, run)
        }
        State::Symbiote(state) => symbiote::Symbiote.observe(state, run),
        State::StoneOfAllTime(state) => stone_of_all_time::StoneOfAllTime.observe(state, run),
        State::TheArchitect(state) => the_architect::TheArchitect.observe(state, run),
        State::TheLanternKey(state) => the_lantern_key::TheLanternKey.observe(state, run),
        State::ZenWeaver(state) => zen_weaver::ZenWeaver.observe(state, run),
        State::RanwidTheElder(state) => ranwid_the_elder::RanwidTheElder.observe(state, run),
        State::RelicTrader(state) => relic_trader::RelicTrader.observe(state, run),
        State::DollRoom(state) => doll_room::DollRoom.observe(state, run),
        State::FakeMerchant(state) => fake_merchant::FakeMerchant.observe(state, run),
        State::SpiritGrafter(state) => spirit_grafter::SpiritGrafter.observe(state, run),
        State::ColossalFlower(state) => colossal_flower::ColossalFlower.observe(state, run),
        State::CrystalSphere(state) => crystal_sphere::CrystalSphere.observe(state, run),
        State::Bugslayer(state) => bugslayer::Bugslayer.observe(state, run),
        State::AromaOfChaos(state) => aroma_of_chaos::AromaOfChaos.observe(state, run),
        State::FieldOfManSizedHoles(state) => {
            field_of_man_sized_holes::FieldOfManSizedHoles.observe(state, run)
        }
        State::BrainLeech(state) => brain_leech::BrainLeech.observe(state, run),
        State::ByrdonisNest(state) => byrdonis_nest::ByrdonisNest.observe(state, run),
        State::Darv(state) => darv::Darv.observe(state, run),
        State::DenseVegetation(state) => dense_vegetation::DenseVegetation.observe(state, run),
        State::PunchOff(state) => punch_off::PunchOff.observe(state, run),
        State::JungleMazeAdventure(state) => {
            jungle_maze_adventure::JungleMazeAdventure.observe(state, run)
        }
        State::LostWisp(state) => lost_wisp::LostWisp.observe(state, run),
        State::LuminousChoir(state) => luminous_choir::LuminousChoir.observe(state, run),
        State::MorphicGrove(state) => morphic_grove::MorphicGrove.observe(state, run),
        State::Orobas(state) => orobas::Orobas.observe(state, run),
        State::Nonupeipe(state) => nonupeipe::Nonupeipe.observe(state, run),
        State::Pael(state) => pael::Pael.observe(state, run),
        State::RoomFullOfCheese(state) => room_full_of_cheese::RoomFullOfCheese.observe(state, run),
        State::GraveOfTheForgotten(state) => {
            grave_of_the_forgotten::GraveOfTheForgotten.observe(state, run)
        }
        State::HungryForMushrooms(state) => {
            hungry_for_mushrooms::HungryForMushrooms.observe(state, run)
        }
        State::Reflections(state) => reflections::Reflections.observe(state, run),
        State::RoundTeaParty(state) => round_tea_party::RoundTeaParty.observe(state, run),
        State::Trial(state) => trial::Trial.observe(state, run),
        State::WarHistorianRepy(state) => war_historian_repy::WarHistorianRepy.observe(state, run),
        State::SapphireSeed(state) => sapphire_seed::SapphireSeed.observe(state, run),
        State::SelfHelpBook(state) => self_help_book::SelfHelpBook.observe(state, run),
        State::SlipperyBridge(state) => slippery_bridge::SlipperyBridge.observe(state, run),
        State::SunkenStatue(state) => sunken_statue::SunkenStatue.observe(state, run),
        State::TrashHeap(state) => trash_heap::TrashHeap.observe(state, run),
        State::EndlessConveyor(state) => endless_conveyor::EndlessConveyor.observe(state, run),
        State::DrowningBeacon(state) => drowning_beacon::DrowningBeacon.observe(state, run),
        State::SpiralingWhirlpool(state) => {
            spiraling_whirlpool::SpiralingWhirlpool.observe(state, run)
        }
        State::DoorsOfLightAndDark(state) => {
            doors_of_light_and_dark::DoorsOfLightAndDark.observe(state, run)
        }
        State::WaterloggedScriptorium(state) => {
            waterlogged_scriptorium::WaterloggedScriptorium.observe(state, run)
        }
        State::AbyssalBaths(state) => abyssal_baths::AbyssalBaths.observe(state, run),
        State::SunkenTreasury(state) => sunken_treasury::SunkenTreasury.observe(state, run),
        State::UnrestSite(state) => unrest_site::UnrestSite.observe(state, run),
        State::TabletOfTruth(state) => tablet_of_truth::TabletOfTruth.observe(state, run),
        State::TeaMaster(state) => tea_master::TeaMaster.observe(state, run),
        State::Tanx(state) => tanx::Tanx.observe(state, run),
        State::Tezcatara(state) => tezcatara::Tezcatara.observe(state, run),
        State::Vakuu(state) => vakuu::Vakuu.observe(state, run),
        State::TheFutureOfPotions(state) => {
            the_future_of_potions::TheFutureOfPotions.observe(state, run)
        }
        State::TheLegendsWereTrue(state) => {
            the_legends_were_true::TheLegendsWereTrue.observe(state, run)
        }
        State::ThisOrThat(state) => this_or_that::ThisOrThat.observe(state, run),
        State::Wellspring(state) => wellspring::Wellspring.observe(state, run),
        State::WelcomeToWongos(state) => welcome_to_wongos::WelcomeToWongos.observe(state, run),
        State::WhisperingHollow(state) => whispering_hollow::WhisperingHollow.observe(state, run),
        State::WoodCarvings(state) => wood_carvings::WoodCarvings.observe(state, run),
    }
}

pub(crate) fn choose<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut State<V>,
    option: u8,
    run: &impl EventView<V>,
    rng: &mut R,
) -> HookEffects<V> {
    match state {
        State::Own(state) => V::OwnEvents::default().choose(state, option, run, rng),
        State::PotionCourier(state) => {
            potion_courier::PotionCourier.choose(state, option, run, rng)
        }
        State::ColorfulPhilosophers(state) => {
            colorful_philosophers::ColorfulPhilosophers.choose(state, option, run, rng)
        }
        State::InfestedAutomaton(state) => {
            infested_automaton::InfestedAutomaton.choose(state, option, run, rng)
        }
        State::Symbiote(state) => symbiote::Symbiote.choose(state, option, run, rng),
        State::StoneOfAllTime(state) => {
            stone_of_all_time::StoneOfAllTime.choose(state, option, run, rng)
        }
        State::TheArchitect(state) => the_architect::TheArchitect.choose(state, option, run, rng),
        State::TheLanternKey(state) => {
            the_lantern_key::TheLanternKey.choose(state, option, run, rng)
        }
        State::ZenWeaver(state) => zen_weaver::ZenWeaver.choose(state, option, run, rng),
        State::RanwidTheElder(state) => {
            ranwid_the_elder::RanwidTheElder.choose(state, option, run, rng)
        }
        State::RelicTrader(state) => relic_trader::RelicTrader.choose(state, option, run, rng),
        State::DollRoom(state) => doll_room::DollRoom.choose(state, option, run, rng),
        State::FakeMerchant(state) => fake_merchant::FakeMerchant.choose(state, option, run, rng),
        State::SpiritGrafter(state) => {
            spirit_grafter::SpiritGrafter.choose(state, option, run, rng)
        }
        State::ColossalFlower(state) => {
            colossal_flower::ColossalFlower.choose(state, option, run, rng)
        }
        State::CrystalSphere(state) => {
            crystal_sphere::CrystalSphere.choose(state, option, run, rng)
        }
        State::Bugslayer(state) => bugslayer::Bugslayer.choose(state, option, run, rng),
        State::AromaOfChaos(state) => aroma_of_chaos::AromaOfChaos.choose(state, option, run, rng),
        State::FieldOfManSizedHoles(state) => {
            field_of_man_sized_holes::FieldOfManSizedHoles.choose(state, option, run, rng)
        }
        State::BrainLeech(state) => brain_leech::BrainLeech.choose(state, option, run, rng),
        State::ByrdonisNest(state) => byrdonis_nest::ByrdonisNest.choose(state, option, run, rng),
        State::Darv(state) => darv::Darv.choose(state, option, run, rng),
        State::PunchOff(state) => punch_off::PunchOff.choose(state, option, run, rng),
        State::DenseVegetation(state) => {
            dense_vegetation::DenseVegetation.choose(state, option, run, rng)
        }
        State::JungleMazeAdventure(state) => {
            jungle_maze_adventure::JungleMazeAdventure.choose(state, option, run, rng)
        }
        State::LostWisp(state) => lost_wisp::LostWisp.choose(state, option, run, rng),
        State::LuminousChoir(state) => {
            luminous_choir::LuminousChoir.choose(state, option, run, rng)
        }
        State::MorphicGrove(state) => morphic_grove::MorphicGrove.choose(state, option, run, rng),
        State::Orobas(state) => orobas::Orobas.choose(state, option, run, rng),
        State::Nonupeipe(state) => nonupeipe::Nonupeipe.choose(state, option, run, rng),
        State::Pael(state) => pael::Pael.choose(state, option, run, rng),
        State::RoomFullOfCheese(state) => {
            room_full_of_cheese::RoomFullOfCheese.choose(state, option, run, rng)
        }
        State::GraveOfTheForgotten(state) => {
            grave_of_the_forgotten::GraveOfTheForgotten.choose(state, option, run, rng)
        }
        State::HungryForMushrooms(state) => {
            hungry_for_mushrooms::HungryForMushrooms.choose(state, option, run, rng)
        }
        State::Reflections(state) => reflections::Reflections.choose(state, option, run, rng),
        State::RoundTeaParty(state) => {
            round_tea_party::RoundTeaParty.choose(state, option, run, rng)
        }
        State::Trial(state) => trial::Trial.choose(state, option, run, rng),
        State::WarHistorianRepy(state) => {
            war_historian_repy::WarHistorianRepy.choose(state, option, run, rng)
        }
        State::SapphireSeed(state) => sapphire_seed::SapphireSeed.choose(state, option, run, rng),
        State::SelfHelpBook(state) => self_help_book::SelfHelpBook.choose(state, option, run, rng),
        State::SlipperyBridge(state) => {
            slippery_bridge::SlipperyBridge.choose(state, option, run, rng)
        }
        State::SunkenStatue(state) => sunken_statue::SunkenStatue.choose(state, option, run, rng),
        State::TrashHeap(state) => trash_heap::TrashHeap.choose(state, option, run, rng),
        State::EndlessConveyor(state) => {
            endless_conveyor::EndlessConveyor.choose(state, option, run, rng)
        }
        State::DrowningBeacon(state) => {
            drowning_beacon::DrowningBeacon.choose(state, option, run, rng)
        }
        State::SpiralingWhirlpool(state) => {
            spiraling_whirlpool::SpiralingWhirlpool.choose(state, option, run, rng)
        }
        State::DoorsOfLightAndDark(state) => {
            doors_of_light_and_dark::DoorsOfLightAndDark.choose(state, option, run, rng)
        }
        State::WaterloggedScriptorium(state) => {
            waterlogged_scriptorium::WaterloggedScriptorium.choose(state, option, run, rng)
        }
        State::AbyssalBaths(state) => abyssal_baths::AbyssalBaths.choose(state, option, run, rng),
        State::SunkenTreasury(state) => {
            sunken_treasury::SunkenTreasury.choose(state, option, run, rng)
        }
        State::UnrestSite(state) => unrest_site::UnrestSite.choose(state, option, run, rng),
        State::TabletOfTruth(state) => {
            tablet_of_truth::TabletOfTruth.choose(state, option, run, rng)
        }
        State::TheFutureOfPotions(state) => {
            the_future_of_potions::TheFutureOfPotions.choose(state, option, run, rng)
        }
        State::TeaMaster(state) => tea_master::TeaMaster.choose(state, option, run, rng),
        State::Tanx(state) => tanx::Tanx.choose(state, option, run, rng),
        State::Tezcatara(state) => tezcatara::Tezcatara.choose(state, option, run, rng),
        State::Vakuu(state) => vakuu::Vakuu.choose(state, option, run, rng),
        State::TheLegendsWereTrue(state) => {
            the_legends_were_true::TheLegendsWereTrue.choose(state, option, run, rng)
        }
        State::ThisOrThat(state) => this_or_that::ThisOrThat.choose(state, option, run, rng),
        State::Wellspring(state) => wellspring::Wellspring.choose(state, option, run, rng),
        State::WelcomeToWongos(state) => {
            welcome_to_wongos::WelcomeToWongos.choose(state, option, run, rng)
        }
        State::WhisperingHollow(state) => {
            whispering_hollow::WhisperingHollow.choose(state, option, run, rng)
        }
        State::WoodCarvings(state) => wood_carvings::WoodCarvings.choose(state, option, run, rng),
    }
}

/// [`EventBehavior::question`] for the event entered.
pub(crate) fn question<V: crate::Build>(state: &State<V>) -> Option<sts2_core::Question<V>> {
    match state {
        State::CrystalSphere(state) => crystal_sphere::CrystalSphere.question(state),
        _ => None,
    }
}

/// [`EventBehavior::answer`] for the event entered.
pub(crate) fn answer<V: crate::Build>(
    state: &mut State<V>,
    answer: &sts2_core::Answer,
) -> HookEffects<V> {
    match state {
        State::CrystalSphere(state) => crystal_sphere::CrystalSphere.answer(state, answer),
        state => unimplemented!("{state:?} answered with {answer:?}"),
    }
}

/// [`EventBehavior::resume`] for the event entered.
pub(crate) fn resume<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut State<V>,
    run: &impl EventView<V>,
    rng: &mut R,
) -> HookEffects<V> {
    match state {
        State::WarHistorianRepy(state) => {
            war_historian_repy::WarHistorianRepy.resume(state, run, rng)
        }
        State::EndlessConveyor(state) => endless_conveyor::EndlessConveyor.resume(state, run, rng),
        State::DenseVegetation(state) => dense_vegetation::DenseVegetation.resume(state, run, rng),
        state => unimplemented!("{state:?} resumed after its pickup"),
    }
}

/// [`EventBehavior::resume_from_fight`] for the event entered.
pub(crate) fn resume_from_fight<V: crate::Build>(
    state: &mut State<V>,
    escaped: bool,
) -> HookEffects<V> {
    match state {
        State::Own(state) => V::OwnEvents::default().resume_from_fight(state, escaped),
        state => unimplemented!("{state:?} resumed after its fight"),
    }
}

/// [`EventBehavior::rewards`] for the event entered.
pub(crate) fn rewards<V: crate::Build>(state: &State<V>) -> Vec<sts2_core::CustomReward<V>> {
    match state {
        State::CrystalSphere(state) => crystal_sphere::CrystalSphere.rewards(state),
        State::FakeMerchant(state) => fake_merchant::FakeMerchant.rewards(state),
        state => unimplemented!("rewards of {state:?}'s own"),
    }
}

/// [`EventBehavior::merchant`] for the event entered.
pub(crate) fn merchant<V: crate::Build>(state: &State<V>) -> bool {
    match state {
        State::FakeMerchant(state) => fake_merchant::FakeMerchant.merchant(state),
        _ => false,
    }
}

/// [`EventBehavior::potion_thrown`] for the event entered.
pub(crate) fn potion_thrown<V: crate::Build>(
    state: &mut State<V>,
    potion: V::Potion,
) -> HookEffects<V> {
    match state {
        State::FakeMerchant(state) => fake_merchant::FakeMerchant.potion_thrown(state, potion),
        state => unimplemented!("{potion:?} thrown at {state:?}"),
    }
}

/// [`EventBehavior::stock`] for the event entered.
pub(crate) fn stock<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut State<V>,
    shops: &mut R,
) {
    if let State::FakeMerchant(state) = state {
        fake_merchant::FakeMerchant.stock(state, shops);
    }
}

/// [`EventBehavior::buy`] for the event entered.
pub(crate) fn buy<V: crate::Build>(state: &mut State<V>, index: u8) -> HookEffects<V> {
    match state {
        State::FakeMerchant(state) => fake_merchant::FakeMerchant.buy(state, index),
        state => unimplemented!("buying entry {index} from {state:?}"),
    }
}
