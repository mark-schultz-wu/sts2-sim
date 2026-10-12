//! The Regent's cards the generator leaves to a hand port, filed by rarity.

use sts2_core::{CardIdent, Fight, Pile, Played};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::V;

mod common;
mod uncommon;

/// Where a Regent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
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
    /// `Glow`, after a question inside its draw was answered.
    GlowDrew,
    /// `BigBang`, after a question inside its draw was answered.
    BigBangDrew,
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
}

/// `BeatIntoShape`: its damage to the target, then a Forge of its calculated
/// number, less its extra for each hit of its own attack.
fn beat_into_shape<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

    let target = fight.target();
    let results = fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    let count = |n: usize| super::Number::try_from(n).expect("an attack's hits fit a number");
    let amount = fight.var(CardVar::CalculatedForge)
        - count(results.len()) * fight.var(CardVar::CalculationExtra);
    fight.forge(Cards::Token(TokenCard::SovereignBlade), amount);
    Played::Done
}

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) -> Played<V> {
    match card {
        // `CrashLanding`: its damage to every enemy, as one attack, then
        // Debris into hand until the hand is full.
        RegentCard::CrashLanding => {
            use sts2_core::{Amount, Effect, Scaling};

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
            let room =
                sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
            if room > 0 {
                use super::super::super::generated::cards::StatusCard;

                let debris = CardIdent::new(Cards::Status(StatusCard::Debris));
                fight.add_generated_cards(debris, Pile::Hand, room);
            }
            Played::Done
        }
        // `Stardust`: its damage once for each star its X spent, each hit at
        // an enemy drawn at random.
        RegentCard::Stardust => {
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
                        damage: Amount::Var(CardVar::Damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        // `Resonance`: its Strength on its owner, then one Strength off each
        // enemy that can be hit.
        RegentCard::Resonance => {
            use super::super::super::generated::powers::Power;
            use sts2_core::PowerAmount;

            let strength = fight.var(CardVar::StrengthPower);
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(strength));
            for target in fight.hittable_opponents() {
                fight.apply_power(target, Power::StrengthPower, PowerAmount::Counted(-1));
            }
            Played::Done
        }
        // `DyingStar`: its damage to every enemy, as one attack, then its
        // `StrengthLoss` of Dying Star on each enemy that could be hit as the
        // play began.
        RegentCard::DyingStar => {
            use super::super::super::generated::powers::Power;
            use sts2_core::{Amount, Effect, PowerAmount, Scaling};

            let enemies = fight.hittable_opponents();
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
            let loss = PowerAmount::Counted(fight.var(CardVar::StrengthLoss));
            for target in enemies {
                fight.apply_power(target, Power::DyingStarPower, loss);
            }
            Played::Done
        }
        // `VoidForm`: its Void Form on its owner, then `PlayerCmd.EndTurn`.
        RegentCard::VoidForm => {
            use super::super::super::generated::powers::Power;
            use sts2_core::PowerAmount;

            let amount = fight.power_var(CardVar::VoidFormPower);
            fight.apply_to_actor(Power::VoidFormPower, PowerAmount::Counted(amount));
            fight.end_turn();
            Played::Done
        }
        RegentCard::Glitterstream => common::glitterstream(fight),
        // `BigBang`: its draw, then its stars, energy and Forge. A shuffle
        // inside the draw can ask, and the rest waits for it.
        RegentCard::BigBang => {
            fight.draw(fight.var(CardVar::Cards));
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Regent(Resume::BigBangDrew)),
                };
            }
            big_bang_drew(fight)
        }
        // `Glow`: its stars, its draw, then Draw Cards Next Turn at as many.
        RegentCard::Glow => {
            fight.gain_stars(fight.var(CardVar::Stars));
            fight.draw(fight.var(CardVar::Cards));
            // A shuffle inside the draw can ask, and the rest waits for it.
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Regent(Resume::GlowDrew)),
                };
            }
            glow_drew(fight)
        }
        // `DecisionsDecisions`: its `Cards` drawn, then a playable Skill of the
        // hand picked. A shuffle inside the draw can ask, and the rest waits.
        RegentCard::DecisionsDecisions => {
            fight.draw(fight.var(CardVar::Cards));
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Regent(Resume::DecisionsDrew)),
                };
            }
            decisions_ask(fight)
        }
        RegentCard::BeatIntoShape => beat_into_shape(fight),
        RegentCard::Quasar => quasar(fight),
        RegentCard::HeirloomHammer => heirloom_hammer(fight),
        // `Guards`: any number of the hand's cards picked, each transformed.
        RegentCard::Guards => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::UpTo(
                nonmax::NonMaxUsize::new(999_999_999).expect("a count below the maximum"),
            ),
            at: super::super::Resume::Card(super::Resume::Regent(Resume::GuardsTransform)),
        },
        RegentCard::Begone => common::begone(),
        // `Charge`: its `Cards` of the draw pile picked, each transformed.
        RegentCard::Charge => Played::Asking {
            pile: Pile::Draw,
            selection: sts2_core::Selection::Exactly(
                nonmax::NonMaxUsize::new(num_traits::cast(fight.var(CardVar::Cards)).unwrap_or(0))
                    .unwrap_or(nonmax::NonMaxUsize::ONE),
            ),
            at: super::super::Resume::Card(super::Resume::Regent(Resume::ChargeTransform)),
        },
        RegentCard::CollisionCourse => common::collision_course(fight),
        RegentCard::Radiate => uncommon::radiate(fight),
        RegentCard::LunarBlast => uncommon::lunar_blast(fight),
        RegentCard::HeavenlyDrill => heavenly_drill(fight),
        // `Monologue`: one Monologue on its owner, whose Strength is the
        // card's `Power`, which the power's own constant holds.
        RegentCard::Monologue => {
            use super::super::super::generated::powers::Power;

            fight.apply_to_actor(Power::MonologuePower, sts2_core::PowerAmount::Counted(1));
            Played::Done
        }
        RegentCard::KnockoutBlow => knockout_blow(fight),
        // `CosmicIndifference`: its block, then one card of the discard pile
        // onto the top of the draw pile.
        RegentCard::CosmicIndifference => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Asking {
                pile: sts2_core::Pile::Discard,
                selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Regent(
                    Resume::CosmicIndifferencePutBack,
                )),
            }
        }
        RegentCard::BundleOfJoy => bundle_of_joy(fight),
        // `ShiningStrike`: its damage to the target and its stars, then onto
        // the top of the draw pile unless it exhausts. `ExhaustOnNextPlay`
        // was cleared when its result pile was picked, before the play.
        RegentCard::ShiningStrike => {
            use sts2_core::{Amount, Effect, Pile, PilePosition, Scaling};

            use super::super::super::generated::cards::CardKeyword;

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
            fight.gain_stars(fight.var(CardVar::Stars));
            if !fight.this_has_keyword(CardKeyword::Exhaust) {
                fight.add_this(Pile::Draw, PilePosition::Top);
            }
            Played::Done
        }
        // `ParticleWall`: its block. Where it goes after is its own
        // `GetResultPileTypeForCardPlay`.
        RegentCard::ParticleWall => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        RegentCard::PhotonCut => photon_cut(fight),
        RegentCard::SummonForth => summon_forth(fight),
        // `Glimmer`: its `Cards` drawn, then its `PutBack` of the hand onto
        // the top of the draw pile.
        RegentCard::Glimmer => {
            fight.draw(fight.var(CardVar::Cards));
            let count = usize::try_from(fight.var(CardVar::PutBack))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: sts2_core::Pile::Hand,
                selection: sts2_core::Selection::Exactly(count),
                at: super::super::Resume::Card(super::Resume::Regent(Resume::GlimmerPutBack)),
            }
        }
        RegentCard::ManifestAuthority => manifest_authority(fight),
        card => card_plays::regent::on_play(card, fight),
    }
}

/// Its damage to the target, as one attack.
fn strike<F: Fight<V>>(fight: &mut F) {
    use sts2_core::{Amount, Effect, Scaling};

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
}

/// `KnockoutBlow`: its damage to the target, then its stars where the hit
/// killed it.
fn knockout_blow<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CreatureView as _;

    let target = fight.target();
    strike(fight);
    let standing = fight
        .creatures()
        .any(|creature| creature.id() == target && creature.alive());
    if !standing {
        fight.gain_stars(fight.var(CardVar::Stars));
    }
    Played::Done
}

/// `PhotonCut`: its damage to the target, its `Cards` drawn, then its
/// `PutBack` of the hand onto the top of the draw pile.
fn photon_cut<F: Fight<V>>(fight: &mut F) -> Played<V> {
    strike(fight);
    fight.draw(fight.var(CardVar::Cards));
    let count = usize::try_from(fight.var(CardVar::PutBack))
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .expect("a count of cards is not negative");
    Played::Asking {
        pile: sts2_core::Pile::Hand,
        selection: sts2_core::Selection::Exactly(count),
        at: super::super::Resume::Card(super::Resume::Regent(Resume::PhotonCutPutBack)),
    }
}

/// `SummonForth`: every Sovereign Blade of its owner's outside the hand into
/// the hand, then its `Forge`.
fn summon_forth<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};
    use sts2_core::Pile;

    let blade = Cards::Token(TokenCard::SovereignBlade);
    let blades: Vec<_> = [Pile::Draw, Pile::Discard, Pile::Exhaust]
        .into_iter()
        .flat_map(|pile| fight.cards_in(pile))
        .filter(|&card| fight.card_ident(card).card == blade)
        .collect();
    fight.act_on(sts2_core::PileAct::IntoHand, &blades);
    fight.forge(blade, fight.var(CardVar::Forge));
    Played::Done
}

/// `MakeItSo.AfterCardPlayedLate`: after a Skill of its owner's, into hand
/// from outside it. That is once the Skills finished this turn come to a
/// whole multiple of its `Cards`. A fight here has one player.
fn make_it_so<P: sts2_core::Printed<V> + ?Sized>(
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use sts2_core::{CardKind, Rules as _};

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
    let skills = super::Number::try_from(skills).expect("a turn's plays fit a game number");
    if skills % printed.face(CardVar::Cards) == 0 {
        asked.push(sts2_core::HookEffect::AddSelfToHand { only_from: None });
    }
}

/// `BundleOfJoy`: `GetDistinctForCombat` of its `Cards` of colorless cards
/// off `CombatCardGeneration`, into hand.
fn bundle_of_joy<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Rules as _, RunStreamKinds as _};

    let mut cards = V::combat_colorless_cards().to_vec();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
    let made: Vec<_> = match fight.var(CardVar::Cards) {
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
fn heavenly_drill<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

    let mut hits = fight.x_value();
    if hits >= fight.var(CardVar::Energy) {
        hits *= 2;
    }
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
    });
    Played::Done
}

/// `ManifestAuthority`: its block, then one colorless card off
/// `CombatCardGeneration` into hand, upgraded where it is.
fn manifest_authority<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Rules as _, RunStreamKinds as _, Scaling};

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
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
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: RegentCard,
    id: sts2_core::CardId<V>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match card {
        RegentCard::KinglyKick => uncommon::kingly_kick(id, hook, asked),
        RegentCard::KinglyPunch => uncommon::kingly_punch(id, printed, hook, asked),
        RegentCard::MakeItSo => make_it_so(printed, history, hook, asked),
        card => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// `HeirloomHammer`: its damage to the target, then one card of the hand
/// that shows as colorless picked, to be copied.
fn heirloom_hammer<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, CardTable as _, Effect, Scaling};

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
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| fight.card_ident(card).card.visually_colorless())
        .collect();
    Played::AskingAmong {
        cards,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Regent(Resume::HeirloomHammerCopy)),
    }
}

/// `Quasar`: three distinct colorless cards off `CombatCardGeneration`, each
/// upgraded where Quasar is, of which its owner takes one into hand or none.
fn quasar<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

    let mut pool = <V as sts2_core::Rules>::combat_colorless_cards().to_vec();
    let stream = <<V as sts2_core::ContentVersion>::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION;
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
        at: super::super::Resume::Card(super::Resume::Regent(Resume::QuasarChoice)),
    }
}

/// The rest of a play that stopped on a choice, once the player chose.
pub(super) fn resume_chosen<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
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
pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::BegoneTransform => common::begone_transform(fight, chosen),
        // `CardCmd.Transform` of each into a `MinionDiveBomb` for Charge,
        // or a `MinionSacrifice` for Guards, upgraded where the card is.
        Resume::ChargeTransform | Resume::GuardsTransform => {
            let token = match at {
                Resume::ChargeTransform => TokenCard::MinionDiveBomb,
                _ => TokenCard::MinionSacrifice,
            };
            for &card in chosen.as_slice() {
                let mut into = CardIdent::new(Cards::Token(token));
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
        Resume::GlowDrew
        | Resume::BigBangDrew
        | Resume::DecisionsDrew
        | Resume::DecisionsPlayed { .. } => {
            unreachable!("{at:?} waits on a question, not a pick")
        }
        Resume::DecisionsPicked => match chosen.as_slice() {
            [] => Played::Done,
            &[card] => decisions_play(fight, card, fight.var(CardVar::Repeat)),
            picked => unreachable!("Decisions, Decisions picked {} cards", picked.len()),
        },
        // Its `Repeat` of clones of the one picked, each into hand.
        Resume::HeirloomHammerCopy => {
            if let Some(card) = chosen.one_or_none() {
                for _ in 0..fight.var(CardVar::Repeat) {
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
fn decisions_ask<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{CardKind, Rules as _};

    use super::super::super::generated::cards::CardKeyword;

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
        at: super::super::Resume::Card(super::Resume::Regent(Resume::DecisionsPicked)),
    }
}

/// `CardCmd.AutoPlay` of the Skill picked, its `Repeat` times, wherever the
/// card stands by each play.
///
/// # Panics
///
/// A play that stops on a question with more to come, and a card no pile
/// holds by its turn.
fn decisions_play<F: Fight<V>>(
    fight: &mut F,
    card: sts2_core::CardAt<V>,
    plays: super::Number,
) -> Played<V> {
    for played in 1..=plays {
        let at = super::standing(fight, card)
            .unwrap_or_else(|| unimplemented!("Decisions, Decisions playing a card no pile holds"));
        fight.auto_play_card(at);
        // `CardCmd.AutoPlay` is awaited, so the plays left wait on the answer.
        if fight.waiting() {
            let left = u8::try_from(plays - played).expect("a few plays are left");
            return Played::Waiting {
                at: super::super::Resume::Card(super::Resume::Regent(Resume::DecisionsPlayed {
                    card,
                    left,
                })),
            };
        }
    }
    Played::Done
}

fn glow_drew<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::PowerAmount;

    use super::super::super::generated::powers::Power;

    fight.apply_to_actor(
        Power::DrawCardsNextTurnPower,
        PowerAmount::Counted(fight.var(CardVar::Cards)),
    );
    Played::Done
}

/// `BigBang` after its draw: its stars, its energy, then a Forge of its
/// `Forge`.
fn big_bang_drew<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};

    fight.gain_stars(fight.var(CardVar::Stars));
    fight.modify_energy(fight.var(CardVar::Energy));
    fight.forge(
        Cards::Token(TokenCard::SovereignBlade),
        fight.var(CardVar::Forge),
    );
    Played::Done
}

/// The rest of a Regent card's play that stood still on a question its own
/// command raised.
pub(super) fn resume_waited<F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::GlowDrew => glow_drew(fight),
        Resume::BigBangDrew => big_bang_drew(fight),
        Resume::DecisionsDrew => decisions_ask(fight),
        Resume::DecisionsPlayed { card, left } => {
            decisions_play(fight, card, super::Number::from(left))
        }
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}
