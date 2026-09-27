//! The Defect's cards the generator leaves to a hand port.

use sts2_core::{Amount, Effect, Fight, Pile, PileAct, Played, Scaling};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, DefectCard};
use super::super::super::generated::orbs::Orb;
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: DefectCard, fight: &mut F) -> Played<V> {
    match card {
        DefectCard::Dualcast => dualcast(fight),
        DefectCard::Tempest => tempest(fight),
        DefectCard::Scrape => scrape(fight),
        DefectCard::MomentumStrike => momentum_strike(fight),
        DefectCard::TeslaCoil => tesla_coil(fight),
        card => card_plays::defect::on_play(card, fight),
    }
}

/// `MomentumStrike`: its damage to the target, then its own cost set to
/// nothing for the rest of the fight.
fn momentum_strike<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    fight.modify_this(
        sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
        sts2_core::Lasting::ThisFight,
    );
    Played::Done
}

/// `Scrape`: its damage to the target, then its `Cards` drawn, and each drawn
/// card whose own cost is anything but nothing discarded.
fn scrape<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    let before = fight.cards_in(Pile::Hand);
    fight.draw(fight.var(CardVar::Cards));
    let costly: Vec<sts2_core::CardAt<V>> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|card| !before.iter().any(|held| held.id() == card.id()))
        .filter(|&card| fight.local_cost(card) != sts2_core::Cost::fixed(0))
        .collect();
    fight.act_on(PileAct::Discard, &costly);
    Played::Done
}

/// `Tempest`: a Lightning channelled for each energy of its X, and one more
/// once upgraded.
fn tempest<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let orbs = fight.x_value() + super::Number::from(fight.upgraded());
    for _ in 0..orbs {
        fight.channel(Orb::LightningOrb);
    }
    Played::Done
}

/// `Dualcast`. Evoke the first orb twice: once where it stands, then once
/// as it leaves. Nothing with no orb.
fn dualcast<F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.orbs_held() > 0 {
        fight.evoke_next(false);
        fight.evoke_next(true);
    }
    Played::Done
}

/// `TeslaCoil`: its `Damage` at its target, then each Lightning's passive at
/// the same target, twice for an upgraded copy. The Lightnings are listed as
/// the damage leaves them.
fn tesla_coil<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let hit = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(hit);
    });
    let times = if fight.upgraded() { 2 } else { 1 };
    let lightning: Vec<_> = fight
        .channelled()
        .into_iter()
        .filter(|orb| orb.orb == Orb::LightningOrb)
        .collect();
    for orb in &lightning {
        for _ in 0..times {
            super::super::orbs::passive_at(orb, target, fight);
        }
    }
    Played::Done
}
