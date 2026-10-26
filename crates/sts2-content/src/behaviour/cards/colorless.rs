//! The colorless cards the generator leaves to a hand port.

use crate::behaviour::Played;
use sts2_core::{CardKind, EventHook, Fight, FightKind, HookEffect, Pile, PileAct, Selection};

use crate::behaviour::cards::Number;
use crate::behaviour::paused::{Ask, Lent};

/// Cards a play offers from a pile.
type Cards<V> = smallvec::SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]>;

/// One card, as a pick asks for it.
const ONE: Selection = Selection::Exactly(nonmax::NonMaxUsize::ONE);
use crate::generated::cards::{CardKeyword, CardRarity};
use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match card {
        __c0 if __c0 == V::Card::VOLLEY => volley::<V, _>(fight),
        // `Scrawl`: cards drawn until the hand is full.
        __c0 if __c0 == V::Card::SCRAWL => {
            let room =
                sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
            fight.draw(Number::try_from(room).expect("a hand's room fits a game number"));
            Played::Done
        }
        // `TheBomb`: its `Turns` of The Bomb on its owner, then its
        // `BombDamage` written on the instance.
        __c0 if __c0 == V::Card::THE_BOMB => {
            let actor = fight.actor();
            let turns = fight.power_var(V::CardVar::TURNS);
            let damage = fight.var(V::CardVar::BOMB_DAMAGE);
            fight.apply_to_actor(
                V::Power::THE_BOMB_POWER,
                sts2_core::PowerAmount::Counted(turns),
            );
            fight.write_power(actor, V::Power::THE_BOMB_POWER, |state| {
                state.set_bomb_damage(damage)
            });
            Played::Done
        }
        // `Alchemize`: a potion made off `CombatPotionGeneration` among those
        // a fight can make, procured.
        __c0 if __c0 == V::Card::ALCHEMIZE => {
            use sts2_core::RunStreamKinds as _;

            fight.procure_random_potion(
                <V as sts2_core::ContentVersion>::RunRngType::COMBAT_POTION_GENERATION,
            );
            Played::Done
        }
        // `Prolong`: as much Block Next Turn on its owner as the block it
        // holds.
        __c0 if __c0 == V::Card::PROLONG => {
            use sts2_core::CreatureView as _;

            let owner = fight.actor();
            let block = fight
                .creatures()
                .find(|creature| creature.id() == owner)
                .map_or(0, |creature| creature.block());
            fight.apply_to_actor(
                V::Power::BLOCK_NEXT_TURN_POWER,
                sts2_core::PowerAmount::Counted(block),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::IMPATIENCE => impatience::<V, _>(fight),
        __c0 if __c0 == V::Card::ANOINTED => anointed::<V, _>(fight),
        __c0 if __c0 == V::Card::JACK_OF_ALL_TRADES => jack_of_all_trades::<V, _>(fight),
        __c0 if __c0 == V::Card::HIDDEN_GEM => hidden_gem::<V, _>(fight),
        __c0 if __c0 == V::Card::OMNISLICE => omnislice::<V, _>(fight),
        __c0 if __c0 == V::Card::CATASTROPHE
            || __c0 == V::Card::HAND_OF_GREED
            || __c0 == V::Card::FISTICUFFS
            || __c0 == V::Card::SEEKER_STRIKE
            || __c0 == V::Card::JACKPOT
            || __c0 == V::Card::DISCOVERY
            || __c0 == V::Card::SECRET_WEAPON
            || __c0 == V::Card::SECRET_TECHNIQUE
            || __c0 == V::Card::SHOCKWAVE
            || __c0 == V::Card::PURITY
            || __c0 == V::Card::THINKING_AHEAD
            || __c0 == V::Card::BEAT_DOWN
            || __c0 == V::Card::SPLASH
            || __c0 == V::Card::RESTLESSNESS =>
        {
            start::<V, K>(card, fight)
        }
        card => crate::behaviour::cards::generated::<V, K>(card, fight),
    }
}

/// A colorless card's play that can stop on a question: from its start to
/// its end, yielding each question it stops on. Each step is lent the fight,
/// and after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `Catastrophe`: its `Cards` times, a card of the draw pile
        // auto-played. Each is the first of the playable ones
        // `StableShuffle`d off the shuffle stream, or of the whole pile where
        // none is playable. A card it plays that asks stands the loop still.
        __c0 if __c0 == V::Card::CATASTROPHE => {
            let plays: Number = resumed!().fight.var(V::CardVar::CARDS);
            for _ in 0..plays {
                catastrophe_play::<V, _>(resumed!().fight);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
            }
        }
        // `HandOfGreed`: its damage to the target, then its `Gold` where a
        // result says `WasTargetKilled` of a target whose death, read before
        // the hit, triggers Fatal.
        __c0 if __c0 == V::Card::HAND_OF_GREED => {
            let fatal: bool = {
                let fight = &mut *resumed!().fight;
                fight.death_triggers_fatal(fight.target())
            };
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
            if fatal && killed {
                let fight = &mut *resumed!().fight;
                fight.gain_gold(fight.var(V::CardVar::GOLD));
            }
        }
        // `Fisticuffs`: its damage to the target, then as much block as the
        // hit dealt, overkill included.
        __c0 if __c0 == V::Card::FISTICUFFS => {
            let mut hit: Number = dealt::<V>(&strike::<V, _>(resumed!().fight));
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                hit = dealt::<V>(&resumed!().fight.waited_attack_results());
            }
            block::<V, _>(resumed!().fight, hit);
        }
        // `SeekerStrike`: its damage to the target, then one of three cards
        // of the draw pile, `StableShuffle`d off `CombatCardSelection`, into
        // hand.
        __c0 if __c0 == V::Card::SEEKER_STRIKE => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let offered: Cards<V> = seeker_strike_offer::<V, _>(resumed!().fight);
            yield_!(Ask::AskingAmong {
                cards: offered,
                selection: ONE,
            });
            into_hand_taken::<V, K>(resumed!());
        }
        // `Jackpot`: its damage to the target, then its `Cards` of the
        // character's pool that cost nothing, each `NextItem` off
        // `CombatCardGeneration` and upgraded where Jackpot is, into hand.
        __c0 if __c0 == V::Card::JACKPOT => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            jackpot_hit::<V, _>(resumed!().fight);
        }
        // `Discovery`: three distinct cards of the character's pool, off
        // `CombatCardGeneration`, of which its owner takes one into hand,
        // free this turn, or none.
        __c0 if __c0 == V::Card::DISCOVERY => {
            let offered: sts2_core::Offered<V> = discovery_offer::<V, _>(resumed!().fight);
            yield_!(Ask::Choosing {
                offered,
                skippable: true,
            });
            free_into_hand::<V, K>(resumed!());
        }
        // `SecretWeapon`: one Attack of the draw pile picked, into hand.
        __c0 if __c0 == V::Card::SECRET_WEAPON => {
            let attacks: Cards<V> = of_kind_in_draw::<V, _>(resumed!().fight, CardKind::Attack);
            yield_!(Ask::AskingAmong {
                cards: attacks,
                selection: ONE,
            });
            into_hand_taken::<V, K>(resumed!());
        }
        // `SecretTechnique`: one Skill of the draw pile picked, into hand.
        __c0 if __c0 == V::Card::SECRET_TECHNIQUE => {
            let skills: Cards<V> = of_kind_in_draw::<V, _>(resumed!().fight, CardKind::Skill);
            yield_!(Ask::AskingAmong {
                cards: skills,
                selection: ONE,
            });
            into_hand_taken::<V, K>(resumed!());
        }
        // `Shockwave`: its `Power` of Weak, then of Vulnerable, on each enemy
        // that can be hit, one enemy at a time. A question left before one
        // waits, and the amount is read again after.
        __c0 if __c0 == V::Card::SHOCKWAVE => {
            let held: crate::behaviour::cards::Held =
                crate::behaviour::cards::Held::of(&resumed!().fight.hittable_opponents());
            let count: usize = held.creatures().count();
            let mut amount: Number = resumed!().fight.var(V::CardVar::POWER);
            for at in 0..count {
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                    amount = resumed!().fight.var(V::CardVar::POWER);
                }
                let target = held.creatures().nth(at).expect("one of those held");
                let fight = &mut *resumed!().fight;
                let power = sts2_core::PowerAmount::Counted(amount);
                fight.apply_power(target, V::Power::WEAK_POWER, power);
                fight.apply_power(target, V::Power::VULNERABLE_POWER, power);
            }
        }
        // `Purity`: `FromHand` for up to its `Cards`, each then exhausted.
        __c0 if __c0 == V::Card::PURITY => {
            let most: Option<nonmax::NonMaxUsize> =
                nonmax::NonMaxUsize::new(cards::<V, _>(resumed!().fight));
            if let Some(count) = most {
                yield_!(Ask::Asking {
                    pile: Pile::Hand,
                    selection: Selection::UpTo(count),
                });
                let Lent { fight, taken } = resumed!();
                fight.act_on(
                    PileAct::Exhaust,
                    crate::behaviour::paused::selected::<V>(taken).as_slice(),
                );
            }
        }
        // `ThinkingAhead`: its `Cards` drawn, then one card of the hand onto
        // the top of the draw pile, as `CardPileCmd.Add`.
        __c0 if __c0 == V::Card::THINKING_AHEAD => {
            let fight = &mut *resumed!().fight;
            fight.draw(fight.var(V::CardVar::CARDS));
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::OntoDrawTop,
                crate::behaviour::paused::selected::<V>(taken).as_slice(),
            );
        }
        // `BeatDown`: up to its `Cards` of the discard pile's playable
        // Attacks, each auto-played. Its plays wait together on a question
        // the last left.
        __c0 if __c0 == V::Card::BEAT_DOWN => {
            beat_down_plays::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
        }
        // `Splash`: three distinct Attacks of the other characters' pools,
        // off `CombatCardGeneration` and upgraded where Splash is. Its owner
        // takes one into hand, free this turn, or none.
        __c0 if __c0 == V::Card::SPLASH => {
            let offered: sts2_core::Offered<V> = splash_offer::<V, _>(resumed!().fight);
            yield_!(Ask::Choosing {
                offered,
                skippable: true,
            });
            free_into_hand::<V, K>(resumed!());
        }
        // `Restlessness`: with no other card in hand, its `Cards` drawn one
        // at a time, then its `Energy`. It stands in the play pile as it
        // runs. The game's loop reads its count before each pass. A shuffle
        // inside a draw can ask, and the rest waits for it.
        __c0 if __c0 == V::Card::RESTLESSNESS => {
            if resumed!().fight.cards_in(Pile::Hand).is_empty() {
                let mut pass: Number = 0;
                while pass < resumed!().fight.var(V::CardVar::CARDS) {
                    pass += 1;
                    resumed!().fight.draw(1);
                    if resumed!().fight.waiting() {
                        yield_!(Ask::Waiting);
                    }
                }
                let fight = &mut *resumed!().fight;
                fight.modify_energy(fight.var(V::CardVar::ENERGY));
            }
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(V::Card, crate::behaviour::cards::Resume::Colorless);

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

/// `amount` of block onto its owner.
fn block<V: crate::Build, F: Fight<V>>(fight: &mut F, amount: Number) {
    use sts2_core::{Amount, Effect, Scaling};

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Number(amount),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
}

/// The cards of the draw pile of `kind`.
fn of_kind_in_draw<V: crate::Build, F: Fight<V>>(fight: &F, kind: CardKind) -> Cards<V> {
    fight
        .cards_in(Pile::Draw)
        .into_iter()
        .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(kind))
        .collect()
}

/// `CardPileCmd.Add` of the one a pick took, into hand.
fn into_hand_taken<V: crate::Build, K: FightKind<V>>(lent: &mut Lent<'_, '_, V, K>) {
    let Lent { fight, taken } = lent;
    fight.act_on(
        PileAct::IntoHand,
        crate::behaviour::paused::selected::<V>(taken).as_slice(),
    );
}

/// `SetToFreeThisTurn`, then `CardPileCmd.AddGeneratedCardToCombat` into
/// hand, of the one a choice took.
fn free_into_hand<V: crate::Build, K: FightKind<V>>(lent: &mut Lent<'_, '_, V, K>) {
    let Lent { fight, taken } = lent;
    if let Some(chosen) = crate::behaviour::paused::chosen::<V>(taken) {
        for at in fight.add_generated_cards(chosen, Pile::Hand, 1) {
            crate::behaviour::free_this_turn(&mut **fight, at);
        }
    }
}

/// One of `Catastrophe`'s plays, an auto-play of one card of the draw pile.
/// It is the first of the playable ones `StableShuffle`d off the shuffle
/// stream, or of the whole pile where none is playable.
fn catastrophe_play<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::RunStreamKinds as _;

    let stream = <V as sts2_core::ContentVersion>::RunRngType::SHUFFLE;
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
}

/// `BeatDown`'s plays: up to its `Cards` of the discard pile's playable
/// Attacks, `StableShuffle`d off `Shuffle`, each auto-played. One aimed at
/// one enemy draws its enemy off `CombatTargets` as its play begins. None
/// plays once the fight is ending.
///
/// It takes the list first, and plays each where it stands by its turn.
///
/// # Panics
///
/// A card no pile holds by its turn.
fn beat_down_plays<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::{RunStreamKinds as _, TargetKinds as _};

    let mut attacks: Vec<_> = fight
        .cards_in(Pile::Discard)
        .into_iter()
        .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack))
        .filter(|&at| !fight.card_keyword(at, CardKeyword::Unplayable))
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::SHUFFLE;
    fight.stable_shuffle_cards(&mut attacks, stream);
    attacks.truncate(cards::<V, _>(fight));
    for card in attacks {
        if fight.over_or_ending() {
            break;
        }
        let card = crate::behaviour::cards::standing::<V, _>(fight, card)
            .unwrap_or_else(|| unimplemented!("Beat Down playing a card no pile holds"));
        let aimed = fight.card_target_type(fight.card_ident(card))
            == <V as sts2_core::ContentVersion>::TargetType::ANY_ENEMY;
        if aimed {
            fight.auto_play_card_at_random(card);
        } else {
            fight.auto_play_card(card);
        }
    }
}

/// `Discovery`'s offer: three distinct cards of the character's pool, off
/// `CombatCardGeneration`.
fn discovery_offer<V: crate::Build, F: Fight<V>>(fight: &mut F) -> sts2_core::Offered<V> {
    let mut pool =
        <V as sts2_core::Rules>::combat_cards(fight.character(), fight.multiplayer()).to_vec();
    let stream = <crate::generated::runstate::RunRngType as sts2_core::RunStreamKinds>::COMBAT_CARD_GENERATION;
    let offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] = fight
        .generate_cards(&mut pool, stream)
        .unwrap_or_else(|_| unimplemented!("a pool offering fewer than three"));
    offered.into_iter().collect()
}

/// `Splash`'s offer: three distinct Attacks of the other characters' pools,
/// off `CombatCardGeneration` and upgraded where Splash is.
///
/// The pools run in `UnlockState.CharacterCardPools`' order, every
/// character unlocked, each narrowed for combat.
fn splash_offer<V: crate::Build, F: Fight<V>>(fight: &mut F) -> sts2_core::Offered<V> {
    use sts2_core::RunStreamKinds as _;

    let (own, multiplayer) = (fight.character(), fight.multiplayer());
    let mut attacks: Vec<_> = <V as crate::build::Lists>::ALL_CHARACTERS
        .iter()
        .copied()
        .filter(|&character| character != own)
        .flat_map(|character| V::combat_cards(character, multiplayer).iter().copied())
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
    offered.into_iter().collect()
}

/// `Omnislice`: its damage to the target, then what that hit dealt, block
/// and overkill included, to each other enemy that can be hit, unpowered.
/// Both are hits of one attack.
fn omnislice<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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

/// `HiddenGem`: its `Replay` added to one card of the draw pile, drawn off
/// `CombatCardSelection`. The card can be played, is no Curse or Quest, and
/// has no replays yet. Attacks, Skills and Powers go first, and any other
/// card only where none is left.
///
/// # Panics
///
/// A card whose type the fight works out.
fn hidden_gem<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
        fight.add_replays(Writing::One(card), fight.var(V::CardVar::REPLAY));
    }
    Played::Done
}

/// `JackOfAllTrades`: `GetDistinctForCombat` of its `Cards` of colorless
/// cards other than itself, off `CombatCardGeneration`, into hand.
fn jack_of_all_trades<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    use sts2_core::RunStreamKinds as _;

    let mut pool: Vec<_> = V::combat_colorless_cards(fight.multiplayer())
        .iter()
        .copied()
        .filter(|&card| card != V::Card::JACK_OF_ALL_TRADES)
        .collect();
    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
    let made: Vec<_> = match cards::<V, _>(fight) {
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

/// What the hits of an attack dealt, overkill included.
fn dealt<V: crate::Build>(results: &sts2_core::AttackResults<V>) -> Number {
    results
        .iter()
        .map(|result| result.total + result.overkill)
        .sum()
}

/// `SeekerStrike`'s offer, once its attack is over: its `Cards` of the draw
/// pile drawn off `CombatCardSelection`, in the pile's order.
fn seeker_strike_offer<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Cards<V> {
    use sts2_core::RunStreamKinds as _;

    let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_SELECTION;
    let mut shuffled = fight.cards_in(Pile::Draw);
    fight.stable_shuffle_cards(&mut shuffled, stream);
    let options = usize::try_from(fight.var(V::CardVar::CARDS)).expect("a few cards");
    shuffled.truncate(options);
    fight
        .cards_in(Pile::Draw)
        .into_iter()
        .filter(|card| shuffled.contains(card))
        .collect()
}

/// The rest of `Jackpot`, once its attack is over: its `Cards` of the
/// character's free cards into hand, upgraded once Jackpot is.
fn jackpot_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::{CardTable as _, Cost};

    let free: Vec<_> =
        <V as sts2_core::Rules>::combat_cards(fight.character(), fight.multiplayer())
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
}

/// The card's `Cards`, as a count.
fn cards<V: crate::Build, F: Fight<V>>(fight: &F) -> usize {
    num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of cards is not negative")
}

/// `Impatience`: its `Cards` drawn, with no attack in hand.
fn impatience<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let attack = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .any(|at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack));
    if !attack {
        fight.draw(fight.var(V::CardVar::CARDS));
    }
    Played::Done
}

/// `Anointed`: the rare cards in the draw pile, `TakeRandom` off
/// `CombatCardSelection` for as many as the hand has room for, into hand.
fn anointed<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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

/// Deliver a notification to one colorless card in the fight.
pub(super) fn event_hook<V: crate::Build>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Bolas.BeforeHandDraw` and `ThrummingHatchet.BeforeHandDraw`: back
        // into hand, from wherever it is, once this copy finished a play in
        // its owner's last turn. A fight here has one player.
        (__c0, EventHook::BeforeHandDraw { turn, .. })
            if (__c0 == V::Card::BOLAS || __c0 == V::Card::THRUMMING_HATCHET) =>
        {
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
fn volley<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
    Played::Done
}
