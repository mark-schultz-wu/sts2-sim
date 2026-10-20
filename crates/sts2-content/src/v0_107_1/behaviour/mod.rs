//! What this build's content does, as ported code.
//!
//! The generated tables beside this say what the game's data holds. This says
//! what its code does with it. A row is dumped. A rule is read and written out
//! by hand, against the decompiled source.
//!
//! Filed the way `generated/` is, so a thing's behaviour sits under the same
//! name as its row.

pub(crate) mod cards;
pub mod events;
pub(crate) mod potions;
pub(crate) mod powers;
pub(crate) mod relics;

use super::V0_107_1 as V;

impl crate::build::OwnRules for V {
    type OwnEvents = events::OwnEvents;
    /// Diamond Diadem is this build's own, and its state is shared.
    type OwnRelic = std::convert::Infallible;

    /// No deck card is its own.
    type OwnCard = std::convert::Infallible;

    fn own_card_state(_card: <V as sts2_core::ContentVersion>::Card) -> Option<Self::OwnCard> {
        None
    }

    fn own_card_written(own: Self::OwnCard) -> Option<sts2_core::Change<V>> {
        match own {}
    }

    fn own_deck_card_event_hook(
        _card: <V as sts2_core::ContentVersion>::Card,
        own: &mut Self::OwnCard,
        _hook: sts2_core::EventHook<V>,
        _asked: &mut sts2_core::HookEffects<V>,
    ) {
        match *own {}
    }

    fn own_relic_state(_relic: <V as sts2_core::ContentVersion>::Relic) -> Option<Self::OwnRelic> {
        None
    }

    fn own_event_allowed(
        event: <V as sts2_core::ContentVersion>::Event,
        run: &impl sts2_core::EventView<V>,
    ) -> Option<bool> {
        events::allowed(event, run)
    }

    fn own_event_enter<R: sts2_core::rng::GameRng>(
        event: <V as sts2_core::ContentVersion>::Event,
        run: &impl sts2_core::EventView<V>,
        rng: &mut R,
    ) -> Option<events::State> {
        events::enter(event, run, rng)
    }

    fn own_relic_event_hook(
        state: &mut crate::behaviour::relics::State<V>,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) {
        relics::own_relic_event_hook(state, hook, asked);
    }

    fn own_card_play<F: sts2_core::Fight<V>>(
        card: <V as sts2_core::ContentVersion>::Card,
        fight: &mut F,
    ) -> Option<sts2_core::Played<V>> {
        cards::on_play(card, fight)
    }

    fn own_card_calculated<P: sts2_core::Printed<V> + ?Sized>(
        card: <V as sts2_core::ContentVersion>::Card,
        key: <V as sts2_core::ContentVersion>::CardVar,
        printed: &P,
        owner: sts2_core::CardOwner<'_, V>,
    ) -> Option<i32> {
        cards::calculated(card, key, printed, owner)
    }

    fn generated_upgrade(
        card: <V as sts2_core::ContentVersion>::Card,
    ) -> &'static [sts2_core::Change<V>] {
        use super::generated::card_plays;
        use super::generated::cards::Cards;

        match card {
            Cards::Ironclad(card) => card_plays::ironclad::upgrade(card),
            Cards::Silent(card) => card_plays::silent::upgrade(card),
            Cards::Regent(card) => card_plays::regent::upgrade(card),
            Cards::Necrobinder(card) => card_plays::necrobinder::upgrade(card),
            Cards::Defect(card) => card_plays::defect::upgrade(card),
            Cards::Colorless(card) => card_plays::colorless::upgrade(card),
            Cards::Curse(card) => card_plays::curse::upgrade(card),
            Cards::Event(card) => card_plays::event::upgrade(card),
            Cards::Quest(card) => card_plays::quest::upgrade(card),
            Cards::Status(card) => card_plays::status::upgrade(card),
            Cards::Token(card) => card_plays::token::upgrade(card),
            Cards::Deprecated(card) => card_plays::deprecated::upgrade(card),
        }
    }

    fn generated_play_from<F: sts2_core::Fight<V>>(
        card: <V as sts2_core::ContentVersion>::Card,
        fight: &mut F,
        from: u8,
    ) -> Result<sts2_core::Played<V>, u8> {
        use super::generated::card_plays;
        use super::generated::cards::Cards;

        match card {
            Cards::Ironclad(card) => card_plays::ironclad::play_from(card, fight, from),
            Cards::Silent(card) => card_plays::silent::play_from(card, fight, from),
            Cards::Regent(card) => card_plays::regent::play_from(card, fight, from),
            Cards::Necrobinder(card) => card_plays::necrobinder::play_from(card, fight, from),
            Cards::Defect(card) => card_plays::defect::play_from(card, fight, from),
            Cards::Colorless(card) => card_plays::colorless::play_from(card, fight, from),
            Cards::Curse(card) => card_plays::curse::play_from(card, fight, from),
            Cards::Event(card) => card_plays::event::play_from(card, fight, from),
            Cards::Quest(card) => card_plays::quest::play_from(card, fight, from),
            Cards::Status(card) => card_plays::status::play_from(card, fight, from),
            Cards::Token(card) => card_plays::token::play_from(card, fight, from),
            Cards::Deprecated(card) => card_plays::deprecated::play_from(card, fight, from),
        }
    }

    fn generated_turn_end<F: sts2_core::Fight<V>>(
        card: <V as sts2_core::ContentVersion>::Card,
        fight: &mut F,
    ) {
        use super::generated::card_plays;
        use super::generated::cards::Cards;

        match card {
            Cards::Ironclad(card) => card_plays::ironclad::on_turn_end(card, fight),
            Cards::Silent(card) => card_plays::silent::on_turn_end(card, fight),
            Cards::Regent(card) => card_plays::regent::on_turn_end(card, fight),
            Cards::Necrobinder(card) => card_plays::necrobinder::on_turn_end(card, fight),
            Cards::Defect(card) => card_plays::defect::on_turn_end(card, fight),
            Cards::Colorless(card) => card_plays::colorless::on_turn_end(card, fight),
            Cards::Curse(card) => card_plays::curse::on_turn_end(card, fight),
            Cards::Event(card) => card_plays::event::on_turn_end(card, fight),
            Cards::Quest(card) => card_plays::quest::on_turn_end(card, fight),
            Cards::Status(card) => card_plays::status::on_turn_end(card, fight),
            Cards::Token(card) => card_plays::token::on_turn_end(card, fight),
            Cards::Deprecated(card) => card_plays::deprecated::on_turn_end(card, fight),
        }
    }

    /// No move is its own.
    fn own_move<F: sts2_core::Fight<V>>(
        _monster: <V as sts2_core::ContentVersion>::Monster,
        _move_id: <V as sts2_core::ContentVersion>::MoveId,
        _step: u8,
        _fight: &mut F,
    ) -> Option<Option<u8>> {
        None
    }

    fn generated_move<F: sts2_core::Fight<V>>(
        monster: <V as sts2_core::ContentVersion>::Monster,
        move_id: <V as sts2_core::ContentVersion>::MoveId,
        step: u8,
        fight: &mut F,
    ) -> Option<u8> {
        use super::generated::monster_moves;
        use crate::generated::monster_moves::Filed;

        match monster_moves::filed(monster) {
            Filed::Glory => monster_moves::glory::perform(monster, move_id, step, fight),
            Filed::Hive => monster_moves::hive::perform(monster, move_id, step, fight),
            Filed::Overgrowth => monster_moves::overgrowth::perform(monster, move_id, step, fight),
            Filed::Shared => monster_moves::shared::perform(monster, move_id, step, fight),
            Filed::Summoned => monster_moves::summoned::perform(monster, move_id, step, fight),
            Filed::Underdocks => monster_moves::underdocks::perform(monster, move_id, step, fight),
        }
    }

    /// No power's death is its own.
    fn own_power_death<F: sts2_core::Fight<V>>(
        _power: <V as sts2_core::ContentVersion>::Power,
        _holder: sts2_core::CombatId,
        _fight: &mut F,
    ) -> Option<()> {
        None
    }

    fn own_power(power: <V as sts2_core::ContentVersion>::Power) -> bool {
        powers::own_power(power)
    }

    fn own_power_state(
        power: <V as sts2_core::ContentVersion>::Power,
    ) -> Option<crate::behaviour::powers::State<V>> {
        powers::own_power_state(power)
    }

    fn own_power_modify_hook(
        power: <V as sts2_core::ContentVersion>::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, V>,
        state: &crate::behaviour::powers::State<V>,
        hook: sts2_core::ModifyHook<V>,
    ) -> Option<sts2_core::Modify<i32>> {
        powers::own_power_modify_hook(power, amount, held, state, hook)
    }

    fn own_power_event_hook(
        power: <V as sts2_core::ContentVersion>::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, V>,
        state: &mut crate::behaviour::powers::State<V>,
        hook: sts2_core::EventHook<V>,
    ) -> Option<Option<sts2_core::HookEffect<V>>> {
        powers::own_power_event_hook(power, amount, held, state, hook)
    }
    fn own_power_decide_hook(
        power: <V as sts2_core::ContentVersion>::Power,
        amount: sts2_core::PowerAmount<i32>,
        held: crate::behaviour::powers::Held<'_, V>,
        state: &crate::behaviour::powers::State<V>,
        hook: sts2_core::DecisionHook<V>,
    ) -> Option<sts2_core::HookResponse<bool>> {
        powers::own_power_decide_hook(power, amount, held, state, hook)
    }

    fn own_potion_use<F: sts2_core::Fight<V>>(
        potion: <V as sts2_core::ContentVersion>::Potion,
        fight: &mut F,
    ) -> Option<sts2_core::Played<V>> {
        potions::own_potion_use(potion, fight)
    }

    fn own_monster_hp_bounds(
        _monster: <V as sts2_core::ContentVersion>::Monster,
        _model: &crate::behaviour::monsters::Model<V>,
        _ascension: crate::generated::ascension::AscensionLevel,
    ) -> Option<sts2_core::rng::Interval<i32>> {
        None
    }

    fn own_relic(_relic: <V as sts2_core::ContentVersion>::Relic) -> bool {
        false
    }

    fn own_shared_relic_event_hook(
        _state: &mut crate::behaviour::relics::State<V>,
        _hook: sts2_core::EventHook<V>,
        _asked: &mut sts2_core::HookEffects<V>,
    ) -> Option<()> {
        None
    }

    fn own_potion_use_on_run(
        potion: <V as sts2_core::ContentVersion>::Potion,
        run: &impl sts2_core::EventView<V>,
        in_fight: bool,
    ) -> Option<sts2_core::HookEffects<V>> {
        potions::own_potion_use_on_run(potion, run, in_fight)
    }
}
