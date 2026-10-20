//! The Silent's cards the generator leaves to a hand port.

use sts2_core::{
    Amount, CardIdent, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight,
    HookEffect, Lasting, Pile, Played, PowerAmount, Rules as _, Scaling, Selection,
};

use super::super::super::generated::cards::{CardKeyword, CardVar, Cards, SilentCard, TokenCard};
use super::super::super::generated::powers::Power;
use super::V;

/// Where a Silent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Survivor`, after its owner picked the card to discard.
    Survivor,
    /// `Prepared`, after its owner picked the cards to discard.
    Prepared,
    /// `DaggerThrow`, after its owner picked the card to discard.
    DaggerThrow,
    /// `HiddenDaggers`, after its owner picked the cards to discard.
    HiddenDaggers,
    /// `HiddenDaggers`, once a Sly card its discard played has finished.
    HiddenDaggersShivs,
    /// `Acrobatics`, after its owner picked the card to discard.
    Acrobatics,
    /// `HandTrick`, after its owner picked the skill to make Sly.
    HandTrick,
    /// `Nightmare`, after its owner picked the card to copy.
    Nightmare,
    /// `EscapePlan`, after a question inside its draw was answered.
    EscapePlanDrew,
    /// `ShadowStep`, once a card its discard played has finished.
    ShadowStepDiscarded,
}

pub(super) fn on_play<F: Fight<V>>(card: SilentCard, fight: &mut F) -> Played<V> {
    match card {
        SilentCard::Survivor => survivor(fight),
        SilentCard::Prepared => prepared(fight),
        SilentCard::DaggerSpray => dagger_spray(fight),
        SilentCard::LeadingStrike => leading_strike(fight),
        SilentCard::Skewer => skewer(fight),
        SilentCard::DaggerThrow => dagger_throw(fight),
        SilentCard::Haze => haze(fight),
        SilentCard::Malaise => malaise(fight),
        SilentCard::UpMySleeve => up_my_sleeve(fight),
        SilentCard::BladeDance => blade_dance(fight),
        SilentCard::PiercingWail => piercing_wail(fight),
        SilentCard::EchoingSlash => echoing_slash(fight),
        SilentCard::HandTrick => hand_trick(fight),
        // `Nightmare`: one card of the hand picked, to copy into hand before
        // the next hand is drawn.
        SilentCard::Nightmare => Played::Asking {
            pile: Pile::Hand,
            selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Card(super::Resume::Silent(Resume::Nightmare)),
        },
        // `BladeOfInk`: its `Cards` of Shivs into hand, each enchanted with
        // one Inky. `CardCmd.Enchant` raises no hook, so the Shivs arrive
        // enchanted.
        SilentCard::BladeOfInk => {
            use super::super::super::generated::enchantments::Enchantment;

            let mut shiv = CardIdent::new(Cards::Token(TokenCard::Shiv));
            <V as sts2_core::Rules>::enchant(Enchantment::Inky, 1, &mut shiv);
            let count: usize =
                num_traits::cast(fight.var(CardVar::Cards)).expect("a count of Shivs");
            fight.add_generated_cards(shiv, Pile::Hand, count);
            Played::Done
        }
        // `KnifeTrap`: each Shiv of the exhaust pile, upgraded where Knife
        // Trap is, auto-played at its target, one at a time.
        SilentCard::KnifeTrap => {
            let target = fight.target();
            let shivs: Vec<_> = fight
                .cards_in(Pile::Exhaust)
                .into_iter()
                .filter(|&at| {
                    sts2_core::CardTable::tagged(
                        fight.card_ident(at).card,
                        sts2_core::CardTagKinds::SHIV,
                    )
                })
                .map(|at| at.id())
                .collect();
            let upgraded = fight.upgraded();
            for id in shivs {
                let find = |fight: &F| {
                    fight
                        .cards_in(Pile::Exhaust)
                        .into_iter()
                        .find(|at| at.id() == id)
                };
                if upgraded && let Some(at) = find(fight) {
                    fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
                }
                if let Some(at) = find(fight) {
                    fight.auto_play_card_at(at, target);
                }
            }
            Played::Done
        }
        // `Scare`: one Weak on each enemy that can be hit.
        SilentCard::Scare => {
            for target in fight.hittable_opponents() {
                fight.apply_power(target, Power::WeakPower, PowerAmount::Counted(1));
            }
            Played::Done
        }
        // `Expose`: the target loses all its block and any Artifact, then
        // takes its `Power` of Vulnerable.
        SilentCard::Expose => {
            let target = fight.target();
            let block = fight
                .creatures()
                .find(|creature| creature.id() == target)
                .map_or(0, |creature| creature.block());
            fight.lose_block(target, block);
            fight.remove_power(target, Power::ArtifactPower);
            let amount = fight.var(CardVar::Power);
            fight.apply_power(target, Power::VulnerablePower, PowerAmount::Counted(amount));
            Played::Done
        }
        // `DodgeAndRoll`: its block, then as much as it gave as Block Next
        // Turn.
        SilentCard::DodgeAndRoll => {
            let actor = fight.actor();
            let held = |fight: &F| {
                fight
                    .creatures()
                    .find(|creature| creature.id() == actor)
                    .map_or(0, |creature| creature.block())
            };
            let before = held(fight);
            let block = fight.resolve(Effect::GainBlock {
                recipient: actor,
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let gained = held(fight) - before;
            fight.apply_to_actor(Power::BlockNextTurnPower, PowerAmount::Counted(gained));
            Played::Done
        }
        // `EscapePlan`: a card drawn, then its block where that card is a
        // Skill. A shuffle inside the draw can ask, and the block waits.
        SilentCard::EscapePlan => {
            let drew = fight.draw(1);
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Silent(Resume::EscapePlanDrew)),
                };
            }
            escape_plan_drew(fight, &drew)
        }
        // `CloakAndDagger`: its block, then its `Cards` of Shivs into hand
        // one at a time.
        SilentCard::CloakAndDagger => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let count: usize = num_traits::cast(fight.var(CardVar::Cards))
                .expect("a count of Shivs is not negative");
            for _ in 0..count {
                fight.add_generated_cards(
                    CardIdent::new(Cards::Token(TokenCard::Shiv)),
                    Pile::Hand,
                    1,
                );
            }
            Played::Done
        }
        // `Tracking`: Tracking on its owner, two the first time and one after.
        SilentCard::Tracking => {
            let actor = fight.actor();
            let tracking = fight.creatures().any(|creature| {
                creature.id() == actor && creature.power(Power::TrackingPower).is_some()
            });
            let amount = if tracking { 1 } else { 2 };
            fight.apply_to_actor(Power::TrackingPower, PowerAmount::Counted(amount));
            Played::Done
        }
        // `Acrobatics`: its `Cards` drawn, then one card of the hand
        // discarded.
        SilentCard::Acrobatics => {
            fight.draw(fight.var(CardVar::Cards));
            Played::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Silent(Resume::Acrobatics)),
            }
        }
        // `Finisher`: its damage to the target, once for each attack its
        // owner finished this turn.
        SilentCard::Finisher => {
            hit_target(fight, fight.var(CardVar::CalculatedHits));
            Played::Done
        }
        // `Flechettes`: its damage to the target, once for each Skill in its
        // owner's hand.
        SilentCard::Flechettes => {
            hit_target(fight, fight.var(CardVar::CalculatedHits));
            Played::Done
        }
        // `Expertise`: cards drawn up to its `Cards` in hand.
        SilentCard::Expertise => {
            let hand = super::Number::try_from(fight.cards_in(Pile::Hand).len())
                .expect("a hand fits a game number");
            fight.draw((fight.var(CardVar::Cards) - hand).max(0));
            Played::Done
        }
        // `HiddenDaggers`: its `Cards` of the hand discarded, then Shivs.
        SilentCard::HiddenDaggers => {
            let count = usize::try_from(fight.var(CardVar::Cards))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(count),
                at: super::super::Resume::Card(super::Resume::Silent(Resume::HiddenDaggers)),
            }
        }
        // `BouncingFlask`: its Poison on an enemy drawn off `CombatTargets`,
        // its `Repeat` times.
        SilentCard::BouncingFlask => {
            let poison = PowerAmount::Counted(fight.var(CardVar::PoisonPower));
            for _ in 0..fight.var(CardVar::Repeat) {
                if let Some(target) = fight.random_opponent() {
                    fight.apply_power(target, Power::PoisonPower, poison);
                }
            }
            Played::Done
        }
        // `BubbleBubble`: its Poison on the target, where it already carries
        // Poison.
        SilentCard::BubbleBubble => {
            let target = fight.target();
            let poisoned = fight.creatures().any(|creature| {
                creature.id() == target && creature.power(Power::PoisonPower).is_some()
            });
            if poisoned {
                let poison = PowerAmount::Counted(fight.var(CardVar::PoisonPower));
                fight.apply_power(target, Power::PoisonPower, poison);
            }
            Played::Done
        }
        // `Adrenaline`: its energy, then its `Cards` drawn.
        SilentCard::Adrenaline => {
            fight.modify_energy(fight.var(CardVar::Energy));
            fight.draw(fight.var(CardVar::Cards));
            Played::Done
        }
        // `CalculatedGamble`: the whole hand discarded, then as many drawn.
        SilentCard::CalculatedGamble => {
            let hand = fight.cards_in(Pile::Hand);
            let count = super::Number::try_from(hand.len()).expect("a hand fits a game number");
            fight.discard_and_draw(&hand, count);
            Played::Done
        }
        // `Mirage`: its calculated block.
        SilentCard::Mirage => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::CalculatedBlock),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        // `StormOfSteel`: the whole hand discarded, then as many Shivs into
        // hand, each upgraded once Storm of Steel is.
        SilentCard::StormOfSteel => {
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(sts2_core::PileAct::Discard, &hand);
            let shiv = CardIdent::new(Cards::Token(TokenCard::Shiv));
            let upgraded = fight.upgraded();
            for at in fight.add_generated_cards(shiv, Pile::Hand, hand.len()) {
                if upgraded {
                    fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
                }
            }
            Played::Done
        }
        // `BulletTime`: each card of the hand that does not cost X free this
        // turn, then one No Draw on its owner.
        SilentCard::BulletTime => {
            for at in fight.cards_in(Pile::Hand) {
                if sts2_core::CardTable::cost(fight.card_ident(at).card) != sts2_core::Cost::X {
                    super::super::free_this_turn(fight, at);
                }
            }
            fight.apply_to_actor(Power::NoDrawPower, PowerAmount::Counted(1));
            Played::Done
        }
        // `FanOfKnives`: one Fan of Knives on its owner, then its `Shivs`
        // into hand one at a time.
        SilentCard::FanOfKnives => {
            fight.apply_to_actor(Power::FanOfKnivesPower, PowerAmount::Counted(1));
            let count: usize =
                num_traits::cast(fight.var(CardVar::Shivs)).expect("a count of Shivs");
            for _ in 0..count {
                fight.add_generated_cards(
                    CardIdent::new(Cards::Token(TokenCard::Shiv)),
                    Pile::Hand,
                    1,
                );
            }
            Played::Done
        }
        // `GrandFinale`: its damage to every enemy, as one attack.
        SilentCard::GrandFinale => {
            fight.attack(Scaling::Dynamic, |fight| {
                let targets = fight.opponents();
                // One `CreatureCmd.Damage` over them all.
                fight.damage_command(|fight| {
                    for target in targets {
                        let attack = fight.resolve(Effect::Attack {
                            dealer: fight.actor(),
                            target,
                            damage: Amount::Var(CardVar::Damage),
                            scaling: Scaling::Dynamic,
                        });
                        fight.apply(attack);
                    }
                });
            });
            Played::Done
        }
        // `ShadowStep`: the whole hand discarded, then one Shadow Step on its
        // owner.
        // `ShadowStep`: the hand discarded, which a Sly card it holds can
        // stand still behind a question. Then its power.
        SilentCard::ShadowStep => {
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(sts2_core::PileAct::Discard, &hand);
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Silent(
                        Resume::ShadowStepDiscarded,
                    )),
                };
            }
            shadow_step_discarded(fight)
        }
        // `TheHunt`: its damage to the target. Where a result says
        // `WasTargetKilled` of a target whose death, read before the hit,
        // triggers Fatal, the room offers one more card reward, and its owner
        // takes one The Hunt.
        SilentCard::TheHunt => {
            let target = fight.target();
            let fatal = fight.death_triggers_fatal(target);
            let results = hit_target(fight, 1);
            if fatal && results.iter().any(|result| result.killed) {
                fight.add_card_reward();
                fight.apply_to_actor(Power::TheHuntPower, PowerAmount::Counted(1));
            }
            Played::Done
        }
        card => super::generated_from(Cards::Silent(card), fight, 0),
    }
}

/// `Malaise`: its X, one more once upgraded, off the target's Strength, then
/// as Weak on it.
fn malaise<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = fight.x_value() + super::Number::from(fight.upgraded());
    let target = fight.target();
    fight.apply_power(target, Power::StrengthPower, PowerAmount::Counted(-amount));
    fight.apply_power(target, Power::WeakPower, PowerAmount::Counted(amount));
    Played::Done
}

/// `Haze`: its Poison onto each enemy that can be hit, one at a time.
fn haze<F: Fight<V>>(fight: &mut F) -> Played<V> {
    for target in fight.hittable_opponents() {
        fight.apply_power(
            target,
            Power::PoisonPower,
            PowerAmount::Counted(fight.var(CardVar::PoisonPower)),
        );
    }
    Played::Done
}

/// `Nightmare`'s `PowerCmd.Apply<NightmarePower>(…, 3m, …)`, which it prints
/// nowhere.
const NIGHTMARE: i32 = 3;

/// `HandTrick`: its block, then one skill of the hand that is not Sly this
/// turn picked, to be Sly until the turn ends.
fn hand_trick<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| {
            V::card_kind(fight.card_ident(card).card) == Some(CardKind::Skill)
                && !fight.card_keyword(card, CardKeyword::Sly)
        })
        .collect();
    Played::AskingAmong {
        cards,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::HandTrick)),
    }
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardCmd.Discard` of the one taken.
        Resume::Survivor | Resume::Prepared | Resume::DaggerThrow | Resume::Acrobatics => {
            fight.act_on(sts2_core::PileAct::Discard, chosen.as_slice());
            Played::Done
        }
        // Three of `NightmarePower` on its owner, which keeps a clone of the
        // card taken, where a card was taken.
        Resume::Nightmare => {
            if let Some(card) = chosen.one_or_none() {
                let owner = fight.actor();
                fight.apply_to_actor(Power::NightmarePower, PowerAmount::Counted(NIGHTMARE));
                fight.keep_card_on_power(owner, Power::NightmarePower, card);
            }
            Played::Done
        }
        // `CardCmd.ApplySingleTurnSly` on the one taken.
        Resume::HandTrick => {
            for &card in chosen.as_slice() {
                let sly = Change::AddKeyword(CardKeyword::Sly);
                fight.modify_card(card, sly, Lasting::ThisTurn);
            }
            Played::Done
        }
        // `CardCmd.Discard` of those taken, then its `Shivs` into hand,
        // each upgraded where Hidden Daggers is.
        // `CardCmd.Discard`, which awaits a Sly card it plays, then Shivs.
        Resume::HiddenDaggers => {
            fight.act_on(sts2_core::PileAct::Discard, chosen.as_slice());
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Silent(
                        Resume::HiddenDaggersShivs,
                    )),
                };
            }
            hidden_daggers_shivs(fight)
        }
        Resume::HiddenDaggersShivs => {
            unreachable!("Hidden Daggers' Shivs wait on a card, not a pick")
        }
        Resume::EscapePlanDrew => unreachable!("Escape Plan waits on its draw, not a pick"),
        Resume::ShadowStepDiscarded => {
            unreachable!("Shadow Step waits on a card, not a pick")
        }
    }
}

/// The rest of a Silent card's play that waited on a card it auto-played.
///
/// # Panics
///
/// A resume point no play waits at.
/// `EscapePlan`, once its draw is over: its block where the card it drew is
/// a Skill, wherever that card stands now.
fn escape_plan_drew<F: Fight<V>>(fight: &mut F, drew: &[sts2_core::CardId<V>]) -> Played<V> {
    let skill = drew.first().is_some_and(|&id| {
        let drawn = fight
            .find_card(id)
            .unwrap_or_else(|| unimplemented!("Escape Plan reading {id:?}, which left the fight"));
        V::card_kind(fight.card_ident(drawn).card) == Some(CardKind::Skill)
    });
    if skill {
        let block = fight.resolve(Effect::GainBlock {
            recipient: fight.actor(),
            amount: Amount::Var(CardVar::Block),
            scaling: Scaling::Dynamic,
        });
        fight.apply(block);
    }
    Played::Done
}

/// The rest of a Silent card's play that waited on its own draw, with every
/// copy that draw drew.
pub(super) fn resume_drew<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    match at {
        Resume::EscapePlanDrew => escape_plan_drew(fight, drew),
        at => resume_waited(fight, at),
    }
}

pub(super) fn resume_waited<F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::HiddenDaggersShivs => hidden_daggers_shivs(fight),
        Resume::ShadowStepDiscarded => shadow_step_discarded(fight),
        // A draw that never ran, as the fight ended, drew nothing.
        Resume::EscapePlanDrew => escape_plan_drew(fight, &[]),
        at => unimplemented!("{at:?} resumed after waiting"),
    }
}

/// `ShadowStep`'s power on its owner, once its discard is over.
fn shadow_step_discarded<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.apply_to_actor(Power::ShadowStepPower, PowerAmount::Counted(1));
    Played::Done
}

/// `HiddenDaggers`' Shivs into the hand, upgraded where it is.
fn hidden_daggers_shivs<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let shivs: usize =
        num_traits::cast(fight.var(CardVar::Shivs)).expect("a count of Shivs is not negative");
    let upgraded = fight.upgraded();
    let shiv = CardIdent::new(Cards::Token(TokenCard::Shiv));
    for at in fight.add_generated_cards(shiv, Pile::Hand, shivs) {
        if upgraded {
            fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
        }
    }
    Played::Done
}

/// `DaggerThrow`: its damage to the target, one card drawn, then one card
/// from hand to discard.
fn dagger_throw<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    fight.draw(1);
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::DaggerThrow)),
    }
}

/// Its damage to the target, `hits` times, as one attack.
fn hit_target<F: Fight<V>>(fight: &mut F, hits: super::Number) -> sts2_core::AttackResults<V> {
    let target = fight.target();
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
    })
}

/// `DaggerSpray`: its damage to every enemy, twice.
fn dagger_spray<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..2 {
            let targets = fight.opponents();
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for target in targets {
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
    });
    Played::Done
}

/// `LeadingStrike`: its damage to the target, then its `Shivs` into hand one
/// at a time.
fn leading_strike<F: Fight<V>>(fight: &mut F) -> Played<V> {
    hit_target(fight, 1);
    for _ in 0..fight.var(CardVar::Shivs) {
        fight.add_generated_cards(CardIdent::new(Cards::Token(TokenCard::Shiv)), Pile::Hand, 1);
    }
    Played::Done
}

/// `Skewer`: its damage to the target, once for each energy its X spent.
fn skewer<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let hits = fight.x_value();
    hit_target(fight, hits);
    Played::Done
}

/// `Prepared`: its `Cards` drawn, then as many from hand to discard.
fn prepared<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let cards = fight.var(CardVar::Cards);
    fight.draw(cards);
    let count = usize::try_from(cards)
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .unwrap_or_else(|| unimplemented!("Prepared discarding {cards} cards"));
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(count),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::Prepared)),
    }
}

/// `Survivor`: its block, then one card from hand to discard.
fn survivor<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::Survivor)),
    }
}

/// `UpMySleeve`: its `Cards` of Shivs into hand one at a time, then one off
/// its own cost for the rest of the fight.
fn up_my_sleeve<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let count: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of Shivs is not negative");
    for _ in 0..count {
        fight.add_generated_cards(CardIdent::new(Cards::Token(TokenCard::Shiv)), Pile::Hand, 1);
    }
    fight.modify_this(Change::ChangeCost(CostPool::Energy, -1), Lasting::ThisFight);
    Played::Done
}

/// `BladeDance`: its `Cards` of Shivs into hand one at a time.
fn blade_dance<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let count: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of Shivs is not negative");
    for _ in 0..count {
        fight.add_generated_cards(CardIdent::new(Cards::Token(TokenCard::Shiv)), Pile::Hand, 1);
    }
    Played::Done
}

/// `PiercingWail`: its `StrengthLoss` as Piercing Wail onto each enemy that
/// can be hit, one at a time.
fn piercing_wail<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = PowerAmount::Counted(fight.var(CardVar::StrengthLoss));
    for target in fight.hittable_opponents() {
        fight.apply_power(target, Power::PiercingWailPower, amount);
    }
    Played::Done
}

/// `EchoingSlash`: its damage to every enemy that can be hit, and again for
/// each result of it that says `WasTargetKilled`, as one attack.
fn echoing_slash<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        let mut left = 1;
        while left > 0 {
            left -= 1;
            let targets = fight.hittable_opponents();
            let before = fight.attack_results().len();
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for &target in &targets {
                    let attack = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(CardVar::Damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(attack);
                }
            });
            left += fight.attack_results()[before..]
                .iter()
                .filter(|result| result.killed)
                .count();
        }
    });
    Played::Done
}

/// Deliver a notification to one Silent card in the fight.
pub(super) fn event_hook(
    card: SilentCard,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let skill = |card: Cards| V::card_kind(card) == Some(CardKind::Skill);
    match (card, hook) {
        // `Pinpoint.AfterCardPlayed`: one off its cost this turn for each
        // Skill its owner plays. A fight here has one player.
        (SilentCard::Pinpoint, EventHook::CardPlayed { card, .. }) => {
            if skill(card) {
                asked.push(HookEffect::ModifySelf {
                    change: Change::ChangeCost(CostPool::Energy, -1),
                    lasting: Lasting::ThisTurn,
                });
            }
        }
        // `Pinpoint.AfterCardEnteredCombat`, for its own entry alone and not
        // a clone's: one off this turn for each Skill play finished this turn
        // before it came.
        (SilentCard::Pinpoint, EventHook::CardEnteredCombat { at, clone, .. }) => {
            if at.id() == id && !clone {
                let skills: super::Number = history
                    .finished_this_turn()
                    .filter(|&card| skill(card))
                    .count()
                    .try_into()
                    .expect("a turn's plays fit a game number");
                if skills != 0 {
                    asked.push(HookEffect::ModifySelf {
                        change: Change::ChangeCost(CostPool::Energy, -skills),
                        lasting: Lasting::ThisTurn,
                    });
                }
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
