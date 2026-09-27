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
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
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
        IroncladCard::Armaments => armaments(fight),
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
pub(super) fn event_hook(
    card: IroncladCard,
    id: sts2_core::CardId<V>,
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
                unimplemented!("a Drum of Battle exhausted, which gains energy for each play")
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
