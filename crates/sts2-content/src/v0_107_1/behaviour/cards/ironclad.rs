//! The Ironclad's cards the generator leaves to a hand port.

use sts2_core::{
    Amount, CardKind, Change, CostPool, CreatureView as _, Effect, EventHook, Fight, HookEffect,
    Lasting, Pile, PileAct, Played, PowerAmount, Rules as _, Scaling,
};

use super::super::super::generated::cards::Cards;
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
    /// `Pillage`, after a question inside one of its draws was answered.
    PillageDrew,
    /// `FiendFire`, after a question inside its exhausts was answered, with
    /// the cards left to exhaust and this many hits to land.
    FiendFireExhausting {
        left: super::Left,
        hits: u8,
    },
    /// After a question inside the card's loss of health was answered.
    BloodlettingLost,
    OfferingLost,
    BrandLost,
    HemokinesisLost,
    BreakthroughLost,
    BloodWallLost,
    /// After a question inside the card's attack was answered.
    RampageHit,
    AngerHit,
    HeadbuttHit,
    ThrashHit,
    UppercutHit,
    MoltenFistHit,
    PillageHit,
    CinderHit,
    /// `Feed`, whose owner gains its `MaxHp` where `gains` says.
    FeedHit {
        gains: bool,
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
        left: super::Left,
        blocked: bool,
    },
    /// `Stoke`, with the cards left to exhaust and how many the hand held.
    StokeExhausting {
        left: super::Left,
        count: u8,
    },
}

/// The rest of the play waits at `at`, where a command left a question.
fn wait(at: Resume) -> Played<V> {
    Played::Waiting {
        at: super::super::Resume::Card(super::Resume::Ironclad(at)),
    }
}

/// The rest of an Ironclad card's play that stood still on a question its
/// own command raised.
pub(super) fn resume_waited<F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::FiendFireExhausting { left, hits } => fiend_fire_exhausting(fight, left, hits),
        // A draw that never ran, as the fight ended, drew nothing.
        Resume::PillageDrew => Played::Done,
        Resume::BloodlettingLost => bloodletting_lost(fight),
        Resume::OfferingLost => offering_lost(fight),
        Resume::BrandLost => brand_lost(),
        Resume::HemokinesisLost => {
            let target = fight.target();
            strike(fight, target);
            Played::Done
        }
        Resume::BreakthroughLost => breakthrough_lost(fight),
        Resume::BloodWallLost => {
            block(fight);
            Played::Done
        }
        Resume::RampageHit => rampage_hit(fight),
        Resume::AngerHit => {
            fight.clone_this(Pile::Discard);
            Played::Done
        }
        Resume::HeadbuttHit => headbutt_hit(),
        Resume::ThrashHit => thrash_hit(fight),
        Resume::UppercutHit => uppercut_hit(fight),
        Resume::UppercutWeak => uppercut_weak(fight),
        Resume::MoltenFistHit => molten_fist_hit(fight),
        Resume::PillageHit => pillage_draws(fight),
        Resume::CinderHit => cinder_hit(fight),
        Resume::FeedHit { gains } => feed_hit(fight, gains),
        Resume::DominateApplied => dominate_applied(fight),
        Resume::CrimsonMantleApplied => {
            fight.note_on_power(fight.actor(), Power::CrimsonMantlePower, 1);
            Played::Done
        }
        Resume::InfernoApplied => {
            fight.note_on_power(fight.actor(), Power::InfernoPower, 1);
            Played::Done
        }
        Resume::EvilEyeBlocked => {
            block(fight);
            Played::Done
        }
        Resume::TrueGritBlocked => true_grit_blocked(fight),
        Resume::ArmamentsBlocked => armaments_blocked(fight),
        Resume::SecondWind { left, blocked } => second_wind(fight, left, blocked),
        Resume::StokeExhausting { left, count } => stoke_exhausting(fight, left, count),
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
        at => unreachable!("{at:?} waits on a question, not a pick"),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: IroncladCard, fight: &mut F) -> Played<V> {
    match card {
        IroncladCard::Thrash => thrash(fight),
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
            if fight.waiting() {
                return wait(Resume::CrimsonMantleApplied);
            }
            fight.note_on_power(fight.actor(), Power::CrimsonMantlePower, 1);
            Played::Done
        }
        IroncladCard::Inferno => {
            let inferno = PowerAmount::Counted(fight.var(CardVar::InfernoPower));
            fight.apply_to_actor(Power::InfernoPower, inferno);
            if fight.waiting() {
                return wait(Resume::InfernoApplied);
            }
            fight.note_on_power(fight.actor(), Power::InfernoPower, 1);
            Played::Done
        }
        // `Dominate`: its Vulnerable on the target, then as much Strength on
        // its owner as the target then carries Vulnerable.
        IroncladCard::Dominate => {
            let target = fight.target();
            let vulnerable = PowerAmount::Counted(fight.var(CardVar::VulnerablePower));
            fight.apply_power(target, Power::VulnerablePower, vulnerable);
            if fight.waiting() {
                return wait(Resume::DominateApplied);
            }
            dominate_applied(fight)
        }
        // `Feed`: its damage to the target, then its `MaxHp` onto its owner's
        // maximum where a result says `WasTargetKilled` of a target whose
        // death, read before the hit, triggers Fatal.
        IroncladCard::Feed => {
            let target = fight.target();
            let fatal = fight.death_triggers_fatal(target);
            let results = fight.attack(Scaling::Dynamic, |fight| {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            });
            let gains = fatal && results.iter().any(|result| result.killed);
            if fight.waiting() {
                return wait(Resume::FeedHit { gains });
            }
            feed_hit(fight, gains)
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
            second_wind(fight, super::Left::of(&cards), true)
        }
        // `PactsEnd`: its damage to every enemy, once the exhaust pile holds
        // its `Cards`.
        IroncladCard::PactsEnd => {
            let exhausted = fight.cards_in(Pile::Exhaust).len();
            if num_traits::cast::<_, super::Number>(exhausted)
                .is_some_and(|exhausted| exhausted >= fight.var(CardVar::Cards))
            {
                fight.attack(Scaling::Dynamic, |fight| {
                    let targets = fight.opponents();
                    // One `CreatureCmd.Damage` over them all.
                    fight.damage_command(|fight| {
                        for target in targets {
                            let hit = fight.resolve(Effect::Attack {
                                dealer: fight.actor(),
                                target,
                                damage: Amount::Var(CardVar::Damage),
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
        IroncladCard::FiendFire => {
            let cards = fight.cards_in(Pile::Hand);
            let hits = u8::try_from(cards.len()).expect("a hand's worth of hits fits a u8");
            fiend_fire_exhausting(fight, super::Left::of(&cards), hits)
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
            if fight.waiting() {
                return wait(Resume::RampageHit);
            }
            rampage_hit(fight)
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
            if fight.waiting() {
                return wait(Resume::AngerHit);
            }
            fight.clone_this(Pile::Discard);
            Played::Done
        }
        // `Headbutt`: its damage to the target, then one card of the discard
        // pile onto the top of the draw pile.
        IroncladCard::Headbutt => {
            let target = fight.target();
            strike(fight, target);
            if fight.waiting() {
                return wait(Resume::HeadbuttHit);
            }
            headbutt_hit()
        }
        IroncladCard::Whirlwind => whirlwind(fight),
        IroncladCard::Bloodletting => bloodletting(fight),
        // `Offering`: its `HpLoss` off its owner, unblockable, then its
        // energy and its cards drawn.
        IroncladCard::Offering => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            if fight.waiting() {
                return wait(Resume::OfferingLost);
            }
            offering_lost(fight)
        }
        IroncladCard::Uppercut => uppercut(fight),
        IroncladCard::MoltenFist => molten_fist(fight),
        // `Cinder`: its damage to the target, then a card from hand drawn off
        // `CombatCardSelection`, exhausted.
        IroncladCard::Cinder => {
            let target = fight.target();
            strike(fight, target);
            if fight.waiting() {
                return wait(Resume::CinderHit);
            }
            cinder_hit(fight)
        }
        IroncladCard::Pillage => pillage(fight),
        IroncladCard::TrueGrit => true_grit(fight),
        IroncladCard::Dismantle => dismantle(fight),
        // `Brand`: its `HpLoss` off its owner, unblockable, then one card of
        // the hand exhausted, then its Strength.
        IroncladCard::Brand => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            if fight.waiting() {
                return wait(Resume::BrandLost);
            }
            brand_lost()
        }
        IroncladCard::PrimalForce => primal_force(fight),
        // `Hemokinesis`: its `HpLoss` off its owner, unblockable, then its
        // damage to the target.
        IroncladCard::Hemokinesis => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            if fight.waiting() {
                return wait(Resume::HemokinesisLost);
            }
            let target = fight.target();
            strike(fight, target);
            Played::Done
        }
        // `Breakthrough`: its `HpLoss` off its owner, unblockable, then its
        // damage to every enemy.
        IroncladCard::Breakthrough => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            if fight.waiting() {
                return wait(Resume::BreakthroughLost);
            }
            breakthrough_lost(fight)
        }
        // `BloodWall`: its `HpLoss` off its owner, unblockable, then its
        // block.
        IroncladCard::BloodWall => {
            fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
            if fight.waiting() {
                return wait(Resume::BloodWallLost);
            }
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
            block(fight);
            if gains == 2 {
                if fight.waiting() {
                    return wait(Resume::EvilEyeBlocked);
                }
                block(fight);
            }
            Played::Done
        }
        // `Stoke`: the whole hand exhausted, one card at a time. Then as many
        // cards of the character's pool, each `NextItem` off
        // `CombatCardGeneration`, upgraded once Stoke is, into hand.
        IroncladCard::Stoke => {
            let hand = fight.cards_in(Pile::Hand);
            let count = u8::try_from(hand.len()).expect("a hand's worth fits a u8");
            stoke_exhausting(fight, super::Left::of(&hand), count)
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
        card => super::generated_from(Cards::Ironclad(card), fight, 0),
    }
}

/// `Dismantle`: its damage to the target, twice where the target carries
/// Vulnerable.
fn dismantle<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let vulnerable = fight.creatures().any(|creature| {
        creature.id() == target && creature.power(Power::VulnerablePower).is_some()
    });
    // One `DamageCmd.Attack` with its hit count, so what hears the attack's
    // end hears it once.
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..1 + u8::from(vulnerable) {
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
    if fight.waiting() {
        return wait(Resume::TrueGritBlocked);
    }
    true_grit_blocked(fight)
}

/// The rest of `TrueGrit`, once its block is in.
fn true_grit_blocked<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    block(fight);
    if fight.waiting() {
        return wait(Resume::ArmamentsBlocked);
    }
    armaments_blocked(fight)
}

/// The rest of `Armaments`, once its block is in.
fn armaments_blocked<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
        at: super::super::Resume::Card(super::Resume::Ironclad(Resume::ArmamentsUpgrade)),
    }
}

/// `Bloodletting`: its `HpLoss` off its owner, unblockable, then its energy.
fn bloodletting<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.damage_unblockable(fight.actor(), fight.var(CardVar::HpLoss));
    if fight.waiting() {
        return wait(Resume::BloodlettingLost);
    }
    bloodletting_lost(fight)
}

/// The rest of `Bloodletting`, once its loss is over: its energy.
fn bloodletting_lost<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.modify_energy(fight.var(CardVar::Energy));
    Played::Done
}

/// The rest of `Offering`, once its loss is over: its energy, then its
/// cards drawn.
fn offering_lost<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.modify_energy(fight.var(CardVar::Energy));
    fight.draw(fight.var(CardVar::Cards));
    Played::Done
}

/// The rest of `Brand`, once its loss is over: one card of the hand
/// exhausted, then its Strength.
fn brand_lost() -> Played<V> {
    Played::Asking {
        pile: Pile::Hand,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Ironclad(Resume::BrandExhaust)),
    }
}

/// The rest of `Breakthrough`, once its loss is over: its damage to every
/// enemy.
fn breakthrough_lost<F: Fight<V>>(fight: &mut F) -> Played<V> {
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

/// The rest of `Rampage`, once its attack is over: its `Increase` onto its
/// own damage for the fight.
fn rampage_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let increase = fight.var(CardVar::Increase);
    fight.modify_this(
        Change::Own(super::CardWrite::BuffFromPlay(increase)),
        Lasting::ThisFight,
    );
    Played::Done
}

/// The rest of `Headbutt`, once its attack is over: one card of the discard
/// pile onto the top of the draw pile.
fn headbutt_hit() -> Played<V> {
    Played::Asking {
        pile: Pile::Discard,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Ironclad(Resume::HeadbuttOntoDraw)),
    }
}

/// The rest of `Cinder`, once its attack is over: a card from hand drawn
/// off `CombatCardSelection`, exhausted.
fn cinder_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    if let Some(card) = fight.random_card_in(Pile::Hand) {
        fight.act_on(PileAct::Exhaust, &[card]);
    }
    Played::Done
}

/// The rest of `Feed`, once its attack is over: its `MaxHp` where `gains`.
fn feed_hit<F: Fight<V>>(fight: &mut F, gains: bool) -> Played<V> {
    if gains {
        fight.gain_max_hp(fight.actor(), fight.var(CardVar::MaxHp));
    }
    Played::Done
}

/// The rest of `Dominate`, once its Vulnerable is on: as much Strength on
/// its owner as the target then carries Vulnerable.
fn dominate_applied<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
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

/// `SecondWind` over the cards `left`: each exhausted, then its block.
/// `blocked` says the block for the card before them is in.
fn second_wind<F: Fight<V>>(fight: &mut F, left: super::Left, blocked: bool) -> Played<V> {
    if !blocked {
        block(fight);
    }
    let cards: Vec<_> = left.cards().collect();
    for (at, &card) in cards.iter().enumerate() {
        if fight.waiting() {
            let left = super::Left::of(&cards[at..]);
            return wait(Resume::SecondWind {
                left,
                blocked: true,
            });
        }
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = super::Left::of(&cards[at + 1..]);
            return wait(Resume::SecondWind {
                left,
                blocked: false,
            });
        }
        block(fight);
    }
    Played::Done
}

/// `FiendFire` over the cards `left`: each exhausted, then `hits` hits.
fn fiend_fire_exhausting<F: Fight<V>>(fight: &mut F, left: super::Left, hits: u8) -> Played<V> {
    let cards: Vec<_> = left.cards().collect();
    for (at, &card) in cards.iter().enumerate() {
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = super::Left::of(&cards[at + 1..]);
            return wait(Resume::FiendFireExhausting { left, hits });
        }
    }
    fiend_fire_hits(fight, usize::from(hits))
}

/// `Stoke` over the cards `left`: each exhausted, then `count` cards of the
/// character's pool, each `NextItem` off `CombatCardGeneration`, upgraded
/// once Stoke is, into hand.
fn stoke_exhausting<F: Fight<V>>(fight: &mut F, left: super::Left, count: u8) -> Played<V> {
    let cards: Vec<_> = left.cards().collect();
    for (at, &card) in cards.iter().enumerate() {
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = super::Left::of(&cards[at + 1..]);
            return wait(Resume::StokeExhausting { left, count });
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
    if fight.waiting() {
        return wait(Resume::ThrashHit);
    }
    thrash_hit(fight)
}

/// The rest of `Thrash`, once its attack is over.
fn thrash_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
        Change::Own(super::CardWrite::ExtraDamage(damage)),
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
    if fight.waiting() {
        return wait(Resume::UppercutHit);
    }
    uppercut_hit(fight)
}

/// The rest of `Uppercut`, once its attack is over: its Weak, then its
/// Vulnerable.
fn uppercut_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = PowerAmount::Counted(fight.var(CardVar::Power));
    fight.apply_power(fight.target(), Power::WeakPower, amount);
    if fight.waiting() {
        return wait(Resume::UppercutWeak);
    }
    uppercut_weak(fight)
}

/// The rest of `Uppercut`, once its Weak is on: its Vulnerable.
fn uppercut_weak<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let amount = PowerAmount::Counted(fight.var(CardVar::Power));
    fight.apply_power(fight.target(), Power::VulnerablePower, amount);
    Played::Done
}

/// `MoltenFist`: its damage to the target, then as much Vulnerable again as
/// the target holds, if it still stands.
fn molten_fist<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    strike(fight, target);
    if fight.waiting() {
        return wait(Resume::MoltenFistHit);
    }
    molten_fist_hit(fight)
}

/// The rest of `MoltenFist`, once its attack is over.
fn molten_fist_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
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
    if fight.waiting() {
        return wait(Resume::PillageHit);
    }
    pillage_draws(fight)
}

/// `Pillage`'s draws, one card at a time. A shuffle inside one can ask, and
/// the rest waits for it.
fn pillage_draws<F: Fight<V>>(fight: &mut F) -> Played<V> {
    loop {
        let drew = fight.draw(1);
        if fight.waiting() {
            return Played::Waiting {
                at: super::super::Resume::Card(super::Resume::Ironclad(Resume::PillageDrew)),
            };
        }
        if !pillage_goes_on(fight, &drew) {
            return Played::Done;
        }
    }
}

/// Whether `Pillage` draws again after `drew`: the card it drew is an Attack,
/// wherever it stands now, and the hand has room.
fn pillage_goes_on<F: Fight<V>>(fight: &F, drew: &[sts2_core::CardId<V>]) -> bool {
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
pub(super) fn resume_drew<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    match at {
        Resume::PillageDrew => match pillage_goes_on(fight, drew) {
            true => pillage_draws(fight),
            false => Played::Done,
        },
        at => resume_waited(fight, at),
    }
}

/// `Whirlwind`: its damage to every enemy, once for each energy of its X.
fn whirlwind<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
