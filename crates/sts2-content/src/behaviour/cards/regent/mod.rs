//! The Regent's cards the generator leaves to a hand port, filed by rarity.

use crate::behaviour::Played;
use crate::behaviour::cards::{Ask, Lent, Number};
use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};
use sts2_core::{CardIdent, Fight, FightKind, Pile, PowerAmount};

/// Cards a play offers from the hand.
type Hand<V> = smallvec::SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]>;

/// One card, as a pick asks for it.
const ONE: sts2_core::Selection = sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE);

mod common;
mod uncommon;

/// The rest of `CrashLanding`, once its attack is over: Debris into every
/// free place in the hand.
fn crash_landing_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    if room > 0 {
        let debris = CardIdent::new(V::Card::DEBRIS);
        fight.add_generated_cards(debris, Pile::Hand, room);
    }
}

/// The rest of `DyingStar`, once its attack is over: its Dying Star on each
/// of `held`.
fn dying_star_hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    held: crate::behaviour::cards::Held,
) {
    use sts2_core::PowerAmount;

    let loss = PowerAmount::Counted(fight.var(V::CardVar::STRENGTH_LOSS));
    for target in held.creatures() {
        fight.apply_power(target, V::Power::DYING_STAR_POWER, loss);
    }
}

/// The rest of `ShiningStrike`, once its attack is over: its stars, then
/// itself onto the draw pile, unless it exhausts.
fn shining_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use crate::generated::cards::CardKeyword;
    use sts2_core::{Pile, PilePosition};

    fight.gain_stars(fight.var(V::CardVar::STARS));
    if !fight.this_has_keyword(CardKeyword::Exhaust) {
        fight.add_this(Pile::Draw, PilePosition::Top);
    }
}

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match card {
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
        // `VoidForm`: its Void Form on its owner, then `PlayerCmd.EndTurn`.
        __c0 if __c0 == V::Card::VOID_FORM => {
            use sts2_core::PowerAmount;

            let amount = fight.power_var(V::CardVar::VOID_FORM_POWER);
            fight.apply_to_actor(V::Power::VOID_FORM_POWER, PowerAmount::Counted(amount));
            fight.end_turn();
            Played::Done
        }
        __c0 if __c0 == V::Card::GLITTERSTREAM => common::glitterstream::<V, _>(fight),
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
        __c0 if __c0 == V::Card::BUNDLE_OF_JOY => bundle_of_joy::<V, _>(fight),
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
        __c0 if __c0 == V::Card::SUMMON_FORTH => summon_forth::<V, _>(fight),
        __c0 if __c0 == V::Card::MANIFEST_AUTHORITY => manifest_authority::<V, _>(fight),
        __c0 if __c0 == V::Card::CRASH_LANDING
            || __c0 == V::Card::DYING_STAR
            || __c0 == V::Card::CRUSH_UNDER
            || __c0 == V::Card::DECISIONS_DECISIONS
            || __c0 == V::Card::BEAT_INTO_SHAPE
            || __c0 == V::Card::QUASAR
            || __c0 == V::Card::HEIRLOOM_HAMMER
            || __c0 == V::Card::GUARDS
            || __c0 == V::Card::BEGONE
            || __c0 == V::Card::CHARGE
            || __c0 == V::Card::COLLISION_COURSE
            || __c0 == V::Card::KNOCKOUT_BLOW
            || __c0 == V::Card::COSMIC_INDIFFERENCE
            || __c0 == V::Card::SHINING_STRIKE
            || __c0 == V::Card::PHOTON_CUT
            || __c0 == V::Card::GLIMMER =>
        {
            start::<V, K>(card, fight)
        }
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// A Regent card's play that can stop on a question: from its start to its
/// end, yielding each question it stops on. Each step is lent the fight, and
/// after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `CrashLanding`: its damage to every enemy, as one attack, then
        // Debris into hand until the hand is full.
        __c0 if __c0 == V::Card::CRASH_LANDING => {
            hit_all::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            crash_landing_hit::<V, _>(resumed!().fight);
        }
        // `DyingStar`: its damage to every enemy, as one attack, then its
        // `StrengthLoss` of Dying Star on each enemy that could be hit as the
        // play began.
        __c0 if __c0 == V::Card::DYING_STAR => {
            let held: crate::behaviour::cards::Held =
                crate::behaviour::cards::Held::of(&resumed!().fight.hittable_opponents());
            hit_all::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            dying_star_hit::<V, _>(resumed!().fight, held);
        }
        // `CrushUnder`: the opponents taken, its damage to every enemy, then
        // its Crush Under on each one taken, its amount read once. A question
        // left before one waits, and the amount is read again after.
        __c0 if __c0 == V::Card::CRUSH_UNDER => {
            let held: crate::behaviour::cards::Held =
                crate::behaviour::cards::Held::of(&resumed!().fight.opponents());
            hit_all::<V, _>(resumed!().fight);
            let count: usize = held.creatures().count();
            let mut amount: Number = resumed!().fight.power_var(V::CardVar::STRENGTH_LOSS);
            for at in 0..count {
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                    amount = resumed!().fight.power_var(V::CardVar::STRENGTH_LOSS);
                }
                let target = held.creatures().nth(at).expect("one of those held");
                resumed!().fight.apply_power(
                    target,
                    V::Power::CRUSH_UNDER_POWER,
                    PowerAmount::Counted(amount),
                );
            }
        }
        // `DecisionsDecisions`: its `Cards` drawn, then a playable Skill of
        // the hand picked, then `CardCmd.AutoPlay` of it its `Repeat` times,
        // wherever the card stands by each play. A shuffle inside the draw
        // can ask, and the rest waits. Each auto-play is awaited, so the
        // plays left wait on its answer.
        __c0 if __c0 == V::Card::DECISIONS_DECISIONS => {
            let fight = &mut *resumed!().fight;
            fight.draw(fight.var(V::CardVar::CARDS));
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let skills: Hand<V> = playable_skills::<V, _>(resumed!().fight);
            yield_!(Ask::AskingAmong {
                cards: skills,
                selection: ONE,
            });
            let picked: Option<sts2_core::CardAt<V>> =
                match crate::behaviour::cards::selected::<V>(&resumed!().taken).as_slice() {
                    [] => None,
                    &[skill] => Some(skill),
                    many => unreachable!("Decisions, Decisions picked {} cards", many.len()),
                };
            if let Some(found) = picked {
                let skill: sts2_core::CardAt<V> = found;
                let plays: Number = resumed!().fight.var(V::CardVar::REPEAT);
                for _ in 0..plays {
                    let fight = &mut *resumed!().fight;
                    let at = crate::behaviour::cards::standing::<V, _>(fight, skill)
                        .unwrap_or_else(|| {
                            unimplemented!("Decisions, Decisions playing a card no pile holds")
                        });
                    fight.auto_play_card(at);
                    if resumed!().fight.waiting() {
                        yield_!(Ask::Waiting);
                    }
                }
            }
        }
        // `BeatIntoShape`: its damage to the target, then a Forge of its
        // calculated number, less its extra for each hit of its own attack.
        __c0 if __c0 == V::Card::BEAT_INTO_SHAPE => {
            let mut hits: usize = strike::<V, _>(resumed!().fight).len();
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                hits = resumed!().fight.waited_attack_results().len();
            }
            let fight = &mut *resumed!().fight;
            let hits = Number::try_from(hits).expect("an attack's hits fit a number");
            let amount = fight.var(V::CardVar::CALCULATED_FORGE)
                - hits * fight.var(V::CardVar::CALCULATION_EXTRA);
            fight.forge(V::Card::SOVEREIGN_BLADE, amount);
        }
        // `Quasar`: three distinct colorless cards off
        // `CombatCardGeneration`, each upgraded where Quasar is, of which its
        // owner takes one into hand or none, as
        // `CardPileCmd.AddGeneratedCardToCombat`.
        __c0 if __c0 == V::Card::QUASAR => {
            let offered: sts2_core::Offered<V> = quasar_offer::<V, _>(resumed!().fight);
            yield_!(Ask::Choosing {
                offered,
                skippable: true,
            });
            if let Some(taken) = crate::behaviour::cards::chosen::<V>(&resumed!().taken) {
                resumed!().fight.add_generated_cards(taken, Pile::Hand, 1);
            }
        }
        // `HeirloomHammer`: its damage to the target, then one card of the
        // hand that shows as colorless picked, and its `Repeat` of clones of
        // it into hand.
        __c0 if __c0 == V::Card::HEIRLOOM_HAMMER => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let colorless: Hand<V> = colorless_in_hand::<V, _>(resumed!().fight);
            yield_!(Ask::AskingAmong {
                cards: colorless,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            if let Some(picked) = crate::behaviour::cards::selected::<V>(taken).one_or_none() {
                // The game's loop reads its count before each pass.
                let mut pass = 0;
                while pass < fight.var(V::CardVar::REPEAT) {
                    pass += 1;
                    fight.clone_card(picked, Pile::Hand);
                }
            }
        }
        // `Guards`: any number of the hand's cards picked, each
        // `CardCmd.Transform` into a `MinionSacrifice`.
        __c0 if __c0 == V::Card::GUARDS => {
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: sts2_core::Selection::UpTo(
                    nonmax::NonMaxUsize::new(999_999_999).expect("a count below the maximum"),
                ),
            });
            transform_taken::<V, K>(resumed!(), V::Card::MINION_SACRIFICE);
        }
        // `Begone`: a card in hand turned into a Minion Strike.
        __c0 if __c0 == V::Card::BEGONE => {
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            common::begone_transform::<V, _>(
                &mut **fight,
                crate::behaviour::cards::selected::<V>(taken),
            );
        }
        // `Charge`: its `Cards` of the draw pile picked, each
        // `CardCmd.Transform` into a `MinionDiveBomb`.
        __c0 if __c0 == V::Card::CHARGE => {
            let count: nonmax::NonMaxUsize = nonmax::NonMaxUsize::new(
                num_traits::cast(resumed!().fight.var(V::CardVar::CARDS)).unwrap_or(0),
            )
            .unwrap_or(nonmax::NonMaxUsize::ONE);
            yield_!(Ask::Asking {
                pile: Pile::Draw,
                selection: sts2_core::Selection::Exactly(count),
            });
            transform_taken::<V, K>(resumed!(), V::Card::MINION_DIVE_BOMB);
        }
        // `CollisionCourse`: its damage to the target, then a Debris into
        // hand.
        __c0 if __c0 == V::Card::COLLISION_COURSE => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            common::collision_course_hit::<V, _>(resumed!().fight);
        }
        // `KnockoutBlow`: its damage to the target, then its stars where a
        // result says `WasTargetKilled`, whatever brought the target back
        // after.
        __c0 if __c0 == V::Card::KNOCKOUT_BLOW => {
            let mut killed: bool = strike::<V, _>(resumed!().fight)
                .iter()
                .any(|result| result.killed);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                killed = resumed!()
                    .fight
                    .waited_attack_results()
                    .iter()
                    .any(|result| result.killed);
            }
            if killed {
                let fight = &mut *resumed!().fight;
                fight.gain_stars(fight.var(V::CardVar::STARS));
            }
        }
        // `CosmicIndifference`: its block, then one card of the discard pile
        // onto the top of the draw pile.
        __c0 if __c0 == V::Card::COSMIC_INDIFFERENCE => {
            block::<V, _>(resumed!().fight);
            yield_!(Ask::Asking {
                pile: Pile::Discard,
                selection: ONE,
            });
            put_back_taken::<V, K>(resumed!());
        }
        // `ShiningStrike`: its damage to the target and its stars, then onto
        // the top of the draw pile unless it exhausts. `ExhaustOnNextPlay`
        // was cleared when its result pile was picked, before the play.
        __c0 if __c0 == V::Card::SHINING_STRIKE => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            shining_strike_hit::<V, _>(resumed!().fight);
        }
        // `PhotonCut`: its damage to the target, its `Cards` drawn, then its
        // `PutBack` of the hand onto the top of the draw pile.
        __c0 if __c0 == V::Card::PHOTON_CUT => {
            strike::<V, _>(resumed!().fight);
            let fight = &mut *resumed!().fight;
            fight.draw(fight.var(V::CardVar::CARDS));
            let count: nonmax::NonMaxUsize = count_of(resumed!().fight.var(V::CardVar::PUT_BACK));
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: sts2_core::Selection::Exactly(count),
            });
            put_back_taken::<V, K>(resumed!());
        }
        // `Glimmer`: its `Cards` drawn, then its `PutBack` of the hand onto
        // the top of the draw pile.
        __c0 if __c0 == V::Card::GLIMMER => {
            let fight = &mut *resumed!().fight;
            fight.draw(fight.var(V::CardVar::CARDS));
            let count: nonmax::NonMaxUsize = count_of(resumed!().fight.var(V::CardVar::PUT_BACK));
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: sts2_core::Selection::Exactly(count),
            });
            put_back_taken::<V, K>(resumed!());
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

crate::behaviour::cards::paused_play!(Regent);

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

/// `SummonForth`: every Sovereign Blade of its owner's outside the hand into
/// the hand, then its `Forge`.
fn summon_forth<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
fn bundle_of_joy<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    use sts2_core::RunStreamKinds as _;

    let mut cards = V::combat_colorless_cards(fight.multiplayer()).to_vec();
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
fn heavenly_drill<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
fn manifest_authority<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    use sts2_core::{Amount, Effect, RunStreamKinds as _, Scaling};

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let mut cards = V::combat_colorless_cards(fight.multiplayer()).to_vec();
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

/// `DecisionsDecisions`' `FromHand`: the Skills of the hand that can be
/// played.
///
/// # Panics
///
/// A card in hand whose type the fight works out.
fn playable_skills<V: crate::Build, F: Fight<V>>(fight: &F) -> Hand<V> {
    use sts2_core::CardKind;

    use crate::generated::cards::CardKeyword;

    fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&at| {
            let kind = V::card_kind(fight.card_ident(at).card).unwrap_or_else(|| {
                unimplemented!("Decisions, Decisions reading a card whose type is computed")
            });
            kind == CardKind::Skill && !fight.card_keyword(at, CardKeyword::Unplayable)
        })
        .collect()
}

/// The cards of the hand that show as colorless.
fn colorless_in_hand<V: crate::Build, F: Fight<V>>(fight: &F) -> Hand<V> {
    use sts2_core::CardTable as _;

    fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| fight.card_ident(card).card.visually_colorless())
        .collect()
}

/// `Quasar`'s offer: three distinct colorless cards off
/// `CombatCardGeneration`, each upgraded where Quasar is.
fn quasar_offer<V: crate::Build, F: Fight<V>>(fight: &mut F) -> sts2_core::Offered<V> {
    use sts2_core::CardTable as _;

    let mut pool = <V as sts2_core::Rules>::combat_colorless_cards(fight.multiplayer()).to_vec();
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
    offered.into_iter().collect()
}

/// Its damage to every enemy, as one attack and one `CreatureCmd.Damage`.
fn hit_all<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::{Amount, Effect, Scaling};

    fight.attack(Scaling::Dynamic, |fight| {
        let targets = fight.opponents();
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

/// Its block, onto its owner.
fn block<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::{Amount, Effect, Scaling};

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
}

/// A count of cards a card's number gives.
fn count_of(number: crate::behaviour::cards::Number) -> nonmax::NonMaxUsize {
    usize::try_from(number)
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .expect("a count of cards is not negative")
}

/// `CardCmd.Transform` of each card a pick took into `token`, upgraded where
/// the card is.
fn transform_taken<V: crate::Build, K: FightKind<V>>(
    lent: &mut Lent<'_, '_, V, K>,
    token: V::Card,
) {
    let Lent { fight, taken } = lent;
    let upgrade = u8::from(fight.upgraded());
    for &picked in crate::behaviour::cards::selected::<V>(taken).as_slice() {
        let mut into = CardIdent::new(token);
        into.upgrade = upgrade;
        fight.transform(picked, into);
    }
}

/// `CardPileCmd.Add` of each card a pick took onto the top of the draw pile.
fn put_back_taken<V: crate::Build, K: FightKind<V>>(lent: &mut Lent<'_, '_, V, K>) {
    let Lent { fight, taken } = lent;
    fight.act_on(
        sts2_core::PileAct::OntoDrawTop,
        crate::behaviour::cards::selected::<V>(taken).as_slice(),
    );
}
