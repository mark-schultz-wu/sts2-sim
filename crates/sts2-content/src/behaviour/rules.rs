//! What a build's `Hooks` and `Rules` do, for every build: one function per
//! method, which the `rules!` macro in `build.rs` forwards each build's
//! method to.

use crate::ids::{AfflictionIds as _, CardIds as _, EventIds as _, RelicIds as _};

pub(crate) fn power_noted<V: crate::Build>(state: &mut V::PowerState, note: i32) {
    crate::behaviour::powers::noted::<V>(state, note);
}

pub(crate) fn power_kept_card<V: crate::Build>(
    state: &mut V::PowerState,
    kept: sts2_core::CardId<V>,
) {
    crate::behaviour::powers::kept_card::<V>(state, kept);
}

pub(crate) fn modify_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    listener: sts2_core::Listener<'_, V, P>,
    hook: sts2_core::ModifyHook<V>,
) -> sts2_core::Modify<i32> {
    match listener {
        sts2_core::Listener::Power {
            power,
            amount,
            owner,
            holder,
            applier,
            monster,
            on_turn_start,
            state,
            history,
            fight,
        } => {
            let held = crate::behaviour::powers::Held {
                owner,
                holder,
                applier,
                monster,
                on_turn_start,
                history,
                fight,
            };
            crate::behaviour::powers::modify_hook::<V>(power, amount, held, state, hook)
        }
        sts2_core::Listener::Relic { state, fight, .. } => {
            crate::behaviour::relics::modify_hook(state, fight, hook)
        }
        // The fight asks only a monster whose class overrides the hook,
        // and no monster this build ports overrides one of these.
        sts2_core::Listener::Monster { monster, .. } => {
            unimplemented!("what {monster:?} does for {hook:?}")
        }
        // The fight asks only a card whose class overrides the hook, and
        // no card in this build overrides one of these.
        sts2_core::Listener::Card { card, .. } => {
            unimplemented!("what {card:?} does for {hook:?}")
        }
        // No potion in this build overrides a number's hook.
        sts2_core::Listener::Potion(potion) => {
            unimplemented!("what {potion:?} does for {hook:?}")
        }
        sts2_core::Listener::Modifier(modifier) => {
            crate::behaviour::modifiers::modify_hook(modifier, hook)
        }
    }
}

pub(crate) fn decide_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    listener: sts2_core::Listener<'_, V, P>,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    match listener {
        sts2_core::Listener::Power {
            power,
            amount,
            owner,
            holder,
            applier,
            monster,
            on_turn_start,
            state,
            history,
            fight,
            ..
        } => {
            let held = crate::behaviour::powers::Held {
                owner,
                holder,
                applier,
                monster,
                on_turn_start,
                history,
                fight,
            };
            crate::behaviour::powers::decide_hook::<V>(power, amount, held, state, hook)
        }
        sts2_core::Listener::Relic { state, belt, .. } => {
            crate::behaviour::relics::decide_hook(state, belt, hook)
        }
        sts2_core::Listener::Potion(potion) => crate::behaviour::potions::decide_hook(potion, hook),
        sts2_core::Listener::Modifier(modifier) => {
            crate::behaviour::modifiers::decide_hook(modifier, hook)
        }
        sts2_core::Listener::Card {
            card,
            pile,
            history,
            ..
        } => crate::behaviour::cards::decide_hook::<V>(card, pile, history, hook),
        // A raised hook reaches only a listener that overrides it, and no
        // monster this build ports overrides one of these.
        sts2_core::Listener::Monster { .. } => match hook.model_hook() {
            Some(_) => unimplemented!("a relic, a monster or a card deciding {hook:?}"),
            None => sts2_core::HookResponse::DoesNotImplement,
        },
    }
}

pub(crate) fn power_event_hook<V: crate::Build>(
    power: sts2_core::EventPower<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let sts2_core::EventPower {
        power,
        amount,
        owner,
        holder,
        applier,
        monster,
        on_turn_start,
        state,
        history,
        fight,
    } = power;
    let held = crate::behaviour::powers::Held {
        owner,
        holder,
        applier,
        monster,
        on_turn_start,
        history,
        fight,
    };
    crate::behaviour::powers::event_hook::<V>(power, amount, held, state, hook, asked)
}

pub(crate) fn keyword_while_afflicted<V: crate::Build>(
    affliction: V::Affliction,
) -> Option<crate::generated::cards::CardKeyword> {
    use crate::generated::cards::CardKeyword;

    match affliction {
        __c0 if __c0 == V::Affliction::HEXED => Some(CardKeyword::Ethereal),
        _ => None,
    }
}

pub(crate) fn card_eternal<V: crate::Build>(card: &sts2_core::CardIdent<V>) -> bool {
    crate::behaviour::enchantments::card_eternal(card)
}

pub(crate) fn enchanted_changes<V: crate::Build>(
    enchantment: V::Enchantment,
) -> &'static [sts2_core::Change<V>] {
    crate::behaviour::enchantments::enchanted_changes(enchantment)
}

pub(crate) fn card_result_pile<V: crate::Build>(
    card: V::Card,
    base: (Option<sts2_core::Pile>, sts2_core::PilePosition),
) -> sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)> {
    crate::behaviour::cards::result_pile::<V>(card, base)
}

pub(crate) fn write_deck_card<V: crate::Build>(
    card: &mut sts2_core::DeckCard<V>,
    write: crate::behaviour::DeckWrite,
) {
    crate::behaviour::cards::write_deck_card::<V>(card, write);
}

pub(crate) fn card_is_playable<V: crate::Build>(
    card: V::Card,
    fight: &impl sts2_core::CardLogic,
) -> sts2_core::HookResponse<bool> {
    crate::behaviour::cards::is_playable::<V>(card, fight)
}

pub(crate) fn result_pile<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    listener: sts2_core::Listener<'_, V, P>,
    hook: sts2_core::ResultPile<V>,
) -> sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)> {
    match listener {
        sts2_core::Listener::Power {
            power,
            amount,
            holder,
            state,
            history,
            ..
        } => {
            crate::behaviour::powers::result_pile::<V>(power, amount, holder, state, history, hook)
        }
        // The fight asks only a listener whose class overrides the hook,
        // and no relic, monster or card this build ports does.
        _ => unimplemented!("a listener other than a power moving a played card"),
    }
}

pub(crate) fn deck_card_event_hook<V: crate::Build>(
    card: V::Card,
    state: &mut V::CardState,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::cards::deck_event_hook::<V>(card, state, hook, asked)
}

pub(crate) fn modifier_event_hook<V: crate::Build>(
    modifier: sts2_core::RunModifier<V>,
    state: &mut V::ModifierState,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::modifiers::event_hook(modifier, state, hook, asked)
}

pub(crate) fn modifier_ported<V: crate::Build>(modifier: V::Modifier) -> bool {
    crate::behaviour::modifiers::ported::<V>(modifier)
}

pub(crate) fn relic_event_hook<V: crate::Build>(
    state: &mut V::RelicState,
    belt: &[V::Relic],
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::relics::event_hook(state, belt, hook, asked)
}

pub(crate) fn potion_event_hook<V: crate::Build>(
    potion: V::Potion,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::potions::event_hook(potion, hook, asked)
}

pub(crate) fn monster_event_hook<V: crate::Build>(
    monster: V::Monster,
    model: &mut V::MonsterState,
    _creature: sts2_core::CombatId,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::monsters::event_hook::<V>(monster, model, hook, asked)
}

pub(crate) fn enchant_modify<V: crate::Build>(
    enchanted: sts2_core::Enchanted<V>,
    state: &V::EnchantmentState,
    hook: sts2_core::EnchantHook,
) -> sts2_core::Modify<i32> {
    crate::behaviour::enchantments::modify(enchanted, state, hook)
}

pub(crate) fn enchantment_event_hook<V: crate::Build>(
    enchanted: sts2_core::Enchanted<V>,
    id: sts2_core::CardId<V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::enchantments::event_hook(enchanted, id, hook, asked);
}

pub(crate) fn card_event_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    pile: Option<sts2_core::Pile>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    crate::behaviour::cards::event_hook::<V, _>(card, id, pile, printed, history, hook, asked)
}

pub(crate) fn act_layout<V: crate::Build>(act: V::Act) -> sts2_core::Layout {
    crate::behaviour::acts::layout::<V>(act)
}

pub(crate) fn ascension_deck<V: crate::Build>(
    ascension: crate::generated::ascension::AscensionLevel,
) -> Option<V::Card> {
    use crate::generated::ascension::AscensionLevel;

    ascension
        .has(AscensionLevel::AscendersBane)
        .then_some(V::Card::ASCENDERS_BANE)
}

pub(crate) fn channelled<V: crate::Build>(orb: V::Orb) -> sts2_core::Channelled<V> {
    crate::behaviour::orbs::channelled::<V>(orb)
}

pub(crate) fn orb_turn_end<V: crate::Build, F: sts2_core::Fight<V>>(
    orb: &mut sts2_core::Channelled<V>,
    fight: &mut F,
) {
    crate::behaviour::orbs::turn_end(orb, fight);
}

pub(crate) fn orb_turn_start<V: crate::Build, F: sts2_core::Fight<V>>(
    orb: &mut sts2_core::Channelled<V>,
    fight: &mut F,
) {
    crate::behaviour::orbs::turn_start(orb, fight);
}

pub(crate) fn enchantment_on_play<V: crate::Build, F: sts2_core::Fight<V>>(
    enchanted: sts2_core::Enchanted<V>,
    state: &mut V::EnchantmentState,
    fight: &mut F,
) {
    crate::behaviour::enchantments::on_play(enchanted, state, fight);
}

pub(crate) fn orb_evoke<V: crate::Build, F: sts2_core::Fight<V>>(
    orb: &sts2_core::Channelled<V>,
    fight: &mut F,
) -> sts2_core::Opponents {
    crate::behaviour::orbs::evoke(orb, fight)
}

pub(crate) fn orb_passive<V: crate::Build, F: sts2_core::Fight<V>>(
    orb: &mut sts2_core::Channelled<V>,
    fight: &mut F,
) {
    crate::behaviour::orbs::passive(orb, fight);
}

pub(crate) fn orb_passive_val<V: crate::Build>(
    orb: &sts2_core::Channelled<V>,
    modify: impl FnOnce(i32) -> i32,
) -> i32 {
    crate::behaviour::orbs::passive_val(orb, modify)
}

pub(crate) fn orb_evoke_val<V: crate::Build>(
    orb: &sts2_core::Channelled<V>,
    modify: impl FnOnce(i32) -> i32,
) -> i32 {
    crate::behaviour::orbs::evoke_val(orb, modify)
}

pub(crate) fn starting_potion_slots() -> usize {
    3
}

pub(crate) fn neow_event<V: crate::Build>() -> V::Event {
    V::Event::NEOW
}

pub(crate) fn reward_rarity<V: crate::Build>(card: V::Card) -> sts2_core::RewardRarity {
    use crate::generated::cards::CardRarity;
    match sts2_core::CardTable::rarity(card) {
        CardRarity::Common => sts2_core::RewardRarity::Common,
        CardRarity::Uncommon => sts2_core::RewardRarity::Uncommon,
        CardRarity::Rare => sts2_core::RewardRarity::Rare,
        _ => sts2_core::RewardRarity::Other,
    }
}

pub(crate) fn card_upgrade<V: crate::Build>(card: V::Card) -> &'static [sts2_core::Change<V>] {
    crate::behaviour::cards::upgrade::<V>(card)
}

pub(crate) fn on_use<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    potion: V::Potion,
    fight: &mut K::Fight<'a>,
) -> crate::behaviour::Played<V, K> {
    crate::behaviour::potions::on_use::<V, K>(potion, fight)
}

pub(crate) fn on_use_on_run<V: crate::Build>(
    potion: V::Potion,
    run: &impl sts2_core::EventView<V>,
    in_fight: bool,
) -> sts2_core::HookEffects<V> {
    crate::behaviour::potions::on_use_on_run(potion, run, in_fight)
}

pub(crate) fn on_play<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> crate::behaviour::Played<V, K> {
    crate::behaviour::cards::on_play::<V, K>(card, fight)
}

pub(crate) fn on_card_turn_end<V: crate::Build, F: sts2_core::Fight<V>>(
    card: V::Card,
    fight: &mut F,
) {
    crate::behaviour::cards::on_turn_end::<V, _>(card, fight);
}

pub(crate) fn resume_play<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    fight: &mut K::Fight<'a>,
    at: crate::behaviour::Resume<V, K>,
    chosen: sts2_core::Taken<V>,
) -> crate::behaviour::Played<V, K> {
    match (at, chosen) {
        (crate::behaviour::Resume::Card(at), taken) => {
            crate::behaviour::cards::resume::<V, K>(fight, at, taken)
        }
        (crate::behaviour::Resume::Potion(at), chosen) => {
            crate::behaviour::potions::resume::<V, K>(fight, at, chosen)
        }
        (crate::behaviour::Resume::Monster(at), sts2_core::Taken::Chosen(card)) => {
            crate::behaviour::monsters::resume(fight, at, card)
        }
        (at, chosen) => unimplemented!("{at:?} resumed with {chosen:?}"),
    }
}

/// The card whose type the fight works out is on no shelf. A merchant
/// stands outside a fight, so nothing can answer for it there either.
pub(crate) fn merchant_cards<V: crate::Build>(
    character: V::Character,
    kind: sts2_core::CardKind,
    rarity: sts2_core::RewardRarity,
    multiplayer: bool,
) -> Vec<V::Card> {
    narrowed::<V>(
        V::unlocked_cards(character, multiplayer),
        Some(kind),
        rarity,
    )
}

pub(crate) fn merchant_colorless_cards<V: crate::Build>(
    rarity: sts2_core::RewardRarity,
    multiplayer: bool,
) -> Vec<V::Card> {
    narrowed::<V>(V::unlocked_colorless_cards(multiplayer), None, rarity)
}

/// A quest card, and an event, ancient or token one, transforms within
/// the colorless pool. Any other transforms within its own, a curse or a
/// status included. The shop lists are each pool's unlocked cards, in the
/// pool's own order.
pub(crate) fn transform_options<V: crate::Build>(
    card: V::Card,
    in_combat: bool,
    multiplayer: bool,
) -> Vec<V::Card> {
    use crate::generated::cards::CardRarity;
    use sts2_core::CardTable as _;

    let rarity = card.rarity();
    let colorless = matches!(
        rarity,
        CardRarity::Event | CardRarity::Ancient | CardRarity::Token
    ) || V::card_kind(card) == Some(sts2_core::CardKind::Quest);
    let (pool, in_fight) = match (colorless, card.pool()) {
        (false, Some(pool)) => (
            <V as crate::build::Tables>::merchant_pool_cards(pool, multiplayer),
            <V as crate::build::Tables>::combat_pool_cards(pool, multiplayer),
        ),
        _ if rarity == CardRarity::Curse => (
            <V as crate::build::Tables>::unlocked_curse_cards(),
            <V as crate::build::Tables>::combat_curse_cards(),
        ),
        _ if rarity == CardRarity::Status => (
            <V as crate::build::Tables>::unlocked_status_cards(),
            <V as crate::build::Tables>::combat_status_cards(),
        ),
        _ => (
            V::unlocked_colorless_cards(multiplayer),
            V::combat_colorless_cards(multiplayer),
        ),
    };
    let any_rarity = matches!(rarity, CardRarity::Status | CardRarity::Curse);
    pool.iter()
        .copied()
        .filter(|option| {
            any_rarity
                || matches!(
                    option.rarity(),
                    CardRarity::Common | CardRarity::Uncommon | CardRarity::Rare
                )
        })
        .filter(|option| !in_combat || in_fight.contains(option))
        .filter(|option| *option != card)
        .collect()
}

pub(crate) fn can_enchant<V: crate::Build>(
    enchantment: V::Enchantment,
    card: &sts2_core::CardIdent<V>,
) -> bool {
    crate::behaviour::enchantments::can_enchant(enchantment, card)
}

pub(crate) fn enchant<V: crate::Build>(
    enchantment: V::Enchantment,
    amount: u8,
    card: &mut sts2_core::CardIdent<V>,
) {
    crate::behaviour::enchantments::enchant(enchantment, amount, card);
}

pub(crate) fn should_start_at_bottom_of_draw_pile<V: crate::Build>(
    enchantment: V::Enchantment,
) -> sts2_core::HookResponse<bool> {
    crate::behaviour::enchantments::should_start_at_bottom_of_draw_pile::<V>(enchantment)
}

pub(crate) fn monster_health_after_added<V: crate::Build>(
    monster: V::Monster,
    ascension: crate::generated::ascension::AscensionLevel,
    rolled: i32,
    teammates: &[i32],
) -> Option<i32> {
    crate::behaviour::monsters::health_after_added::<V>(monster, ascension, rolled, teammates)
}

pub(crate) fn encounter_gold_proportion<V: crate::Build>(
    encounter: V::Encounter,
    state: &V::EncounterState,
    escaped: &[V::Monster],
) -> Option<f32> {
    crate::behaviour::encounters::gold_proportion::<V>(encounter, state, escaped)
}

pub(crate) fn perform_power_death<V: crate::Build, F: sts2_core::Fight<V>>(
    power: V::Power,
    holder: sts2_core::CombatId,
    fight: &mut F,
) {
    crate::behaviour::powers::perform_death::<V, _>(power, holder, fight)
}

pub(crate) fn monster_hp_after_added<V: crate::Build>(
    monster: V::Monster,
    model: &V::MonsterState,
    hp: i32,
) -> Option<i32> {
    crate::behaviour::monsters::hp_after_added::<V>(monster, model, hp)
}

pub(crate) fn monster_hp_bounds<V: crate::Build>(
    monster: V::Monster,
    model: &V::MonsterState,
    ascension: crate::generated::ascension::AscensionLevel,
) -> sts2_core::rng::Interval<i32> {
    if let Some(bounds) = V::own_monster_hp_bounds(monster, model, ascension) {
        return bounds;
    }
    let data = sts2_core::Table::data(monster);
    sts2_core::rng::Interval::new(
        data.min_hp.at(ascension as u8).into(),
        data.max_hp.at(ascension as u8).into(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn roll_monster_move<
    'a,
    V: crate::Build,
    R: sts2_core::rng::GameRng,
    P: sts2_core::Printed<V> + ?Sized + 'a,
>(
    monster: V::Monster,
    ascension: crate::generated::ascension::AscensionLevel,
    spawn: sts2_core::MonsterSpawn<V::Encounter>,
    powers: impl IntoIterator<Item = sts2_core::Listener<'a, V, P>>,
    advertised: impl Fn(V::MoveId) -> bool,
    state: &V::MonsterState,
    machine: &mut sts2_core::MoveMachine,
    rng: &mut R,
) -> Result<Option<sts2_core::NextMove<V>>, sts2_core::ContentError> {
    roll_monster_move_from::<V, _, _>(
        monster,
        ascension as u8,
        spawn,
        powers,
        advertised,
        state,
        machine,
        rng,
    )
}

pub(crate) fn follow_up<V: crate::Build>(
    monster: V::Monster,
    machine: &sts2_core::MoveMachine,
) -> u8 {
    use crate::generated::monsters::{FollowUp, State};

    if machine.built {
        unimplemented!("the state after {monster:?}'s built state");
    }
    match sts2_core::Table::data(monster)
        .states
        .get(usize::from(machine.at))
    {
        Some(State::Move(held)) => match held.follow_up {
            FollowUp::Next(next) | FollowUp::OncePerformed(next) => next,
            FollowUp::Never => unimplemented!("the state after {monster:?}'s last move"),
        },
        _ => unimplemented!("the state after {monster:?}'s branch"),
    }
}

pub(crate) fn can_leave_current_move<V: crate::Build>(
    monster: V::Monster,
    machine: &sts2_core::MoveMachine,
) -> bool {
    use crate::generated::monsters::{FollowUp, State};

    // A built state is one nothing names, and both callers that build one
    // set `MustPerformOnceBeforeTransitioning`.
    if machine.built {
        return machine.performed;
    }
    let data = sts2_core::Table::data(monster);
    let State::Move(held) = data
        .states
        .get(usize::from(machine.at))
        .copied()
        .unwrap_or_else(|| unimplemented!("{monster:?} standing outside its own graph"))
    else {
        // A branch state is not a move, and nothing pins one.
        return true;
    };
    !matches!(held.follow_up, FollowUp::OncePerformed(_)) || machine.performed
}

pub(crate) fn set_move_immediate<V: crate::Build>(
    monster: V::Monster,
    ascension: crate::generated::ascension::AscensionLevel,
    move_id: V::MoveId,
    force: bool,
    machine: &mut sts2_core::MoveMachine,
) -> Result<sts2_core::NextMove<V>, sts2_core::MovePinned> {
    use crate::generated::monsters::State;

    if !force && !V::can_leave_current_move(monster, machine) {
        return Err(sts2_core::MovePinned);
    }
    let data = sts2_core::Table::data(monster);
    let at = crate::behaviour::monsters::state_of::<V>(monster, move_id, machine.at)
        .unwrap_or_else(|_| {
            unimplemented!("{monster:?} sent to {move_id:?}, which its graph has no state for")
        });
    // `ForceCurrentState`, which does not write the state into the log.
    // The named state takes the place of any state a caller built.
    machine.built = false;
    machine.enter(at);
    let State::Move(held) = data.states[usize::from(at)] else {
        unimplemented!("{monster:?} sent to {move_id:?}, which is not a move")
    };
    Ok(intents::<V>(
        held,
        ascension as u8,
        || unimplemented!("{monster:?} sent to {move_id:?}, whose damage the fight decides"),
        || unimplemented!("{monster:?} sent to {move_id:?}, whose hits the fight decides"),
    )
    .expect("a move state publishes its own intents"))
}

pub(crate) fn opening_move_machine<V: crate::Build>(
    monster: V::Monster,
    opens_on: Option<V::MoveId>,
) -> sts2_core::MoveMachine {
    let data = sts2_core::Table::data(monster);
    // `GenerateMoveStateMachine` takes the state it is constructed on. A
    // roster that wrote onto this monster names it; the rest open where
    // the table says.
    let at = match opens_on {
        None => data.initial,
        Some(move_id) => crate::behaviour::monsters::state_of::<V>(monster, move_id, data.initial)
            .unwrap_or_else(|_| {
                unimplemented!(
                    "{monster:?} opening on {move_id:?}, which its graph has no state for"
                )
            }),
    };
    // `ShouldAppearInLogs`, which a branch state answers no to. A graph
    // opening on a move has that move in its history already.
    let logged = matches!(
        data.states.get(usize::from(at)),
        Some(crate::generated::monsters::State::Move(_))
    );
    sts2_core::MoveMachine::new(at, logged)
}

pub(crate) fn monster_starting_powers<V: crate::Build>(
    monster: V::Monster,
    ascension: crate::generated::ascension::AscensionLevel,
    side: crate::generated::runstate::CombatSide,
    model: &V::MonsterState,
) -> impl Iterator<Item = (V::Power, sts2_core::PowerAmount<i32>)> {
    crate::behaviour::monsters::starting_powers::<V>(monster, ascension, side, model).into_iter()
}

pub(crate) fn monster_starting_opponent_powers<V: crate::Build>(
    monster: V::Monster,
    _ascension: crate::generated::ascension::AscensionLevel,
) -> impl Iterator<Item = (V::Power, sts2_core::PowerAmount<i32>)> {
    crate::behaviour::monsters::starting_opponent_powers::<V>(monster).into_iter()
}

pub(crate) fn computed_card_target<V: crate::Build>(
    card: V::Card,
    powers: &[sts2_core::Stack<V>],
) -> crate::generated::cards::TargetType {
    crate::behaviour::cards::computed_target::<V>(card, powers)
}

pub(crate) fn deck_card_next_event<V: crate::Build>(
    card: V::Card,
    act: usize,
    event: V::Event,
) -> V::Event {
    crate::behaviour::cards::next_event::<V>(card, act, event)
}

pub(crate) fn computed_potion_target<V: crate::Build>(
    potion: V::Potion,
    in_combat: bool,
) -> crate::generated::cards::TargetType {
    crate::behaviour::potions::computed_target::<V>(potion, in_combat)
}

pub(crate) fn computed_potion_check<V: crate::Build>(
    potion: V::Potion,
    in_combat: bool,
    merchant: bool,
) -> bool {
    crate::behaviour::potions::computed_check::<V>(potion, in_combat, merchant)
}

pub(crate) fn calculated_card_var<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    key: V::CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V>,
) -> i32 {
    crate::behaviour::cards::calculated::<V, _>(card, key, printed, owner)
}

pub(crate) fn perform_move<V: crate::Build, F: sts2_core::Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    fight: &mut F,
) -> crate::behaviour::Played<V, F::Kind> {
    crate::behaviour::monsters::perform::<V, _>(monster, move_id, fight)
}

pub(crate) fn intent_repeats_now<V: crate::Build>(
    monster: V::Monster,
    move_id: V::MoveId,
    model: &V::MonsterState,
) -> Option<u32> {
    use crate::generated::monsters::State;

    let held = sts2_core::Table::data(monster)
        .states
        .iter()
        .find_map(|state| match state {
            State::Move(held) if held.id == move_id => Some(held),
            _ => None,
        })?;
    match held.intents.attack?.repeats {
        crate::Field::Computed => Some(crate::behaviour::monsters::computed_repeats::<V>(
            monster, move_id, model,
        )),
        crate::Field::Constant(_) => None,
    }
}

pub(crate) fn intent_damage_now<V: crate::Build>(
    monster: V::Monster,
    move_id: V::MoveId,
    ascension: V::AscensionLevel,
    model: &V::MonsterState,
) -> Option<i32> {
    use crate::generated::monsters::State;

    let held = sts2_core::Table::data(monster)
        .states
        .iter()
        .find_map(|state| match state {
            State::Move(held) if held.id == move_id => Some(held),
            _ => None,
        })?;
    match held.intents.attack?.damage {
        crate::Field::Computed => {
            crate::behaviour::monsters::model_damage::<V>(monster, move_id, ascension as u8, model)
        }
        crate::Field::Constant(_) => None,
    }
}

pub(crate) fn resume_move<V: crate::Build, F: sts2_core::Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    at: crate::behaviour::Resume<V, F::Kind>,
    fight: &mut F,
) -> crate::behaviour::Played<V, F::Kind> {
    let crate::behaviour::Resume::Monster(at) = at else {
        unreachable!("{at:?} resumes no move")
    };
    crate::behaviour::monsters::resume_move::<V, _>(monster, move_id, at, fight)
}

pub(crate) fn random_encounter_monsters<V: crate::Build, R: sts2_core::rng::GameRng>(
    encounter: V::Encounter,
    rng: &mut R,
) -> Result<Vec<sts2_core::Spawned<V>>, sts2_core::ContentError> {
    Ok(crate::behaviour::encounters::generate(encounter, rng))
}

pub(crate) fn the_architect<V: crate::Build>() -> V::Event {
    V::Event::THE_ARCHITECT
}

pub(crate) fn event_allowed<V: crate::Build>(
    event: V::Event,
    run: &impl sts2_core::EventView<V>,
) -> bool {
    crate::behaviour::events::allowed::<V>(event, run)
}

pub(crate) fn enter_event<V: crate::Build, R: sts2_core::rng::GameRng>(
    event: V::Event,
    run: &impl sts2_core::EventView<V>,
    rng: &mut R,
) -> V::EventState {
    crate::behaviour::events::enter::<V, _>(event, run, rng)
}

pub(crate) fn set_up_event<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut V::EventState,
    run: &impl sts2_core::EventView<V>,
    rewards: &mut R,
) {
    crate::behaviour::events::set_up::<V, _>(state, run, rewards);
}

pub(crate) fn event_relics_pulled<V: crate::Build>(
    state: &V::EventState,
) -> &'static [sts2_core::RelicPull<V>] {
    crate::behaviour::events::relics_pulled::<V>(state)
}

pub(crate) fn hand_event_pulled_relics<V: crate::Build>(
    state: &mut V::EventState,
    relics: &[V::Relic],
) {
    crate::behaviour::events::hand_pulled_relics::<V>(state, relics);
}

pub(crate) fn event_holds_potions<V: crate::Build>(state: &V::EventState) -> bool {
    crate::behaviour::events::holds_potions::<V>(state)
}

pub(crate) fn observe_event<V: crate::Build>(
    state: &V::EventState,
    run: &impl sts2_core::EventView<V>,
) -> sts2_core::Room<V> {
    crate::behaviour::events::observe::<V>(state, run)
}

pub(crate) fn choose_event<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut V::EventState,
    option: u8,
    run: &impl sts2_core::EventView<V>,
    rng: &mut R,
) -> sts2_core::HookEffects<V> {
    crate::behaviour::events::choose::<V, _>(state, option, run, rng)
}

pub(crate) fn stock_event<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut V::EventState,
    shops: &mut R,
) {
    crate::behaviour::events::stock::<V, _>(state, shops)
}

pub(crate) fn buy_in_event<V: crate::Build>(
    state: &mut V::EventState,
    index: u8,
) -> sts2_core::HookEffects<V> {
    crate::behaviour::events::buy::<V>(state, index)
}

pub(crate) fn event_question<V: crate::Build>(
    state: &V::EventState,
) -> Option<sts2_core::Question<V>> {
    crate::behaviour::events::question::<V>(state)
}

pub(crate) fn answer_event<V: crate::Build>(
    state: &mut V::EventState,
    answer: &sts2_core::Answer,
) -> sts2_core::HookEffects<V> {
    crate::behaviour::events::answer::<V>(state, answer)
}

pub(crate) fn resume_event<V: crate::Build, R: sts2_core::rng::GameRng>(
    state: &mut V::EventState,
    run: &impl sts2_core::EventView<V>,
    rng: &mut R,
) -> sts2_core::HookEffects<V> {
    crate::behaviour::events::resume::<V, _>(state, run, rng)
}

pub(crate) fn resume_event_from_fight<V: crate::Build>(
    state: &mut V::EventState,
    escaped: bool,
) -> sts2_core::HookEffects<V> {
    crate::behaviour::events::resume_from_fight::<V>(state, escaped)
}

pub(crate) fn event_rewards<V: crate::Build>(
    state: &V::EventState,
) -> Vec<sts2_core::CustomReward<V>> {
    crate::behaviour::events::rewards::<V>(state)
}

/// The curses, the blessings, then the six options a coin flip picks
/// from, which `AllPossibleOptions` adds one by one in this order.
pub(crate) fn neow_options<V: crate::Build>() -> impl Iterator<Item = V::Relic> {
    let added: [V::Relic; 6] = [
        V::Relic::LAVA_ROCK,
        V::Relic::NEOWS_TALISMAN,
        V::Relic::NUTRITIOUS_OYSTER,
        V::Relic::POMANDER,
        V::Relic::SMALL_CAPSULE,
        V::Relic::STONE_HUMIDIFIER,
    ];
    V::neow_curses()
        .iter()
        .chain(V::neow_positives())
        .copied()
        .chain(added)
}

/// `RelicModel.IsAllowedAtNeow`, which is `IsAllowed` on the run as Neow
/// meets it, on its first floor.
///
/// Kaleidoscope and Scroll Boxes also ask about unlocks, which a run here
/// always has. It is set up with everything unlocked, so that no scenario
/// depends on a save file.
pub(crate) fn allowed_at_neow<V: crate::Build>(relic: V::Relic, players: usize) -> bool {
    relic_allowed::<V>(relic, 1, players)
}

pub(crate) fn relic_allowed<V: crate::Build>(
    relic: V::Relic,
    floor: usize,
    players: usize,
) -> bool {
    // `IsBeforeAct3TreasureChest`.
    let before_act_3_chest = floor < if players > 1 { 38 } else { 41 };
    match relic {
        __c0 if __c0 == V::Relic::SILVER_CRUCIBLE || __c0 == V::Relic::WINGED_BOOTS => players == 1,
        __c0 if __c0 == V::Relic::MASSIVE_SCROLL => players > 1,
        // `LastingCandy` also refuses an Ironclad on a profile's first
        // run, and a run here has every unlock, which counts runs past
        // any first.
        __c0 if __c0 == V::Relic::AMETHYST_AUBERGINE
            || __c0 == V::Relic::BOOK_OF_FIVE_RINGS
            || __c0 == V::Relic::BOWLER_HAT
            || __c0 == V::Relic::DRAGON_FRUIT
            || __c0 == V::Relic::FROZEN_EGG
            || __c0 == V::Relic::GIRYA
            || __c0 == V::Relic::JUZU_BRACELET
            || __c0 == V::Relic::LASTING_CANDY
            || __c0 == V::Relic::LUCKY_FYSH
            || __c0 == V::Relic::MEAL_TICKET
            || __c0 == V::Relic::MOLTEN_EGG
            || __c0 == V::Relic::OLD_COIN
            || __c0 == V::Relic::PLANISPHERE
            || __c0 == V::Relic::SHOVEL
            || __c0 == V::Relic::TOXIC_EGG
            || __c0 == V::Relic::WHITE_BEAST_STATUE
            || __c0 == V::Relic::WHITE_STAR =>
        {
            before_act_3_chest
        }
        _ => true,
    }
}

pub(crate) fn run_rng_types() -> &'static [crate::generated::runstate::RunRngType] {
    &crate::generated::runstate::RunRngType::ALL
}

pub(crate) fn player_rng_types() -> &'static [crate::generated::runstate::PlayerRngType] {
    &crate::generated::runstate::PlayerRngType::ALL
}

pub(crate) fn enemy_side() -> crate::generated::runstate::CombatSide {
    crate::generated::runstate::CombatSide::Enemy
}

/// Walk to the move a monster makes next, which is `RollMove`.
///
/// `FindNextMoveState` returns early while the machine is pinned. Two things
/// pin it. One is the move it started on, until any move has been performed.
/// The other is a move the graph holds it on until that move has been.
///
/// Otherwise it steps until it lands on a move, and logs the move it lands on.
#[allow(clippy::too_many_arguments)]
fn roll_monster_move_from<
    'a,
    V: crate::Build,
    R: sts2_core::rng::GameRng,
    P: sts2_core::Printed<V> + ?Sized + 'a,
>(
    monster: V::Monster,
    ascension: u8,
    spawn: sts2_core::MonsterSpawn<V::Encounter>,
    powers: impl IntoIterator<Item = sts2_core::Listener<'a, V, P>>,
    advertised: impl Fn(V::MoveId) -> bool,
    model: &crate::behaviour::monsters::Model<V>,
    machine: &mut sts2_core::MoveMachine,
    rng: &mut R,
) -> Result<Option<sts2_core::NextMove<V>>, sts2_core::ContentError> {
    use crate::generated::monsters::{FollowUp, State};

    // The powers, read the first time a conditional state asks for them. Few
    // monsters' graphs hold one.
    let mut powers = Some(powers);
    let mut read = Vec::new();
    let data = sts2_core::Table::data(monster);
    let state = |at: u8| {
        data.states
            .get(usize::from(at))
            .copied()
            .ok_or(sts2_core::ContentError::InvalidMonsterState { index: at })
    };

    // A machine standing on a built state performs that state, and goes back
    // to the one it follows on to once it has. Until then it advertises what
    // the caller built, which the walk has nothing to say about.
    if machine.built {
        if !machine.performed {
            return Ok(None);
        }
        machine.resume();
        let State::Move(held) = state(machine.at)? else {
            return Err(sts2_core::ContentError::InvalidMonsterState { index: machine.at });
        };
        return intents::<V>(
            held,
            ascension,
            || {
                read.extend(powers.take().into_iter().flatten());
                crate::behaviour::monsters::computed_damage::<V, _>(
                    monster, held.id, ascension, &read, model,
                )
            },
            || crate::behaviour::monsters::computed_repeats::<V>(monster, held.id, model),
        )
        .map(Some);
    }

    if !pinned::<V>(state(machine.at)?, machine) {
        loop {
            let next = match state(machine.at)? {
                State::Move(held) => match held.follow_up {
                    FollowUp::Next(at) | FollowUp::OncePerformed(at) => at,
                    // `MoveState.GetNextState` throws on a missing follow-up.
                    FollowUp::Never => unimplemented!("a move with no follow-up"),
                },
                State::Random(branches) => branch(branches, machine, rng, |at| {
                    crate::behaviour::monsters::weight::<V>(monster, at, spawn, model, &advertised)
                })?,
                State::Conditional(_) => {
                    read.extend(powers.take().into_iter().flatten());
                    crate::behaviour::monsters::conditional::<V, _>(
                        monster, machine.at, spawn, &read, model,
                    )?
                }
            };
            machine.enter(next);
            if let State::Move(_) = state(machine.at)? {
                machine.log.push(machine.at);
                break;
            }
        }
    }

    let State::Move(held) = state(machine.at)? else {
        return Err(sts2_core::ContentError::InvalidMonsterState { index: machine.at });
    };
    intents::<V>(
        held,
        ascension,
        || {
            read.extend(powers.take().into_iter().flatten());
            crate::behaviour::monsters::computed_damage::<V, _>(
                monster, held.id, ascension, &read, model,
            )
        },
        || crate::behaviour::monsters::computed_repeats::<V>(monster, held.id, model),
    )
    .map(Some)
}

/// Whether the machine cannot leave the state it stands on.
fn pinned<V: crate::Build>(
    state: crate::generated::monsters::State<V::MoveId>,
    machine: &sts2_core::MoveMachine,
) -> bool {
    use crate::generated::monsters::{FollowUp, State};

    let State::Move(held) = state else {
        return false;
    };
    // `CanTransitionAway`, then the machine-wide "no move performed yet".
    matches!(held.follow_up, FollowUp::OncePerformed(_)) && !machine.performed
        || !machine.ever_performed
}

/// One branch of a random node, which is `RandomBranchState.GetNextState`.
///
/// One draw, scaled by the total weight, walking the branches in order. A
/// branch its own history bars weighs nothing, which is `GetStateWeight`.
/// `computed` is the weight of the branch at a position whose `weightLambda`
/// reads the fight.
fn branch<R: sts2_core::rng::GameRng>(
    branches: &'static [crate::generated::monsters::Branch],
    machine: &sts2_core::MoveMachine,
    rng: &mut R,
    computed: impl Fn(usize) -> f32,
) -> Result<u8, sts2_core::ContentError> {
    use crate::Field;
    use sts2_core::rng::Interval;

    let weight = |(at, branch): (usize, &crate::generated::monsters::Branch)| match branch.weight {
        _ if !allowed(branch, machine) => 0.0,
        Field::Constant(weight) => weight,
        Field::Computed => computed(at),
    };

    let total: f32 = branches.iter().enumerate().map(weight).sum();
    let range = Interval::try_new(0.0, total)
        .map_err(|_| sts2_core::ContentError::InvalidMonsterWeights)?;
    let mut draw = rng.next_f32_range(range);
    for branch in branches.iter().enumerate() {
        let target = branch.1.target;
        draw -= weight(branch);
        if draw <= 0.0 {
            return Ok(target);
        }
    }
    Err(sts2_core::ContentError::InvalidMonsterWeights)
}

/// Whether a branch's own history leaves it takeable.
fn allowed(branch: &crate::generated::monsters::Branch, machine: &sts2_core::MoveMachine) -> bool {
    use crate::generated::monsters::Repeat;

    let recent = |n: usize| machine.log.recent(n);
    match branch.repeat {
        Repeat::Forever => true,
        // Barred while the last `n` log entries are all this branch.
        Repeat::InARow(n) => {
            let n = usize::from(n.get());
            machine.log.len() < n || recent(n).any(|at| at != branch.target)
        }
        // Barred while this branch is anywhere in the last `n`.
        Repeat::Cooldown(n) => recent(usize::from(n.get())).all(|at| at != branch.target),
        Repeat::Once => !machine.log.contains(branch.target),
    }
}

/// The intents a move state publishes, at this ascension.
///
/// `computed` works out a damage the table leaves to the fight, and
/// `computed_repeats` a hit count, which the monster's own port answers.
fn intents<V: crate::Build>(
    held: crate::generated::monsters::Move<V::MoveId>,
    ascension: u8,
    computed: impl FnOnce() -> i32,
    computed_repeats: impl FnOnce() -> u32,
) -> Result<sts2_core::NextMove<V>, sts2_core::ContentError> {
    use crate::Field;
    use crate::generated::monsters::IntentKind;

    let attack = match held.intents.attack {
        None => None,
        Some(attack) => {
            let damage = match attack.damage {
                // The table holds each column's own width, and a fight counts
                // in the build's own types. Widening here is where they meet.
                Field::Constant(damage) => i32::from(damage.at(ascension)),
                Field::Computed => computed(),
            };
            let repeats = match attack.repeats {
                Field::Constant(repeats) => u32::from(repeats.at(ascension)),
                Field::Computed => computed_repeats(),
            };
            Some(sts2_core::Attack { damage, repeats })
        }
    };
    let intents = held
        .intents
        .kinds
        .iter()
        .map(|kind| sts2_core::Intent {
            kind,
            attack: matches!(kind, IntentKind::Attack | IntentKind::DeathBlow)
                .then_some(attack)
                .flatten(),
        })
        .collect();
    Ok(sts2_core::NextMove {
        id: held.id,
        intents,
    })
}

/// One merchant pool, narrowed to a rarity and optionally to one card type.
///
/// `CreateForMerchant` filters the pool it was handed, and a filter keeps the
/// order. So an index into this means what an index into the game's own query
/// means.
fn narrowed<V: crate::Build>(
    pool: &'static [V::Card],
    kind: Option<sts2_core::CardKind>,
    rarity: sts2_core::RewardRarity,
) -> Vec<V::Card> {
    pool.iter()
        .copied()
        .filter(|&card| {
            <V as sts2_core::Rules>::reward_rarity(card) == rarity
                && kind.is_none_or(|kind| V::card_kind(card) == Some(kind))
        })
        .collect()
}
