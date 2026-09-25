//! Event behavior dispatch for this build.

mod aroma_of_chaos;
mod byrdonis_nest;
mod morphic_grove;
mod orobas;
mod room_full_of_cheese;
mod sunken_statue;
mod tea_master;
mod the_legends_were_true;
mod this_or_that;
mod wellspring;
mod whispering_hollow;

use sts2_core::{
    CardTable as _, EventBehavior, EventOption, EventOptionKey, EventView, HookEffects, Room,
};

use super::super::EventOptionId;
use super::super::V0_107_1 as V;
use super::super::generated::cards::{CardRarity, CardTag};
use super::super::generated::enchantments::Enchantment;
use super::super::generated::events::{Event, EventOption as Said};
use super::super::generated::potions::Potion;
use super::enchantments::can_enchant;
use super::relics::adds_pet;

#[derive(Debug, Clone)]
pub enum State {
    AromaOfChaos(aroma_of_chaos::State),
    ByrdonisNest(byrdonis_nest::State),
    MorphicGrove(morphic_grove::State),
    Orobas(orobas::State),
    RoomFullOfCheese(room_full_of_cheese::State),
    SunkenStatue(sunken_statue::State),
    TeaMaster(tea_master::State),
    TheLegendsWereTrue(the_legends_were_true::State),
    ThisOrThat(this_or_that::State),
    Wellspring(wellspring::State),
    WhisperingHollow(whispering_hollow::State),
}

/// A page of the options its keys name, each with whether it is locked, and
/// the one taken marked.
///
/// An option that asks a question leaves the page up, with its choice
/// marked, until the question is answered.
pub(super) fn page(event: Event, options: &[(Said, bool)], chosen: Option<u8>) -> Room<V> {
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
                    chosen: chosen == Some(index),
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
    let removable = |card: &sts2_core::CardIdent<V>| !card.card.eternal();
    let basic = |card: &sts2_core::CardIdent<V>| card.card.rarity() == CardRarity::Basic;
    match event {
        Event::AromaOfChaos => aroma_of_chaos::AromaOfChaos.allowed(run),
        Event::ByrdonisNest => byrdonis_nest::ByrdonisNest.allowed(run),
        Event::MorphicGrove => morphic_grove::MorphicGrove.allowed(run),
        Event::Orobas => orobas::Orobas.allowed(run),
        Event::RoomFullOfCheese => room_full_of_cheese::RoomFullOfCheese.allowed(run),
        Event::SunkenStatue => sunken_statue::SunkenStatue.allowed(run),
        Event::TeaMaster => tea_master::TeaMaster.allowed(run),
        Event::TheLegendsWereTrue => the_legends_were_true::TheLegendsWereTrue.allowed(run),
        Event::ThisOrThat => this_or_that::ThisOrThat.allowed(run),
        Event::Wellspring => wellspring::Wellspring.allowed(run),
        Event::WhisperingHollow => whispering_hollow::WhisperingHollow.allowed(run),

        Event::DollRoom | Event::StoneOfAllTime if act != 1 => false,
        Event::DollRoom => true,
        Event::StoneOfAllTime => run.potions().next().is_some(),
        Event::BrainLeech => act < 2,
        Event::PotionCourier | Event::Symbiote => act > 0,
        Event::WelcomeToWongos => act == 1 && gold >= 100,
        Event::CrystalSphere => gold >= 100 && act > 0,
        Event::WarHistorianRepy => false,

        Event::EndlessConveyor => gold >= 120,
        Event::WaterloggedScriptorium => gold >= 55,
        Event::ZenWeaver => gold >= 125,
        // `GoldVar(149)`, before `CalculateVars` takes up to 50 off it.
        Event::LuminousChoir => gold >= 149 && run.relics_left(),

        Event::RoundTeaParty => run.hp() >= 12,
        Event::ColossalFlower => run.hp() >= 19,
        Event::TrashHeap => run.hp() > 5,
        // `CurrentHp <= MaxHp * 0.70m`, in whole numbers.
        Event::UnrestSite => run.hp() * 100 <= run.max_hp() * 70,
        // Its HP check is for a run of more than one player.
        Event::DenseVegetation => true,

        Event::PunchOff => run.floor() >= 6,
        Event::SlipperyBridge => run.floor() > 6 && run.deck().iter().any(removable),
        Event::TheFutureOfPotions => run.potions().count() >= 2,
        // A run has every character's card pool unlocked.
        Event::ColorfulPhilosophers => true,
        Event::FakeMerchant => {
            act >= 1 && (gold >= 100 || run.potions().any(|potion| potion == Potion::FoulPotion))
        }

        Event::WoodCarvings => run.deck().iter().any(|card| basic(card) && removable(card)),
        Event::Amalgamator => [CardTag::Strike, CardTag::Defend].into_iter().all(|tag| {
            let valid = |card: &&sts2_core::CardIdent<V>| {
                card.card.tagged(tag) && basic(card) && removable(card)
            };
            run.deck().iter().filter(valid).count() >= 2
        }),

        Event::FieldOfManSizedHoles => enchantable(run, Enchantment::PerfectFit),
        Event::SpiralingWhirlpool => enchantable(run, Enchantment::Spiral),
        Event::GraveOfTheForgotten => enchantable(run, Enchantment::SoulsPower),

        Event::RelicTrader | Event::RanwidTheElder if act == 0 => false,
        Event::RelicTrader | Event::RanwidTheElder => {
            unimplemented!("which relics {event:?} can trade for")
        }

        _ => true,
    }
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
        Event::AromaOfChaos => State::AromaOfChaos(aroma_of_chaos::AromaOfChaos.enter(run, rng)),
        Event::ByrdonisNest => State::ByrdonisNest(byrdonis_nest::ByrdonisNest.enter(run, rng)),
        Event::MorphicGrove => State::MorphicGrove(morphic_grove::MorphicGrove.enter(run, rng)),
        Event::Orobas => State::Orobas(orobas::Orobas.enter(run, rng)),
        Event::RoomFullOfCheese => {
            State::RoomFullOfCheese(room_full_of_cheese::RoomFullOfCheese.enter(run, rng))
        }
        Event::SunkenStatue => State::SunkenStatue(sunken_statue::SunkenStatue.enter(run, rng)),
        Event::TeaMaster => State::TeaMaster(tea_master::TeaMaster.enter(run, rng)),
        Event::TheLegendsWereTrue => {
            State::TheLegendsWereTrue(the_legends_were_true::TheLegendsWereTrue.enter(run, rng))
        }
        Event::ThisOrThat => State::ThisOrThat(this_or_that::ThisOrThat.enter(run, rng)),
        Event::Wellspring => State::Wellspring(wellspring::Wellspring.enter(run, rng)),
        Event::WhisperingHollow => {
            State::WhisperingHollow(whispering_hollow::WhisperingHollow.enter(run, rng))
        }
        _ => unimplemented!("entering event {event:?}"),
    }
}

pub(crate) fn observe(state: &State, run: &impl EventView<V>) -> Room<V> {
    match state {
        State::AromaOfChaos(state) => aroma_of_chaos::AromaOfChaos.observe(state, run),
        State::ByrdonisNest(state) => byrdonis_nest::ByrdonisNest.observe(state, run),
        State::MorphicGrove(state) => morphic_grove::MorphicGrove.observe(state, run),
        State::Orobas(state) => orobas::Orobas.observe(state, run),
        State::RoomFullOfCheese(state) => room_full_of_cheese::RoomFullOfCheese.observe(state, run),
        State::SunkenStatue(state) => sunken_statue::SunkenStatue.observe(state, run),
        State::TeaMaster(state) => tea_master::TeaMaster.observe(state, run),
        State::TheLegendsWereTrue(state) => {
            the_legends_were_true::TheLegendsWereTrue.observe(state, run)
        }
        State::ThisOrThat(state) => this_or_that::ThisOrThat.observe(state, run),
        State::Wellspring(state) => wellspring::Wellspring.observe(state, run),
        State::WhisperingHollow(state) => whispering_hollow::WhisperingHollow.observe(state, run),
    }
}

pub(crate) fn choose<R: sts2_core::rng::GameRng>(
    state: &mut State,
    option: u8,
    run: &impl EventView<V>,
    rng: &mut R,
) -> HookEffects<V> {
    match state {
        State::AromaOfChaos(state) => aroma_of_chaos::AromaOfChaos.choose(state, option, run, rng),
        State::ByrdonisNest(state) => byrdonis_nest::ByrdonisNest.choose(state, option, run, rng),
        State::MorphicGrove(state) => morphic_grove::MorphicGrove.choose(state, option, run, rng),
        State::Orobas(state) => orobas::Orobas.choose(state, option, run, rng),
        State::RoomFullOfCheese(state) => {
            room_full_of_cheese::RoomFullOfCheese.choose(state, option, run, rng)
        }
        State::SunkenStatue(state) => sunken_statue::SunkenStatue.choose(state, option, run, rng),
        State::TeaMaster(state) => tea_master::TeaMaster.choose(state, option, run, rng),
        State::TheLegendsWereTrue(state) => {
            the_legends_were_true::TheLegendsWereTrue.choose(state, option, run, rng)
        }
        State::ThisOrThat(state) => this_or_that::ThisOrThat.choose(state, option, run, rng),
        State::Wellspring(state) => wellspring::Wellspring.choose(state, option, run, rng),
        State::WhisperingHollow(state) => {
            whispering_hollow::WhisperingHollow.choose(state, option, run, rng)
        }
    }
}
