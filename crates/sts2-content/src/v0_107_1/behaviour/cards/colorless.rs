//! The colorless cards the generator leaves to a hand port.

use sts2_core::{
    CardKind, EventHook, Fight, HookEffect, Pile, PileAct, Played, Rules as _, Selection,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardKeyword, CardRarity, CardVar, ColorlessCard};
use super::{Number, V};

/// `Catastrophe`: its `Cards` times, a card of the draw pile auto-played.
/// Each is the first of the playable ones `StableShuffle`d off the shuffle
/// stream, or of the whole pile where none is playable.
///
/// A card it plays that asks stands the loop still. `left` is how many it
/// still has to play once that card's play is over.
fn catastrophe<F: Fight<V>>(fight: &mut F, left: Number) -> Played<V> {
    use sts2_core::RunStreamKinds as _;

    let stream = <V as sts2_core::ContentVersion>::RunRngType::SHUFFLE;
    for played in 1..=left {
        let mut playable: Vec<_> = fight
            .cards_in(Pile::Draw)
            .into_iter()
            .filter(|&at| !fight.card_keyword(at, CardKeyword::Unplayable))
            .collect();
        fight.stable_shuffle_cards(&mut playable, stream);
        let chosen = match playable.first() {
            Some(&card) => Some(card),
            None => {
                let mut every = fight.cards_in(Pile::Draw);
                fight.stable_shuffle_cards(&mut every, stream);
                every.first().copied()
            }
        };
        if let Some(card) = chosen {
            fight.auto_play_card(card);
        }
        if fight.waiting() {
            let left = u8::try_from(left - played).expect("a few cards left to play");
            return Played::Waiting {
                at: super::super::Resume::Card(super::Resume::Colorless(Resume::CatastropheLeft(
                    left,
                ))),
            };
        }
    }
    Played::Done
}

pub(super) fn on_play<F: Fight<V>>(card: ColorlessCard, fight: &mut F) -> Played<V> {
    match card {
        ColorlessCard::Volley => volley(fight),
        ColorlessCard::Catastrophe => catastrophe(fight, fight.var(CardVar::Cards)),
        // `Fisticuffs`: its damage to the target, then as much block as the
        // hit dealt, overkill included.
        ColorlessCard::Fisticuffs => {
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
            let dealt = results
                .iter()
                .map(|result| result.total + result.overkill)
                .sum();
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(dealt),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        // `SecretTechnique`: one skill of the draw pile picked, into hand.
        // `SeekerStrike`: its damage to the target, then one of three cards of
        // the draw pile, `StableShuffle`d off `CombatCardSelection`, into hand.
        ColorlessCard::SeekerStrike => {
            use sts2_core::{Amount, Effect, RunStreamKinds as _, Scaling};

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
            let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_SELECTION;
            let mut shuffled = fight.cards_in(Pile::Draw);
            fight.stable_shuffle_cards(&mut shuffled, stream);
            let options = usize::try_from(fight.var(CardVar::Cards)).expect("a few cards");
            shuffled.truncate(options);
            let cards = fight
                .cards_in(Pile::Draw)
                .into_iter()
                .filter(|card| shuffled.contains(card))
                .collect();
            Played::AskingAmong {
                cards,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Colorless(Resume::SeekerStrikeTakes)),
            }
        }
        ColorlessCard::SecretTechnique => {
            let skills = fight
                .cards_in(Pile::Draw)
                .into_iter()
                .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Skill))
                .collect();
            Played::AskingAmong {
                cards: skills,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Colorless(
                    Resume::SecretTechniqueTakes,
                )),
            }
        }
        // `Shockwave`: its `Power` of Weak, then of Vulnerable, on each enemy
        // that can be hit, one enemy at a time.
        ColorlessCard::Shockwave => {
            use super::super::super::generated::powers::Power;

            let amount = sts2_core::PowerAmount::Counted(fight.var(CardVar::Power));
            for target in fight.hittable_opponents() {
                fight.apply_power(target, Power::WeakPower, amount);
                fight.apply_power(target, Power::VulnerablePower, amount);
            }
            Played::Done
        }
        // `Alchemize`: a potion made off `CombatPotionGeneration` among those
        // a fight can make, procured.
        ColorlessCard::Alchemize => {
            use sts2_core::RunStreamKinds as _;

            fight.procure_random_potion(
                <V as sts2_core::ContentVersion>::RunRngType::COMBAT_POTION_GENERATION,
            );
            Played::Done
        }
        // `Prolong`: as much Block Next Turn on its owner as the block it
        // holds.
        ColorlessCard::Prolong => {
            use super::super::super::generated::powers::Power;
            use sts2_core::CreatureView as _;

            let owner = fight.actor();
            let block = fight
                .creatures()
                .find(|creature| creature.id() == owner)
                .map_or(0, |creature| creature.block());
            fight.apply_to_actor(
                Power::BlockNextTurnPower,
                sts2_core::PowerAmount::Counted(block),
            );
            Played::Done
        }
        ColorlessCard::Restlessness => restlessness(fight),
        ColorlessCard::Impatience => impatience(fight),
        ColorlessCard::Anointed => anointed(fight),
        // `FromHand` for up to its `Cards`, each then exhausted.
        ColorlessCard::Purity => match nonmax::NonMaxUsize::new(cards(fight)) {
            Some(count) => Played::Asking {
                pile: Pile::Hand,
                selection: Selection::UpTo(count),
                at: super::super::Resume::Card(super::Resume::Colorless(Resume::PurityExhausts)),
            },
            None => Played::Done,
        },
        // `ThinkingAhead`: its `Cards` drawn, then one card of the hand onto
        // the top of the draw pile.
        ColorlessCard::ThinkingAhead => {
            fight.draw(fight.var(CardVar::Cards));
            Played::Asking {
                pile: Pile::Hand,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Colorless(
                    Resume::ThinkingAheadPutBack,
                )),
            }
        }
        ColorlessCard::JackOfAllTrades => jack_of_all_trades(fight),
        ColorlessCard::Omnislice => omnislice(fight),
        card => card_plays::colorless::on_play(card, fight),
    }
}

/// `Omnislice`: its damage to the target, then what that hit dealt, block
/// and overkill included, to each other enemy that can be hit, unpowered.
/// Both are hits of one attack.
fn omnislice<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
        let Some(first) = fight.attack_results().first().copied() else {
            return;
        };
        let others: Vec<_> = fight
            .hittable_opponents()
            .into_iter()
            .filter(|&other| other != target)
            .collect();
        for other in others {
            let hit = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target: other,
                damage: Amount::Number(first.total + first.overkill),
                scaling: Scaling::Static,
            });
            fight.apply(hit);
        }
    });
    Played::Done
}

/// `JackOfAllTrades`: `GetDistinctForCombat` of its `Cards` of colorless
/// cards other than itself, off `CombatCardGeneration`, into hand.
fn jack_of_all_trades<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::RunStreamKinds as _;

    use super::super::super::generated::cards::Cards;

    let mut pool: Vec<_> = V::combat_colorless_cards()
        .iter()
        .copied()
        .filter(|&card| card != Cards::Colorless(ColorlessCard::JackOfAllTrades))
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
    let made: Vec<_> = match cards(fight) {
        1 => fight.generate_cards::<1>(&mut pool, stream).map(Vec::from),
        2 => fight.generate_cards::<2>(&mut pool, stream).map(Vec::from),
        count => unimplemented!("Jack of All Trades making {count} cards"),
    }
    .unwrap_or_else(|_| unimplemented!("a colorless pool short of Jack of All Trades' cards"));
    for card in made {
        fight.add_generated_cards(card, Pile::Hand, 1);
    }
    Played::Done
}

/// Where a colorless card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Purity`, after the cards picked from hand to exhaust.
    PurityExhausts,
    /// `ThinkingAhead`, after the card picked from hand to put back.
    ThinkingAheadPutBack,
    /// `SecretTechnique`, after the skill picked from the draw pile.
    SecretTechniqueTakes,
    /// `SeekerStrike`, after the card picked from the three it offered.
    SeekerStrikeTakes,
    /// `Catastrophe`, once a card it played has its answer, with this many
    /// cards still to play.
    CatastropheLeft(u8),
}

/// The rest of a play that waited on a card it auto-played.
pub(super) fn resume_waited<F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::CatastropheLeft(left) => catastrophe(fight, Number::from(left)),
        at => unreachable!("{at:?} stops on a pick, not on a card it played"),
    }
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::PurityExhausts => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            Played::Done
        }
        Resume::CatastropheLeft(_) => unreachable!("Catastrophe waits, and picks nothing"),
        // `CardPileCmd.Add` of the one taken, into hand.
        Resume::SecretTechniqueTakes | Resume::SeekerStrikeTakes => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        // `CardPileCmd.Add` onto the top of the draw pile.
        Resume::ThinkingAheadPutBack => {
            fight.act_on(PileAct::OntoDrawTop, chosen.as_slice());
            Played::Done
        }
    }
}

/// The card's `Cards`, as a count.
fn cards<F: Fight<V>>(fight: &F) -> usize {
    num_traits::cast(fight.var(CardVar::Cards)).expect("a count of cards is not negative")
}

/// `Impatience`: its `Cards` drawn, with no attack in hand.
fn impatience<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let attack = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .any(|at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack));
    if !attack {
        fight.draw(fight.var(CardVar::Cards));
    }
    Played::Done
}

/// `Anointed`: the rare cards in the draw pile, `TakeRandom` off
/// `CombatCardSelection` for as many as the hand has room for, into hand.
fn anointed<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    let mut rares: Vec<_> = fight
        .cards_in(Pile::Draw)
        .into_iter()
        .filter(|&at| fight.card_ident(at).card.rarity() == CardRarity::Rare)
        .collect();
    let taken = fight.take_random_cards(&mut rares, room);
    fight.act_on(PileAct::IntoHand, &taken);
    Played::Done
}

/// `Restlessness`: with no other card in hand, its `Cards` drawn one at a
/// time, then its `Energy`. It stands in the play pile as it runs.
fn restlessness<F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.cards_in(Pile::Hand).is_empty() {
        for _ in 0..fight.var(CardVar::Cards) {
            fight.draw(1);
        }
        fight.modify_energy(fight.var(CardVar::Energy));
    }
    Played::Done
}

/// Deliver a notification to one colorless card in the fight.
pub(super) fn event_hook(
    card: ColorlessCard,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Bolas.BeforeHandDraw` and `ThrummingHatchet.BeforeHandDraw`: back
        // into hand, from wherever it is, once this copy finished a play in
        // its owner's last turn. A fight here has one player.
        (
            ColorlessCard::Bolas | ColorlessCard::ThrummingHatchet,
            EventHook::BeforeHandDraw { turn, .. },
        ) => {
            if history
                .finished_last_player_turn(turn)
                .any(|finished| finished.id == id)
            {
                asked.push(HookEffect::ToHand);
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// `Volley`: its damage once for each energy its X spent, each hit at an
/// enemy drawn at random.
fn volley<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

    let hits = fight.x_value();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let Some(target) = fight.random_target() else {
                break;
            };
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
