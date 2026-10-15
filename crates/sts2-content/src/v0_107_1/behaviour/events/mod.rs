//! Event behavior dispatch for this build.

mod abyssal_baths;
mod amalgamator;
mod ancient;
mod aroma_of_chaos;
mod battleworn_dummy;
mod brain_leech;
mod bugslayer;
mod byrdonis_nest;
mod colorful_philosophers;
mod colossal_flower;
mod crystal_sphere;
mod darv;
mod dense_vegetation;
mod doll_room;
mod doors_of_light_and_dark;
mod drowning_beacon;
mod endless_conveyor;
mod fake_merchant;
mod field_of_man_sized_holes;
mod grave_of_the_forgotten;
mod hungry_for_mushrooms;
mod infested_automaton;
mod jungle_maze_adventure;
mod lost_wisp;
mod luminous_choir;
mod morphic_grove;
mod nonupeipe;
mod orobas;
mod pael;
mod potion_courier;
mod punch_off;
mod ranwid_the_elder;
mod reflections;
mod relic_trader;
mod room_full_of_cheese;
mod round_tea_party;
mod sapphire_seed;
mod self_help_book;
mod slippery_bridge;
mod spiraling_whirlpool;
mod spirit_grafter;
mod stone_of_all_time;
mod sunken_statue;
mod sunken_treasury;
mod symbiote;
mod tablet_of_truth;
mod tanx;
mod tea_master;
mod tezcatara;
mod the_architect;
mod the_future_of_potions;
mod the_lantern_key;
mod the_legends_were_true;
mod this_or_that;
mod tinker_time;
mod trash_heap;
mod trial;
mod unrest_site;
mod vakuu;
mod war_historian_repy;
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

use super::super::EventOptionId;
use super::super::V0_107_1 as V;
use super::super::generated::cards::{CardRarity, CardTag, Cards, QuestCard};
use super::super::generated::enchantments::Enchantment;
use super::super::generated::events::{Event, EventOption as Said};
use super::super::generated::potions::Potion;
use super::enchantments::can_enchant;
use super::relics::adds_pet;
use super::relics::tradable;

#[derive(Debug, Clone)]
pub enum State {
    InfestedAutomaton(infested_automaton::State),
    Symbiote(symbiote::State),
    StoneOfAllTime(stone_of_all_time::State),
    TheArchitect(the_architect::State),
    TheLanternKey(the_lantern_key::State),
    ZenWeaver(zen_weaver::State),
    RanwidTheElder(ranwid_the_elder::State),
    RelicTrader(relic_trader::State),
    DollRoom(doll_room::State),
    FakeMerchant(fake_merchant::State),
    SpiritGrafter(spirit_grafter::State),
    ColossalFlower(colossal_flower::State),
    CrystalSphere(crystal_sphere::State),
    Bugslayer(bugslayer::State),
    ColorfulPhilosophers(colorful_philosophers::State),
    PotionCourier(potion_courier::State),
    Amalgamator(amalgamator::State),
    AromaOfChaos(aroma_of_chaos::State),
    BrainLeech(brain_leech::State),
    ByrdonisNest(byrdonis_nest::State),
    Darv(darv::State),
    DenseVegetation(dense_vegetation::State),
    PunchOff(punch_off::State),
    FieldOfManSizedHoles(field_of_man_sized_holes::State),
    JungleMazeAdventure(jungle_maze_adventure::State),
    LostWisp(lost_wisp::State),
    LuminousChoir(luminous_choir::State),
    MorphicGrove(morphic_grove::State),
    Orobas(orobas::State),
    Nonupeipe(ancient::Offered),
    Pael(ancient::Offered),
    RoomFullOfCheese(room_full_of_cheese::State),
    BattlewornDummy(battleworn_dummy::State),
    GraveOfTheForgotten(grave_of_the_forgotten::State),
    HungryForMushrooms(hungry_for_mushrooms::State),
    Reflections(reflections::State),
    RoundTeaParty(round_tea_party::State),
    TinkerTime(tinker_time::State),
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
    Tanx(ancient::Offered),
    Tezcatara(ancient::Offered),
    Vakuu(ancient::Offered),
    TheFutureOfPotions(the_future_of_potions::State),
    TheLegendsWereTrue(the_legends_were_true::State),
    ThisOrThat(this_or_that::State),
    WelcomeToWongos(welcome_to_wongos::State),
    Wellspring(wellspring::State),
    WhisperingHollow(whispering_hollow::State),
    WoodCarvings(wood_carvings::State),
}

/// A page of the options its keys name, each with whether it is locked, and
/// the one taken marked.
///
/// An option that asks a question leaves the page up, with its choice
/// marked, until the question is answered.
pub(super) fn page(event: Event, options: &[(Said, bool)], chosen: Option<NonMaxU8>) -> Room<V> {
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
pub(crate) fn allowed(event: Event, run: &impl EventView<V>) -> bool {
    let act = run.act();
    let gold = run.gold();
    let removable = |card: &sts2_core::CardIdent<V>| !super::enchantments::card_eternal(card);
    let basic = |card: &sts2_core::CardIdent<V>| card.card.rarity() == CardRarity::Basic;
    match event {
        Event::SelfHelpBook => self_help_book::SelfHelpBook.allowed(run),
        Event::JungleMazeAdventure => jungle_maze_adventure::JungleMazeAdventure.allowed(run),
        Event::TabletOfTruth => tablet_of_truth::TabletOfTruth.allowed(run),
        Event::Darv => darv::Darv.allowed(run),
        Event::DenseVegetation => dense_vegetation::DenseVegetation.allowed(run),
        Event::BrainLeech => brain_leech::BrainLeech.allowed(run),
        Event::SapphireSeed => sapphire_seed::SapphireSeed.allowed(run),
        Event::WoodCarvings => wood_carvings::WoodCarvings.allowed(run),
        Event::AromaOfChaos => aroma_of_chaos::AromaOfChaos.allowed(run),
        Event::ByrdonisNest => byrdonis_nest::ByrdonisNest.allowed(run),
        Event::MorphicGrove => morphic_grove::MorphicGrove.allowed(run),
        Event::Orobas => orobas::Orobas.allowed(run),
        Event::Nonupeipe => nonupeipe::Nonupeipe.allowed(run),
        Event::Pael => pael::Pael.allowed(run),
        Event::RoomFullOfCheese => room_full_of_cheese::RoomFullOfCheese.allowed(run),
        Event::BattlewornDummy => battleworn_dummy::BattlewornDummy.allowed(run),
        Event::GraveOfTheForgotten => grave_of_the_forgotten::GraveOfTheForgotten.allowed(run),
        Event::HungryForMushrooms => hungry_for_mushrooms::HungryForMushrooms.allowed(run),
        Event::Reflections => reflections::Reflections.allowed(run),
        Event::RoundTeaParty => round_tea_party::RoundTeaParty.allowed(run),
        Event::TinkerTime => tinker_time::TinkerTime.allowed(run),
        Event::Trial => trial::Trial.allowed(run),
        Event::WarHistorianRepy => war_historian_repy::WarHistorianRepy.allowed(run),
        Event::SunkenStatue => sunken_statue::SunkenStatue.allowed(run),
        Event::TeaMaster => tea_master::TeaMaster.allowed(run),
        Event::Tanx => tanx::Tanx.allowed(run),
        Event::Tezcatara => tezcatara::Tezcatara.allowed(run),
        Event::Vakuu => vakuu::Vakuu.allowed(run),
        Event::TheLegendsWereTrue => the_legends_were_true::TheLegendsWereTrue.allowed(run),
        Event::ThisOrThat => this_or_that::ThisOrThat.allowed(run),
        Event::Wellspring => wellspring::Wellspring.allowed(run),
        Event::WhisperingHollow => whispering_hollow::WhisperingHollow.allowed(run),

        Event::DollRoom | Event::StoneOfAllTime if act != 1 => false,
        Event::DollRoom => true,
        Event::StoneOfAllTime => run.potions().next().is_some(),
        Event::PotionCourier | Event::Symbiote => act > 0,
        Event::WelcomeToWongos => act == 1 && gold >= 100,
        Event::CrystalSphere => gold >= 100 && act > 0,

        Event::EndlessConveyor => gold >= 120,
        Event::WaterloggedScriptorium => gold >= 55,
        Event::ZenWeaver => gold >= 125,
        // `GoldVar(149)`, before `CalculateVars` takes up to 50 off it.
        Event::LuminousChoir => gold >= 149 && run.relics_left(),

        Event::ColossalFlower => run.hp() >= 19,
        Event::TrashHeap => run.hp() > 5,
        // `CurrentHp <= MaxHp * 0.70m`, in whole numbers.
        Event::UnrestSite => run.hp() * 100 <= run.max_hp() * 70,

        Event::PunchOff => punch_off::PunchOff.allowed(run),
        Event::SlipperyBridge => run.floor() > 6 && run.deck().iter().any(removable),
        Event::TheFutureOfPotions => run.potions().count() >= 2,
        // A run has every character's card pool unlocked.
        Event::ColorfulPhilosophers => true,
        Event::FakeMerchant => {
            act >= 1 && (gold >= 100 || run.potions().any(|potion| potion == Potion::FoulPotion))
        }

        Event::Amalgamator => [CardTag::Strike, CardTag::Defend].into_iter().all(|tag| {
            let valid = |card: &&sts2_core::CardIdent<V>| {
                card.card.tagged(tag) && basic(card) && removable(card)
            };
            run.deck().iter().filter(valid).count() >= 2
        }),

        Event::FieldOfManSizedHoles => enchantable(run, Enchantment::PerfectFit),
        Event::SpiralingWhirlpool => enchantable(run, Enchantment::Spiral),

        Event::RelicTrader | Event::RanwidTheElder if act == 0 => false,
        Event::RelicTrader => run.relics().filter(|&held| tradable(held)).count() >= 5,
        Event::RanwidTheElder => {
            run.relics().any(tradable) && gold >= 100 && run.potions().next().is_some()
        }

        _ => true,
    }
}

/// `Player.HasEventPet`: a relic that adds a pet, or a Byrdonis Egg in the
/// deck.
fn has_event_pet(run: &impl EventView<V>) -> bool {
    run.relics().any(|held| adds_pet(held.relic))
        || run
            .deck()
            .iter()
            .any(|card| card.card == Cards::Quest(QuestCard::ByrdonisEgg))
}

/// Whether any card in the deck takes `enchantment`.
fn enchantable(run: &impl EventView<V>, enchantment: Enchantment) -> bool {
    run.deck().iter().any(|card| can_enchant(enchantment, card))
}

pub(crate) fn enter<R: sts2_core::rng::GameRng>(
    event: Event,
    run: &impl EventView<V>,
    rng: &mut R,
) -> State {
    match event {
        Event::Amalgamator => State::Amalgamator(amalgamator::Amalgamator.enter(run, rng)),
        Event::PotionCourier => State::PotionCourier(potion_courier::PotionCourier.enter(run, rng)),
        Event::ColorfulPhilosophers => {
            State::ColorfulPhilosophers(colorful_philosophers::ColorfulPhilosophers.enter(run, rng))
        }
        Event::InfestedAutomaton => {
            State::InfestedAutomaton(infested_automaton::InfestedAutomaton.enter(run, rng))
        }
        Event::Symbiote => State::Symbiote(symbiote::Symbiote.enter(run, rng)),
        Event::StoneOfAllTime => {
            State::StoneOfAllTime(stone_of_all_time::StoneOfAllTime.enter(run, rng))
        }
        Event::TheArchitect => State::TheArchitect(the_architect::TheArchitect.enter(run, rng)),
        Event::TheLanternKey => {
            State::TheLanternKey(the_lantern_key::TheLanternKey.enter(run, rng))
        }
        Event::ZenWeaver => State::ZenWeaver(zen_weaver::ZenWeaver.enter(run, rng)),
        Event::RanwidTheElder => {
            State::RanwidTheElder(ranwid_the_elder::RanwidTheElder.enter(run, rng))
        }
        Event::RelicTrader => State::RelicTrader(relic_trader::RelicTrader.enter(run, rng)),
        Event::DollRoom => State::DollRoom(doll_room::DollRoom.enter(run, rng)),
        Event::SpiritGrafter => State::SpiritGrafter(spirit_grafter::SpiritGrafter.enter(run, rng)),
        Event::ColossalFlower => {
            State::ColossalFlower(colossal_flower::ColossalFlower.enter(run, rng))
        }
        Event::Bugslayer => State::Bugslayer(bugslayer::Bugslayer.enter(run, rng)),
        Event::AromaOfChaos => State::AromaOfChaos(aroma_of_chaos::AromaOfChaos.enter(run, rng)),
        Event::FieldOfManSizedHoles => State::FieldOfManSizedHoles(
            field_of_man_sized_holes::FieldOfManSizedHoles.enter(run, rng),
        ),
        Event::BrainLeech => State::BrainLeech(brain_leech::BrainLeech.enter(run, rng)),
        Event::ByrdonisNest => State::ByrdonisNest(byrdonis_nest::ByrdonisNest.enter(run, rng)),
        Event::Darv => State::Darv(darv::Darv.enter(run, rng)),
        Event::PunchOff => State::PunchOff(punch_off::PunchOff.enter(run, rng)),
        Event::DenseVegetation => {
            State::DenseVegetation(dense_vegetation::DenseVegetation.enter(run, rng))
        }
        Event::JungleMazeAdventure => {
            State::JungleMazeAdventure(jungle_maze_adventure::JungleMazeAdventure.enter(run, rng))
        }
        Event::LostWisp => State::LostWisp(lost_wisp::LostWisp.enter(run, rng)),
        Event::LuminousChoir => State::LuminousChoir(luminous_choir::LuminousChoir.enter(run, rng)),
        Event::MorphicGrove => State::MorphicGrove(morphic_grove::MorphicGrove.enter(run, rng)),
        Event::Orobas => State::Orobas(orobas::Orobas.enter(run, rng)),
        Event::Nonupeipe => State::Nonupeipe(nonupeipe::Nonupeipe.enter(run, rng)),
        Event::Pael => State::Pael(pael::Pael.enter(run, rng)),
        Event::RoomFullOfCheese => {
            State::RoomFullOfCheese(room_full_of_cheese::RoomFullOfCheese.enter(run, rng))
        }
        Event::BattlewornDummy => {
            State::BattlewornDummy(battleworn_dummy::BattlewornDummy.enter(run, rng))
        }
        Event::GraveOfTheForgotten => {
            State::GraveOfTheForgotten(grave_of_the_forgotten::GraveOfTheForgotten.enter(run, rng))
        }
        Event::HungryForMushrooms => {
            State::HungryForMushrooms(hungry_for_mushrooms::HungryForMushrooms.enter(run, rng))
        }
        Event::Reflections => State::Reflections(reflections::Reflections.enter(run, rng)),
        Event::RoundTeaParty => {
            State::RoundTeaParty(round_tea_party::RoundTeaParty.enter(run, rng))
        }
        Event::TinkerTime => State::TinkerTime(tinker_time::TinkerTime.enter(run, rng)),
        Event::Trial => State::Trial(trial::Trial.enter(run, rng)),
        Event::WarHistorianRepy => {
            State::WarHistorianRepy(war_historian_repy::WarHistorianRepy.enter(run, rng))
        }
        Event::SapphireSeed => State::SapphireSeed(sapphire_seed::SapphireSeed.enter(run, rng)),
        Event::SelfHelpBook => State::SelfHelpBook(self_help_book::SelfHelpBook.enter(run, rng)),
        Event::SlipperyBridge => {
            State::SlipperyBridge(slippery_bridge::SlipperyBridge.enter(run, rng))
        }
        Event::SunkenStatue => State::SunkenStatue(sunken_statue::SunkenStatue.enter(run, rng)),
        Event::TrashHeap => State::TrashHeap(trash_heap::TrashHeap.enter(run, rng)),
        Event::EndlessConveyor => {
            State::EndlessConveyor(endless_conveyor::EndlessConveyor.enter(run, rng))
        }
        Event::DrowningBeacon => {
            State::DrowningBeacon(drowning_beacon::DrowningBeacon.enter(run, rng))
        }
        Event::SpiralingWhirlpool => {
            State::SpiralingWhirlpool(spiraling_whirlpool::SpiralingWhirlpool.enter(run, rng))
        }
        Event::DoorsOfLightAndDark => {
            State::DoorsOfLightAndDark(doors_of_light_and_dark::DoorsOfLightAndDark.enter(run, rng))
        }
        Event::WaterloggedScriptorium => State::WaterloggedScriptorium(
            waterlogged_scriptorium::WaterloggedScriptorium.enter(run, rng),
        ),
        Event::AbyssalBaths => State::AbyssalBaths(abyssal_baths::AbyssalBaths.enter(run, rng)),
        Event::SunkenTreasury => {
            State::SunkenTreasury(sunken_treasury::SunkenTreasury.enter(run, rng))
        }
        Event::UnrestSite => State::UnrestSite(unrest_site::UnrestSite.enter(run, rng)),
        Event::TabletOfTruth => {
            State::TabletOfTruth(tablet_of_truth::TabletOfTruth.enter(run, rng))
        }
        Event::TeaMaster => State::TeaMaster(tea_master::TeaMaster.enter(run, rng)),
        Event::Tanx => State::Tanx(tanx::Tanx.enter(run, rng)),
        Event::Tezcatara => State::Tezcatara(tezcatara::Tezcatara.enter(run, rng)),
        Event::Vakuu => State::Vakuu(vakuu::Vakuu.enter(run, rng)),
        Event::TheFutureOfPotions => {
            State::TheFutureOfPotions(the_future_of_potions::TheFutureOfPotions.enter(run, rng))
        }
        Event::TheLegendsWereTrue => {
            State::TheLegendsWereTrue(the_legends_were_true::TheLegendsWereTrue.enter(run, rng))
        }
        Event::ThisOrThat => State::ThisOrThat(this_or_that::ThisOrThat.enter(run, rng)),
        Event::Wellspring => State::Wellspring(wellspring::Wellspring.enter(run, rng)),
        Event::FakeMerchant => State::FakeMerchant(fake_merchant::FakeMerchant.enter(run, rng)),
        Event::WelcomeToWongos => {
            State::WelcomeToWongos(welcome_to_wongos::WelcomeToWongos.enter(run, rng))
        }
        Event::CrystalSphere => State::CrystalSphere(crystal_sphere::CrystalSphere.enter(run, rng)),
        Event::WhisperingHollow => {
            State::WhisperingHollow(whispering_hollow::WhisperingHollow.enter(run, rng))
        }
        Event::WoodCarvings => State::WoodCarvings(wood_carvings::WoodCarvings.enter(run, rng)),
        _ => unimplemented!("entering event {event:?}"),
    }
}

/// `SetupForPlayer` on what the options offer, off the player's rewards
/// stream. Darv's Dusty Tome is the one option that sets itself up.
pub(crate) fn set_up<R: sts2_core::rng::GameRng>(
    state: &mut State,
    run: &impl EventView<V>,
    rewards: &mut R,
) {
    if let State::Darv(state) = state {
        darv::Darv.set_up(state, run, rewards);
    }
}

/// [`EventBehavior::relics_pulled`] for the event entered.
pub(crate) fn relics_pulled(state: &State) -> &'static [sts2_core::RelicPull<V>] {
    match state {
        State::RelicTrader(state) => relic_trader::RelicTrader.relics_pulled(state),
        State::WelcomeToWongos(state) => welcome_to_wongos::WelcomeToWongos.relics_pulled(state),
        _ => &[],
    }
}

/// [`EventBehavior::hand_pulled_relics`] for the event entered.
pub(crate) fn hand_pulled_relics(
    state: &mut State,
    relics: &[super::super::generated::relics::Relic],
) {
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
pub(crate) fn holds_potions(state: &State) -> bool {
    matches!(
        state,
        State::TheFutureOfPotions(_) | State::RanwidTheElder(_) | State::StoneOfAllTime(_)
    )
}

pub(crate) fn observe(state: &State, run: &impl EventView<V>) -> Room<V> {
    match state {
        State::Amalgamator(state) => amalgamator::Amalgamator.observe(state, run),
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
        State::BattlewornDummy(state) => battleworn_dummy::BattlewornDummy.observe(state, run),
        State::GraveOfTheForgotten(state) => {
            grave_of_the_forgotten::GraveOfTheForgotten.observe(state, run)
        }
        State::HungryForMushrooms(state) => {
            hungry_for_mushrooms::HungryForMushrooms.observe(state, run)
        }
        State::Reflections(state) => reflections::Reflections.observe(state, run),
        State::RoundTeaParty(state) => round_tea_party::RoundTeaParty.observe(state, run),
        State::TinkerTime(state) => tinker_time::TinkerTime.observe(state, run),
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

pub(crate) fn choose<R: sts2_core::rng::GameRng>(
    state: &mut State,
    option: u8,
    run: &impl EventView<V>,
    rng: &mut R,
) -> HookEffects<V> {
    match state {
        State::Amalgamator(state) => amalgamator::Amalgamator.choose(state, option, run, rng),
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
        State::BattlewornDummy(state) => {
            battleworn_dummy::BattlewornDummy.choose(state, option, run, rng)
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
        State::TinkerTime(state) => tinker_time::TinkerTime.choose(state, option, run, rng),
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
pub(crate) fn question(state: &State) -> Option<sts2_core::Question<V>> {
    match state {
        State::CrystalSphere(state) => crystal_sphere::CrystalSphere.question(state),
        _ => None,
    }
}

/// [`EventBehavior::answer`] for the event entered.
pub(crate) fn answer(state: &mut State, answer: &sts2_core::Answer) -> HookEffects<V> {
    match state {
        State::CrystalSphere(state) => crystal_sphere::CrystalSphere.answer(state, answer),
        state => unimplemented!("{state:?} answered with {answer:?}"),
    }
}

/// [`EventBehavior::resume`] for the event entered.
pub(crate) fn resume<R: sts2_core::rng::GameRng>(
    state: &mut State,
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
pub(crate) fn resume_from_fight(state: &mut State, escaped: bool) -> HookEffects<V> {
    match state {
        State::BattlewornDummy(state) => {
            battleworn_dummy::BattlewornDummy.resume_from_fight(state, escaped)
        }
        state => unimplemented!("{state:?} resumed after its fight"),
    }
}

/// [`EventBehavior::rewards`] for the event entered.
pub(crate) fn rewards(state: &State) -> Vec<sts2_core::CustomReward<V>> {
    match state {
        State::CrystalSphere(state) => crystal_sphere::CrystalSphere.rewards(state),
        state => unimplemented!("rewards of {state:?}'s own"),
    }
}

/// [`EventBehavior::stock`] for the event entered.
pub(crate) fn stock<R: sts2_core::rng::GameRng>(state: &mut State, shops: &mut R) {
    if let State::FakeMerchant(state) = state {
        fake_merchant::FakeMerchant.stock(state, shops);
    }
}

/// [`EventBehavior::buy`] for the event entered.
pub(crate) fn buy(state: &mut State, index: u8) -> HookEffects<V> {
    match state {
        State::FakeMerchant(state) => fake_merchant::FakeMerchant.buy(state, index),
        state => unimplemented!("buying entry {index} from {state:?}"),
    }
}
