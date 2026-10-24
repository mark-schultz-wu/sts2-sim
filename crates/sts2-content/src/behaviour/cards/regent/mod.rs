//! The Regent's cards the generator leaves to a hand port, filed by rarity.

use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};
use sts2_core::{CardIdent, Fight, Pile, Played};

mod common;
mod uncommon;

/// Where a Regent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume<V: crate::Build> {
    /// `Begone`, after it selected the card to turn into a Minion Strike.
    BegoneTransform,
    /// `Guards`, after its owner picked the cards to turn into Minion
    /// Sacrifices.
    GuardsTransform,
    /// `Quasar`, after its owner chose a colorless card or none.
    QuasarChoice,
    /// `HeirloomHammer`, after its owner picked the colorless card to copy.
    HeirloomHammerCopy,
    /// `Glimmer`, after its owner picked the cards to put back.
    GlimmerPutBack,
    /// `PhotonCut`, after its owner picked the cards to put back.
    PhotonCutPutBack,
    /// `CosmicIndifference`, after its owner picked the card to put back.
    CosmicIndifferencePutBack,
    /// `Charge`, after its owner picked the cards to turn into Minion Dive
    /// Bombs.
    ChargeTransform,
    /// `DecisionsDecisions`, after a question inside its draw was answered.
    DecisionsDrew,
    /// `DecisionsDecisions`, after its owner picked the Skill to play.
    DecisionsPicked,
    /// `DecisionsDecisions`, once a play of its Skill has its answer, with
    /// `left` plays of `card` still to make.
    DecisionsPlayed {
        card: sts2_core::CardAt<V>,
        left: u8,
    },
    /// `CrushUnder`, after a question inside its attack or a power. `held`
    /// is the opponents it took before the attack, and `at` the one to go on
    /// from.
    CrushUnder {
        held: crate::behaviour::cards::Held,
        at: u8,
    },
    /// After a question inside the card's attack was answered.
    CrashLandingHit,
    ShiningStrikeHit,
    HeirloomHammerHit,
    CollisionCourseHit,
    BeatIntoShapeHit,
    KnockoutBlowHit,
    /// `DyingStar`, after a question inside its attack, with the enemies it
    /// took before it.
    DyingStarHit {
        held: crate::behaviour::cards::Held,
    },
}

/// The rest of the play waits at `at`, where a command left a question and
/// more commands follow.
pub(super) fn wait<V: crate::Build>(at: Resume<V>) -> Played<V> {
    Played::Waiting {
        at: crate::behaviour::cards::Resume::Regent(at).into(),
    }
}

/// The rest of `CrushUnder` from the opponent at `from` on: its Crush Under
/// on each, its amount read once. A question left before one waits.
fn crush_under_powers<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    held: crate::behaviour::cards::Held,
    from: u8,
) -> Played<V> {
    use sts2_core::PowerAmount;

    let amount = PowerAmount::Counted(fight.power_var(V::CardVar::STRENGTH_LOSS));
    for (at, target) in held.creatures().enumerate().skip(usize::from(from)) {
        if fight.waiting() {
            let at = u8::try_from(at).expect("a fight's creatures fit a u8");
            return Played::Waiting {
                at: crate::behaviour::cards::Resume::Regent(Resume::CrushUnder { held, at }).into(),
            };
        }
        fight.apply_power(target, V::Power::CRUSH_UNDER_POWER, amount);
    }
    Played::Done
}

/// The rest of `CrashLanding`, once its attack is over: Debris into every
/// free place in the hand.
fn crash_landing_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    if room > 0 {
        let debris = CardIdent::new(V::Card::DEBRIS);
        fight.add_generated_cards(debris, Pile::Hand, room);
    }
    Played::Done
}

/// The rest of `DyingStar`, once its attack is over: its Dying Star on each
/// of `held`.
fn dying_star_hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    held: crate::behaviour::cards::Held,
) -> Played<V> {
    use sts2_core::PowerAmount;

    let loss = PowerAmount::Counted(fight.var(V::CardVar::STRENGTH_LOSS));
    for target in held.creatures() {
        fight.apply_power(target, V::Power::DYING_STAR_POWER, loss);
    }
    Played::Done
}

/// The rest of `ShiningStrike`, once its attack is over: its stars, then
/// itself onto the draw pile, unless it exhausts.
fn shining_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use crate::generated::cards::CardKeyword;
    use sts2_core::{Pile, PilePosition};

    fight.gain_stars(fight.var(V::CardVar::STARS));
    if !fight.this_has_keyword(CardKeyword::Exhaust) {
        fight.add_this(Pile::Draw, PilePosition::Top);
    }
    Played::Done
}

/// `BeatIntoShape`: its damage to the target, then a Forge of its calculated
/// number, less its extra for each hit of its own attack.
fn beat_into_shape<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

    let target = fight.target();
    let results = fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    if fight.waiting() {
        return wait::<V>(Resume::BeatIntoShapeHit);
    }
    beat_into_shape_hit::<V, _>(fight, &results)
}

/// The rest of `BeatIntoShape`, once its attack is over: a Forge of its
/// calculated number, less its extra for each of `results`.
fn beat_into_shape_hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    results: &sts2_core::AttackResults<V>,
) -> Played<V> {
    let count = |n: usize| {
        crate::behaviour::cards::Number::try_from(n).expect("an attack's hits fit a number")
    };
    let amount = fight.var(V::CardVar::CALCULATED_FORGE)
        - count(results.len()) * fight.var(V::CardVar::CALCULATION_EXTRA);
    fight.forge(V::Card::SOVEREIGN_BLADE, amount);
    Played::Done
}

pub(super) fn on_play<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V> {
    match card {
        // `CrashLanding`: its damage to every enemy, as one attack, then
        // Debris into hand until the hand is full.
        __c0 if __c0 == V::Card::CRASH_LANDING => {
            use sts2_core::{Amount, Effect, Scaling};

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
            if fight.waiting() {
                return wait::<V>(Resume::CrashLandingHit);
            }
            crash_landing_hit::<V, _>(fight)
        }
        // `Stardust`: its damage once for each star its X spent, each hit at
        // an enemy drawn at random.
        __c0 if __c0 == V::Card::STARDUST => {
            use sts2_core::{Amount, Effect, Scaling};

            let hits = fight.star_x_value();
            fight.attack(Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let Some(target) = fight.random_target() else {
                        break;
                    };
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
        // `Resonance`: its Strength on its owner, then one Strength off each
        // enemy that can be hit.
        __c0 if __c0 == V::Card::RESONANCE => {
            use sts2_core::PowerAmount;

            let strength = fight.var(V::CardVar::STRENGTH_POWER);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
            for target in fight.hittable_opponents() {
                fight.apply_power(target, V::Power::STRENGTH_POWER, PowerAmount::Counted(-1));
            }
            Played::Done
        }
        // `DyingStar`: its damage to every enemy, as one attack, then its
        // `StrengthLoss` of Dying Star on each enemy that could be hit as the
        // play began.
        __c0 if __c0 == V::Card::DYING_STAR => {
            use sts2_core::{Amount, Effect, Scaling};

            let held = crate::behaviour::cards::Held::of(&fight.hittable_opponents());
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
            if fight.waiting() {
                return wait::<V>(Resume::DyingStarHit { held });
            }
            dying_star_hit::<V, _>(fight, held)
        }
        // `VoidForm`: its Void Form on its owner, then `PlayerCmd.EndTurn`.
        __c0 if __c0 == V::Card::VOID_FORM => {
            use sts2_core::PowerAmount;

            let amount = fight.power_var(V::CardVar::VOID_FORM_POWER);
            fight.apply_to_actor(V::Power::VOID_FORM_POWER, PowerAmount::Counted(amount));
            fight.end_turn();
            Played::Done
        }
        __c0 if __c0 == V::Card::GLITTERSTREAM => common::glitterstream::<V, _>(fight),
        // `CrushUnder`: the opponents taken, its damage to every enemy, then
        // its Crush Under on each one taken.
        __c0 if __c0 == V::Card::CRUSH_UNDER => {
            use sts2_core::{Amount, Effect, Scaling};

            let held = crate::behaviour::cards::Held::of(&fight.opponents());
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
            crush_under_powers::<V, _>(fight, held, 0)
        }
        // `DecisionsDecisions`: its `Cards` drawn, then a playable Skill of the
        // hand picked. A shuffle inside the draw can ask, and the rest waits.
        __c0 if __c0 == V::Card::DECISIONS_DECISIONS => {
            fight.draw(fight.var(V::CardVar::CARDS));
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Regent(Resume::DecisionsDrew).into(),
                };
            }
            decisions_ask::<V, _>(fight)
        }
        __c0 if __c0 == V::Card::BEAT_INTO_SHAPE => beat_into_shape::<V, _>(fight),
        __c0 if __c0 == V::Card::QUASAR => quasar::<V, _>(fight),
        __c0 if __c0 == V::Card::HEIRLOOM_HAMMER => heirloom_hammer::<V, _>(fight),
        // `Guards`: any number of the hand's cards picked, each transformed.
        __c0 if __c0 == V::Card::GUARDS => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::UpTo(
                nonmax::NonMaxUsize::new(999_999_999).expect("a count below the maximum"),
            ),
            at: crate::behaviour::cards::Resume::Regent(Resume::GuardsTransform).into(),
        },
        __c0 if __c0 == V::Card::BEGONE => common::begone::<V>(),
        // `Charge`: its `Cards` of the draw pile picked, each transformed.
        __c0 if __c0 == V::Card::CHARGE => Played::Asking {
            pile: Pile::Draw,
            selection: sts2_core::Selection::Exactly(
                nonmax::NonMaxUsize::new(
                    num_traits::cast(fight.var(V::CardVar::CARDS)).unwrap_or(0),
                )
                .unwrap_or(nonmax::NonMaxUsize::ONE),
            ),
            at: crate::behaviour::cards::Resume::Regent(Resume::ChargeTransform).into(),
        },
        __c0 if __c0 == V::Card::COLLISION_COURSE => common::collision_course::<V, _>(fight),
        __c0 if __c0 == V::Card::RADIATE => uncommon::radiate::<V, _>(fight),
        __c0 if __c0 == V::Card::LUNAR_BLAST => uncommon::lunar_blast::<V, _>(fight),
        __c0 if __c0 == V::Card::HEAVENLY_DRILL => heavenly_drill::<V, _>(fight),
        // `Monologue`: one Monologue on its owner, whose Strength is the
        // card's `Power`, which the power's own constant holds.
        __c0 if __c0 == V::Card::MONOLOGUE => {
            fight.apply_to_actor(
                V::Power::MONOLOGUE_POWER,
                sts2_core::PowerAmount::Counted(1),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::KNOCKOUT_BLOW => knockout_blow::<V, _>(fight),
        // `CosmicIndifference`: its block, then one card of the discard pile
        // onto the top of the draw pile.
        __c0 if __c0 == V::Card::COSMIC_INDIFFERENCE => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Asking {
                pile: sts2_core::Pile::Discard,
                selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: crate::behaviour::cards::Resume::Regent(Resume::CosmicIndifferencePutBack)
                    .into(),
            }
        }
        __c0 if __c0 == V::Card::BUNDLE_OF_JOY => bundle_of_joy::<V, _>(fight),
        // `ShiningStrike`: its damage to the target and its stars, then onto
        // the top of the draw pile unless it exhausts. `ExhaustOnNextPlay`
        // was cleared when its result pile was picked, before the play.
        __c0 if __c0 == V::Card::SHINING_STRIKE => {
            use sts2_core::{Amount, Effect, Scaling};

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
            if fight.waiting() {
                return wait::<V>(Resume::ShiningStrikeHit);
            }
            shining_strike_hit::<V, _>(fight)
        }
        // `ParticleWall`: its block. Where it goes after is its own
        // `GetResultPileTypeForCardPlay`.
        __c0 if __c0 == V::Card::PARTICLE_WALL => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        __c0 if __c0 == V::Card::PHOTON_CUT => photon_cut::<V, _>(fight),
        __c0 if __c0 == V::Card::SUMMON_FORTH => summon_forth::<V, _>(fight),
        // `Glimmer`: its `Cards` drawn, then its `PutBack` of the hand onto
        // the top of the draw pile.
        __c0 if __c0 == V::Card::GLIMMER => {
            fight.draw(fight.var(V::CardVar::CARDS));
            let count = usize::try_from(fight.var(V::CardVar::PUT_BACK))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: sts2_core::Pile::Hand,
                selection: sts2_core::Selection::Exactly(count),
                at: crate::behaviour::cards::Resume::Regent(Resume::GlimmerPutBack).into(),
            }
        }
        __c0 if __c0 == V::Card::MANIFEST_AUTHORITY => manifest_authority::<V, _>(fight),
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// Its damage to the target, as one attack, and the attack's results.
fn strike<V: crate::Build, F: Fight<V>>(fight: &mut F) -> sts2_core::AttackResults<V> {
    use sts2_core::{Amount, Effect, Scaling};

    let target = fight.target();
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

/// `KnockoutBlow`: its damage to the target, then its stars where a result
/// says `WasTargetKilled`, whatever brought the target back after.
fn knockout_blow<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let results = strike::<V, _>(fight);
    if fight.waiting() {
        return wait::<V>(Resume::KnockoutBlowHit);
    }
    knockout_blow_hit::<V, _>(fight, &results)
}

/// The rest of `KnockoutBlow`, once its attack is over: its stars where
/// `results` killed.
fn knockout_blow_hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    results: &sts2_core::AttackResults<V>,
) -> Played<V> {
    if results.iter().any(|result| result.killed) {
        fight.gain_stars(fight.var(V::CardVar::STARS));
    }
    Played::Done
}

/// `PhotonCut`: its damage to the target, its `Cards` drawn, then its
/// `PutBack` of the hand onto the top of the draw pile.
fn photon_cut<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    strike::<V, _>(fight);
    fight.draw(fight.var(V::CardVar::CARDS));
    let count = usize::try_from(fight.var(V::CardVar::PUT_BACK))
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .expect("a count of cards is not negative");
    Played::Asking {
        pile: sts2_core::Pile::Hand,
        selection: sts2_core::Selection::Exactly(count),
        at: crate::behaviour::cards::Resume::Regent(Resume::PhotonCutPutBack).into(),
    }
}

/// `SummonForth`: every Sovereign Blade of its owner's outside the hand into
/// the hand, then its `Forge`.
fn summon_forth<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::Pile;

    let blade = V::Card::SOVEREIGN_BLADE;
    let blades: Vec<_> = [Pile::Draw, Pile::Discard, Pile::Exhaust]
        .into_iter()
        .flat_map(|pile| fight.cards_in(pile))
        .filter(|&card| fight.card_ident(card).card == blade)
        .collect();
    fight.act_on(sts2_core::PileAct::IntoHand, &blades);
    fight.forge(blade, fight.var(V::CardVar::FORGE));
    Played::Done
}

/// `MakeItSo.AfterCardPlayedLate`: after a Skill of its owner's, into hand
/// from outside it. That is once the Skills finished this turn come to a
/// whole multiple of its `Cards`. A fight here has one player.
fn make_it_so<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use sts2_core::CardKind;

    let sts2_core::EventHook::CardPlayedLate { card, .. } = hook else {
        unimplemented!("what MakeItSo does for {hook:?}");
    };
    if V::card_kind(card) != Some(CardKind::Skill) {
        return;
    }
    let skills = history
        .finished_this_turn()
        .filter(|&card| V::card_kind(card) == Some(CardKind::Skill))
        .count();
    let skills = crate::behaviour::cards::Number::try_from(skills)
        .expect("a turn's plays fit a game number");
    if skills % printed.face(V::CardVar::CARDS) == 0 {
        asked.push(sts2_core::HookEffect::AddSelfToHand { only_from: None });
    }
}

/// `BundleOfJoy`: `GetDistinctForCombat` of its `Cards` of colorless cards
/// off `CombatCardGeneration`, into hand.
fn bundle_of_joy<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::RunStreamKinds as _;

    let mut cards = V::combat_colorless_cards().to_vec();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
    let made: Vec<_> = match fight.var(V::CardVar::CARDS) {
        3 => fight.generate_cards::<3>(&mut cards, stream).map(Vec::from),
        4 => fight.generate_cards::<4>(&mut cards, stream).map(Vec::from),
        count => unimplemented!("Bundle of Joy making {count} cards"),
    }
    .unwrap_or_else(|_| unimplemented!("a colorless pool short of Bundle of Joy's cards"));
    for card in made {
        fight.add_generated_cards(card, sts2_core::Pile::Hand, 1);
    }
    Played::Done
}

/// `HeavenlyDrill`: its damage to the target, once for each energy its X
/// spent, and twice that from its `Energy` on.
fn heavenly_drill<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

    let mut hits = fight.x_value();
    if hits >= fight.var(V::CardVar::ENERGY) {
        hits *= 2;
    }
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
    });
    Played::Done
}

/// `ManifestAuthority`: its block, then one colorless card off
/// `CombatCardGeneration` into hand, upgraded where it is.
fn manifest_authority<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, RunStreamKinds as _, Scaling};

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let mut cards = V::combat_colorless_cards().to_vec();
    let Ok([card]) = fight.generate_cards::<1>(
        &mut cards,
        <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
    ) else {
        return Played::Done;
    };
    let upgraded = fight.upgraded();
    for at in fight.add_generated_cards(card, sts2_core::Pile::Hand, 1) {
        if upgraded {
            fight.modify_card(
                at,
                sts2_core::Change::Upgrade,
                sts2_core::Lasting::ThisFight,
            );
        }
    }
    Played::Done
}

/// Deliver a notification to one Regent card in the fight.
pub(super) fn event_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match card {
        __c0 if __c0 == V::Card::KINGLY_KICK => uncommon::kingly_kick::<V>(id, hook, asked),
        __c0 if __c0 == V::Card::KINGLY_PUNCH => {
            uncommon::kingly_punch::<V, _>(id, printed, hook, asked)
        }
        __c0 if __c0 == V::Card::MAKE_IT_SO => make_it_so::<V, _>(printed, history, hook, asked),
        card => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// `HeirloomHammer`: its damage to the target, then one card of the hand
/// that shows as colorless picked, to be copied.
fn heirloom_hammer<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

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
    if fight.waiting() {
        return wait::<V>(Resume::HeirloomHammerHit);
    }
    heirloom_hammer_hit::<V, _>(fight)
}

/// The rest of `HeirloomHammer`, once its attack is over: one colorless
/// card of the hand picked, to copy.
fn heirloom_hammer_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| fight.card_ident(card).card.visually_colorless())
        .collect();
    Played::AskingAmong {
        cards,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Regent(Resume::HeirloomHammerCopy).into(),
    }
}

/// `Quasar`: three distinct colorless cards off `CombatCardGeneration`, each
/// upgraded where Quasar is, of which its owner takes one into hand or none.
fn quasar<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

    let mut pool = <V as sts2_core::Rules>::combat_colorless_cards().to_vec();
    let stream = <crate::generated::runstate::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION;
    let mut offered: [CardIdent<V>; sts2_core::CHOICES] = fight
        .generate_cards(&mut pool, stream)
        .unwrap_or_else(|_| unimplemented!("a colorless pool offering fewer than three"));
    if fight.upgraded() {
        for card in &mut offered {
            if card.upgrade < card.card.max_upgrade() {
                card.upgrade += 1;
            }
        }
    }
    Played::Choosing {
        offered: offered.into_iter().collect(),
        skippable: true,
        at: crate::behaviour::cards::Resume::Regent(Resume::QuasarChoice).into(),
    }
}

/// The rest of a play that stopped on a choice, once the player chose.
pub(super) fn resume_chosen<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    chosen: Option<CardIdent<V>>,
) -> Played<V> {
    match at {
        // `CardPileCmd.AddGeneratedCardToCombat` of the one taken, into hand.
        Resume::QuasarChoice => {
            if let Some(card) = chosen {
                fight.add_generated_cards(card, Pile::Hand, 1);
            }
            Played::Done
        }
        at => unreachable!("{at:?} stops on a pick of a pile"),
    }
}

/// The rest of a play that stopped, once the fight has its answer.
pub(super) fn resume_play<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::BegoneTransform => common::begone_transform::<V, _>(fight, chosen),
        // `CardCmd.Transform` of each into a `MinionDiveBomb` for Charge,
        // or a `MinionSacrifice` for Guards, upgraded where the card is.
        Resume::ChargeTransform | Resume::GuardsTransform => {
            let token = match at {
                Resume::ChargeTransform => V::Card::MINION_DIVE_BOMB,
                _ => V::Card::MINION_SACRIFICE,
            };
            for &card in chosen.as_slice() {
                let mut into = CardIdent::new(token);
                into.upgrade = u8::from(fight.upgraded());
                fight.transform(card, into);
            }
            Played::Done
        }
        // `CardPileCmd.Add` onto the top of the draw pile.
        Resume::GlimmerPutBack | Resume::PhotonCutPutBack | Resume::CosmicIndifferencePutBack => {
            fight.act_on(sts2_core::PileAct::OntoDrawTop, chosen.as_slice());
            Played::Done
        }
        Resume::QuasarChoice => unreachable!("Quasar stops on a choice, not a pick"),
        Resume::DecisionsDrew
        | Resume::DecisionsPlayed { .. }
        | Resume::CrushUnder { .. }
        | Resume::CrashLandingHit
        | Resume::ShiningStrikeHit
        | Resume::HeirloomHammerHit
        | Resume::CollisionCourseHit
        | Resume::BeatIntoShapeHit
        | Resume::KnockoutBlowHit
        | Resume::DyingStarHit { .. } => {
            unreachable!("{at:?} waits on a question, not a pick")
        }
        Resume::DecisionsPicked => match chosen.as_slice() {
            [] => Played::Done,
            &[card] => decisions_play::<V, _>(fight, card, fight.var(V::CardVar::REPEAT)),
            picked => unreachable!("Decisions, Decisions picked {} cards", picked.len()),
        },
        // Its `Repeat` of clones of the one picked, each into hand.
        Resume::HeirloomHammerCopy => {
            if let Some(card) = chosen.one_or_none() {
                // The game's loop reads its count before each pass.
                let mut pass = 0;
                while pass < fight.var(V::CardVar::REPEAT) {
                    pass += 1;
                    fight.clone_card(card, Pile::Hand);
                }
            }
            Played::Done
        }
    }
}

/// `Glow`'s Draw Cards Next Turn, once its draw is over.
/// `DecisionsDecisions`' `FromHand` of one Skill that can be played.
///
/// # Panics
///
/// A card in hand whose type the fight works out.
fn decisions_ask<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardKind;

    use crate::generated::cards::CardKeyword;

    let skills = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&at| {
            let kind = V::card_kind(fight.card_ident(at).card).unwrap_or_else(|| {
                unimplemented!("Decisions, Decisions reading a card whose type is computed")
            });
            kind == CardKind::Skill && !fight.card_keyword(at, CardKeyword::Unplayable)
        })
        .collect();
    Played::AskingAmong {
        cards: skills,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Regent(Resume::DecisionsPicked).into(),
    }
}

/// `CardCmd.AutoPlay` of the Skill picked, its `Repeat` times, wherever the
/// card stands by each play.
///
/// # Panics
///
/// A play that stops on a question with more to come, and a card no pile
/// holds by its turn.
fn decisions_play<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    card: sts2_core::CardAt<V>,
    plays: crate::behaviour::cards::Number,
) -> Played<V> {
    for played in 1..=plays {
        let at = crate::behaviour::cards::standing::<V, _>(fight, card)
            .unwrap_or_else(|| unimplemented!("Decisions, Decisions playing a card no pile holds"));
        fight.auto_play_card(at);
        // `CardCmd.AutoPlay` is awaited, so the plays left wait on the answer.
        if fight.waiting() {
            let left = u8::try_from(plays - played).expect("a few plays are left");
            return Played::Waiting {
                at: crate::behaviour::cards::Resume::Regent(Resume::DecisionsPlayed { card, left })
                    .into(),
            };
        }
    }
    Played::Done
}

/// The rest of a Regent card's play that stood still on a question its own
/// command raised.
pub(super) fn resume_waited<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
) -> Played<V> {
    match at {
        Resume::CrushUnder { held, at } => crush_under_powers::<V, _>(fight, held, at),
        Resume::CrashLandingHit => crash_landing_hit::<V, _>(fight),
        Resume::ShiningStrikeHit => shining_strike_hit::<V, _>(fight),
        Resume::HeirloomHammerHit => heirloom_hammer_hit::<V, _>(fight),
        Resume::CollisionCourseHit => common::collision_course_hit::<V, _>(fight),
        Resume::BeatIntoShapeHit => {
            let results = fight.waited_attack_results();
            beat_into_shape_hit::<V, _>(fight, &results)
        }
        Resume::KnockoutBlowHit => {
            let results = fight.waited_attack_results();
            knockout_blow_hit::<V, _>(fight, &results)
        }
        Resume::DyingStarHit { held } => dying_star_hit::<V, _>(fight, held),
        Resume::DecisionsDrew => decisions_ask::<V, _>(fight),
        Resume::DecisionsPlayed { card, left } => {
            decisions_play::<V, _>(fight, card, crate::behaviour::cards::Number::from(left))
        }
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}
