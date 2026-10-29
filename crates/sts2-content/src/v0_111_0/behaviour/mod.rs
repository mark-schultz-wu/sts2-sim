//! What this build's content does, as ported code.
//!
//! The generated tables beside this say what the game's data holds. This says
//! what its code does with it. A row is dumped. A rule is read and written out
//! by hand, against the decompiled source.
//!
//! Filed the way `generated/` is, so a thing's behaviour sits under the same
//! name as its row.

pub(crate) mod cards;
pub(crate) mod enchantments;
pub mod events;
pub(crate) mod monsters;
pub(crate) mod potions;
pub(crate) mod powers;
pub(crate) mod relics;

use super::V0_111_0 as V;

impl crate::build::OwnRules for V {
    type OwnEvents = events::OwnEvents;
    type OwnRelic = relics::OwnRelic;

    type OwnCard = cards::OwnCard;

    fn own_card_state(card: <V as sts2_core::ContentVersion>::Card) -> Option<Self::OwnCard> {
        cards::own_card_state(card)
    }

    fn own_card_written(own: Self::OwnCard) -> Option<sts2_core::Change<V>> {
        cards::own_card_written(own)
    }

    fn own_deck_card_event_hook(
        card: <V as sts2_core::ContentVersion>::Card,
        own: &mut Self::OwnCard,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) {
        cards::own_deck_card_event_hook(card, own, hook, asked);
    }

    fn own_relic_state(relic: <V as sts2_core::ContentVersion>::Relic) -> Option<Self::OwnRelic> {
        relics::own_relic_state(relic)
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

    fn own_card_hooks(card: <V as sts2_core::ContentVersion>::Card) -> bool {
        cards::own_card_hooks(card)
    }

    fn own_card_event_hook(
        card: <V as sts2_core::ContentVersion>::Card,
        id: sts2_core::CardId<V>,
        owner: sts2_core::CombatId,
        history: sts2_core::History<'_, V>,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) -> Option<()> {
        cards::own_card_event_hook(card, id, owner, history, hook, asked)
    }

    fn own_card_result_pile(
        card: <V as sts2_core::ContentVersion>::Card,
        base: (Option<sts2_core::Pile>, sts2_core::PilePosition),
    ) -> Option<sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)>> {
        cards::result_pile(card, base)
    }

    fn own_card_play<'a, K: sts2_core::FightKind<V>>(
        card: <V as sts2_core::ContentVersion>::Card,
        fight: &mut K::Fight<'a>,
    ) -> Option<crate::behaviour::Played<V, K>> {
        cards::on_play::<K>(card, fight)
    }

    fn own_lends(
        power: <V as sts2_core::ContentVersion>::Power,
    ) -> Option<(<V as sts2_core::ContentVersion>::Power, bool)> {
        powers::own_lends(power)
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

    type Paused<K: sts2_core::FightKind<V>> = Paused<K>;

    fn generated_play<'a, K: sts2_core::FightKind<V>>(
        card: <V as sts2_core::ContentVersion>::Card,
        fight: &mut K::Fight<'a>,
    ) -> crate::behaviour::Played<V, K> {
        super::generated::card_plays::play::<K>(card, fight)
    }

    fn resume_paused<'a, K: sts2_core::FightKind<V>>(
        fight: &mut K::Fight<'a>,
        paused: Self::Paused<K>,
        taken: sts2_core::Taken<V>,
    ) -> crate::behaviour::Played<V, K> {
        match paused {
            Paused::Generated(paused) => {
                super::generated::card_plays::resume::<K>(fight, paused, taken)
            }
            Paused::Own(paused) => cards::resume::<K>(fight, paused, taken),
            Paused::Moves(paused) => {
                super::generated::monster_moves::resume::<K>(fight, paused, taken)
            }
            Paused::OwnMoves(paused) => monsters::resume::<K>(fight, paused, taken),
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

    fn own_move<'a, K: sts2_core::FightKind<V>>(
        monster: <V as sts2_core::ContentVersion>::Monster,
        move_id: <V as sts2_core::ContentVersion>::MoveId,
        fight: &mut K::Fight<'a>,
    ) -> Option<crate::behaviour::Played<V, K>> {
        monsters::own_move::<K>(monster, move_id, fight)
    }

    fn generated_move<'a, K: sts2_core::FightKind<V>>(
        monster: <V as sts2_core::ContentVersion>::Monster,
        move_id: <V as sts2_core::ContentVersion>::MoveId,
        fight: &mut K::Fight<'a>,
    ) -> crate::behaviour::Played<V, K> {
        super::generated::monster_moves::perform::<K>(monster, move_id, fight)
    }

    fn own_power_death<F: sts2_core::Fight<V>>(
        power: <V as sts2_core::ContentVersion>::Power,
        holder: sts2_core::CombatId,
        fight: &mut F,
    ) -> Option<()> {
        powers::own_power_death(power, holder, fight)
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

    fn own_power_result_pile(
        power: <V as sts2_core::ContentVersion>::Power,
        amount: sts2_core::PowerAmount<i32>,
        holder: sts2_core::CombatId,
        state: &crate::behaviour::powers::State<V>,
        hook: sts2_core::ResultPile<V>,
    ) -> Option<sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)>> {
        powers::own_power_result_pile(power, amount, holder, state, hook)
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

    fn own_potion_use<'a, K: sts2_core::FightKind<V>>(
        potion: <V as sts2_core::ContentVersion>::Potion,
        fight: &mut K::Fight<'a>,
    ) -> Option<crate::behaviour::Played<V, K>> {
        potions::own_potion_use(potion, fight)
    }

    fn own_monster_hp_bounds(
        monster: <V as sts2_core::ContentVersion>::Monster,
        model: &crate::behaviour::monsters::Model<V>,
        ascension: crate::generated::ascension::AscensionLevel,
    ) -> Option<sts2_core::rng::Interval<i32>> {
        monsters::own_hp_bounds(monster, model, ascension)
    }

    fn own_relic(relic: <V as sts2_core::ContentVersion>::Relic) -> bool {
        relics::own_relic(relic)
    }

    fn own_enchantment(enchantment: <V as sts2_core::ContentVersion>::Enchantment) -> bool {
        enchantments::own_enchantment(enchantment)
    }

    fn own_enchantment_modify(
        enchanted: sts2_core::Enchanted<V>,
        hook: sts2_core::EnchantHook,
    ) -> Option<sts2_core::Modify<i32>> {
        enchantments::own_modify(enchanted, hook)
    }

    fn own_shared_relic_modify_hook(
        state: &crate::behaviour::relics::State<V>,
        owner: sts2_core::CombatId,
        hook: sts2_core::ModifyHook<V>,
    ) -> Option<sts2_core::Modify<i32>> {
        relics::own_shared_relic_modify_hook(state, owner, hook)
    }

    fn own_shared_relic_event_hook(
        state: &mut crate::behaviour::relics::State<V>,
        owner: sts2_core::CombatId,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) -> Option<()> {
        relics::own_shared_relic_event_hook(state, owner, hook, asked)
    }

    fn own_potion_use_on_run(
        potion: <V as sts2_core::ContentVersion>::Potion,
        run: &impl sts2_core::EventView<V>,
        in_fight: bool,
    ) -> Option<sts2_core::HookEffects<V>> {
        potions::own_potion_use_on_run(potion, run, in_fight)
    }
}

/// A play or a move of this build's standing still on a question.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Paused<K: sts2_core::FightKind<V>> {
    /// One of its generated plays.
    Generated(super::generated::card_plays::Paused<K>),
    /// One of the plays it ports for itself.
    Own(cards::paused::State<K>),
    /// One of its generated moves.
    Moves(super::generated::monster_moves::Paused<K>),
    /// One of the moves it ports for itself.
    OwnMoves(monsters::paused::State<K>),
}

impl<K: sts2_core::FightKind<V>> From<super::generated::monster_moves::Paused<K>> for Paused<K> {
    fn from(paused: super::generated::monster_moves::Paused<K>) -> Self {
        Paused::Moves(paused)
    }
}

impl<K: sts2_core::FightKind<V>> From<super::generated::card_plays::Paused<K>> for Paused<K> {
    fn from(paused: super::generated::card_plays::Paused<K>) -> Self {
        Paused::Generated(paused)
    }
}
