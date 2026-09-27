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

mod alchemical_coffer;
mod anchor;
mod arcane_scroll;
mod archaic_tooth;
mod bone_tea;
mod booming_conch;
mod bound_phylactery;
mod burning_blood;
mod byrdpip;
mod chosen_cheese;
mod cracked_core;
mod cursed_pearl;
mod divine_right;
mod dollys_mirror;
mod fishing_rod;
mod glass_eye;
mod golden_pearl;
mod gorget;
mod gremlin_horn;
mod hefty_tablet;
mod lava_rock;
mod lead_paperweight;
mod leafy_poultice;
mod lost_coffer;
mod mercury_hourglass;
mod neows_talisman;
mod neows_torment;
mod new_leaf;
mod nutritious_oyster;
mod paels_blood;
mod paels_flesh;
mod paels_horn;
mod paels_tears;
mod permafrost;
mod phial_holster;
mod pocketwatch;
mod pomander;
mod precarious_shears;
mod precise_scissors;
mod ring_of_the_snake;
mod sand_castle;
mod seal_of_gold;
mod stone_humidifier;
mod storybook;
mod strike_dummy;
mod sword_of_stone;
mod very_hot_cocoa;
mod winged_boots;

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
    FishingRod(fishing_rod::State),
    GlassEye,
    GoldenPearl,
    Gorget,
    GremlinHorn,
    HeftyTablet,
    LavaRock(lava_rock::State),
    LeadPaperweight,
    LeafyPoultice,
    LostCoffer,
    MercuryHourglass,
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
    WingedBoots(winged_boots::State),
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
        State::FishingRod(state) => fishing_rod::event_hook(state, hook, asked),
        State::GlassEye => glass_eye::event_hook(hook, asked),
        State::GoldenPearl => golden_pearl::event_hook(hook, asked),
        State::Gorget => gorget::event_hook(hook, asked),
        State::GremlinHorn => gremlin_horn::event_hook(hook, asked),
        State::HeftyTablet => hefty_tablet::event_hook(hook, asked),
        State::LavaRock(state) => lava_rock::event_hook(state, hook, asked),
        State::LeadPaperweight => lead_paperweight::event_hook(hook),
        State::LeafyPoultice => leafy_poultice::event_hook(hook, asked),
        State::LostCoffer => lost_coffer::event_hook(hook, asked),
        State::MercuryHourglass => mercury_hourglass::event_hook(hook, asked),
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
        State::SwordOfStone(state) => sword_of_stone::event_hook(state, hook),
        State::VeryHotCocoa => very_hot_cocoa::event_hook(hook, asked),
        State::WingedBoots(state) => winged_boots::event_hook(state, hook),
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
        State::RingOfTheSnake => ring_of_the_snake::modify_hook(hook),
        State::BoomingConch => booming_conch::modify_hook(hook),
        State::StrikeDummy => strike_dummy::modify_hook(hook),
        State::PaelsBlood => paels_blood::modify_hook(hook),
        State::PaelsFlesh(_) => paels_flesh::modify_hook(hook),
        // The fight asks only a relic that overrides the hook, and none of
        // the rest overrides one of these.
        State::AlchemicalCoffer
        | State::Anchor
        | State::ArcaneScroll
        | State::ArchaicTooth
        | State::BoneTea(_)
        | State::BoundPhylactery
        | State::BurningBlood
        | State::Byrdpip
        | State::ChosenCheese
        | State::CrackedCore
        | State::CursedPearl
        | State::DivineRight
        | State::DollysMirror
        | State::FishingRod(_)
        | State::GlassEye
        | State::GoldenPearl
        | State::Gorget
        | State::GremlinHorn
        | State::HeftyTablet
        | State::LavaRock(_)
        | State::LeadPaperweight
        | State::LeafyPoultice
        | State::LostCoffer
        | State::MercuryHourglass
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
        | State::WingedBoots(_) => unimplemented!("what {state:?} does for {hook:?}"),
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
            Relic::FishingRod => State::FishingRod(fishing_rod::State::default()),
            Relic::GlassEye => State::GlassEye,
            Relic::GoldenPearl => State::GoldenPearl,
            Relic::Gorget => State::Gorget,
            Relic::GremlinHorn => State::GremlinHorn,
            Relic::HeftyTablet => State::HeftyTablet,
            Relic::LavaRock => State::LavaRock(lava_rock::State::default()),
            Relic::LeadPaperweight => State::LeadPaperweight,
            Relic::LeafyPoultice => State::LeafyPoultice,
            Relic::LostCoffer => State::LostCoffer,
            Relic::MercuryHourglass => State::MercuryHourglass,
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
            Relic::Pomander => State::Pomander,
            Relic::PrecariousShears => State::PrecariousShears,
            Relic::PreciseScissors => State::PreciseScissors,
            Relic::RingOfTheSnake => State::RingOfTheSnake,
            Relic::SandCastle => State::SandCastle,
            Relic::SealOfGold => State::SealOfGold,
            Relic::StoneHumidifier => State::StoneHumidifier,
            Relic::Storybook => State::Storybook,
            Relic::StrikeDummy => State::StrikeDummy,
            Relic::SwordOfStone => State::SwordOfStone(sword_of_stone::State::default()),
            Relic::VeryHotCocoa => State::VeryHotCocoa,
            Relic::WingedBoots => State::WingedBoots(winged_boots::State::default()),
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
                State::SwordOfStone(state) => sword_of_stone::display_amount(state),
                State::WingedBoots(state) => winged_boots::display_amount(state),
                State::Pocketwatch(state) => pocketwatch::display_amount(state),
                State::PaelsFlesh(state) => paels_flesh::display_amount(state),
                _ => unimplemented!("the number {relic:?} shows"),
            },
        }
    }
}
