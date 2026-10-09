//! The Ironclad's cards the generator leaves to a hand port.

use sts2_core::{
    Amount, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight, HookEffect,
    Lasting, Pile, PileAct, Played, PowerAmount, Rules as _, Scaling,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, IroncladCard};
use super::super::super::generated::powers::Power;
use super::V;

/// Where an Ironclad card's play stops to ask something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Headbutt`, after its owner picked the card to put back.
    HeadbuttOntoDraw,
    /// `TrueGrit`, upgraded, after its owner picked the card to exhaust.
    TrueGritExhaust,
    /// `Armaments`, after its owner picked the card to upgrade.
    ArmamentsUpgrade,
    /// `Brand`, after its owner picked the card to exhaust.
    BrandExhaust,
    /// `BurningPact`, after its owner picked the card to exhaust.
    BurningPactExhaust,
    /// `BattleTrance`, after a question inside its draw was answered.
    BattleTranceDrew,
    /// `FiendFire`, after a question inside its exhausts was answered, with
    /// this many hits to land.
    FiendFireExhausted(u8),
}

/// The rest of an Ironclad card's play that stood still on a question its
/// own command raised.
pub(super) fn resume_waited<F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::BattleTranceDrew => battle_trance_drew(fight),
        Resume::FiendFireExhausted(hits) => fiend_fire_hits(fight, usize::from(hits)),
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}

/// `FiendFire`, once its exhausts are over: its damage to the target, `hits`
/// times.
fn fiend_fire_hits<F: Fight<V>>(fight: &mut F, hits: usize) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let hit = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(CardVar::Damage),
                scaling: Scaling::Dynamic,
            });
            fight.apply(hit);
        }
    });
    Played::Done
}

/// `BattleTrance`, once its draw is over: No Draw on its owner.
fn battle_trance_drew<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.apply_to_actor(Power::NoDrawPower, PowerAmount::Counted(1));
    Played::Done
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::BattleTranceDrew | Resume::FiendFireExhausted(_) => {
            unreachable!("{at:?} waits on a question, not a pick")
        }
        // `CardPileCmd.Add(cardModel, PileType.Draw, CardPilePosition.Top)`.
        Resume::HeadbuttOntoDraw => {
            fight.act_on(PileAct::OntoDrawTop, chosen.as_slice());
            Played::Done
        }
        // `CardCmd.Exhaust` of the one taken.
        Resume::TrueGritExhaust => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            Played::Done
        }
        // `CardCmd.Exhaust` of the one taken, then its cards drawn.
        Resume::BurningPactExhaust => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            fight.draw(fight.var(CardVar::Cards));
            Played::Done
        }
        // `CardCmd.Exhaust` of the one taken, then its Strength.
        Resume::BrandExhaust => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            let strength = fight.var(CardVar::StrengthPower);
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
            Played::Done
        }
        // `CardCmd.Upgrade` of the one taken.
        Resume::ArmamentsUpgrade => {
            for &card in chosen.as_slice() {
                fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
            }
            Played::Done
        }
    }
}

pub(super) fn on_play<F: Fight<V>>(card: IroncladCard, fight: &mut F) -> Played<V> {
    match card {
        IroncladCard::Thrash => thrash(fight),
        // `BattleTrance`: its `Cards` drawn, then No Draw. A shuffle inside
        // the draw can ask, and No Draw waits for it.
        IroncladCard::BattleTrance => {
            fight.draw(fight.var(CardVar::Cards));
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Ironclad(
                        Resume::BattleTranceDrew,
                    )),
                };
            }
            battle_trance_drew(fight)
        }
        // `NotYet`: its `Heal` on its owner.
        IroncladCard::NotYet => {
            fight.heal(fight.actor(), fight.var(CardVar::Heal));
            Played::Done
        }
        // `TearAsunder`: its damage to the target, as many times as its
        // calculated hits, counted before the first, as one attack.
        IroncladCard::TearAsunder => {
            let target = fight.target();
            let hits = fight.var(CardVar::CalculatedHits);
            fight.attack(Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(CardVar::Damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        // `Inferno`: its Inferno on its owner, then one more self damage on
        // the power it landed or stacked into.
        // `CrimsonMantle`: its Crimson Mantle on its owner, then the power's
        // self damage raised by one.
        IroncladCard::CrimsonMantle => {
            let mantle = PowerAmount::Counted(fight.var(CardVar::CrimsonMantlePower));
            fight.apply_to_actor(Power::CrimsonMantlePower, mantle);
            fight.note_on_power(fight.actor(), Power::CrimsonMantlePower, 1);
            Played::Done
        }
        IroncladCard::Inferno => {
            let inferno = PowerAmount::Counted(fight.var(CardVar::InfernoPower));
            fight.apply_to_actor(Power::InfernoPower, inferno);
            fight.note_on_power(fight.actor(), Power::InfernoPower, 1);
            Played::Done
        }
        // `Dominate`: its Vulnerable on the target, then as much Strength on
        // its owner as the target then carries Vulnerable.
        IroncladCard::Dominate => {
            let target = fight.target();
            let vulnerable = PowerAmount::Counted(fight.var(CardVar::VulnerablePower));
            fight.apply_power(target, Power::VulnerablePower, vulnerable);
            let carried = fight
                .creatures()
                .find(|creature| creature.id() == target)
                .and_then(|creature| creature.power(Power::VulnerablePower));
            let strength = match carried {
                Some(PowerAmount::Counted(amount)) => amount,
                Some(PowerAmount::Endless) => unimplemented!("an endless Vulnerable"),
                None => 0,
            };
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
            Played::Done
        }
        // `Feed`: its damage to the target, then its `MaxHp` onto its owner's
        // maximum where the hit killed a target whose death, read before the
        // hit, triggers Fatal.
        IroncladCard::Feed => {
            let target = fight.target();
            let fatal = fight.death_triggers_fatal(target);
            fight.attack(Scaling::Dynamic, |fight| {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            });
            let standing = fight
                .creatures()
                .any(|creature| creature.id() == target && creature.alive());
            if fatal && !standing {
                fight.gain_max_hp(fight.actor(), fight.var(CardVar::MaxHp));
            }
            Played::Done
        }
        // `ForgottenRitual`: its energy, where its owner exhausted a card this
        // turn.
        IroncladCard::ForgottenRitual => {
            if fight.history().exhausted_this_turn() {
                fight.modify_energy(fight.var(CardVar::Energy));
            }
            Played::Done
        }
        // `Spite`: its damage to the target, once, or its `Repeat` times
        // where its owner lost health this turn.
        IroncladCard::Spite => {
            let owner = fight.actor();
            let hurt = fight
                .history()
                .damage_received_this_turn()
                .any(|received| received.receiver == owner && received.unblocked > 0);
            let hits = if hurt { fight.var(CardVar::Repeat) } else { 1 };
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(CardVar::Damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        // `SecondWind`: each card in hand that is not an Attack exhausted,
        // and its block for each, one after another.
        IroncladCard::SecondWind => {
            let cards: Vec<_> = fight
                .cards_in(Pile::Hand)
                .into_iter()
                .filter(|&card| V::card_kind(fight.card_ident(card).card) != Some(CardKind::Attack))
                .collect();
            for card in cards {
                fight.act_on(PileAct::Exhaust, &[card]);
                block(fight);
            }
            Played::Done
        }
        // `PactsEnd`: its damage to every enemy, once the exhaust pile holds
        // its `Cards`.
        IroncladCard::PactsEnd => {
            let exhausted = fight.cards_in(Pile::Exhaust).len();
            if num_traits::cast::<_, super::Number>(exhausted)
                .is_some_and(|exhausted| exhausted >= fight.var(CardVar::Cards))
            {
                fight.attack(Scaling::Dynamic, |fight| {
                    for target in fight.opponents() {
                        let hit = fight.resolve(Effect::Attack {
                            dealer: fight.actor(),
                            target,
                            damage: Amount::Var(CardVar::Damage),
                            scaling: Scaling::Dynamic,
                        });
                        fight.apply(hit);
                    }
                });
            }
            Played::Done
        }
        // `FiendFire`: every card in hand exhausted, then its damage to the
        // target once for each.
        // A hook's question inside an exhaust stands the rest still.
        IroncladCard::FiendFire => {
            let cards = fight.cards_in(Pile::Hand);
            let hits = cards.len();
            for card in cards {
                fight.act_on(PileAct::Exhaust, &[card]);
            }
            if fight.waiting() {
                let hits = u8::try_from(hits).expect("a hand's worth of hits fits a u8");
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Ironclad(
                        Resume::FiendFireExhausted(hits),
                    )),
                };
            }
            fiend_fire_hits(fight, hits)
        }
        // `BurningPact`: one card of the hand exhausted, then its `Cards`
        // drawn.
        IroncladCard::BurningPact => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Card(super::Resume::Ironclad(Resume::BurningPactExhaust)),
        },
        // `Rampage`: its damage to the target, then its `Increase` onto its
        // own damage for the fight.
        IroncladCard::Rampage => {
            let target = fight.target();
            strike(fight, target);
            let increase = fight.var(CardVar::Increase);
            fight.modify_this(Change::Raise(CardVar::Damage, increase), Lasting::ThisFight);
            Played::Done
        }
        // `InfernalBlade`: an Attack of the character's pool, free this turn,
        // into the hand.
        IroncladCard::InfernalBlade => {
            super::super::free_card_into_hand(
                fight,
                |card| V::card_kind(card) == Some(CardKind::Attack),
                true,
            );
            Played::Done
        }
        // `Anger`: its damage to the target, then a clone of itself into the
        // discard pile.
        IroncladCard::Anger => {
            let target = fight.target();
            strike(fight, target);
            fight.clone_this(Pile::Discard);
            Played::Done
        }
        // `Headbutt`: its damage to the target, then one card of the discard
        // pile onto the top of the draw pile.
        IroncladCard::Headbutt => {
            let target = fight.target();
            strike(fight, target);
            Played::Asking {
                pile: Pile::Discard,
                selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Ironclad(Resume::HeadbuttOntoDraw)),
            }
        }
        IroncladCard::Whirlwind => whirlwind(fight),
        IroncladCard::Bloodletting => bloodletting(fight),
        // `Offering`: its `HpLoss` off its owner, unblockable, then its
        // energy and its cards drawn.
        IroncladCard::Offering => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            fight.modify_energy(fight.var(CardVar::Energy));
            fight.draw(fight.var(CardVar::Cards));
            Played::Done
        }
        IroncladCard::Uppercut => uppercut(fight),
        IroncladCard::MoltenFist => molten_fist(fight),
        // `Cinder`: its damage to the target, then a card from hand drawn off
        // `CombatCardSelection`, exhausted.
        IroncladCard::Cinder => {
            let target = fight.target();
            strike(fight, target);
            if let Some(card) = fight.random_card_in(Pile::Hand) {
                fight.act_on(PileAct::Exhaust, &[card]);
            }
            Played::Done
        }
        IroncladCard::Pillage => pillage(fight),
        IroncladCard::TrueGrit => true_grit(fight),
        IroncladCard::Dismantle => dismantle(fight),
        // `Brand`: its `HpLoss` off its owner, unblockable, then one card of
        // the hand exhausted, then its Strength.
        IroncladCard::Brand => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            Played::Asking {
                pile: Pile::Hand,
                selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Ironclad(Resume::BrandExhaust)),
            }
        }
        IroncladCard::PrimalForce => primal_force(fight),
        // `Hemokinesis`: its `HpLoss` off its owner, unblockable, then its
        // damage to the target.
        IroncladCard::Hemokinesis => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            let target = fight.target();
            strike(fight, target);
            Played::Done
        }
        // `Breakthrough`: its `HpLoss` off its owner, unblockable, then its
        // damage to every enemy.
        IroncladCard::Breakthrough => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            fight.attack(Scaling::Dynamic, |fight| {
                for target in fight.opponents() {
                    let attack = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(CardVar::Damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(attack);
                }
            });
            Played::Done
        }
        // `BloodWall`: its `HpLoss` off its owner, unblockable, then its
        // block.
        IroncladCard::BloodWall => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            block(fight);
            Played::Done
        }
        IroncladCard::Armaments => armaments(fight),
        // `ExpectAFight`: its calculated energy, then No Energy Gain on its
        // owner.
        IroncladCard::ExpectAFight => {
            fight.modify_energy(fight.var(CardVar::CalculatedEnergy));
            fight.apply_to_actor(Power::NoEnergyGainPower, PowerAmount::Counted(1));
            Played::Done
        }
        // `Cascade`: the top cards of the draw pile auto-played, as many as
        // its X spent and one more once upgraded.
        IroncladCard::Cascade => {
            let count = fight.x_value() + super::Number::from(fight.upgraded());
            let count = usize::try_from(count).expect("an X is not negative");
            fight.auto_play_from_draw(count, false);
            Played::Done
        }
        // `EvilEye`: its block, and again where its owner exhausted a card
        // this turn. A fight here has one player, whose cards are all its
        // owner's.
        IroncladCard::EvilEye => {
            let gains = if fight.history().exhausted_this_turn() {
                2
            } else {
                1
            };
            for _ in 0..gains {
                let block = fight.resolve(Effect::GainBlock {
                    recipient: fight.actor(),
                    amount: Amount::Var(CardVar::Block),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(block);
            }
            Played::Done
        }
        // `Stoke`: the whole hand exhausted, one card at a time. Then as many
        // cards of the character's pool, each `NextItem` off
        // `CombatCardGeneration`, upgraded once Stoke is, into hand.
        IroncladCard::Stoke => {
            let hand = fight.cards_in(Pile::Hand);
            let count = hand.len();
            fight.act_on(sts2_core::PileAct::Exhaust, &hand);
            let pool = <V as sts2_core::Rules>::combat_cards(fight.character());
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
            Played::Done
        }
        // `Havoc`: the top card of the draw pile auto-played, and exhausted.
        IroncladCard::Havoc => {
            fight.auto_play_from_draw(1, true);
            Played::Done
        }
        // `PerfectedStrike`: its calculated damage to the target. Its test of
        // that damage against twelve picks an animation.
        IroncladCard::PerfectedStrike => {
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::CalculatedDamage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            });
            Played::Done
        }
        card => card_plays::ironclad::on_play(card, fight),
    }
}

/// `Dismantle`: its damage to the target, twice where the target carries
/// Vulnerable.
fn dismantle<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let vulnerable = fight.creatures().any(|creature| {
        creature.id() == target && creature.power(Power::VulnerablePower).is_some()
    });
    for _ in 0..1 + u8::from(vulnerable) {
        strike(fight, target);
    }
    Played::Done
}

/// `PrimalForce`: each Attack in hand transformed into a Giant Rock,
/// upgraded where Primal Force is. A card in a fight is always
/// transformable.
fn primal_force<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};

    let attacks: Vec<_> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| V::card_kind(fight.card_ident(card).card) == Some(CardKind::Attack))
        .collect();
    for card in attacks {
        let mut into = sts2_core::CardIdent::new(Cards::Token(TokenCard::GiantRock));
        into.upgrade = u8::from(fight.upgraded());
        fight.transform(card, into);
    }
    Played::Done
}

/// Its block, onto its owner.
fn block<F: Fight<V>>(fight: &mut F) {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
}

/// `TrueGrit`: its block, then a card from hand exhausted. Upgraded, its
/// owner picks the card. Otherwise it is drawn off `CombatCardSelection`.
fn true_grit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    block(fight);
    if fight.upgraded() {
        return Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Card(super::Resume::Ironclad(Resume::TrueGritExhaust)),
        };
    }
    if let Some(card) = fight.random_card_in(Pile::Hand) {
        fight.act_on(PileAct::Exhaust, &[card]);
    }
    Played::Done
}

/// `Armaments`: its block, then a card in hand upgraded. Upgraded, every
/// upgradable card in hand is. Otherwise its owner picks one of them, which
/// `FromHandForUpgrade` takes without asking where there is one.
fn armaments<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

    block(fight);
    let upgradable: smallvec::SmallVec<[_; 10]> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| {
            let ident = fight.card_ident(card);
            ident.upgrade < ident.card.max_upgrade()
        })
        .collect();
    if fight.upgraded() {
        for card in upgradable {
            fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
        }
        return Played::Done;
    }
    Played::AskingAmong {
        cards: upgradable,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Ironclad(Resume::ArmamentsUpgrade)),
    }
}

/// `Bloodletting`: its `HpLoss` off its owner, unblockable, then its energy.
fn bloodletting<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
    fight.modify_energy(fight.var(CardVar::Energy));
    Played::Done
}

/// One hit of the card's damage on `target`.
fn strike<F: Fight<V>>(fight: &mut F, target: sts2_core::CombatId) {
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

/// `Thrash`: its damage to the target twice. Then an Attack in hand drawn
/// off `CombatCardSelection` is exhausted. Its damage, modified as its owner
/// deals it to nobody, joins Thrash's own for the rest of the fight.
///
/// The damage read is its `CalculatedDamage` worked out against nobody,
/// else its `Damage`, else its `OstyDamage`, else nothing.
fn thrash<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{CardTable as _, RunStreamKinds as _};

    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..2 {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(CardVar::Damage),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
    let attacks: Vec<_> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack))
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_SELECTION;
    let Some(chosen) = fight.random_card(&attacks, stream) else {
        return Played::Done;
    };
    let vars = fight.card_ident(chosen).card.vars();
    let printed = [
        CardVar::CalculatedDamage,
        CardVar::Damage,
        CardVar::OstyDamage,
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
        Change::RaiseExactly(CardVar::Damage, damage),
        Lasting::ThisFight,
    );
    fight.act_on(PileAct::Exhaust, &[chosen]);
    Played::Done
}

/// `Uppercut`: its damage to the target, then its `Power` as Weak and as
/// Vulnerable on it.
fn uppercut<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    strike(fight, target);
    let amount = PowerAmount::Counted(fight.var(CardVar::Power));
    fight.apply_power(target, Power::WeakPower, amount);
    fight.apply_power(target, Power::VulnerablePower, amount);
    Played::Done
}

/// `MoltenFist`: its damage to the target, then as much Vulnerable again as
/// the target holds, if it still stands.
fn molten_fist<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    strike(fight, target);
    let held = fight
        .creatures()
        .find(|creature| creature.id() == target && creature.alive())
        .and_then(|creature| creature.power(Power::VulnerablePower));
    match held {
        Some(PowerAmount::Counted(held)) if held > 0 => {
            fight.apply_power(target, Power::VulnerablePower, PowerAmount::Counted(held))
        }
        Some(PowerAmount::Endless) => unimplemented!("an endless Vulnerable"),
        _ => {}
    }
    Played::Done
}

/// `Pillage`: its damage to the target, then one card drawn at a time. The
/// draws go on while each is an Attack and the hand has room.
fn pillage<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    strike(fight, target);
    loop {
        let before = fight.cards_in(Pile::Hand);
        fight.draw(1);
        let after = fight.cards_in(Pile::Hand);
        let Some(drawn) = after
            .iter()
            .find(|card| !before.iter().any(|held| held.id() == card.id()))
        else {
            break;
        };
        let attack = V::card_kind(fight.card_ident(*drawn).card) == Some(CardKind::Attack);
        if !attack || after.len() >= sts2_core::MAX_CARDS_IN_HAND {
            break;
        }
    }
    Played::Done
}

/// `Whirlwind`: its damage to every enemy, once for each energy of its X.
fn whirlwind<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let hits = fight.x_value();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            for target in fight.opponents() {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        }
    });
    Played::Done
}

/// Deliver a notification to one Ironclad card in the fight.
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: IroncladCard,
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
        (IroncladCard::Stomp, EventHook::CardPlaying { card, .. }) => {
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
        (IroncladCard::Stomp, EventHook::CardEnteredCombat { at, clone, .. }) => {
            if at.id() == id && !clone {
                let attacks: super::Number = history
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
        (IroncladCard::DrumOfBattle, EventHook::CardExhausted { id: exhausted, .. }) => {
            if exhausted == id {
                asked.push(HookEffect::GainEnergyForEachPlay {
                    card: id,
                    energy: printed.face(CardVar::Energy),
                });
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
