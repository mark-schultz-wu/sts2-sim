//! The colorless cards the generator leaves to a hand port.

use sts2_core::{
    CardKind, EventHook, Fight, HookEffect, Pile, PileAct, Played, Rules as _, Selection,
};

use super::super::super::generated::cards::Cards;
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
        // `HandOfGreed`: its damage to the target, then its `Gold` where a
        // result says `WasTargetKilled` of a target whose death, read before
        // the hit, triggers Fatal.
        ColorlessCard::HandOfGreed => {
            use sts2_core::{Amount, Effect, Scaling};

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
            if fight.waiting() {
                return wait(Resume::HandOfGreedHit { fatal });
            }
            hand_of_greed_hit(fight, fatal, &results)
        }
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
            if fight.waiting() {
                return wait(Resume::FisticuffsHit);
            }
            fisticuffs_hit(fight, &results)
        }
        // `SecretTechnique`: one skill of the draw pile picked, into hand.
        // `SeekerStrike`: its damage to the target, then one of three cards of
        // the draw pile, `StableShuffle`d off `CombatCardSelection`, into hand.
        ColorlessCard::SeekerStrike => {
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
            if fight.waiting() {
                return wait(Resume::SeekerStrikeHit);
            }
            seeker_strike_hit(fight)
        }
        // `Scrawl`: cards drawn until the hand is full.
        ColorlessCard::Scrawl => {
            let room =
                sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
            fight.draw(Number::try_from(room).expect("a hand's room fits a game number"));
            Played::Done
        }
        // `Jackpot`: its damage to the target, then its `Cards` of the
        // character's pool that cost nothing, each `NextItem` off
        // `CombatCardGeneration` and upgraded where Jackpot is, into hand.
        ColorlessCard::Jackpot => {
            use sts2_core::{Amount, Effect, Scaling};

            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(hit);
            });
            if fight.waiting() {
                return wait(Resume::JackpotHit);
            }
            jackpot_hit(fight)
        }
        // `TheBomb`: its `Turns` of The Bomb on its owner, then its
        // `BombDamage` written on the instance.
        ColorlessCard::TheBomb => {
            use super::super::super::generated::powers::Power;

            let actor = fight.actor();
            let turns = fight.power_var(CardVar::Turns);
            let damage = fight.var(CardVar::BombDamage);
            fight.apply_to_actor(Power::TheBombPower, sts2_core::PowerAmount::Counted(turns));
            fight.write_power(actor, Power::TheBombPower, |state| {
                state.set_bomb_damage(damage)
            });
            Played::Done
        }
        // `Discovery`: three distinct cards of the character's pool, off
        // `CombatCardGeneration`, of which its owner takes one into hand,
        // free this turn, or none.
        ColorlessCard::Discovery => {
            let mut pool = <V as sts2_core::Rules>::combat_cards(fight.character()).to_vec();
            let stream = <<V as sts2_core::ContentVersion>::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION;
            let offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] = fight
                .generate_cards(&mut pool, stream)
                .unwrap_or_else(|_| unimplemented!("a pool offering fewer than three"));
            Played::Choosing {
                offered: offered.into_iter().collect(),
                skippable: true,
                at: super::super::Resume::Card(super::Resume::Colorless(Resume::DiscoveryChoice)),
            }
        }
        // `SecretWeapon`: one Attack of the draw pile picked, into hand.
        ColorlessCard::SecretWeapon => {
            let attacks = fight
                .cards_in(Pile::Draw)
                .into_iter()
                .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack))
                .collect();
            Played::AskingAmong {
                cards: attacks,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Colorless(Resume::SecretWeaponTakes)),
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
            let held = super::Held::of(&fight.hittable_opponents());
            shockwave(fight, held, 0)
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
        ColorlessCard::BeatDown => beat_down(fight),
        ColorlessCard::Splash => splash(fight),
        ColorlessCard::HiddenGem => hidden_gem(fight),
        ColorlessCard::Omnislice => omnislice(fight),
        card => super::generated_from(Cards::Colorless(card), fight, 0),
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

/// `BeatDown`: up to its `Cards` of the discard pile's playable Attacks,
/// `StableShuffle`d off `Shuffle`, each auto-played. One aimed at one enemy
/// draws its enemy off `CombatTargets` as its play begins. None plays once
/// the fight is ending.
///
/// It takes the list first, and plays each where it stands by its turn. A
/// question stands the rest still.
///
/// # Panics
///
/// A card no pile holds by its turn.
fn beat_down<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{RunStreamKinds as _, TargetKinds as _};

    let mut attacks: Vec<_> = fight
        .cards_in(Pile::Discard)
        .into_iter()
        .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack))
        .filter(|&at| !fight.card_keyword(at, CardKeyword::Unplayable))
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::SHUFFLE;
    fight.stable_shuffle_cards(&mut attacks, stream);
    attacks.truncate(cards(fight));
    for card in attacks {
        if fight.over_or_ending() {
            break;
        }
        let card = super::standing(fight, card)
            .unwrap_or_else(|| unimplemented!("Beat Down playing a card no pile holds"));
        let aimed = fight.card_target_type(fight.card_ident(card))
            == <V as sts2_core::ContentVersion>::TargetType::ANY_ENEMY;
        if aimed {
            fight.auto_play_card_at_random(card);
        } else {
            fight.auto_play_card(card);
        }
    }
    if fight.waiting() {
        return Played::Waiting {
            at: super::super::Resume::Card(super::Resume::Colorless(Resume::BeatDownPlayed)),
        };
    }
    Played::Done
}

/// `HiddenGem`: its `Replay` added to one card of the draw pile, drawn off
/// `CombatCardSelection`. The card can be played, is no Curse or Quest, and
/// has no replays yet. Attacks, Skills and Powers go first, and any other
/// card only where none is left.
///
/// # Panics
///
/// A card whose type the fight works out.
fn hidden_gem<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{RunStreamKinds as _, Writing};

    let kind = |fight: &F, at| {
        V::card_kind(fight.card_ident(at).card)
            .unwrap_or_else(|| unimplemented!("Hidden Gem reading a card whose type is computed"))
    };
    let eligible: Vec<_> = fight
        .cards_in(Pile::Draw)
        .into_iter()
        .filter(|&at| !fight.card_keyword(at, CardKeyword::Unplayable))
        .filter(|&at| !matches!(kind(fight, at), CardKind::Curse | CardKind::Quest))
        .filter(|&at| fight.card_replays(at) < 1)
        .collect();
    let preferred: Vec<_> = eligible
        .iter()
        .copied()
        .filter(|&at| {
            matches!(
                kind(fight, at),
                CardKind::Attack | CardKind::Skill | CardKind::Power
            )
        })
        .collect();
    let items = if preferred.is_empty() {
        eligible
    } else {
        preferred
    };
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_SELECTION;
    if let Some(card) = fight.random_card(&items, stream) {
        fight.add_replays(Writing::One(card), fight.var(CardVar::Replay));
    }
    Played::Done
}

/// `Splash`: three distinct Attacks of the other characters' pools, off
/// `CombatCardGeneration` and upgraded where Splash is. Its owner takes one
/// into hand, free this turn, or none.
///
/// The pools run in `UnlockState.CharacterCardPools`' order, every
/// character unlocked, each narrowed for combat.
fn splash<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::RunStreamKinds as _;

    let own = fight.character();
    let mut attacks: Vec<_> = super::super::ALL_CHARACTERS
        .into_iter()
        .filter(|&character| character != own)
        .flat_map(|character| V::combat_cards(character).iter().copied())
        .filter(|&card| V::card_kind(card) == Some(CardKind::Attack))
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
    let mut offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] = fight
        .generate_cards(&mut attacks, stream)
        .unwrap_or_else(|_| unimplemented!("other pools offering fewer than three Attacks"));
    if fight.upgraded() {
        for card in &mut offered {
            card.upgrade = 1;
        }
    }
    Played::Choosing {
        offered: offered.into_iter().collect(),
        skippable: true,
        at: super::super::Resume::Card(super::Resume::Colorless(Resume::SplashChoice)),
    }
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
    /// `SecretWeapon`, after the Attack picked from the draw pile.
    SecretWeaponTakes,
    /// `Discovery`, after the card chosen from the three it offered.
    DiscoveryChoice,
    /// `Splash`, after the Attack chosen from the three it offered.
    SplashChoice,
    /// `BeatDown`, once the last card it played has its answer.
    BeatDownPlayed,
    /// `Catastrophe`, once a card it played has its answer, with this many
    /// cards still to play.
    CatastropheLeft(u8),
    /// After a question inside the card's attack was answered.
    FisticuffsHit,
    SeekerStrikeHit,
    JackpotHit,
    /// `HandOfGreed`, whose kill counts where `fatal` says.
    HandOfGreedHit {
        fatal: bool,
    },
    /// `Shockwave`, after a question inside one target's Vulnerable. `held`
    /// is the enemies it took, and `at` the one to go on from.
    Shockwave {
        held: super::Held,
        at: u8,
    },
}

/// The rest of the play waits at `at`, where a command left a question and
/// more commands follow.
fn wait(at: Resume) -> Played<V> {
    Played::Waiting {
        at: super::super::Resume::Card(super::Resume::Colorless(at)),
    }
}

/// The rest of a play that stopped on a choice, once the player chose.
pub(super) fn resume_chosen<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: Option<sts2_core::CardIdent<V>>,
) -> Played<V> {
    match at {
        // `SetToFreeThisTurn`, then `CardPileCmd.AddGeneratedCardToCombat`
        // into hand.
        Resume::DiscoveryChoice | Resume::SplashChoice => {
            if let Some(card) = chosen {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    crate::behaviour::free_this_turn(fight, at);
                }
            }
            Played::Done
        }
        at => unreachable!("{at:?} stops on a pick of a pile"),
    }
}

/// The rest of a play that waited on a card it auto-played.
/// The rest of `HandOfGreed`, once its attack is over: its gold where
/// `results` killed a target whose death, read before the hit, triggers
/// Fatal.
fn hand_of_greed_hit<F: Fight<V>>(
    fight: &mut F,
    fatal: bool,
    results: &sts2_core::AttackResults<V>,
) -> Played<V> {
    if fatal && results.iter().any(|result| result.killed) {
        fight.gain_gold(fight.var(CardVar::Gold));
    }
    Played::Done
}

/// The rest of `Fisticuffs`, once its attack is over: as much block as
/// `results` dealt.
fn fisticuffs_hit<F: Fight<V>>(fight: &mut F, results: &sts2_core::AttackResults<V>) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

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

/// The rest of `SeekerStrike`, once its attack is over: its `Cards` of the
/// draw pile drawn off `CombatCardSelection`, one of them picked.
fn seeker_strike_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::RunStreamKinds as _;

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

/// The rest of `Jackpot`, once its attack is over: its `Cards` of the
/// character's free cards into hand, upgraded once Jackpot is.
fn jackpot_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{CardTable as _, Cost};

    let free: Vec<_> = <V as sts2_core::Rules>::combat_cards(fight.character())
        .iter()
        .copied()
        .filter(|card| card.cost() == Cost::fixed(0))
        .collect();
    let upgrade = u8::from(fight.upgraded());
    let made: Vec<_> = (0..cards(fight))
        .filter_map(|_| fight.generate_card(&free))
        .collect();
    for card in made {
        let card = sts2_core::CardIdent {
            upgrade,
            ..sts2_core::CardIdent::new(card)
        };
        fight.add_generated_cards(card, Pile::Hand, 1);
    }
    Played::Done
}

/// `Shockwave`'s Weak and Vulnerable on each of `held` from the one at `from`
/// on. A question left before one waits.
fn shockwave<F: Fight<V>>(fight: &mut F, held: super::Held, from: u8) -> Played<V> {
    use super::super::super::generated::powers::Power;

    let amount = sts2_core::PowerAmount::Counted(fight.var(CardVar::Power));
    for (at, target) in held.creatures().enumerate().skip(usize::from(from)) {
        if fight.waiting() {
            let at = u8::try_from(at).expect("a fight's creatures fit a u8");
            return wait(Resume::Shockwave { held, at });
        }
        fight.apply_power(target, Power::WeakPower, amount);
        fight.apply_power(target, Power::VulnerablePower, amount);
    }
    Played::Done
}

pub(super) fn resume_waited<F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::CatastropheLeft(left) => catastrophe(fight, Number::from(left)),
        Resume::BeatDownPlayed => Played::Done,
        Resume::FisticuffsHit => {
            let results = fight.waited_attack_results();
            fisticuffs_hit(fight, &results)
        }
        Resume::SeekerStrikeHit => seeker_strike_hit(fight),
        Resume::JackpotHit => jackpot_hit(fight),
        Resume::HandOfGreedHit { fatal } => {
            let results = fight.waited_attack_results();
            hand_of_greed_hit(fight, fatal, &results)
        }
        Resume::Shockwave { held, at } => shockwave(fight, held, at),
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
        Resume::DiscoveryChoice | Resume::SplashChoice => {
            unreachable!("{at:?} chooses, and picks nothing")
        }
        Resume::BeatDownPlayed => unreachable!("Beat Down waits, and picks nothing"),
        // `CardPileCmd.Add` of the one taken, into hand.
        Resume::SecretTechniqueTakes | Resume::SeekerStrikeTakes | Resume::SecretWeaponTakes => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        // `CardPileCmd.Add` onto the top of the draw pile.
        Resume::ThinkingAheadPutBack => {
            fight.act_on(PileAct::OntoDrawTop, chosen.as_slice());
            Played::Done
        }
        at => unreachable!("{at:?} waits on a question, not a pick"),
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
