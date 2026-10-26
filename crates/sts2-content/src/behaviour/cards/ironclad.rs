//! The Ironclad's cards the generator leaves to a hand port.

use crate::behaviour::Played;
use crate::behaviour::cards::{Ask, Lent};
use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};
use sts2_core::{
    Amount, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight, FightKind,
    HookEffect, Lasting, Pile, PileAct, PowerAmount, Scaling,
};

/// Cards a play took from the hand, which it goes through after a wait.
type Hand<V> = smallvec::SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]>;

/// One card, as a pick asks for it.
const ONE: sts2_core::Selection = sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE);

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match card {
        // `NotYet`: its `Heal` on its owner.
        __c0 if __c0 == V::Card::NOT_YET => {
            fight.heal(
                fight.actor(),
                fight.var(<V::CardVar as crate::ids::CardVarIds>::HEAL),
            );
            Played::Done
        }
        // `TearAsunder`: its damage to the target, as many times as its
        // calculated hits, counted before the first, as one attack.
        __c0 if __c0 == V::Card::TEAR_ASUNDER => {
            let target = fight.target();
            let hits = fight.var(V::CardVar::CALCULATED_HITS);
            fight.attack(Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(V::CardVar::DAMAGE),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        // `ForgottenRitual`: its energy, where its owner exhausted a card this
        // turn.
        __c0 if __c0 == V::Card::FORGOTTEN_RITUAL => {
            if fight.history().exhausted_this_turn() {
                fight.modify_energy(fight.var(V::CardVar::ENERGY));
            }
            Played::Done
        }
        // `Spite`: its damage to the target, once, or its `Repeat` times
        // where its owner lost health this turn.
        __c0 if __c0 == V::Card::SPITE => {
            let owner = fight.actor();
            let hurt = fight
                .history()
                .damage_received_this_turn()
                .any(|received| received.receiver == owner && received.unblocked > 0);
            let hits = if hurt {
                fight.var(V::CardVar::REPEAT)
            } else {
                1
            };
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(V::CardVar::DAMAGE),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        // `PactsEnd`: its damage to every enemy, once the exhaust pile holds
        // its `Cards`.
        __c0 if __c0 == V::Card::PACTS_END => {
            let exhausted = fight.cards_in(Pile::Exhaust).len();
            if num_traits::cast::<_, crate::behaviour::cards::Number>(exhausted)
                .is_some_and(|exhausted| exhausted >= fight.var(V::CardVar::CARDS))
            {
                fight.attack(Scaling::Dynamic, |fight| {
                    let targets = fight.opponents();
                    // One `CreatureCmd.Damage` over them all.
                    fight.damage_command(|fight| {
                        for target in targets {
                            let hit = fight.resolve(Effect::Attack {
                                dealer: fight.actor(),
                                target,
                                damage: Amount::Var(V::CardVar::DAMAGE),
                                scaling: Scaling::Dynamic,
                            });
                            fight.apply(hit);
                        }
                    });
                });
            }
            Played::Done
        }
        // `InfernalBlade`: an Attack of the character's pool, free this turn,
        // into the hand.
        __c0 if __c0 == V::Card::INFERNAL_BLADE => {
            crate::behaviour::free_card_into_hand(
                fight,
                |card| V::card_kind(card) == Some(CardKind::Attack),
                true,
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::WHIRLWIND => whirlwind::<V, _>(fight),
        __c0 if __c0 == V::Card::DISMANTLE => dismantle::<V, _>(fight),
        __c0 if __c0 == V::Card::PRIMAL_FORCE => primal_force::<V, _>(fight),
        // `Cascade`: the top cards of the draw pile auto-played, as many as
        // its X spent and one more once upgraded.
        __c0 if __c0 == V::Card::CASCADE => {
            let count = fight.x_value() + crate::behaviour::cards::Number::from(fight.upgraded());
            let count = usize::try_from(count).expect("an X is not negative");
            fight.auto_play_from_draw(count, false);
            Played::Done
        }
        // `Havoc`: the top card of the draw pile auto-played, and exhausted.
        __c0 if __c0 == V::Card::HAVOC => {
            fight.auto_play_from_draw(1, true);
            Played::Done
        }
        // `PerfectedStrike`: its calculated damage to the target. Its test of
        // that damage against twelve picks an animation.
        __c0 if __c0 == V::Card::PERFECTED_STRIKE => {
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(V::CardVar::CALCULATED_DAMAGE),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            });
            Played::Done
        }
        __c0 if __c0 == V::Card::THRASH
            || __c0 == V::Card::CRIMSON_MANTLE
            || __c0 == V::Card::INFERNO
            || __c0 == V::Card::DOMINATE
            || __c0 == V::Card::FEED
            || __c0 == V::Card::SECOND_WIND
            || __c0 == V::Card::FIEND_FIRE
            || __c0 == V::Card::BURNING_PACT
            || __c0 == V::Card::RAMPAGE
            || __c0 == V::Card::ANGER
            || __c0 == V::Card::HEADBUTT
            || __c0 == V::Card::BLOODLETTING
            || __c0 == V::Card::OFFERING
            || __c0 == V::Card::UPPERCUT
            || __c0 == V::Card::MOLTEN_FIST
            || __c0 == V::Card::CINDER
            || __c0 == V::Card::PILLAGE
            || __c0 == V::Card::TRUE_GRIT
            || __c0 == V::Card::BRAND
            || __c0 == V::Card::HEMOKINESIS
            || __c0 == V::Card::BREAKTHROUGH
            || __c0 == V::Card::BLOOD_WALL
            || __c0 == V::Card::ARMAMENTS
            || __c0 == V::Card::EVIL_EYE
            || __c0 == V::Card::STOKE =>
        {
            start::<V, K>(card, fight)
        }
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// An Ironclad card's play that can stop on a question: from its start to
/// its end, yielding each question it stops on. Each step is lent the fight,
/// and after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `Thrash`: its damage to the target twice. Then an Attack in hand
        // drawn off `CombatCardSelection` is exhausted. Its damage, modified
        // as its owner deals it to nobody, joins Thrash's own for the rest of
        // the fight.
        __c0 if __c0 == V::Card::THRASH => {
            thrash::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            thrash_hit::<V, _>(resumed!().fight);
        }
        // `CrimsonMantle`: its Crimson Mantle on its owner, then the power's
        // self damage raised by one.
        __c0 if __c0 == V::Card::CRIMSON_MANTLE => {
            let fight = &mut *resumed!().fight;
            let mantle = PowerAmount::Counted(fight.var(V::CardVar::CRIMSON_MANTLE_POWER));
            fight.apply_to_actor(V::Power::CRIMSON_MANTLE_POWER, mantle);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.note_on_power(fight.actor(), V::Power::CRIMSON_MANTLE_POWER, 1);
        }
        // `Inferno`: its Inferno on its owner, then one more self damage on
        // the power it landed or stacked into.
        __c0 if __c0 == V::Card::INFERNO => {
            let fight = &mut *resumed!().fight;
            let inferno = PowerAmount::Counted(fight.var(V::CardVar::INFERNO_POWER));
            fight.apply_to_actor(V::Power::INFERNO_POWER, inferno);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.note_on_power(fight.actor(), V::Power::INFERNO_POWER, 1);
        }
        // `Dominate`: its Vulnerable on the target, then as much Strength on
        // its owner as the target then carries Vulnerable.
        __c0 if __c0 == V::Card::DOMINATE => {
            let fight = &mut *resumed!().fight;
            let target = fight.target();
            let vulnerable = PowerAmount::Counted(fight.var(V::CardVar::VULNERABLE_POWER));
            fight.apply_power(target, V::Power::VULNERABLE_POWER, vulnerable);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            dominate_applied::<V, _>(resumed!().fight);
        }
        // `Feed`: its damage to the target, then its `MaxHp` onto its owner's
        // maximum where a result says `WasTargetKilled` of a target whose
        // death, read before the hit, triggers Fatal.
        __c0 if __c0 == V::Card::FEED => {
            let fatal: bool = {
                let fight = &mut *resumed!().fight;
                fight.death_triggers_fatal(fight.target())
            };
            let mut killed: bool = feed_attack::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                killed = resumed!()
                    .fight
                    .waited_attack_results()
                    .iter()
                    .any(|result| result.killed);
            }
            let fight = &mut *resumed!().fight;
            if fatal && killed {
                fight.gain_max_hp(fight.actor(), fight.var(V::CardVar::MAX_HP));
            }
        }
        // `SecondWind`: each card in hand that is not an Attack exhausted,
        // and its block for each, one after another. A question inside a
        // block or an exhaust stands the rest still.
        __c0 if __c0 == V::Card::SECOND_WIND => {
            let cards: Hand<V> = not_attacks_in_hand::<V, _>(resumed!().fight);
            let count: usize = cards.len();
            for at in 0..count {
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
                resumed!().fight.act_on(PileAct::Exhaust, &[cards[at]]);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
                block::<V, _>(resumed!().fight);
            }
        }
        // `FiendFire`: every card in hand exhausted, then its damage to the
        // target once for each. A hook's question inside an exhaust stands
        // the rest still.
        __c0 if __c0 == V::Card::FIEND_FIRE => {
            let cards: Hand<V> = resumed!().fight.cards_in(Pile::Hand).into_iter().collect();
            let count: usize = cards.len();
            for at in 0..count {
                resumed!().fight.act_on(PileAct::Exhaust, &[cards[at]]);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
            }
            fiend_fire_hits::<V, _>(resumed!().fight, count);
        }
        // `BurningPact`: one card of the hand exhausted, then its `Cards`
        // drawn.
        __c0 if __c0 == V::Card::BURNING_PACT => {
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::Exhaust,
                crate::behaviour::cards::selected::<V>(taken).as_slice(),
            );
            let cards = fight.var(V::CardVar::CARDS);
            fight.draw(cards);
        }
        // `Rampage`: its damage to the target, then its `Increase` onto its
        // own damage for the fight.
        __c0 if __c0 == V::Card::RAMPAGE => {
            strike_target::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            let increase = fight.var(V::CardVar::INCREASE);
            fight.modify_this(
                Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(increase)),
                Lasting::ThisFight,
            );
        }
        // `Anger`: its damage to the target, then a clone of itself into the
        // discard pile.
        __c0 if __c0 == V::Card::ANGER => {
            strike_target::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            resumed!().fight.clone_this(Pile::Discard);
        }
        // `Headbutt`: its damage to the target, then one card of the discard
        // pile onto the top of the draw pile, as
        // `CardPileCmd.Add(cardModel, PileType.Draw, CardPilePosition.Top)`.
        __c0 if __c0 == V::Card::HEADBUTT => {
            strike_target::<V, _>(resumed!().fight);
            yield_!(Ask::Asking {
                pile: Pile::Discard,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::OntoDrawTop,
                crate::behaviour::cards::selected::<V>(taken).as_slice(),
            );
        }
        // `Bloodletting`: its `HpLoss` off its owner, unblockable, then its
        // energy.
        __c0 if __c0 == V::Card::BLOODLETTING => {
            lose_hp::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.modify_energy(fight.var(V::CardVar::ENERGY));
        }
        // `Offering`: its `HpLoss` off its owner, unblockable, then its
        // energy and its cards drawn.
        __c0 if __c0 == V::Card::OFFERING => {
            lose_hp::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.modify_energy(fight.var(V::CardVar::ENERGY));
            fight.draw(fight.var(V::CardVar::CARDS));
        }
        // `Uppercut`: its damage to the target, then its `Power` as Weak and
        // as Vulnerable on it.
        __c0 if __c0 == V::Card::UPPERCUT => {
            strike_target::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            power_on_target::<V, _>(resumed!().fight, V::Power::WEAK_POWER);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            power_on_target::<V, _>(resumed!().fight, V::Power::VULNERABLE_POWER);
        }
        // `MoltenFist`: its damage to the target, then as much Vulnerable
        // again as the target holds, if it still stands.
        __c0 if __c0 == V::Card::MOLTEN_FIST => {
            strike_target::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            molten_fist_hit::<V, _>(resumed!().fight);
        }
        // `Cinder`: its damage to the target, then a card from hand drawn off
        // `CombatCardSelection`, exhausted.
        __c0 if __c0 == V::Card::CINDER => {
            strike_target::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            exhaust_random_in_hand::<V, _>(resumed!().fight);
        }
        // `Pillage`: its damage to the target, then one card drawn at a time.
        // The draws go on while each is an Attack and the hand has room. A
        // shuffle inside a draw can ask, and the rest waits for it. A draw
        // the ended fight never ran drew nothing.
        __c0 if __c0 == V::Card::PILLAGE => {
            strike_target::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            loop {
                let mut drew: sts2_core::Drew<V> = resumed!().fight.draw(1);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                    drew = match resumed!().taken.take() {
                        Some(sts2_core::Taken::Drew(drawn)) => drawn,
                        _ => sts2_core::Drew::new(),
                    };
                }
                if !pillage_goes_on::<V, _>(resumed!().fight, &drew) {
                    break;
                }
            }
        }
        // `TrueGrit`: its block, then a card from hand exhausted. Upgraded,
        // its owner picks the card, and `CardCmd.Exhaust` takes it.
        // Otherwise it is drawn off `CombatCardSelection`.
        __c0 if __c0 == V::Card::TRUE_GRIT => {
            block::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            if resumed!().fight.upgraded() {
                yield_!(Ask::Asking {
                    pile: Pile::Hand,
                    selection: ONE,
                });
                let Lent { fight, taken } = resumed!();
                fight.act_on(
                    PileAct::Exhaust,
                    crate::behaviour::cards::selected::<V>(taken).as_slice(),
                );
            } else {
                exhaust_random_in_hand::<V, _>(resumed!().fight);
            }
        }
        // `Brand`: its `HpLoss` off its owner, unblockable, then one card of
        // the hand exhausted, then its Strength.
        __c0 if __c0 == V::Card::BRAND => {
            lose_hp::<V, _>(resumed!().fight);
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::Exhaust,
                crate::behaviour::cards::selected::<V>(taken).as_slice(),
            );
            let strength = fight.var(V::CardVar::STRENGTH_POWER);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        // `Hemokinesis`: its `HpLoss` off its owner, unblockable, then its
        // damage to the target.
        __c0 if __c0 == V::Card::HEMOKINESIS => {
            lose_hp::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            strike_target::<V, _>(resumed!().fight);
        }
        // `Breakthrough`: its `HpLoss` off its owner, unblockable, then its
        // damage to every enemy.
        __c0 if __c0 == V::Card::BREAKTHROUGH => {
            lose_hp::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            breakthrough_hit::<V, _>(resumed!().fight);
        }
        // `BloodWall`: its `HpLoss` off its owner, unblockable, then its
        // block.
        __c0 if __c0 == V::Card::BLOOD_WALL => {
            lose_hp::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            block::<V, _>(resumed!().fight);
        }
        // `Armaments`: its block, then a card in hand upgraded. Upgraded,
        // every upgradable card in hand is. Otherwise its owner picks one of
        // them, which `FromHandForUpgrade` takes without asking where there
        // is one, and `CardCmd.Upgrade` takes it.
        __c0 if __c0 == V::Card::ARMAMENTS => {
            block::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let upgradable: Hand<V> = upgradable_in_hand::<V, _>(resumed!().fight);
            if resumed!().fight.upgraded() {
                upgrade::<V, _>(resumed!().fight, &upgradable);
            } else {
                yield_!(Ask::AskingAmong {
                    cards: upgradable,
                    selection: ONE,
                });
                let Lent { fight, taken } = resumed!();
                upgrade::<V, _>(
                    &mut **fight,
                    crate::behaviour::cards::selected::<V>(taken).as_slice(),
                );
            }
        }
        // `EvilEye`: its block, and again where its owner exhausted a card
        // this turn. A fight here has one player, whose cards are all its
        // owner's.
        __c0 if __c0 == V::Card::EVIL_EYE => {
            let twice: bool = resumed!().fight.history().exhausted_this_turn();
            block::<V, _>(resumed!().fight);
            if twice {
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
                block::<V, _>(resumed!().fight);
            }
        }
        // `Stoke`: the whole hand exhausted, one card at a time. Then as many
        // cards of the character's pool, each `NextItem` off
        // `CombatCardGeneration`, upgraded once Stoke is, into hand.
        __c0 if __c0 == V::Card::STOKE => {
            let cards: Hand<V> = resumed!().fight.cards_in(Pile::Hand).into_iter().collect();
            let count: usize = cards.len();
            for at in 0..count {
                resumed!().fight.act_on(PileAct::Exhaust, &[cards[at]]);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
            }
            stoke_generate::<V, _>(resumed!().fight, count);
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

crate::behaviour::cards::paused_play!(Ironclad);

/// `Dismantle`: its damage to the target, twice where the target carries
/// Vulnerable.
fn dismantle<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let target = fight.target();
    let vulnerable = fight.creatures().any(|creature| {
        creature.id() == target && creature.power(V::Power::VULNERABLE_POWER).is_some()
    });
    // One `DamageCmd.Attack` with its hit count, so what hears the attack's
    // end hears it once.
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..1 + u8::from(vulnerable) {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
    Played::Done
}

/// `PrimalForce`: each Attack in hand transformed into a Giant Rock,
/// upgraded where Primal Force is. A card in a fight is always
/// transformable.
fn primal_force<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let attacks: Vec<_> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| V::card_kind(fight.card_ident(card).card) == Some(CardKind::Attack))
        .collect();
    for card in attacks {
        let mut into = sts2_core::CardIdent::new(V::Card::GIANT_ROCK);
        into.upgrade = u8::from(fight.upgraded());
        fight.transform(card, into);
    }
    Played::Done
}

/// Its block, onto its owner.
fn block<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
}

/// Its `HpLoss` off its owner, unblockable.
fn lose_hp<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
}

/// Its `Power` as `power` on the target.
fn power_on_target<V: crate::Build, F: Fight<V>>(fight: &mut F, power: V::Power) {
    let amount = PowerAmount::Counted(fight.var(V::CardVar::POWER));
    fight.apply_power(fight.target(), power, amount);
}

/// The cards in hand that are not Attacks, in order.
fn not_attacks_in_hand<V: crate::Build, F: Fight<V>>(fight: &F) -> Hand<V> {
    fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| V::card_kind(fight.card_ident(card).card) != Some(CardKind::Attack))
        .collect()
}

/// The cards in hand that can be upgraded, in order.
fn upgradable_in_hand<V: crate::Build, F: Fight<V>>(fight: &F) -> Hand<V> {
    use sts2_core::CardTable as _;

    fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| {
            let ident = fight.card_ident(card);
            ident.upgrade < ident.card.max_upgrade()
        })
        .collect()
}

/// `CardCmd.Upgrade` of each of `cards`.
fn upgrade<V: crate::Build, F: Fight<V>>(fight: &mut F, cards: &[sts2_core::CardAt<V>]) {
    for &card in cards {
        fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
    }
}

/// A card from hand drawn off `CombatCardSelection`, exhausted.
fn exhaust_random_in_hand<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    if let Some(card) = fight.random_card_in(Pile::Hand) {
        fight.act_on(PileAct::Exhaust, &[card]);
    }
}

/// Its damage to every enemy, as one `CreatureCmd.Damage`.
fn breakthrough_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.attack(Scaling::Dynamic, |fight| {
        let targets = fight.opponents();
        fight.damage_command(|fight| {
            for target in targets {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(V::CardVar::DAMAGE),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        });
    });
}

/// The rest of `Dominate`, once its Vulnerable is on: as much Strength on
/// its owner as the target then carries Vulnerable.
fn dominate_applied<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    let carried = fight
        .creatures()
        .find(|creature| creature.id() == target)
        .and_then(|creature| creature.power(V::Power::VULNERABLE_POWER));
    let strength = match carried {
        Some(PowerAmount::Counted(amount)) => amount,
        Some(PowerAmount::Endless) => unimplemented!("an endless Vulnerable"),
        None => 0,
    };
    fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
}

/// `FiendFire`, once its exhausts are over: its damage to the target, `hits`
/// times.
fn fiend_fire_hits<V: crate::Build, F: Fight<V>>(fight: &mut F, hits: usize) {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let hit = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(hit);
        }
    });
}

/// `Stoke`, once its exhausts are over: `count` cards of the character's
/// pool, each `NextItem` off `CombatCardGeneration`, upgraded once Stoke is,
/// into hand.
fn stoke_generate<V: crate::Build, F: Fight<V>>(fight: &mut F, count: usize) {
    let pool = <V as sts2_core::Rules>::combat_cards(fight.character(), fight.multiplayer());
    let upgrade = u8::from(fight.upgraded());
    let made: Vec<_> = (0..count)
        .filter_map(|_| fight.generate_card(pool))
        .map(|card| sts2_core::CardIdent {
            upgrade,
            ..sts2_core::CardIdent::new(card)
        })
        .collect();
    for card in made {
        fight.add_generated_cards(card, Pile::Hand, 1);
    }
}

/// One hit of the card's damage on `target`.
fn strike<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    target: sts2_core::CombatId,
) -> sts2_core::AttackResults<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    })
}

/// `Feed`'s attack, and whether a result says `WasTargetKilled`.
fn feed_attack<V: crate::Build, F: Fight<V>>(fight: &mut F) -> bool {
    let target = fight.target();
    strike::<V, _>(fight, target)
        .iter()
        .any(|result| result.killed)
}

/// One hit of the card's damage on the target.
fn strike_target<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    strike::<V, _>(fight, target);
}

/// `Thrash`'s attack: its damage to the target twice.
fn thrash<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..2 {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
}

/// The rest of `Thrash`, once its attack is over.
///
/// The damage read is its `CalculatedDamage` worked out against nobody,
/// else its `Damage`, else its `OstyDamage`, else nothing.
fn thrash_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::{CardTable as _, RunStreamKinds as _};

    let attacks: Vec<_> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack))
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_SELECTION;
    let Some(chosen) = fight.random_card(&attacks, stream) else {
        return;
    };
    let vars = fight.card_ident(chosen).card.vars();
    let printed = [
        V::CardVar::CALCULATED_DAMAGE,
        V::CardVar::DAMAGE,
        V::CardVar::OSTY_DAMAGE,
    ]
    .into_iter()
    .find(|&key| vars.iter().any(|number| number.id == key));
    let damage = printed.map_or(sts2_core::Decimal::ZERO, |key| {
        fight.card_number(chosen, key)
    });
    let damage = fight.card_damage(chosen, damage);
    let damage = sts2_core::Fixed::from_exact(damage)
        .unwrap_or_else(|| unimplemented!("Thrash taking in {damage}"));
    fight.modify_this(
        Change::Own(crate::behaviour::cards::CardWrite::ExtraDamage(damage)),
        Lasting::ThisFight,
    );
    fight.act_on(PileAct::Exhaust, &[chosen]);
}

/// The rest of `MoltenFist`, once its attack is over.
fn molten_fist_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    let held = fight
        .creatures()
        .find(|creature| creature.id() == target && creature.alive())
        .and_then(|creature| creature.power(V::Power::VULNERABLE_POWER));
    match held {
        Some(PowerAmount::Counted(held)) if held > 0 => fight.apply_power(
            target,
            V::Power::VULNERABLE_POWER,
            PowerAmount::Counted(held),
        ),
        Some(PowerAmount::Endless) => unimplemented!("an endless Vulnerable"),
        _ => {}
    }
}

/// Whether `Pillage` draws again after `drew`: the card it drew is an Attack,
/// wherever it stands now, and the hand has room.
fn pillage_goes_on<V: crate::Build, F: Fight<V>>(fight: &F, drew: &[sts2_core::CardId<V>]) -> bool {
    let Some(&id) = drew.first() else {
        return false;
    };
    let drawn = fight
        .find_card(id)
        .unwrap_or_else(|| unimplemented!("Pillage reading {id:?}, which left the fight"));
    V::card_kind(fight.card_ident(drawn).card) == Some(CardKind::Attack)
        && fight.cards_in(Pile::Hand).len() < sts2_core::MAX_CARDS_IN_HAND
}

/// `Whirlwind`: its damage to every enemy, once for each energy of its X.
fn whirlwind<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let hits = fight.x_value();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let targets = fight.opponents();
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for target in targets {
                    let attack = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(V::CardVar::DAMAGE),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(attack);
                }
            });
        }
    });
    Played::Done
}

/// Deliver a notification to one Ironclad card in the fight.
pub(super) fn event_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Stomp.BeforeCardPlayed`: one off its cost this turn for each
        // Attack its owner plays, itself included. A fight here has one
        // player.
        (__c0, EventHook::CardPlaying { card, .. }) if __c0 == V::Card::STOMP => {
            if V::card_kind(card) == Some(CardKind::Attack) {
                asked.push(HookEffect::ModifySelf {
                    change: Change::ChangeCost(CostPool::Energy, -1),
                    lasting: Lasting::ThisTurn,
                });
            }
        }
        // `Stomp.AfterCardEnteredCombat`, for its own entry alone and not a
        // clone's: one off this turn for each Attack play finished this turn
        // before it came.
        (__c0, EventHook::CardEnteredCombat { at, clone, .. }) if __c0 == V::Card::STOMP => {
            if at.id() == id && !clone {
                let attacks: crate::behaviour::cards::Number = history
                    .finished_this_turn()
                    .filter(|&card| V::card_kind(card) == Some(CardKind::Attack))
                    .count()
                    .try_into()
                    .expect("a turn's plays fit a game number");
                if attacks != 0 {
                    asked.push(HookEffect::ModifySelf {
                        change: Change::ChangeCost(CostPool::Energy, -attacks),
                        lasting: Lasting::ThisTurn,
                    });
                }
            }
        }
        // `DrumOfBattle.AfterCardExhausted`, for its own exhaust alone.
        (__c0, EventHook::CardExhausted { id: exhausted, .. })
            if __c0 == V::Card::DRUM_OF_BATTLE =>
        {
            if exhausted == id {
                asked.push(HookEffect::GainEnergyForEachPlay {
                    card: id,
                    energy: printed.face(V::CardVar::ENERGY),
                });
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
