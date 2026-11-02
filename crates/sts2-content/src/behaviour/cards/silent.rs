//! The Silent's cards the generator leaves to a hand port.

use crate::behaviour::Played;
use sts2_core::{
    Amount, CardIdent, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight,
    FightKind, HookEffect, Lasting, Pile, PowerAmount, Scaling, Selection,
};

use crate::generated::cards::CardKeyword;
use crate::ids::{CardIds as _, CardVarIds as _, EnchantmentIds as _, PowerIds as _};

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match card {
        __c0 if __c0 == V::Card::SURVIVOR => start::<V, K>(card, fight),
        __c0 if __c0 == V::Card::PREPARED => start::<V, K>(card, fight),
        __c0 if __c0 == V::Card::DAGGER_SPRAY => dagger_spray::<V, _>(fight),
        __c0 if __c0 == V::Card::LEADING_STRIKE => start::<V, K>(card, fight),
        __c0 if __c0 == V::Card::SKEWER => skewer::<V, _>(fight),
        __c0 if __c0 == V::Card::DAGGER_THROW => start::<V, K>(card, fight),
        __c0 if __c0 == V::Card::HAZE => haze::<V, _>(fight),
        __c0 if __c0 == V::Card::MALAISE => malaise::<V, _>(fight),
        __c0 if __c0 == V::Card::UP_MY_SLEEVE => up_my_sleeve::<V, _>(fight),
        __c0 if __c0 == V::Card::BLADE_DANCE => blade_dance::<V, _>(fight),
        __c0 if __c0 == V::Card::PIERCING_WAIL => piercing_wail::<V, _>(fight),
        __c0 if __c0 == V::Card::ECHOING_SLASH => echoing_slash::<V, _>(fight),
        __c0 if __c0 == V::Card::HAND_TRICK => start::<V, K>(card, fight),
        // `Nightmare`: one card of the hand picked, to copy into hand before
        // the next hand is drawn.
        __c0 if __c0 == V::Card::NIGHTMARE => start::<V, K>(card, fight),
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
                let find = |fight: &K::Fight<'a>| {
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
        // `DodgeAndRoll`: its block, then the decimal `GainBlock` hands back
        // as Block Next Turn. A fraction below one lands a power of nothing.
        __c0 if __c0 == V::Card::DODGE_AND_ROLL => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            use sts2_core::EffectResolution as _;
            let (gained, positive) = (block.amount(), block.positive());
            fight.apply(block);
            match gained == 0 && positive {
                true => fight.apply_fraction_to_actor(V::Power::BLOCK_NEXT_TURN_POWER),
                false => fight.apply_to_actor(
                    V::Power::BLOCK_NEXT_TURN_POWER,
                    PowerAmount::Counted(gained),
                ),
            }
            Played::Done
        }
        // `EscapePlan`: a card drawn, then its block where that card is a
        // Skill. A shuffle inside the draw can ask, and the block waits.
        __c0 if __c0 == V::Card::ESCAPE_PLAN => start::<V, K>(card, fight),
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
        __c0 if __c0 == V::Card::ACROBATICS => start::<V, K>(card, fight),
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
        __c0 if __c0 == V::Card::HIDDEN_DAGGERS => start::<V, K>(card, fight),
        // `BouncingFlask`: its Poison on an enemy drawn off `CombatTargets`,
        // its `Repeat` times.
        __c0 if __c0 == V::Card::BOUNCING_FLASK => {
            let poison = PowerAmount::Counted(fight.var(V::CardVar::POISON_POWER));
            // The game's loop reads its count before each pass.
            let mut pass = 0;
            while pass < fight.var(V::CardVar::REPEAT) {
                pass += 1;
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
        __c0 if __c0 == V::Card::STORM_OF_STEEL => start::<V, K>(card, fight),
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
        // `ShadowStep`: the hand discarded, which a Sly card it holds can
        // stand still behind a question. Then its power.
        __c0 if __c0 == V::Card::SHADOW_STEP => start::<V, K>(card, fight),
        // `TheHunt`: its damage to the target. Where a result says
        // `WasTargetKilled` of a target whose death, read before the hit,
        // triggers Fatal, the room offers one more card reward, and its owner
        // takes one The Hunt.
        __c0 if __c0 == V::Card::THE_HUNT => start::<V, K>(card, fight),
        card => crate::behaviour::cards::generated::<V, K>(card, fight),
    }
}

/// A Silent card's play that can stop on a question: from its start to its
/// end, yielding each question it stops on. Each step is lent the fight, and
/// after a question the answer.
#[diapause::coroutine(
    yield = crate::behaviour::paused::Ask<V>,
    resume = for<'f, 'a> crate::behaviour::paused::Lent<'f, 'a, V, K>
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `Survivor`: its block, then one card from hand to discard.
        __c0 if __c0 == V::Card::SURVIVOR => {
            let fight = &mut *resumed!().fight;
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            });
            discard_taken::<V, K>(resumed!());
        }
        // `Prepared`: its `Cards` drawn, then as many from hand to discard.
        __c0 if __c0 == V::Card::PREPARED => {
            let fight = &mut *resumed!().fight;
            let cards = fight.var(V::CardVar::CARDS);
            fight.draw(cards);
            let count = usize::try_from(cards)
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .unwrap_or_else(|| unimplemented!("Prepared discarding {cards} cards"));
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(count),
            });
            discard_taken::<V, K>(resumed!());
        }
        // `DaggerThrow`: its damage to the target, one card drawn, then one
        // card from hand to discard.
        __c0 if __c0 == V::Card::DAGGER_THROW => {
            let fight = &mut *resumed!().fight;
            hit_target::<V, _>(fight, 1);
            fight.draw(1);
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            });
            discard_taken::<V, K>(resumed!());
        }
        // `Acrobatics`: its `Cards` drawn, then one card of the hand
        // discarded.
        __c0 if __c0 == V::Card::ACROBATICS => {
            let fight = &mut *resumed!().fight;
            fight.draw(fight.var(V::CardVar::CARDS));
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            });
            discard_taken::<V, K>(resumed!());
        }
        // `HandTrick`: its block, then one skill of the hand that is not Sly
        // this turn picked, to be Sly until the turn ends.
        __c0 if __c0 == V::Card::HAND_TRICK => {
            let fight = &mut *resumed!().fight;
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            yield_!(crate::behaviour::paused::Ask::AskingAmong {
                pile: Pile::Hand,
                filter: crate::behaviour::CardFilter::SkillNotSlyThisTurn,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            });
            // `CardCmd.ApplySingleTurnSly` on the one taken.
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            for &picked in crate::behaviour::paused::selected::<V>(taken).as_slice() {
                fight.modify_card(
                    picked,
                    Change::AddKeyword(CardKeyword::Sly),
                    Lasting::ThisTurn,
                );
            }
        }
        // `Nightmare`: one card of the hand picked, then three of
        // `NightmarePower` on its owner, which keeps a clone of it, where a
        // card was taken.
        __c0 if __c0 == V::Card::NIGHTMARE => {
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            });
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            if let Some(picked) = crate::behaviour::paused::selected::<V>(taken).one_or_none() {
                let owner = fight.actor();
                fight.apply_to_actor(V::Power::NIGHTMARE_POWER, PowerAmount::Counted(NIGHTMARE));
                fight.keep_card_on_power(owner, V::Power::NIGHTMARE_POWER, picked);
            }
        }
        // `HiddenDaggers`: its `Cards` of the hand discarded, which awaits a
        // Sly card it plays, then Shivs.
        __c0 if __c0 == V::Card::HIDDEN_DAGGERS => {
            let count = usize::try_from(resumed!().fight.var(V::CardVar::CARDS))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(count),
            });
            discard_taken::<V, K>(resumed!());
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
            }
            hidden_daggers_shivs::<V, _>(resumed!().fight);
        }
        // `EscapePlan`: a card drawn, then its block where that card is a
        // Skill. A shuffle inside the draw can ask, and the block waits. A
        // draw the ended fight never ran drew nothing.
        __c0 if __c0 == V::Card::ESCAPE_PLAN => {
            let waits: bool = {
                let fight = &mut *resumed!().fight;
                let drew = fight.draw(1);
                let waits = fight.waiting();
                if !waits {
                    escape_plan_drew::<V, _>(fight, &drew);
                }
                waits
            };
            if waits {
                yield_!(crate::behaviour::paused::Ask::Waiting);
                let crate::behaviour::paused::Lent { fight, taken } = resumed!();
                let drew = match taken.take() {
                    Some(sts2_core::Taken::Drew(drew)) => drew,
                    _ => sts2_core::Drew::new(),
                };
                escape_plan_drew::<V, _>(&mut **fight, &drew);
            }
        }
        // `StormOfSteel`: the whole hand discarded, then as many Shivs into
        // hand, each upgraded once Storm of Steel is.
        __c0 if __c0 == V::Card::STORM_OF_STEEL => {
            let fight = &mut *resumed!().fight;
            let hand = fight.cards_in(Pile::Hand);
            let count: u8 = u8::try_from(hand.len()).expect("a hand's worth fits a u8");
            fight.act_on(sts2_core::PileAct::Discard, &hand);
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
            }
            storm_of_steel_discarded::<V, _>(resumed!().fight, count);
        }
        // `ShadowStep`: the hand discarded, which a Sly card it holds can
        // stand still behind a question. Then its power.
        __c0 if __c0 == V::Card::SHADOW_STEP => {
            let fight = &mut *resumed!().fight;
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(sts2_core::PileAct::Discard, &hand);
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
            }
            shadow_step_discarded::<V, _>(resumed!().fight);
        }
        // `TheHunt`: its damage to the target. Where a result says
        // `WasTargetKilled` of a target whose death, read before the hit,
        // triggers Fatal, the room offers one more card reward, and its owner
        // takes one The Hunt.
        __c0 if __c0 == V::Card::THE_HUNT => {
            let fight = &mut *resumed!().fight;
            let target = fight.target();
            let fatal: bool = fight.death_triggers_fatal(target);
            let results = hit_target::<V, _>(fight, 1);
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
                let fight = &mut *resumed!().fight;
                let results = fight.waited_attack_results();
                the_hunt_hit::<V, _>(fight, fatal && results.iter().any(|result| result.killed));
            } else {
                the_hunt_hit::<V, _>(
                    resumed!().fight,
                    fatal && results.iter().any(|result| result.killed),
                );
            }
        }
        // `LeadingStrike`: its damage to the target, then its `Shivs` into
        // hand one at a time.
        __c0 if __c0 == V::Card::LEADING_STRIKE => {
            hit_target::<V, _>(resumed!().fight, 1);
            if resumed!().fight.waiting() {
                yield_!(crate::behaviour::paused::Ask::Waiting);
            }
            leading_strike_hit::<V, _>(resumed!().fight);
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

/// `CardCmd.Discard` of the cards a selection took.
fn discard_taken<V: crate::Build, K: FightKind<V>>(
    lent: &mut crate::behaviour::paused::Lent<'_, '_, V, K>,
) {
    let crate::behaviour::paused::Lent { fight, taken } = lent;
    fight.act_on(
        sts2_core::PileAct::Discard,
        crate::behaviour::paused::selected::<V>(taken).as_slice(),
    );
}

crate::behaviour::paused::paused_play!(V::Card, crate::behaviour::cards::Resume::Silent);

/// `Malaise`: its X, one more once upgraded, off the target's Strength, then
/// as Weak on it.
fn malaise<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
fn haze<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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

/// `EscapePlan`, once its draw is over: its block where the card it drew is
/// a Skill, wherever that card stands now.
fn escape_plan_drew<V: crate::Build, F: Fight<V>>(fight: &mut F, drew: &[sts2_core::CardId<V>]) {
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
}

/// `ShadowStep`'s power on its owner, once its discard is over.
fn shadow_step_discarded<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.apply_to_actor(V::Power::SHADOW_STEP_POWER, PowerAmount::Counted(1));
}

/// `HiddenDaggers`' Shivs into the hand, upgraded where it is.
fn hidden_daggers_shivs<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let shivs: usize =
        num_traits::cast(fight.var(V::CardVar::SHIVS)).expect("a count of Shivs is not negative");
    let upgraded = fight.upgraded();
    let shiv = CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV);
    for at in fight.add_generated_cards(shiv, Pile::Hand, shivs) {
        if upgraded {
            fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
        }
    }
}

/// The rest of `TheHunt`, once its attack is over: one more card reward and
/// The Hunt on its owner, where `gains`.
fn the_hunt_hit<V: crate::Build, F: Fight<V>>(fight: &mut F, gains: bool) {
    if gains {
        fight.add_card_reward();
        fight.apply_to_actor(V::Power::THE_HUNT_POWER, PowerAmount::Counted(1));
    }
}

/// The rest of `StormOfSteel`, once its discard is over: `count` Shivs into
/// hand, each upgraded once Storm of Steel is.
fn storm_of_steel_discarded<V: crate::Build, F: Fight<V>>(fight: &mut F, count: u8) {
    let shiv = CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV);
    let upgraded = fight.upgraded();
    for at in fight.add_generated_cards(shiv, Pile::Hand, usize::from(count)) {
        if upgraded {
            fight.modify_card(at, Change::Upgrade, Lasting::ThisFight);
        }
    }
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
fn dagger_spray<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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

/// The rest of `LeadingStrike`, once its attack is over: its `Shivs` into
/// hand one at a time.
fn leading_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    // The game's loop reads its count before each pass.
    let mut pass = 0;
    while pass < fight.var(V::CardVar::SHIVS) {
        pass += 1;
        fight.add_generated_cards(
            CardIdent::new(<V::Card as crate::ids::CardIds>::SHIV),
            Pile::Hand,
            1,
        );
    }
}

/// `Skewer`: its damage to the target, once for each energy its X spent.
fn skewer<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let hits = fight.x_value();
    hit_target::<V, _>(fight, hits);
    Played::Done
}

/// `UpMySleeve`: its `Cards` of Shivs into hand one at a time, then one off
/// its own cost for the rest of the fight.
fn up_my_sleeve<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
fn blade_dance<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
fn piercing_wail<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let amount = PowerAmount::Counted(fight.var(V::CardVar::STRENGTH_LOSS));
    for target in fight.hittable_opponents() {
        fight.apply_power(target, V::Power::PIERCING_WAIL_POWER, amount);
    }
    Played::Done
}

/// `EchoingSlash`: its damage to every enemy that can be hit, and again for
/// each result of it that says `WasTargetKilled`, as one attack.
fn echoing_slash<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
    owner: sts2_core::CombatId,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let skill = |card: V::Card| V::card_kind(card) == Some(CardKind::Skill);
    match (card, hook) {
        // `Pinpoint.AfterCardPlayed`: one off its cost this turn for each
        // Skill its owner plays.
        (__c0, EventHook::CardPlayed { card, player, .. }) if __c0 == V::Card::PINPOINT => {
            if player == owner && skill(card) {
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
