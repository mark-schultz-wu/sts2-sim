//! The cards whose rules this build ports for itself, which the shared cards
//! hand over through [`crate::build::OwnRules`].

use crate::behaviour::Played;
use crate::behaviour::cards::Listed;
use crate::behaviour::paused::{Ask, Lent};
use sts2_core::{Amount, Effect, Fight, FightKind, Scaling};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{
    CardVar, Cards, DefectCard, EventCard, IroncladCard, NecrobinderCard, QuestCard, RegentCard,
    SilentCard, TokenCard,
};
use super::super::generated::powers::Power;
use crate::generated::runstate::MapPointType;

type Number = i32;

/// `Tracking`'s `PowerCmd.Apply<TrackingPower>(…, 50m, …)`, which it prints
/// nowhere.
const TRACKING: Number = 50;

/// Whether a shared card's in-fight hooks are this build's own.
pub(crate) fn own_card_hooks(card: Cards) -> bool {
    matches!(
        card,
        Cards::Defect(DefectCard::RocketPunch) | Cards::Ironclad(IroncladCard::Midnight)
    )
}

/// `GetResultLocationForCardPlay`, for a card this build ports for itself.
pub(crate) fn result_pile(
    card: Cards,
    base: (Option<sts2_core::Pile>, sts2_core::PilePosition),
) -> Option<sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)>> {
    use sts2_core::{HookResponse, Pile, PilePosition};

    match card {
        // `ShiningStrike`: the top of the draw pile where the base class
        // discards. v0.107.1's put itself there in its own play.
        Cards::Regent(RegentCard::ShiningStrike) => Some(HookResponse::Decision(match base {
            (Some(Pile::Discard), _) => (Some(Pile::Draw), PilePosition::Top),
            base => base,
        })),
        _ => None,
    }
}

/// A card's `EventHook` in a fight, for one this build ports for itself.
pub(crate) fn own_card_event_hook(
    card: Cards,
    id: sts2_core::CardId<V>,
    owner: sts2_core::CombatId,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) -> Option<()> {
    use sts2_core::Rules as _;

    // `EnergyCost.AddThisCombat(-amount)`.
    let cheaper = |amount: usize| sts2_core::HookEffect::ModifySelf {
        change: sts2_core::Change::ChangeCost(
            sts2_core::CostPool::Energy,
            -Number::try_from(amount).expect("a count of exhausts fits a game number"),
        ),
        lasting: sts2_core::Lasting::ThisFight,
    };
    match (card, hook) {
        // `Midnight.AfterCardEnteredCombat`: itself, unless a clone, one
        // cheaper for each card any player exhausted this fight.
        (
            Cards::Ironclad(IroncladCard::Midnight),
            sts2_core::EventHook::CardEnteredCombat { at, clone, .. },
        ) => {
            // `AddThisCombat` adds nothing for none.
            let exhausted = history.exhausted_this_fight();
            if at.id() == id && !clone && exhausted > 0 {
                asked.push(cheaper(exhausted));
            }
            Some(())
        }
        // `Midnight.AfterCardExhausted`: one cheaper for any card exhausted,
        // any player's.
        (Cards::Ironclad(IroncladCard::Midnight), sts2_core::EventHook::CardExhausted { .. }) => {
            asked.push(cheaper(1));
            Some(())
        }
        // `RocketPunch.AfterCardGeneratedForCombat`: a Status its owner made
        // takes one off its cost until it is played. v0.107.1's set the cost
        // to nothing.
        (
            Cards::Defect(DefectCard::RocketPunch),
            sts2_core::EventHook::CardGeneratedForCombat { card, creator, .. },
        ) => {
            // `creator == Owner && card.Owner == Owner`: a Status its own
            // player made, for that player.
            if creator == Some(owner) && V::card_kind(card) == Some(sts2_core::CardKind::Status) {
                asked.push(sts2_core::HookEffect::ModifySelf {
                    change: sts2_core::Change::ChangeCost(sts2_core::CostPool::Energy, -1),
                    lasting: sts2_core::Lasting::UntilPlayed,
                });
            }
            Some(())
        }
        _ => None,
    }
}

/// What a card of this build's own does when it is played, or nothing for a
/// shared one.
pub(crate) fn on_play<'a, K: sts2_core::FightKind<V>>(
    card: Cards,
    fight: &mut K::Fight<'a>,
) -> Option<Played<V, K>> {
    match card {
        // `ExpectAFight`: as much block as its `CalculatedBlock`, a move's.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::CalculatedBlock),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Some(Played::Done)
        }
        // `Plot`: its `Cards` as `DrawCardsNextTurnPower` on each living
        // player, its owner too.
        Cards::Regent(RegentCard::Plot) => {
            let cards = fight.power_var(CardVar::Cards);
            for player in crate::behaviour::cards::teammates::<V, _>(fight) {
                fight.apply_power(
                    player,
                    Power::DrawCardsNextTurnPower,
                    sts2_core::PowerAmount::Counted(cards),
                );
            }
            Some(Played::Done)
        }
        // `Constellation`: its `Cards` drawn by the ally it is pointed at,
        // then its energy to them, then its block on them.
        Cards::Regent(RegentCard::Constellation) => {
            let target = fight.target();
            let cards = fight.var(CardVar::Cards);
            let energy = fight.var(CardVar::Energy);
            fight.for_player(target, |fight| {
                fight.draw(cards);
                fight.modify_energy(energy);
            });
            let block = fight.resolve(Effect::GainBlock {
                recipient: target,
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Some(Played::Done)
        }
        // `Outrage`: its damage to the target, then a clone of itself into
        // each living player's discard pile, its owner's too.
        Cards::Ironclad(IroncladCard::Outrage) => {
            strike(fight);
            fight.clone_this_for_each_player(sts2_core::Pile::Discard);
            Some(Played::Done)
        }
        // `ForgottenRitual`: its energy, every play. v0.107.1's waited on a
        // card exhausted that turn, which the shared port reads, so this build
        // plays its generated script.
        Cards::Ironclad(IroncladCard::ForgottenRitual) => {
            Some(crate::behaviour::cards::generated::<V, K>(card, fight))
        }
        // `Outbreak`: its Poison onto each enemy that can be hit, from its
        // owner, then `PoisonPower.Trigger` on each that carries Poison.
        Cards::Silent(SilentCard::Outbreak) => {
            let poison = fight.power_var(CardVar::PoisonPower);
            fight.apply_to_opponents(Power::PoisonPower, sts2_core::PowerAmount::Counted(poison));
            for enemy in fight.hittable_opponents() {
                trigger_poison(fight, enemy);
            }
            Some(Played::Done)
        }
        // `Compact`: its block, then each Status of the hand that can be
        // transformed made into a Fuel, upgraded with it, in one
        // `CardCmd.Transform`. v0.107.1's transformed each in turn.
        Cards::Defect(DefectCard::Compact) => {
            use sts2_core::{CardKind, Pile, Rules as _};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let mut fuel = sts2_core::CardIdent::new(Cards::Token(TokenCard::Fuel));
            fuel.upgrade = u8::from(fight.upgraded());
            let statuses: Vec<_> = fight
                .cards_in(Pile::Hand)
                .into_iter()
                .filter(|&card| V::card_kind(fight.card_ident(card).card) == Some(CardKind::Status))
                .map(|card| (card, fuel))
                .collect();
            fight.transform_all(&statuses);
            Some(Played::Done)
        }
        // `Haze`: its Poison onto each enemy that can be hit, then its Weak
        // onto each.
        Cards::Silent(SilentCard::Haze) => {
            let poison = fight.power_var(CardVar::PoisonPower);
            for enemy in fight.hittable_opponents() {
                fight.apply_power(
                    enemy,
                    Power::PoisonPower,
                    sts2_core::PowerAmount::Counted(poison),
                );
            }
            let weak = fight.power_var(CardVar::WeakPower);
            for enemy in fight.hittable_opponents() {
                fight.apply_power(
                    enemy,
                    Power::WeakPower,
                    sts2_core::PowerAmount::Counted(weak),
                );
            }
            Some(Played::Done)
        }
        // `BladeSymphony`: its `Cards` of Shivs into the hand of each living
        // player on its owner's side, its owner too, one at a time. Each
        // player makes their own: `Shiv.CreateInHand` with no creator.
        Cards::Silent(SilentCard::BladeSymphony) => {
            use sts2_core::CreatureView as _;

            let count: usize = num_traits::cast(fight.var(CardVar::Cards))
                .expect("a count of Shivs is not negative");
            let living: Vec<_> = fight
                .players()
                .into_iter()
                .filter(|&player| fight.creatures().any(|c| c.id() == player && c.alive()))
                .collect();
            for player in living {
                fight.for_player(player, |fight| {
                    for _ in 0..count {
                        fight.add_generated_cards_by(
                            sts2_core::CardIdent::new(Cards::Token(TokenCard::Shiv)),
                            sts2_core::Pile::Hand,
                            1,
                            player,
                        );
                    }
                });
            }
            Some(Played::Done)
        }
        // `ImitationLearning`: its stack more on the Imitation Learning its
        // owner keeps for the teammate it is played at, or a new one aimed
        // at that teammate.
        Cards::Defect(DefectCard::ImitationLearning) => {
            let (owner, copied) = (fight.actor(), fight.target());
            let power = Power::ImitationLearningPower;
            let amount = fight.power_var(CardVar::ImitationLearningPower);
            let held = fight.power_instances(owner, power).len();
            let mut copying = None;
            for nth in 0..held {
                fight.write_power_at(owner, power, nth, |state| {
                    if let crate::behaviour::powers::State::ImitationLearning { player, .. } = state
                        && *player == Some(copied)
                        && copying.is_none()
                    {
                        copying = Some(nth);
                    }
                });
            }
            match copying {
                Some(nth) => fight.modify_power_at(owner, power, nth, amount, Some(owner)),
                None => {
                    fight.apply_to_actor(power, sts2_core::PowerAmount::Counted(amount));
                    // `PowerCmd.Apply` hands back nothing for a power it
                    // refused, and the card then names no target.
                    if fight.power_instances(owner, power).len() > held {
                        fight.write_power(owner, power, |state| {
                            if let crate::behaviour::powers::State::ImitationLearning {
                                player,
                                ..
                            } = state
                            {
                                *player = Some(copied);
                            }
                        });
                    }
                }
            }
            Some(Played::Done)
        }
        // `Tracking`: 50 Tracking on its owner, every play.
        Cards::Silent(SilentCard::Tracking) => {
            fight.apply_to_actor(
                Power::TrackingPower,
                sts2_core::PowerAmount::Counted(TRACKING),
            );
            Some(Played::Done)
        }
        // `MadScience` with Chaos: its block, then a card of the character's
        // pool into hand through `AddGeneratedCardToCombat`, free this turn.
        // v0.107.1's added it as any other card.
        Cards::Event(EventCard::MadScienceChaos) => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            crate::behaviour::free_card_into_hand(fight, |_| true, true);
            Some(Played::Done)
        }
        Cards::Necrobinder(NecrobinderCard::Misery) => Some(misery(fight)),
        // `Hibernate`: one Hibernate on its owner, then its `Repeat` of Frost
        // orbs channelled.
        Cards::Defect(DefectCard::Hibernate) => {
            fight.apply_to_actor(Power::HibernatePower, sts2_core::PowerAmount::Counted(1));
            for _ in 0..fight.var(CardVar::Repeat) {
                fight.channel(super::super::generated::orbs::Orb::FrostOrb);
            }
            Some(Played::Done)
        }
        // `OneForAll`: its `OneForAllPower` on every player, the dead too,
        // one after another.
        Cards::Defect(DefectCard::OneForAll) => {
            let amount = fight.power_var(CardVar::OneForAllPower);
            for player in fight.players() {
                fight.apply_power(
                    player,
                    Power::OneForAllPower,
                    sts2_core::PowerAmount::Counted(amount),
                );
            }
            Some(Played::Done)
        }
        // `Tutor`'s pick is the ally's, so its play branches: the play
        // ends, and the pick and the rest go on as a hook action on the
        // ally's queue. The scheduler has no branch yet.
        Cards::Regent(RegentCard::Tutor) => {
            unimplemented!("Tutor's pick branching onto the ally's queue")
        }
        Cards::Defect(DefectCard::Hyperbeam)
        | Cards::Defect(DefectCard::Scrape)
        | Cards::Silent(SilentCard::Expertise)
        | Cards::Necrobinder(NecrobinderCard::Eidolon)
        | Cards::Event(EventCard::MadScienceViolence)
        | Cards::Regent(RegentCard::ShiningStrike)
        | Cards::Event(EventCard::Abundance) => Some(start::<K>(card, fight)),
        _ => None,
    }
}

/// `PoisonPower.Trigger` on `creature`, where it carries Poison: `TriggerCount`
/// rounds, its stack or one more than the Accelerant its living opponents
/// carry, whichever is fewer. Each round hits it, unblockable and
/// unpowered, for the stack it holds then, and takes one off while it lives.
fn trigger_poison<F: Fight<V>>(fight: &mut F, creature: sts2_core::CombatId) {
    use sts2_core::{CreatureView as _, PowerAmount};

    let stack_of = |fight: &F| {
        fight
            .creatures()
            .find(|held| held.id() == creature)
            .and_then(|held| held.power(Power::PoisonPower))
    };
    let amount = match stack_of(fight) {
        None => return,
        Some(PowerAmount::Counted(amount)) => amount,
        Some(PowerAmount::Endless) => unimplemented!("an endless Poison"),
    };
    let side = fight
        .creatures()
        .find(|held| held.id() == creature)
        .map(|held| held.side())
        .expect("a creature with Poison stands in the fight");
    let accelerant: Number = fight
        .creatures()
        .filter(|opponent| opponent.side() != side && opponent.alive())
        .filter_map(|opponent| match opponent.power(Power::AccelerantPower) {
            Some(PowerAmount::Counted(amount)) => Some(amount),
            Some(PowerAmount::Endless) => unimplemented!("an endless Accelerant"),
            None => None,
        })
        .sum();
    for _ in 0..amount.min(1 + accelerant) {
        let Some(PowerAmount::Counted(stack)) = stack_of(fight) else {
            break;
        };
        fight.damage_unpowered(None, creature, stack, true);
        let alive = fight
            .creatures()
            .any(|held| held.id() == creature && held.alive());
        if alive {
            fight.modify_power_amount(creature, Power::PoisonPower, -1, None);
        }
    }
}

/// `Misery`: the target's debuffs as they stand, each temporary power's
/// amount added onto the debuff it lends. Then its damage to the target.
/// Each other hittable enemy takes each amount that is not zero, from the
/// debuff's applier. `PowerCmd.ModifyAmount` puts it onto an instance it
/// stacks with, and `PowerCmd.Apply` puts on a copy otherwise.
fn misery<F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    use super::super::generated::powers::PowerInstanceType;
    use sts2_core::{PowerAmount, Table as _};

    let target = fight.target();
    let debuffs: Vec<_> = fight
        .powers_on(target)
        .into_iter()
        .filter(|held| crate::behaviour::cards::debuff_for_amount::<V>(held.power, held.amount))
        .collect();
    let mut amounts: Vec<Number> = debuffs
        .iter()
        .map(|held| match held.amount {
            PowerAmount::Counted(amount) => amount,
            PowerAmount::Endless => unimplemented!("Misery copying an endless {:?}", held.power),
        })
        .collect();
    for (at, held) in debuffs.iter().enumerate() {
        if let Some((lent, _)) = crate::behaviour::powers::lends::<V>(held.power)
            && let Some(onto) = debuffs.iter().position(|other| other.power == lent)
        {
            amounts[onto] += amounts[at];
        }
    }
    crate::behaviour::cards::necrobinder::strike::<V, _>(fight);
    for enemy in fight.hittable_opponents() {
        if enemy == target {
            continue;
        }
        for (held, &amount) in debuffs.iter().zip(&amounts) {
            if amount == 0 {
                continue;
            }
            let power = held.power;
            // `ClonePreservingMutability` builds its internal data anew,
            // which is all a noted play is.
            if !held.fresh && !crate::behaviour::powers::clone_starts_anew::<V>(power) {
                unimplemented!("Misery copying {power:?}, whose model carries state");
            }
            let stacks = |on: &sts2_core::HeldInstance<V>| match power.data().instance {
                None => on.power == power,
                Some(PowerInstanceType::InstancedPerApplier) => {
                    on.power == power && on.applier == held.applier
                }
                Some(PowerInstanceType::Instanced) => false,
            };
            if fight.powers_on(enemy).iter().any(stacks) {
                fight.modify_power_amount(enemy, power, amount, held.applier);
                continue;
            }
            let copy = <crate::behaviour::powers::State<V> as sts2_core::PowerModel<V>>::new(power);
            fight.apply_power_copy(enemy, copy, PowerAmount::Counted(amount), held.applier);
        }
    }
    Played::Done
}

/// A play of this build's own that can stop on a question: from its start
/// to its end, yielding each question it stops on. Each step is lent the
/// fight, and after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<K: FightKind<V>>(card: Cards) {
    match card {
        // `Hyperbeam`: its damage to every enemy, then its Focus as a
        // `HyperbeamFocusDownPower` on its owner.
        Cards::Defect(DefectCard::Hyperbeam) => {
            hyperbeam(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.apply_to_actor(
                Power::HyperbeamFocusDownPower,
                sts2_core::PowerAmount::Counted(fight.var(CardVar::FocusPower)),
            );
        }
        // `Scrape`: its damage to the target, then its `Cards` drawn, and
        // each copy drawn whose cost with every modifier is anything but
        // nothing discarded. v0.107.1's read the copy's own modifiers alone.
        // A shuffle inside the draw can ask, and the discard waits for it.
        Cards::Defect(DefectCard::Scrape) => {
            strike(resumed!().fight);
            let mut drew: sts2_core::Drew<V> = draw_cards(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                drew = drawn(resumed!().taken.take());
            }
            scrape_discards(resumed!().fight, &drew);
        }
        // `Expertise`: its `Cards` drawn, each of which retains this turn. A
        // shuffle inside the draw can ask, and the retain waits for the
        // cards the draw goes on to draw.
        Cards::Silent(SilentCard::Expertise) => {
            let mut drew: sts2_core::Drew<V> = draw_cards(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                drew = drawn(resumed!().taken.take());
            }
            let fight = &mut *resumed!().fight;
            let kept: Vec<_> = drew.iter().filter_map(|&id| fight.find_card(id)).collect();
            fight.act_on(sts2_core::PileAct::RetainThisTurn, &kept);
        }
        // `Eidolon`: `CardCmd.AutoPlay` of each card in the exhaust pile that
        // is Ethereal and not Unplayable, as the play began, with no target
        // named. Each is awaited, so the plays after one that asks wait on
        // the answer.
        Cards::Necrobinder(NecrobinderCard::Eidolon) => {
            let ethereal: Listed<V> = ethereal_in_exhaust(resumed!().fight);
            let count: usize = ethereal.len();
            for at in 0..count {
                let fight = &mut *resumed!().fight;
                // A power card an earlier play took out of the fight stands in
                // no pile, and does not play again.
                match crate::behaviour::cards::standing::<V, _>(fight, ethereal[at]) {
                    Some(standing) => fight.auto_play_card(standing),
                    None => fight.auto_play_departed(ethereal[at].id()),
                }
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
            }
        }
        // `MadScience` with Violence: its damage to the target, as one
        // attack of `ViolenceHits` hits. v0.107.1's made each hit its own.
        Cards::Event(EventCard::MadScienceViolence) => {
            let fight = &mut *resumed!().fight;
            let target = fight.target();
            let hits = usize::try_from(fight.var(CardVar::ViolenceHits))
                .expect("a count of hits is not negative");
            crate::behaviour::cards::event::hit::<V, _>(fight, target, hits);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
        }
        // `ShiningStrike`: its damage to the target, then its stars. Where it
        // goes after is its own `GetResultLocationForCardPlay`.
        Cards::Regent(RegentCard::ShiningStrike) => {
            strike(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.gain_stars(fight.var(CardVar::Stars));
        }
        // `Abundance`: `GetDistinctForCombat` of three Powers from its
        // owner's own pool, off `CombatCardGeneration`, each upgraded, and
        // the one chosen into hand, free this turn, through
        // `SetToFreeThisTurn` and `CardPileCmd.AddGeneratedCardToCombat`.
        Cards::Event(EventCard::Abundance) => {
            let offered: sts2_core::Offered<V> = abundance_offer(resumed!().fight);
            yield_!(Ask::Choosing {
                offered,
                skippable: false,
            });
            let Lent { fight, taken } = resumed!();
            if let Some(chosen) = crate::behaviour::paused::chosen::<V>(taken) {
                for at in fight.add_generated_cards(chosen, sts2_core::Pile::Hand, 1) {
                    crate::behaviour::free_this_turn(&mut **fight, at);
                }
            }
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

/// A play of [`paused`], from its start.
fn start<'a, K: FightKind<V>>(card: Cards, fight: &mut K::Fight<'a>) -> Played<V, K> {
    let mut play = paused::<K>(card);
    let state = play.start_with(Lent { fight, taken: None });
    settle::<K>(play, state)
}

/// A paused play of [`paused`], with the answer it waited for.
pub(crate) fn resume<'a, K: FightKind<V>>(
    fight: &mut K::Fight<'a>,
    mut play: paused::State<K>,
    taken: sts2_core::Taken<V>,
) -> Played<V, K> {
    let state = diapause::Coroutine::resume(
        &mut play,
        Lent {
            fight,
            taken: Some(taken),
        },
    );
    settle::<K>(play, state)
}

/// How far a step of `play` got.
fn settle<K: FightKind<V>>(
    play: paused::State<K>,
    state: diapause::CoroutineState<Ask<V>, ()>,
) -> Played<V, K> {
    match state {
        diapause::CoroutineState::Complete(()) => Played::Done,
        diapause::CoroutineState::Yielded(ask) => {
            ask.at(crate::behaviour::Resume::Build(super::Paused::Own(play)))
        }
    }
}

/// `Hyperbeam`'s attack: its damage to every enemy, as one
/// `CreatureCmd.Damage`.
fn hyperbeam<F: Fight<V>>(fight: &mut F) {
    fight.attack(Scaling::Dynamic, |fight| {
        let targets = fight.opponents();
        fight.damage_command(|fight| {
            for target in targets {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(hit);
            }
        });
    });
}

/// Its damage to the target, as one attack.
fn strike<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
}

/// Its `Cards` drawn.
fn draw_cards<F: Fight<V>>(fight: &mut F) -> sts2_core::Drew<V> {
    let cards = fight.var(CardVar::Cards);
    fight.draw(cards)
}

/// The copies a draw that waited drew. A draw the ended fight never ran
/// drew nothing.
fn drawn(taken: Option<sts2_core::Taken<V>>) -> sts2_core::Drew<V> {
    match taken {
        Some(sts2_core::Taken::Drew(drew)) => drew,
        _ => sts2_core::Drew::new(),
    }
}

/// `Scrape` once its draw is over: each copy it drew whose cost with every
/// modifier is anything but nothing discarded.
fn scrape_discards<F: Fight<V>>(fight: &mut F, drew: &[sts2_core::CardId<V>]) {
    use sts2_core::{Cost, PileAct};

    let costly: Vec<_> = drew
        .iter()
        .map(|&id| {
            fight
                .find_card(id)
                .unwrap_or_else(|| unimplemented!("Scrape discarding {id:?}, which left the fight"))
        })
        .filter(|&card| fight.card_costs(card, true).0 != Cost::fixed(0))
        .collect();
    fight.act_on(PileAct::Discard, &costly);
}

/// The cards of the exhaust pile that are Ethereal and not Unplayable.
fn ethereal_in_exhaust<F: Fight<V>>(fight: &F) -> Listed<V> {
    use crate::generated::cards::CardKeyword;

    fight
        .cards_in(sts2_core::Pile::Exhaust)
        .into_iter()
        .filter(|&at| {
            fight.card_keyword(at, CardKeyword::Ethereal)
                && !fight.card_keyword(at, CardKeyword::Unplayable)
        })
        .collect()
}

/// `Abundance`'s offer: three distinct Powers of its owner's own pool, off
/// `CombatCardGeneration`, each upgraded.
fn abundance_offer<F: Fight<V>>(fight: &mut F) -> sts2_core::Offered<V> {
    use sts2_core::{CardKind, Rules as _, RunStreamKinds as _};

    let mut powers: Vec<_> = V::combat_cards(fight.character(), fight.multiplayer())
        .iter()
        .copied()
        .filter(|&card| V::card_kind(card) == Some(CardKind::Power))
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
    let mut offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] = fight
        .generate_cards(&mut powers, stream)
        .unwrap_or_else(|_| unimplemented!("a pool offering fewer than three Powers"));
    for card in &mut offered {
        card.upgrade = 1;
    }
    offered.into_iter().collect()
}

/// A number a card of this build's own works out, or nothing for a shared one.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    key: CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V>,
) -> Option<Number> {
    match (card, key) {
        // `Sacrifice`: a base, and more for each of three times the maximum
        // health a living Osty has. v0.107.1's counts twice it.
        (Cards::Necrobinder(NecrobinderCard::Sacrifice), CardVar::CalculatedBlock) => {
            let thrice = owner.osty_max_hp.map_or(0, |hp| hp * 3);
            Some(
                printed.face(CardVar::CalculationBase)
                    + printed.face(CardVar::CalculationExtra) * thrice,
            )
        }
        // `ExpectAFight`: a base, and more for each Strength its owner has,
        // none for less than none.
        (Cards::Ironclad(IroncladCard::ExpectAFight), CardVar::CalculatedBlock) => {
            let strength = owner
                .powers
                .iter()
                .find(|held| held.power == Power::StrengthPower)
                .map_or(0, |held| match held.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Strength"),
                });
            Some(
                printed.face(CardVar::CalculationBase)
                    + printed.face(CardVar::CalculationExtra) * strength.max(0),
            )
        }
        _ => None,
    }
}

/// A deck card of this build's own, by what it saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnCard {
    /// `Dowsing.RoomsEntered`.
    Dowsing { rooms_entered: u8 },
}

/// `Dowsing.maxRooms`: the `?` rooms it waits for.
const DOWSING_ROOMS: u8 = 5;

/// What a deck card of this build's own saves as it is made.
pub(crate) fn own_card_state(card: Cards) -> Option<OwnCard> {
    match card {
        Cards::Quest(QuestCard::Dowsing) => Some(OwnCard::Dowsing { rooms_entered: 0 }),
        _ => None,
    }
}

/// `Dowsing.RoomsEntered`'s setter prints `Rooms` as the rooms it has left.
pub(crate) fn own_card_written(own: OwnCard) -> Option<sts2_core::Change<V>> {
    match own {
        OwnCard::Dowsing { rooms_entered } if rooms_entered > 0 => Some(sts2_core::Change::Raise(
            CardVar::Rooms,
            -Number::from(rooms_entered),
        )),
        OwnCard::Dowsing { .. } => None,
    }
}

/// `EventHook` for a deck card of this build's own.
pub(crate) fn own_deck_card_event_hook(
    card: Cards,
    own: &mut OwnCard,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (own, hook) {
        // `Dowsing.BeforeRoomEntered`: one more `?` room, if it is the map
        // point's first, and an Abundance in its place at five.
        // `PlayerCmd.CompleteQuest` writes the map point's history, which
        // the run here does not keep.
        (
            OwnCard::Dowsing { rooms_entered },
            sts2_core::EventHook::RoomEntering { point, base },
        ) => {
            if base && point == Some(MapPointType::Unknown) {
                *rooms_entered += 1;
                if *rooms_entered >= DOWSING_ROOMS {
                    asked.push(sts2_core::HookEffect::TransformTo(Cards::Event(
                        EventCard::Abundance,
                    )));
                }
            }
        }
        (_, hook) => unimplemented!("what {card:?} in the deck does for {hook:?}"),
    }
}
