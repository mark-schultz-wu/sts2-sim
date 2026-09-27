//! The Defect's cards the generator leaves to a hand port.

use sts2_core::{Amount, Effect, Fight, Pile, PileAct, Played, Scaling};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, DefectCard, StatusCard};
use super::super::super::generated::orbs::Orb;
use super::V;

/// Where a Defect card's play stops to ask something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Hologram`, after its owner picked the card to take back.
    HologramIntoHand,
    /// `Scavenge`, after its owner picked the card to exhaust.
    ScavengeExhaust,
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardPileCmd.Add(cardModel, PileType.Hand)`.
        Resume::HologramIntoHand => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        // `CardCmd.Exhaust` of the one taken, then its `Energy` as Energy
        // Next Turn.
        Resume::ScavengeExhaust => {
            use super::super::super::generated::powers::Power;

            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            let energy = sts2_core::PowerAmount::Counted(fight.var(CardVar::Energy));
            fight.apply_to_actor(Power::EnergyNextTurnPower, energy);
            Played::Done
        }
    }
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

/// `CardPileCmd.AddGeneratedCardToCombat` of one `status` into the discard
/// pile.
fn add_status<F: Fight<V>>(fight: &mut F, status: StatusCard) {
    use super::super::super::generated::cards::Cards;

    fight.add_generated_cards(
        sts2_core::CardIdent::new(Cards::Status(status)),
        Pile::Discard,
        1,
    );
}

/// `Ftl`: its damage to the target, then its `Cards` drawn while its owner
/// has finished fewer plays this turn than its `PlayMax`.
fn ftl<F: Fight<V>>(fight: &mut F) -> Played<V> {
    hit(fight, fight.target(), 1);
    let finished = fight.history().finished_this_turn().count();
    let finished = super::Number::try_from(finished).expect("a turn's plays fit a game number");
    if finished < fight.var(CardVar::PlayMax) {
        fight.draw(fight.var(CardVar::Cards));
    }
    Played::Done
}

/// `Compact`: its block, then each Status in hand transformed into a Fuel,
/// upgraded where Compact is. A card in a fight is always transformable.
fn compact<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};
    use sts2_core::Rules as _;

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let statuses: Vec<_> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| {
            V::card_kind(fight.card_ident(card).card) == Some(sts2_core::CardKind::Status)
        })
        .collect();
    for card in statuses {
        let mut into = sts2_core::CardIdent::new(Cards::Token(TokenCard::Fuel));
        into.upgrade = u8::from(fight.upgraded());
        fight.transform(card, into);
    }
    Played::Done
}

pub(super) fn on_play<F: Fight<V>>(card: DefectCard, fight: &mut F) -> Played<V> {
    match card {
        DefectCard::Barrage => barrage(fight),
        // `DoubleEnergy`: as much energy again as its owner has left.
        DefectCard::DoubleEnergy => {
            fight.modify_energy(fight.energy());
            Played::Done
        }
        // `Chill`: a Frost channelled for each enemy that can be hit, the
        // enemies counted once.
        DefectCard::Chill => {
            for _ in fight.hittable_opponents() {
                fight.channel(Orb::FrostOrb);
            }
            Played::Done
        }
        DefectCard::Compact => compact(fight),
        DefectCard::Ftl => ftl(fight),
        // `Turbo`: its energy, then a Void into the discard pile.
        DefectCard::Turbo => {
            fight.modify_energy(fight.var(CardVar::Energy));
            add_status(fight, StatusCard::Void);
            Played::Done
        }
        // `Sunder`: its damage to the target, then its energy where the hit
        // killed it.
        DefectCard::Sunder => {
            use sts2_core::CreatureView as _;

            let target = fight.target();
            hit(fight, target, 1);
            let standing = fight
                .creatures()
                .any(|creature| creature.id() == target && creature.alive());
            if !standing {
                fight.modify_energy(fight.var(CardVar::Energy));
            }
            Played::Done
        }
        // `Capacitor`: its `Repeat` more orb slots.
        DefectCard::Capacitor => {
            fight.add_orb_slots(fight.var(CardVar::Repeat));
            Played::Done
        }
        // `Scavenge`: one card of the hand exhausted, then energy next turn.
        DefectCard::Scavenge => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Card(super::Resume::Defect(Resume::ScavengeExhaust)),
        },
        // `Glacier`: its block, then two Frosts channelled.
        DefectCard::Glacier => {
            block(fight);
            fight.channel(Orb::FrostOrb);
            fight.channel(Orb::FrostOrb);
            Played::Done
        }
        // `Overclock`: its `Cards` drawn, then a Burn into the discard pile.
        DefectCard::Overclock => {
            fight.draw(fight.var(CardVar::Cards));
            add_status(fight, StatusCard::Burn);
            Played::Done
        }
        // `AdaptiveStrike`: its damage to the target, then a clone of itself
        // into the discard pile, free for the rest of the fight.
        DefectCard::AdaptiveStrike => {
            hit(fight, fight.target(), 1);
            if let Some(clone) = fight.clone_this(Pile::Discard) {
                fight.modify_card(
                    clone,
                    sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
                    sts2_core::Lasting::ThisFight,
                );
            }
            Played::Done
        }
        // `BoostAway`: its block, then a Dazed into the discard pile.
        DefectCard::BoostAway => {
            block(fight);
            add_status(fight, StatusCard::Dazed);
            Played::Done
        }
        // `FightThrough`: its block, then two Wounds into the discard pile,
        // one at a time.
        DefectCard::FightThrough => {
            block(fight);
            add_status(fight, StatusCard::Wound);
            add_status(fight, StatusCard::Wound);
            Played::Done
        }
        // `BulkUp`: its `OrbSlots` fewer orb slots, then its Strength and its
        // Dexterity.
        DefectCard::BulkUp => {
            use super::super::super::generated::powers::Power;

            fight.remove_orb_slots(fight.var(CardVar::OrbSlots));
            let strength = fight.var(CardVar::StrengthPower);
            fight.apply_to_actor(
                Power::StrengthPower,
                sts2_core::PowerAmount::Counted(strength),
            );
            let dexterity = fight.var(CardVar::DexterityPower);
            fight.apply_to_actor(
                Power::DexterityPower,
                sts2_core::PowerAmount::Counted(dexterity),
            );
            Played::Done
        }
        // `Quadcast`: with any orb held, the first orb evoked its `Repeat`
        // times, leaving on the last.
        DefectCard::Quadcast => {
            if fight.orbs_held() > 0 {
                let repeat = fight.var(CardVar::Repeat);
                for evoked in 1..=repeat {
                    fight.evoke_next(evoked == repeat);
                }
            }
            Played::Done
        }
        DefectCard::Claw => claw(fight),
        DefectCard::CompileDriver => compile_driver(fight),
        DefectCard::GoForTheEyes => go_for_the_eyes(fight),
        DefectCard::GunkUp => gunk_up(fight),
        DefectCard::Hologram => hologram(fight),
        DefectCard::Dualcast => dualcast(fight),
        DefectCard::Tempest => tempest(fight),
        DefectCard::Scrape => scrape(fight),
        DefectCard::MomentumStrike => momentum_strike(fight),
        DefectCard::TeslaCoil => tesla_coil(fight),
        card => card_plays::defect::on_play(card, fight),
    }
}

/// Its damage to `target`, `hits` times, as one attack.
fn hit<F: Fight<V>>(fight: &mut F, target: sts2_core::CombatId, hits: super::Number) {
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(CardVar::Damage),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
}

/// `Barrage`: its damage to the target, once for each of its
/// `CalculatedHits`.
fn barrage<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let hits = fight.var(CardVar::CalculatedHits);
    hit(fight, target, hits);
    Played::Done
}

/// `Claw`: its damage to the target, then its `Increase` onto the damage of
/// every Claw its owner holds in the fight, itself included.
fn claw<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::Cards;

    let target = fight.target();
    hit(fight, target, 1);
    let raise = sts2_core::Change::Raise(CardVar::Damage, fight.var(CardVar::Increase));
    // `AllCards`: the hand, the draw, discard and exhaust piles, then the
    // play pile, which holds the Claw being played.
    for pile in [Pile::Hand, Pile::Draw, Pile::Discard, Pile::Exhaust] {
        for card in fight.cards_in(pile) {
            if fight.card_ident(card).card == Cards::Defect(DefectCard::Claw) {
                fight.modify_card(card, raise, sts2_core::Lasting::ThisFight);
            }
        }
    }
    fight.modify_this(raise, sts2_core::Lasting::ThisFight);
    Played::Done
}

/// `CompileDriver`: its damage to the target, then as many cards drawn as
/// its `CalculatedCards`.
fn compile_driver<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    hit(fight, target, 1);
    let cards = fight.var(CardVar::CalculatedCards);
    fight.draw(cards);
    Played::Done
}

/// `GoForTheEyes`: its damage to the target, then its Weak on it where the
/// target `IntendsToAttack`: an attack or a death blow among its intents.
fn go_for_the_eyes<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CreatureView as _;

    use super::super::super::generated::monsters::IntentKind;
    use super::super::super::generated::powers::Power;

    let target = fight.target();
    hit(fight, target, 1);
    let attacking = fight
        .creatures()
        .find(|creature| creature.id() == target)
        .is_some_and(|creature| {
            creature
                .intents()
                .any(|intent| matches!(intent, IntentKind::Attack | IntentKind::DeathBlow))
        });
    if attacking {
        let weak = sts2_core::PowerAmount::Counted(fight.var(CardVar::WeakPower));
        fight.apply_power(target, Power::WeakPower, weak);
    }
    Played::Done
}

/// `GunkUp`: its damage to the target, once for each of its `Repeat`, then a
/// Slimed into the discard pile.
fn gunk_up<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, StatusCard};

    let target = fight.target();
    let hits = fight.var(CardVar::Repeat);
    hit(fight, target, hits);
    fight.add_generated_cards(
        sts2_core::CardIdent::new(Cards::Status(StatusCard::Slimed)),
        Pile::Discard,
        1,
    );
    Played::Done
}

/// `Hologram`: its block, then one card of the discard pile into the hand.
fn hologram<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    Played::Asking {
        pile: Pile::Discard,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Defect(Resume::HologramIntoHand)),
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
