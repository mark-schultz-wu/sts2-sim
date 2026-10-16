//! The Ironclad's cards the generator leaves to a hand port.

use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};
use sts2_core::{
    Amount, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight, HookEffect,
    Lasting, Pile, PileAct, Played, PowerAmount, Scaling,
};

/// Where an Ironclad card's play stops to ask something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume<V: crate::Build> {
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
    /// `Pillage`, after a question inside one of its draws was answered.
    PillageDrew,
    /// `FiendFire`, after a question inside its exhausts was answered, with
    /// the cards left to exhaust and this many hits to land.
    FiendFireExhausting {
        left: crate::behaviour::cards::Left<V>,
        hits: u8,
    },
    /// After a question inside the card's loss of health was answered.
    BloodlettingLost,
    OfferingLost,
    HemokinesisLost,
    BreakthroughLost,
    BloodWallLost,
    /// After a question inside the card's attack was answered.
    RampageHit,
    AngerHit,
    ThrashHit,
    UppercutHit,
    MoltenFistHit,
    PillageHit,
    CinderHit,
    /// `Feed`, whose kill counts where `fatal` says.
    FeedHit {
        fatal: bool,
    },
    /// After a question inside the card's power was answered.
    UppercutWeak,
    DominateApplied,
    CrimsonMantleApplied,
    InfernoApplied,
    /// After a question inside the card's block was answered.
    EvilEyeBlocked,
    TrueGritBlocked,
    ArmamentsBlocked,
    /// `SecondWind`, with the cards left to exhaust. `blocked` says whether
    /// the block for the last exhaust is in.
    SecondWind {
        left: crate::behaviour::cards::Left<V>,
        blocked: bool,
    },
    /// `Stoke`, with the cards left to exhaust and how many the hand held.
    StokeExhausting {
        left: crate::behaviour::cards::Left<V>,
        count: u8,
    },
}

/// The rest of the play waits at `at`, where a command left a question.
fn wait<V: crate::Build>(at: Resume<V>) -> Played<V> {
    Played::Waiting {
        at: crate::behaviour::cards::Resume::Ironclad(at).into(),
    }
}

/// The rest of an Ironclad card's play that stood still on a question its
/// own command raised.
pub(super) fn resume_waited<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
) -> Played<V> {
    match at {
        Resume::FiendFireExhausting { left, hits } => {
            fiend_fire_exhausting::<V, _>(fight, left, hits)
        }
        // A draw that never ran, as the fight ended, drew nothing.
        Resume::PillageDrew => Played::Done,
        Resume::BloodlettingLost => bloodletting_lost::<V, _>(fight),
        Resume::OfferingLost => offering_lost::<V, _>(fight),
        Resume::HemokinesisLost => {
            let target = fight.target();
            strike::<V, _>(fight, target);
            Played::Done
        }
        Resume::BreakthroughLost => breakthrough_lost::<V, _>(fight),
        Resume::BloodWallLost => {
            block::<V, _>(fight);
            Played::Done
        }
        Resume::RampageHit => rampage_hit::<V, _>(fight),
        Resume::AngerHit => {
            fight.clone_this(Pile::Discard);
            Played::Done
        }
        Resume::ThrashHit => thrash_hit::<V, _>(fight),
        Resume::UppercutHit => uppercut_hit::<V, _>(fight),
        Resume::UppercutWeak => uppercut_weak::<V, _>(fight),
        Resume::MoltenFistHit => molten_fist_hit::<V, _>(fight),
        Resume::PillageHit => pillage_draws::<V, _>(fight),
        Resume::CinderHit => cinder_hit::<V, _>(fight),
        Resume::FeedHit { fatal } => {
            let results = fight.waited_attack_results();
            feed_hit::<V, _>(fight, fatal && results.iter().any(|result| result.killed))
        }
        Resume::DominateApplied => dominate_applied::<V, _>(fight),
        Resume::CrimsonMantleApplied => {
            fight.note_on_power(fight.actor(), V::Power::CRIMSON_MANTLE_POWER, 1);
            Played::Done
        }
        Resume::InfernoApplied => {
            fight.note_on_power(fight.actor(), V::Power::INFERNO_POWER, 1);
            Played::Done
        }
        Resume::EvilEyeBlocked => {
            block::<V, _>(fight);
            Played::Done
        }
        Resume::TrueGritBlocked => true_grit_blocked::<V, _>(fight),
        Resume::ArmamentsBlocked => armaments_blocked::<V, _>(fight),
        Resume::SecondWind { left, blocked } => second_wind::<V, _>(fight, left, blocked),
        Resume::StokeExhausting { left, count } => stoke_exhausting::<V, _>(fight, left, count),
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}

/// `FiendFire`, once its exhausts are over: its damage to the target, `hits`
/// times.
fn fiend_fire_hits<V: crate::Build, F: Fight<V>>(fight: &mut F, hits: usize) -> Played<V> {
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

pub(super) fn resume_play<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
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
        // `CardCmd.Exhaust` of the one taken, then its cards drawn.
        Resume::BurningPactExhaust => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            fight.draw(fight.var(V::CardVar::CARDS));
            Played::Done
        }
        // `CardCmd.Exhaust` of the one taken, then its Strength.
        Resume::BrandExhaust => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            let strength = fight.var(V::CardVar::STRENGTH_POWER);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
            Played::Done
        }
        // `CardCmd.Upgrade` of the one taken.
        Resume::ArmamentsUpgrade => {
            for &card in chosen.as_slice() {
                fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
            }
            Played::Done
        }
        at => unreachable!("{at:?} waits on a question, not a pick"),
    }
}

pub(super) fn on_play<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V> {
    match card {
        __c0 if __c0 == V::Card::THRASH => thrash::<V, _>(fight),
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
        // `Inferno`: its Inferno on its owner, then one more self damage on
        // the power it landed or stacked into.
        // `CrimsonMantle`: its Crimson Mantle on its owner, then the power's
        // self damage raised by one.
        __c0 if __c0 == V::Card::CRIMSON_MANTLE => {
            let mantle = PowerAmount::Counted(fight.var(V::CardVar::CRIMSON_MANTLE_POWER));
            fight.apply_to_actor(V::Power::CRIMSON_MANTLE_POWER, mantle);
            if fight.waiting() {
                return wait::<V>(Resume::CrimsonMantleApplied);
            }
            fight.note_on_power(fight.actor(), V::Power::CRIMSON_MANTLE_POWER, 1);
            Played::Done
        }
        __c0 if __c0 == V::Card::INFERNO => {
            let inferno = PowerAmount::Counted(fight.var(V::CardVar::INFERNO_POWER));
            fight.apply_to_actor(V::Power::INFERNO_POWER, inferno);
            if fight.waiting() {
                return wait::<V>(Resume::InfernoApplied);
            }
            fight.note_on_power(fight.actor(), V::Power::INFERNO_POWER, 1);
            Played::Done
        }
        // `Dominate`: its Vulnerable on the target, then as much Strength on
        // its owner as the target then carries Vulnerable.
        __c0 if __c0 == V::Card::DOMINATE => {
            let target = fight.target();
            let vulnerable = PowerAmount::Counted(fight.var(V::CardVar::VULNERABLE_POWER));
            fight.apply_power(target, V::Power::VULNERABLE_POWER, vulnerable);
            if fight.waiting() {
                return wait::<V>(Resume::DominateApplied);
            }
            dominate_applied::<V, _>(fight)
        }
        // `Feed`: its damage to the target, then its `MaxHp` onto its owner's
        // maximum where a result says `WasTargetKilled` of a target whose
        // death, read before the hit, triggers Fatal.
        __c0 if __c0 == V::Card::FEED => {
            let target = fight.target();
            let fatal = fight.death_triggers_fatal(target);
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
                return wait::<V>(Resume::FeedHit { fatal });
            }
            feed_hit::<V, _>(fight, fatal && results.iter().any(|result| result.killed))
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
        // `SecondWind`: each card in hand that is not an Attack exhausted,
        // and its block for each, one after another.
        __c0 if __c0 == V::Card::SECOND_WIND => {
            let cards: Vec<_> = fight
                .cards_in(Pile::Hand)
                .into_iter()
                .filter(|&card| V::card_kind(fight.card_ident(card).card) != Some(CardKind::Attack))
                .collect();
            second_wind::<V, _>(fight, crate::behaviour::cards::Left::of(&cards), true)
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
        // `FiendFire`: every card in hand exhausted, then its damage to the
        // target once for each.
        // A hook's question inside an exhaust stands the rest still.
        __c0 if __c0 == V::Card::FIEND_FIRE => {
            let cards = fight.cards_in(Pile::Hand);
            let hits = u8::try_from(cards.len()).expect("a hand's worth of hits fits a u8");
            fiend_fire_exhausting::<V, _>(fight, crate::behaviour::cards::Left::of(&cards), hits)
        }
        // `BurningPact`: one card of the hand exhausted, then its `Cards`
        // drawn.
        __c0 if __c0 == V::Card::BURNING_PACT => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: crate::behaviour::cards::Resume::Ironclad(Resume::BurningPactExhaust).into(),
        },
        // `Rampage`: its damage to the target, then its `Increase` onto its
        // own damage for the fight.
        __c0 if __c0 == V::Card::RAMPAGE => {
            let target = fight.target();
            strike::<V, _>(fight, target);
            if fight.waiting() {
                return wait::<V>(Resume::RampageHit);
            }
            rampage_hit::<V, _>(fight)
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
        // `Anger`: its damage to the target, then a clone of itself into the
        // discard pile.
        __c0 if __c0 == V::Card::ANGER => {
            let target = fight.target();
            strike::<V, _>(fight, target);
            if fight.waiting() {
                return wait::<V>(Resume::AngerHit);
            }
            fight.clone_this(Pile::Discard);
            Played::Done
        }
        // `Headbutt`: its damage to the target, then one card of the discard
        // pile onto the top of the draw pile.
        __c0 if __c0 == V::Card::HEADBUTT => {
            let target = fight.target();
            strike::<V, _>(fight, target);
            headbutt_hit::<V>()
        }
        __c0 if __c0 == V::Card::WHIRLWIND => whirlwind::<V, _>(fight),
        __c0 if __c0 == V::Card::BLOODLETTING => bloodletting::<V, _>(fight),
        // `Offering`: its `HpLoss` off its owner, unblockable, then its
        // energy and its cards drawn.
        __c0 if __c0 == V::Card::OFFERING => {
            fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
            if fight.waiting() {
                return wait::<V>(Resume::OfferingLost);
            }
            offering_lost::<V, _>(fight)
        }
        __c0 if __c0 == V::Card::UPPERCUT => uppercut::<V, _>(fight),
        __c0 if __c0 == V::Card::MOLTEN_FIST => molten_fist::<V, _>(fight),
        // `Cinder`: its damage to the target, then a card from hand drawn off
        // `CombatCardSelection`, exhausted.
        __c0 if __c0 == V::Card::CINDER => {
            let target = fight.target();
            strike::<V, _>(fight, target);
            if fight.waiting() {
                return wait::<V>(Resume::CinderHit);
            }
            cinder_hit::<V, _>(fight)
        }
        __c0 if __c0 == V::Card::PILLAGE => pillage::<V, _>(fight),
        __c0 if __c0 == V::Card::TRUE_GRIT => true_grit::<V, _>(fight),
        __c0 if __c0 == V::Card::DISMANTLE => dismantle::<V, _>(fight),
        // `Brand`: its `HpLoss` off its owner, unblockable, then one card of
        // the hand exhausted, then its Strength.
        __c0 if __c0 == V::Card::BRAND => {
            fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
            brand_lost::<V>()
        }
        __c0 if __c0 == V::Card::PRIMAL_FORCE => primal_force::<V, _>(fight),
        // `Hemokinesis`: its `HpLoss` off its owner, unblockable, then its
        // damage to the target.
        __c0 if __c0 == V::Card::HEMOKINESIS => {
            fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
            if fight.waiting() {
                return wait::<V>(Resume::HemokinesisLost);
            }
            let target = fight.target();
            strike::<V, _>(fight, target);
            Played::Done
        }
        // `Breakthrough`: its `HpLoss` off its owner, unblockable, then its
        // damage to every enemy.
        __c0 if __c0 == V::Card::BREAKTHROUGH => {
            fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
            if fight.waiting() {
                return wait::<V>(Resume::BreakthroughLost);
            }
            breakthrough_lost::<V, _>(fight)
        }
        // `BloodWall`: its `HpLoss` off its owner, unblockable, then its
        // block.
        __c0 if __c0 == V::Card::BLOOD_WALL => {
            fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
            if fight.waiting() {
                return wait::<V>(Resume::BloodWallLost);
            }
            block::<V, _>(fight);
            Played::Done
        }
        __c0 if __c0 == V::Card::ARMAMENTS => armaments::<V, _>(fight),
        // `Cascade`: the top cards of the draw pile auto-played, as many as
        // its X spent and one more once upgraded.
        __c0 if __c0 == V::Card::CASCADE => {
            let count = fight.x_value() + crate::behaviour::cards::Number::from(fight.upgraded());
            let count = usize::try_from(count).expect("an X is not negative");
            fight.auto_play_from_draw(count, false);
            Played::Done
        }
        // `EvilEye`: its block, and again where its owner exhausted a card
        // this turn. A fight here has one player, whose cards are all its
        // owner's.
        __c0 if __c0 == V::Card::EVIL_EYE => {
            let gains = if fight.history().exhausted_this_turn() {
                2
            } else {
                1
            };
            block::<V, _>(fight);
            if gains == 2 {
                if fight.waiting() {
                    return wait::<V>(Resume::EvilEyeBlocked);
                }
                block::<V, _>(fight);
            }
            Played::Done
        }
        // `Stoke`: the whole hand exhausted, one card at a time. Then as many
        // cards of the character's pool, each `NextItem` off
        // `CombatCardGeneration`, upgraded once Stoke is, into hand.
        __c0 if __c0 == V::Card::STOKE => {
            let hand = fight.cards_in(Pile::Hand);
            let count = u8::try_from(hand.len()).expect("a hand's worth fits a u8");
            stoke_exhausting::<V, _>(fight, crate::behaviour::cards::Left::of(&hand), count)
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
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// `Dismantle`: its damage to the target, twice where the target carries
/// Vulnerable.
fn dismantle<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn primal_force<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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

/// `TrueGrit`: its block, then a card from hand exhausted. Upgraded, its
/// owner picks the card. Otherwise it is drawn off `CombatCardSelection`.
fn true_grit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    block::<V, _>(fight);
    if fight.waiting() {
        return wait::<V>(Resume::TrueGritBlocked);
    }
    true_grit_blocked::<V, _>(fight)
}

/// The rest of `TrueGrit`, once its block is in.
fn true_grit_blocked<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.upgraded() {
        return Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: crate::behaviour::cards::Resume::Ironclad(Resume::TrueGritExhaust).into(),
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
fn armaments<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    block::<V, _>(fight);
    if fight.waiting() {
        return wait::<V>(Resume::ArmamentsBlocked);
    }
    armaments_blocked::<V, _>(fight)
}

/// The rest of `Armaments`, once its block is in.
fn armaments_blocked<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

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
        at: crate::behaviour::cards::Resume::Ironclad(Resume::ArmamentsUpgrade).into(),
    }
}

/// `Bloodletting`: its `HpLoss` off its owner, unblockable, then its energy.
fn bloodletting<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.damage_unblockable(fight.actor(), fight.var(V::CardVar::HP_LOSS));
    if fight.waiting() {
        return wait::<V>(Resume::BloodlettingLost);
    }
    bloodletting_lost::<V, _>(fight)
}

/// The rest of `Bloodletting`, once its loss is over: its energy.
fn bloodletting_lost<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.modify_energy(fight.var(V::CardVar::ENERGY));
    Played::Done
}

/// The rest of `Offering`, once its loss is over: its energy, then its
/// cards drawn.
fn offering_lost<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.modify_energy(fight.var(V::CardVar::ENERGY));
    fight.draw(fight.var(V::CardVar::CARDS));
    Played::Done
}

/// The rest of `Brand`, once its loss is over: one card of the hand
/// exhausted, then its Strength.
fn brand_lost<V: crate::Build>() -> Played<V> {
    Played::Asking {
        pile: Pile::Hand,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Ironclad(Resume::BrandExhaust).into(),
    }
}

/// The rest of `Breakthrough`, once its loss is over: its damage to every
/// enemy.
fn breakthrough_lost<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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

/// The rest of `Rampage`, once its attack is over: its `Increase` onto its
/// own damage for the fight.
fn rampage_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let increase = fight.var(V::CardVar::INCREASE);
    fight.modify_this(
        Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(increase)),
        Lasting::ThisFight,
    );
    Played::Done
}

/// The rest of `Headbutt`, once its attack is over: one card of the discard
/// pile onto the top of the draw pile.
fn headbutt_hit<V: crate::Build>() -> Played<V> {
    Played::Asking {
        pile: Pile::Discard,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Ironclad(Resume::HeadbuttOntoDraw).into(),
    }
}

/// The rest of `Cinder`, once its attack is over: a card from hand drawn
/// off `CombatCardSelection`, exhausted.
fn cinder_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    if let Some(card) = fight.random_card_in(Pile::Hand) {
        fight.act_on(PileAct::Exhaust, &[card]);
    }
    Played::Done
}

/// The rest of `Feed`, once its attack is over: its `MaxHp` where `gains`.
fn feed_hit<V: crate::Build, F: Fight<V>>(fight: &mut F, gains: bool) -> Played<V> {
    if gains {
        fight.gain_max_hp(fight.actor(), fight.var(V::CardVar::MAX_HP));
    }
    Played::Done
}

/// The rest of `Dominate`, once its Vulnerable is on: as much Strength on
/// its owner as the target then carries Vulnerable.
fn dominate_applied<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    Played::Done
}

/// `SecondWind` over the cards `left`: each exhausted, then its block.
/// `blocked` says the block for the card before them is in.
fn second_wind<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    left: crate::behaviour::cards::Left<V>,
    blocked: bool,
) -> Played<V> {
    if !blocked {
        block::<V, _>(fight);
    }
    let cards: Vec<_> = left.cards().collect();
    for (at, &card) in cards.iter().enumerate() {
        if fight.waiting() {
            let left = crate::behaviour::cards::Left::of(&cards[at..]);
            return wait::<V>(Resume::SecondWind {
                left,
                blocked: true,
            });
        }
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = crate::behaviour::cards::Left::of(&cards[at + 1..]);
            return wait::<V>(Resume::SecondWind {
                left,
                blocked: false,
            });
        }
        block::<V, _>(fight);
    }
    Played::Done
}

/// `FiendFire` over the cards `left`: each exhausted, then `hits` hits.
fn fiend_fire_exhausting<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    left: crate::behaviour::cards::Left<V>,
    hits: u8,
) -> Played<V> {
    let cards: Vec<_> = left.cards().collect();
    for (at, &card) in cards.iter().enumerate() {
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = crate::behaviour::cards::Left::of(&cards[at + 1..]);
            return wait::<V>(Resume::FiendFireExhausting { left, hits });
        }
    }
    fiend_fire_hits::<V, _>(fight, usize::from(hits))
}

/// `Stoke` over the cards `left`: each exhausted, then `count` cards of the
/// character's pool, each `NextItem` off `CombatCardGeneration`, upgraded
/// once Stoke is, into hand.
fn stoke_exhausting<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    left: crate::behaviour::cards::Left<V>,
    count: u8,
) -> Played<V> {
    let cards: Vec<_> = left.cards().collect();
    for (at, &card) in cards.iter().enumerate() {
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = crate::behaviour::cards::Left::of(&cards[at + 1..]);
            return wait::<V>(Resume::StokeExhausting { left, count });
        }
    }
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

/// One hit of the card's damage on `target`.
fn strike<V: crate::Build, F: Fight<V>>(fight: &mut F, target: sts2_core::CombatId) {
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
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
fn thrash<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    if fight.waiting() {
        return wait::<V>(Resume::ThrashHit);
    }
    thrash_hit::<V, _>(fight)
}

/// The rest of `Thrash`, once its attack is over.
fn thrash_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{CardTable as _, RunStreamKinds as _};

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
    Played::Done
}

/// `Uppercut`: its damage to the target, then its `Power` as Weak and as
/// Vulnerable on it.
fn uppercut<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    strike::<V, _>(fight, target);
    if fight.waiting() {
        return wait::<V>(Resume::UppercutHit);
    }
    uppercut_hit::<V, _>(fight)
}

/// The rest of `Uppercut`, once its attack is over: its Weak, then its
/// Vulnerable.
fn uppercut_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = PowerAmount::Counted(fight.var(V::CardVar::POWER));
    fight.apply_power(fight.target(), V::Power::WEAK_POWER, amount);
    if fight.waiting() {
        return wait::<V>(Resume::UppercutWeak);
    }
    uppercut_weak::<V, _>(fight)
}

/// The rest of `Uppercut`, once its Weak is on: its Vulnerable.
fn uppercut_weak<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = PowerAmount::Counted(fight.var(V::CardVar::POWER));
    fight.apply_power(fight.target(), V::Power::VULNERABLE_POWER, amount);
    Played::Done
}

/// `MoltenFist`: its damage to the target, then as much Vulnerable again as
/// the target holds, if it still stands.
fn molten_fist<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    strike::<V, _>(fight, target);
    if fight.waiting() {
        return wait::<V>(Resume::MoltenFistHit);
    }
    molten_fist_hit::<V, _>(fight)
}

/// The rest of `MoltenFist`, once its attack is over.
fn molten_fist_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    Played::Done
}

/// `Pillage`: its damage to the target, then one card drawn at a time. The
/// draws go on while each is an Attack and the hand has room.
fn pillage<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    strike::<V, _>(fight, target);
    if fight.waiting() {
        return wait::<V>(Resume::PillageHit);
    }
    pillage_draws::<V, _>(fight)
}

/// `Pillage`'s draws, one card at a time. A shuffle inside one can ask, and
/// the rest waits for it.
fn pillage_draws<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    loop {
        let drew = fight.draw(1);
        if fight.waiting() {
            return Played::Waiting {
                at: crate::behaviour::cards::Resume::Ironclad(Resume::PillageDrew).into(),
            };
        }
        if !pillage_goes_on::<V, _>(fight, &drew) {
            return Played::Done;
        }
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

/// The rest of an Ironclad card's play that waited on its own draw, with
/// every copy that draw drew.
pub(super) fn resume_drew<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    match at {
        Resume::PillageDrew => match pillage_goes_on::<V, _>(fight, drew) {
            true => pillage_draws::<V, _>(fight),
            false => Played::Done,
        },
        at => resume_waited::<V, _>(fight, at),
    }
}

/// `Whirlwind`: its damage to every enemy, once for each energy of its X.
fn whirlwind<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
