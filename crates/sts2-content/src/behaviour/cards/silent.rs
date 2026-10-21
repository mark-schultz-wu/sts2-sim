//! The Silent's cards the generator leaves to a hand port.

use sts2_core::{
    Amount, CardIdent, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight,
    HookEffect, Lasting, Pile, Played, PowerAmount, Scaling, Selection,
};

use crate::generated::cards::CardKeyword;
use crate::ids::{CardIds as _, CardVarIds as _, EnchantmentIds as _, PowerIds as _};

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
    /// `LeadingStrike`, after a question inside its attack was answered.
    LeadingStrikeHit,
    /// `TheHunt`, whose kill counts where `fatal` says.
    TheHuntHit { fatal: bool },
    /// `StormOfSteel`, after a question inside its discard was answered,
    /// with how many cards the hand held.
    StormOfSteelDiscarded { count: u8 },
}

/// The rest of the play waits at `at`, where a command left a question and
/// more commands follow.
fn wait<V: crate::Build>(at: Resume) -> Played<V> {
    Played::Waiting {
        at: crate::behaviour::cards::Resume::Silent(at).into(),
    }
}

pub(super) fn on_play<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V> {
    match card {
        __c0 if __c0 == V::Card::SURVIVOR => survivor::<V, _>(fight),
        __c0 if __c0 == V::Card::PREPARED => prepared::<V, _>(fight),
        __c0 if __c0 == V::Card::DAGGER_SPRAY => dagger_spray::<V, _>(fight),
        __c0 if __c0 == V::Card::LEADING_STRIKE => leading_strike::<V, _>(fight),
        __c0 if __c0 == V::Card::SKEWER => skewer::<V, _>(fight),
        __c0 if __c0 == V::Card::DAGGER_THROW => dagger_throw::<V, _>(fight),
        __c0 if __c0 == V::Card::HAZE => haze::<V, _>(fight),
        __c0 if __c0 == V::Card::MALAISE => malaise::<V, _>(fight),
        __c0 if __c0 == V::Card::UP_MY_SLEEVE => up_my_sleeve::<V, _>(fight),
        __c0 if __c0 == V::Card::BLADE_DANCE => blade_dance::<V, _>(fight),
        __c0 if __c0 == V::Card::PIERCING_WAIL => piercing_wail::<V, _>(fight),
        __c0 if __c0 == V::Card::ECHOING_SLASH => echoing_slash::<V, _>(fight),
        __c0 if __c0 == V::Card::HAND_TRICK => hand_trick::<V, _>(fight),
        // `Nightmare`: one card of the hand picked, to copy into hand before
        // the next hand is drawn.
        __c0 if __c0 == V::Card::NIGHTMARE => Played::Asking {
            pile: Pile::Hand,
            selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: crate::behaviour::cards::Resume::Silent(Resume::Nightmare).into(),
        },
        // `BladeOfInk`: its `Cards` of Shivs into hand, each enchanted with
        // one Inky. `CardCmd.Enchant` raises no hook, so the Shivs arrive
        // enchanted.
        __c0 if __c0 == V::Card::BLADE_OF_INK => {
            let mut shiv = CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV);
            <V as sts2_core::Rules>::enchant(V::Enchantment::INKY, 1, &mut shiv);
            let count: usize =
                num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of Shivs");
            fight.add_generated_cards(shiv, Pile::Hand, count);
            Played::Done
        }
        // `KnifeTrap`: each Shiv of the exhaust pile, upgraded where Knife
        // Trap is, auto-played at its target, one at a time.
        __c0 if __c0 == V::Card::KNIFE_TRAP => {
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
        // `Expose`: the target loses all its block and any Artifact, then
        // takes its `Power` of Vulnerable.
        __c0 if __c0 == V::Card::EXPOSE => {
            let target = fight.target();
            let block = fight
                .creatures()
                .find(|creature| creature.id() == target)
                .map_or(0, |creature| creature.block());
            fight.lose_block(target, block, Some(fight.actor()));
            fight.remove_power(target, V::Power::ARTIFACT_POWER);
            let amount = fight.var(V::CardVar::POWER);
            fight.apply_power(
                target,
                V::Power::VULNERABLE_POWER,
                PowerAmount::Counted(amount),
            );
            Played::Done
        }
        // `DodgeAndRoll`: its block, then as much as it gave as Block Next
        // Turn.
        __c0 if __c0 == V::Card::DODGE_AND_ROLL => {
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
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let gained = held(fight) - before;
            fight.apply_to_actor(
                V::Power::BLOCK_NEXT_TURN_POWER,
                PowerAmount::Counted(gained),
            );
            Played::Done
        }
        // `EscapePlan`: a card drawn, then its block where that card is a
        // Skill. A shuffle inside the draw can ask, and the block waits.
        __c0 if __c0 == V::Card::ESCAPE_PLAN => {
            let drew = fight.draw(1);
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Silent(Resume::EscapePlanDrew).into(),
                };
            }
            escape_plan_drew::<V, _>(fight, &drew)
        }
        // `CloakAndDagger`: its block, then its `Cards` of Shivs into hand
        // one at a time.
        __c0 if __c0 == V::Card::CLOAK_AND_DAGGER => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let count: usize = num_traits::cast(fight.var(V::CardVar::CARDS))
                .expect("a count of Shivs is not negative");
            for _ in 0..count {
                fight.add_generated_cards(
                    CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV),
                    Pile::Hand,
                    1,
                );
            }
            Played::Done
        }
        // `Tracking`: Tracking on its owner, two the first time and one after.
        __c0 if __c0 == V::Card::TRACKING => {
            let actor = fight.actor();
            let tracking = fight.creatures().any(|creature| {
                creature.id() == actor && creature.power(V::Power::TRACKING_POWER).is_some()
            });
            let amount = if tracking { 1 } else { 2 };
            fight.apply_to_actor(V::Power::TRACKING_POWER, PowerAmount::Counted(amount));
            Played::Done
        }
        // `Acrobatics`: its `Cards` drawn, then one card of the hand
        // discarded.
        __c0 if __c0 == V::Card::ACROBATICS => {
            fight.draw(fight.var(V::CardVar::CARDS));
            Played::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: crate::behaviour::cards::Resume::Silent(Resume::Acrobatics).into(),
            }
        }
        // `Finisher`: its damage to the target, once for each attack its
        // owner finished this turn.
        __c0 if __c0 == V::Card::FINISHER => {
            hit_target::<V, _>(fight, fight.var(V::CardVar::CALCULATED_HITS));
            Played::Done
        }
        // `Flechettes`: its damage to the target, once for each Skill in its
        // owner's hand.
        __c0 if __c0 == V::Card::FLECHETTES => {
            hit_target::<V, _>(fight, fight.var(V::CardVar::CALCULATED_HITS));
            Played::Done
        }
        // `Expertise`: cards drawn up to its `Cards` in hand.
        __c0 if __c0 == V::Card::EXPERTISE => {
            let hand = crate::behaviour::cards::Number::try_from(fight.cards_in(Pile::Hand).len())
                .expect("a hand fits a game number");
            fight.draw((fight.var(V::CardVar::CARDS) - hand).max(0));
            Played::Done
        }
        // `HiddenDaggers`: its `Cards` of the hand discarded, then Shivs.
        __c0 if __c0 == V::Card::HIDDEN_DAGGERS => {
            let count = usize::try_from(fight.var(V::CardVar::CARDS))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(count),
                at: crate::behaviour::cards::Resume::Silent(Resume::HiddenDaggers).into(),
            }
        }
        // `BouncingFlask`: its Poison on an enemy drawn off `CombatTargets`,
        // its `Repeat` times.
        __c0 if __c0 == V::Card::BOUNCING_FLASK => {
            let poison = PowerAmount::Counted(fight.var(V::CardVar::POISON_POWER));
            for _ in 0..fight.var(V::CardVar::REPEAT) {
                if let Some(target) = fight.random_opponent() {
                    fight.apply_power(target, V::Power::POISON_POWER, poison);
                }
            }
            Played::Done
        }
        // `BubbleBubble`: its Poison on the target, where it already carries
        // Poison.
        __c0 if __c0 == V::Card::BUBBLE_BUBBLE => {
            let target = fight.target();
            let poisoned = fight.creatures().any(|creature| {
                creature.id() == target && creature.power(V::Power::POISON_POWER).is_some()
            });
            if poisoned {
                let poison = PowerAmount::Counted(fight.var(V::CardVar::POISON_POWER));
                fight.apply_power(target, V::Power::POISON_POWER, poison);
            }
            Played::Done
        }
        // `Adrenaline`: its energy, then its `Cards` drawn.
        __c0 if __c0 == V::Card::ADRENALINE => {
            fight.modify_energy(fight.var(V::CardVar::ENERGY));
            fight.draw(fight.var(V::CardVar::CARDS));
            Played::Done
        }
        // `CalculatedGamble`: the whole hand discarded, then as many drawn.
        __c0 if __c0 == V::Card::CALCULATED_GAMBLE => {
            let hand = fight.cards_in(Pile::Hand);
            let count = crate::behaviour::cards::Number::try_from(hand.len())
                .expect("a hand fits a game number");
            fight.discard_and_draw(&hand, count);
            Played::Done
        }
        // `Mirage`: its calculated block.
        __c0 if __c0 == V::Card::MIRAGE => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::CALCULATED_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        // `StormOfSteel`: the whole hand discarded, then as many Shivs into
        // hand, each upgraded once Storm of Steel is.
        __c0 if __c0 == V::Card::STORM_OF_STEEL => {
            let hand = fight.cards_in(Pile::Hand);
            let count = u8::try_from(hand.len()).expect("a hand's worth fits a u8");
            fight.act_on(sts2_core::PileAct::Discard, &hand);
            if fight.waiting() {
                return wait::<V>(Resume::StormOfSteelDiscarded { count });
            }
            storm_of_steel_discarded::<V, _>(fight, count)
        }
        // `BulletTime`: each card of the hand that does not cost X free this
        // turn, then one No Draw on its owner.
        __c0 if __c0 == V::Card::BULLET_TIME => {
            for at in fight.cards_in(Pile::Hand) {
                if sts2_core::CardTable::cost(fight.card_ident(at).card) != sts2_core::Cost::X {
                    crate::behaviour::free_this_turn(fight, at);
                }
            }
            fight.apply_to_actor(V::Power::NO_DRAW_POWER, PowerAmount::Counted(1));
            Played::Done
        }
        // `FanOfKnives`: one Fan of Knives on its owner, then its `Shivs`
        // into hand one at a time.
        __c0 if __c0 == V::Card::FAN_OF_KNIVES => {
            fight.apply_to_actor(V::Power::FAN_OF_KNIVES_POWER, PowerAmount::Counted(1));
            let count: usize =
                num_traits::cast(fight.var(V::CardVar::SHIVS)).expect("a count of Shivs");
            for _ in 0..count {
                fight.add_generated_cards(
                    CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV),
                    Pile::Hand,
                    1,
                );
            }
            Played::Done
        }
        // `GrandFinale`: its damage to every enemy, as one attack.
        __c0 if __c0 == V::Card::GRAND_FINALE => {
            fight.attack(Scaling::Dynamic, |fight| {
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
            });
            Played::Done
        }
        // `ShadowStep`: the whole hand discarded, then one Shadow Step on its
        // owner.
        // `ShadowStep`: the hand discarded, which a Sly card it holds can
        // stand still behind a question. Then its power.
        __c0 if __c0 == V::Card::SHADOW_STEP => {
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(sts2_core::PileAct::Discard, &hand);
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Silent(Resume::ShadowStepDiscarded).into(),
                };
            }
            shadow_step_discarded::<V, _>(fight)
        }
        // `TheHunt`: its damage to the target. Where a result says
        // `WasTargetKilled` of a target whose death, read before the hit,
        // triggers Fatal, the room offers one more card reward, and its owner
        // takes one The Hunt.
        __c0 if __c0 == V::Card::THE_HUNT => {
            let target = fight.target();
            let fatal = fight.death_triggers_fatal(target);
            let results = hit_target::<V, _>(fight, 1);
            if fight.waiting() {
                return wait::<V>(Resume::TheHuntHit { fatal });
            }
            the_hunt_hit::<V, _>(fight, fatal && results.iter().any(|result| result.killed))
        }
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// `Malaise`: its X, one more once upgraded, off the target's Strength, then
/// as Weak on it.
fn malaise<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = fight.x_value() + crate::behaviour::cards::Number::from(fight.upgraded());
    let target = fight.target();
    fight.apply_power(
        target,
        V::Power::STRENGTH_POWER,
        PowerAmount::Counted(-amount),
    );
    fight.apply_power(target, V::Power::WEAK_POWER, PowerAmount::Counted(amount));
    Played::Done
}

/// `Haze`: its Poison onto each enemy that can be hit, one at a time.
fn haze<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    for target in fight.hittable_opponents() {
        fight.apply_power(
            target,
            V::Power::POISON_POWER,
            PowerAmount::Counted(fight.var(V::CardVar::POISON_POWER)),
        );
    }
    Played::Done
}

/// `Nightmare`'s `PowerCmd.Apply<NightmarePower>(…, 3m, …)`, which it prints
/// nowhere.
const NIGHTMARE: i32 = 3;

/// `HandTrick`: its block, then one skill of the hand that is not Sly this
/// turn picked, to be Sly until the turn ends.
fn hand_trick<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| {
            V::card_kind(fight.card_ident(card).card) == Some(CardKind::Skill)
                && !fight.card_keyword_this_turn(card, CardKeyword::Sly)
        })
        .collect();
    Played::AskingAmong {
        cards,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Silent(Resume::HandTrick).into(),
    }
}

pub(super) fn resume_play<V: crate::Build, F: Fight<V>>(
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
                fight.apply_to_actor(V::Power::NIGHTMARE_POWER, PowerAmount::Counted(NIGHTMARE));
                fight.keep_card_on_power(owner, V::Power::NIGHTMARE_POWER, card);
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
                    at: crate::behaviour::cards::Resume::Silent(Resume::HiddenDaggersShivs).into(),
                };
            }
            hidden_daggers_shivs::<V, _>(fight)
        }
        Resume::HiddenDaggersShivs => {
            unreachable!("Hidden Daggers' Shivs wait on a card, not a pick")
        }
        Resume::EscapePlanDrew => unreachable!("Escape Plan waits on its draw, not a pick"),
        Resume::ShadowStepDiscarded => {
            unreachable!("Shadow Step waits on a card, not a pick")
        }
        at => unreachable!("{at:?} waits on a question, not a pick"),
    }
}

/// The rest of a Silent card's play that waited on a card it auto-played.
///
/// # Panics
///
/// A resume point no play waits at.
/// `EscapePlan`, once its draw is over: its block where the card it drew is
/// a Skill, wherever that card stands now.
fn escape_plan_drew<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    let skill = drew.first().is_some_and(|&id| {
        let drawn = fight
            .find_card(id)
            .unwrap_or_else(|| unimplemented!("Escape Plan reading {id:?}, which left the fight"));
        V::card_kind(fight.card_ident(drawn).card) == Some(CardKind::Skill)
    });
    if skill {
        let block = fight.resolve(Effect::GainBlock {
            recipient: fight.actor(),
            amount: Amount::Var(V::CardVar::BLOCK),
            scaling: Scaling::Dynamic,
        });
        fight.apply(block);
    }
    Played::Done
}

/// The rest of a Silent card's play that waited on its own draw, with every
/// copy that draw drew.
pub(super) fn resume_drew<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    match at {
        Resume::EscapePlanDrew => escape_plan_drew::<V, _>(fight, drew),
        at => resume_waited::<V, _>(fight, at),
    }
}

pub(super) fn resume_waited<V: crate::Build, F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::HiddenDaggersShivs => hidden_daggers_shivs::<V, _>(fight),
        Resume::ShadowStepDiscarded => shadow_step_discarded::<V, _>(fight),
        // A draw that never ran, as the fight ended, drew nothing.
        Resume::EscapePlanDrew => escape_plan_drew::<V, _>(fight, &[]),
        Resume::LeadingStrikeHit => leading_strike_hit::<V, _>(fight),
        Resume::TheHuntHit { fatal } => {
            let results = fight.waited_attack_results();
            the_hunt_hit::<V, _>(fight, fatal && results.iter().any(|result| result.killed))
        }
        Resume::StormOfSteelDiscarded { count } => storm_of_steel_discarded::<V, _>(fight, count),
        at => unimplemented!("{at:?} resumed after waiting"),
    }
}

/// `ShadowStep`'s power on its owner, once its discard is over.
fn shadow_step_discarded<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.apply_to_actor(V::Power::SHADOW_STEP_POWER, PowerAmount::Counted(1));
    Played::Done
}

/// `HiddenDaggers`' Shivs into the hand, upgraded where it is.
fn hidden_daggers_shivs<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let shivs: usize =
        num_traits::cast(fight.var(V::CardVar::SHIVS)).expect("a count of Shivs is not negative");
    let upgraded = fight.upgraded();
    let shiv = CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV);
    for at in fight.add_generated_cards(shiv, Pile::Hand, shivs) {
        if upgraded {
            fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
        }
    }
    Played::Done
}

/// `DaggerThrow`: its damage to the target, one card drawn, then one card
/// from hand to discard.
fn dagger_throw<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    fight.draw(1);
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Silent(Resume::DaggerThrow).into(),
    }
}

/// The rest of `TheHunt`, once its attack is over: one more card reward and
/// The Hunt on its owner, where `gains`.
fn the_hunt_hit<V: crate::Build, F: Fight<V>>(fight: &mut F, gains: bool) -> Played<V> {
    if gains {
        fight.add_card_reward();
        fight.apply_to_actor(V::Power::THE_HUNT_POWER, PowerAmount::Counted(1));
    }
    Played::Done
}

/// The rest of `StormOfSteel`, once its discard is over: `count` Shivs into
/// hand, each upgraded once Storm of Steel is.
fn storm_of_steel_discarded<V: crate::Build, F: Fight<V>>(fight: &mut F, count: u8) -> Played<V> {
    let shiv = CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV);
    let upgraded = fight.upgraded();
    for at in fight.add_generated_cards(shiv, Pile::Hand, usize::from(count)) {
        if upgraded {
            fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
        }
    }
    Played::Done
}

/// Its damage to the target, `hits` times, as one attack.
fn hit_target<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    hits: crate::behaviour::cards::Number,
) -> sts2_core::AttackResults<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    })
}

/// `DaggerSpray`: its damage to every enemy, twice.
fn dagger_spray<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..2 {
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

/// `LeadingStrike`: its damage to the target, then its `Shivs` into hand one
/// at a time.
fn leading_strike<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    hit_target::<V, _>(fight, 1);
    if fight.waiting() {
        return wait::<V>(Resume::LeadingStrikeHit);
    }
    leading_strike_hit::<V, _>(fight)
}

/// The rest of `LeadingStrike`, once its attack is over: its `Shivs` into
/// hand one at a time.
fn leading_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    for _ in 0..fight.var(V::CardVar::SHIVS) {
        fight.add_generated_cards(
            CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV),
            Pile::Hand,
            1,
        );
    }
    Played::Done
}

/// `Skewer`: its damage to the target, once for each energy its X spent.
fn skewer<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let hits = fight.x_value();
    hit_target::<V, _>(fight, hits);
    Played::Done
}

/// `Prepared`: its `Cards` drawn, then as many from hand to discard.
fn prepared<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let cards = fight.var(V::CardVar::CARDS);
    fight.draw(cards);
    let count = usize::try_from(cards)
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .unwrap_or_else(|| unimplemented!("Prepared discarding {cards} cards"));
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(count),
        at: crate::behaviour::cards::Resume::Silent(Resume::Prepared).into(),
    }
}

/// `Survivor`: its block, then one card from hand to discard.
fn survivor<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Silent(Resume::Survivor).into(),
    }
}

/// `UpMySleeve`: its `Cards` of Shivs into hand one at a time, then one off
/// its own cost for the rest of the fight.
fn up_my_sleeve<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let count: usize =
        num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of Shivs is not negative");
    for _ in 0..count {
        fight.add_generated_cards(
            CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV),
            Pile::Hand,
            1,
        );
    }
    fight.modify_this(Change::ChangeCost(CostPool::Energy, -1), Lasting::ThisFight);
    Played::Done
}

/// `BladeDance`: its `Cards` of Shivs into hand one at a time.
fn blade_dance<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let count: usize =
        num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of Shivs is not negative");
    for _ in 0..count {
        fight.add_generated_cards(
            CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV),
            Pile::Hand,
            1,
        );
    }
    Played::Done
}

/// `PiercingWail`: its `StrengthLoss` as Piercing Wail onto each enemy that
/// can be hit, one at a time.
fn piercing_wail<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = PowerAmount::Counted(fight.var(V::CardVar::STRENGTH_LOSS));
    for target in fight.hittable_opponents() {
        fight.apply_power(target, V::Power::PIERCING_WAIL_POWER, amount);
    }
    Played::Done
}

/// `EchoingSlash`: its damage to every enemy that can be hit, and again for
/// each result of it that says `WasTargetKilled`, as one attack.
fn echoing_slash<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
                        damage: Amount::Var(V::CardVar::DAMAGE),
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
pub(super) fn event_hook<V: crate::Build>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let skill = |card: V::Card| V::card_kind(card) == Some(CardKind::Skill);
    match (card, hook) {
        // `Pinpoint.AfterCardPlayed`: one off its cost this turn for each
        // Skill its owner plays. A fight here has one player.
        (__c0, EventHook::CardPlayed { card, .. }) if __c0 == V::Card::PINPOINT => {
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
        (__c0, EventHook::CardEnteredCombat { at, clone, .. }) if __c0 == V::Card::PINPOINT => {
            if at.id() == id && !clone {
                let skills: crate::behaviour::cards::Number = history
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
