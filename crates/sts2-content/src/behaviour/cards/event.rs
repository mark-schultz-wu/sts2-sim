//! The event cards the generator leaves to a hand port.

use crate::behaviour::Played;
use crate::behaviour::paused::{Ask, Lent};
use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};
use sts2_core::{
    Amount, Change, CostPool, CreatureView as _, Effect, Fight, FightKind, Lasting, Pile, PileAct,
    PowerAmount, Scaling, Selection,
};

/// Cards a play offers from the hand.
type Cards<V> = smallvec::SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]>;

/// One card, as a pick asks for it.
const ONE: Selection = Selection::Exactly(nonmax::NonMaxUsize::ONE);

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match card {
        // `Enlightenment`: every card in hand costs at most one, this turn or
        // until played, or for the fight once upgraded.
        __c0 if __c0 == V::Card::ENLIGHTENMENT => {
            let lasting = match fight.upgraded() {
                true => Lasting::ThisFight,
                false => Lasting::ThisTurnOrUntilPlayed,
            };
            for card in fight.cards_in(Pile::Hand) {
                fight.modify_card(card, Change::LowerCost(CostPool::Energy, 1), lasting);
            }
            Played::Done
        }
        // `ToricToughness`: its block, then its `Turns` of Toric Toughness on
        // its owner, set to give the block it gained.
        __c0 if __c0 == V::Card::TORIC_TOUGHNESS => {
            use sts2_core::EffectResolution as _;

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            let gained = block.amount();
            fight.apply(block);
            let turns = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::TURNS));
            fight.apply_to_actor(V::Power::TORIC_TOUGHNESS_POWER, turns);
            fight.note_on_power(fight.actor(), V::Power::TORIC_TOUGHNESS_POWER, gained);
            Played::Done
        }
        __c0 if __c0 == V::Card::METAMORPHOSIS => metamorphosis::<V, _>(fight),
        __c0 if __c0 == V::Card::WHISTLE => whistle::<V, _>(fight),
        __c0 if __c0 == V::Card::APOTHEOSIS => apotheosis::<V, _>(fight),
        __c0 if __c0 == V::Card::MAD_SCIENCE_ENERGIZED
            || __c0 == V::Card::MAD_SCIENCE_WISDOM
            || __c0 == V::Card::MAD_SCIENCE_CHAOS
            || __c0 == V::Card::MAD_SCIENCE_EXPERTISE
            || __c0 == V::Card::MAD_SCIENCE_CURIOUS
            || __c0 == V::Card::MAD_SCIENCE_IMPROVEMENT =>
        {
            mad_science::<V, _>(card, fight)
        }
        // Its damage to the target. The Byrdpip pet only animates the hit.
        __c0 if __c0 == V::Card::BYRD_SWOOP => {
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
            Played::Done
        }
        // `Clash`: its damage to the target. Its `IsPlayable` asks for a
        // hand of Attacks alone.
        __c0 if __c0 == V::Card::CLASH => {
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
            Played::Done
        }
        // `Distraction`: a Skill of the character's pool, free this turn,
        // into the hand.
        __c0 if __c0 == V::Card::DISTRACTION => {
            crate::behaviour::free_card_into_hand(
                fight,
                |card| V::card_kind(card) == Some(sts2_core::CardKind::Skill),
                true,
            );
            Played::Done
        }
        // `Entrench`: as much block again as its owner holds, unpowered.
        __c0 if __c0 == V::Card::ENTRENCH => {
            let actor = fight.actor();
            let held = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .map_or(0, |creature| creature.block());
            let block = fight.resolve(Effect::GainBlock {
                recipient: actor,
                amount: Amount::Number(held),
                scaling: Scaling::StaticMove,
            });
            fight.apply(block);
            Played::Done
        }
        // `FeedingFrenzy`: its `StrengthPower` as Feeding Frenzy on its owner.
        __c0 if __c0 == V::Card::FEEDING_FRENZY => {
            let amount = fight.var(V::CardVar::STRENGTH_POWER);
            fight.apply_to_actor(V::Power::FEEDING_FRENZY_POWER, PowerAmount::Counted(amount));
            Played::Done
        }
        // `Stack`: as much block as its `CalculatedBlock`, one more for each
        // card in the discard pile.
        __c0 if __c0 == V::Card::STACK => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::CALCULATED_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        __c0 if __c0 == V::Card::NEOWS_FURY
            || __c0 == V::Card::MAUL
            || __c0 == V::Card::MAD_SCIENCE_SAPPING
            || __c0 == V::Card::MAD_SCIENCE_VIOLENCE
            || __c0 == V::Card::MAD_SCIENCE_CHOKING
            || __c0 == V::Card::WISH
            || __c0 == V::Card::BRIGHTEST_FLAME
            || __c0 == V::Card::DUAL_WIELD =>
        {
            start::<V, K>(card, fight)
        }
        card => crate::behaviour::cards::generated::<V, K>(card, fight),
    }
}

/// An event card's play that can stop on a question: from its start to its
/// end, yielding each question it stops on. Each step is lent the fight, and
/// after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `NeowsFury`: its damage to the target, then up to its `Cards` from
        // the discard pile into hand, as many as the hand has room for, as
        // `CardPileCmd.Add(picked, PileType.Hand)`.
        __c0 if __c0 == V::Card::NEOWS_FURY => {
            let target = resumed!().fight.target();
            hit::<V, _>(resumed!().fight, target, 1);
            let most: Option<nonmax::NonMaxUsize> = neows_fury_room::<V, _>(resumed!().fight);
            if let Some(count) = most {
                yield_!(Ask::Asking {
                    pile: Pile::Discard,
                    selection: Selection::UpTo(count),
                });
                into_hand_taken::<V, K>(resumed!());
            }
        }
        // `Maul`: its damage to the target twice, then its `Increase` onto
        // the damage of every Maul its owner holds in the fight, itself
        // included.
        __c0 if __c0 == V::Card::MAUL => {
            let target = resumed!().fight.target();
            hit::<V, _>(resumed!().fight, target, 2);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            maul_hit::<V, _>(resumed!().fight);
        }
        // A Mad Science attack: its hits on the target, once or
        // `ViolenceHits` times with Violence, each its own attack. Then the
        // debuff Sapping or Choking carries. A question left after a hit
        // waits.
        __c0 if __c0 == V::Card::MAD_SCIENCE_SAPPING
            || __c0 == V::Card::MAD_SCIENCE_VIOLENCE
            || __c0 == V::Card::MAD_SCIENCE_CHOKING =>
        {
            let hits: usize = match card {
                __c1 if __c1 == V::Card::MAD_SCIENCE_VIOLENCE => {
                    num_traits::cast(resumed!().fight.var(V::CardVar::VIOLENCE_HITS))
                        .expect("a few hits")
                }
                _ => 1,
            };
            for _ in 0..hits {
                let target = resumed!().fight.target();
                hit::<V, _>(resumed!().fight, target, 1);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
            }
            mad_science_rider::<V, _>(resumed!().fight, card);
        }
        // `Wish`: one card of the draw pile into hand, as
        // `CardPileCmd.Add(picked, PileType.Hand)`.
        __c0 if __c0 == V::Card::WISH => {
            yield_!(Ask::Asking {
                pile: Pile::Draw,
                selection: ONE,
            });
            into_hand_taken::<V, K>(resumed!());
        }
        // `BrightestFlame`: its Energy, its Cards drawn, then its `MaxHp` off
        // its owner's maximum. A shuffle inside the draw can ask, and the
        // rest waits for it. The hit that takes the health down with the
        // maximum can ask too, as Centennial Puzzle's draw does, and the new
        // maximum waits for the answer.
        __c0 if __c0 == V::Card::BRIGHTEST_FLAME => {
            let fight = &mut *resumed!().fight;
            fight.modify_energy(fight.var(V::CardVar::ENERGY));
            fight.draw(fight.var(V::CardVar::CARDS));
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            let lost: Option<V::Health> =
                fight.lose_max_hp(fight.actor(), fight.var(V::CardVar::MAX_HP));
            if let Some(max_hp) = lost {
                let kept: V::Health = max_hp;
                yield_!(Ask::Waiting);
                let fight = &mut *resumed!().fight;
                fight.set_max_hp(fight.actor(), kept);
            }
        }
        // `DualWield`: one Attack or Power of the hand, then `CreateClone`
        // of it, its `Cards` times, each through `AddGeneratedCardToCombat`
        // into the hand.
        __c0 if __c0 == V::Card::DUAL_WIELD => {
            let cards: Cards<V> = attacks_and_powers_in_hand::<V, _>(resumed!().fight);
            yield_!(Ask::AskingAmong {
                cards,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            if let Some(picked) = crate::behaviour::paused::selected::<V>(taken).one_or_none() {
                // The game's loop reads its count before each pass.
                let mut pass = 0;
                while pass < fight.var(V::CardVar::CARDS) {
                    pass += 1;
                    fight.clone_card(picked, Pile::Hand);
                }
            }
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(Card, crate::behaviour::cards::Resume::Event);

/// `CardPileCmd.Add` of the cards a pick took, into hand.
fn into_hand_taken<V: crate::Build, K: FightKind<V>>(lent: &mut Lent<'_, '_, V, K>) {
    let Lent { fight, taken } = lent;
    fight.act_on(
        PileAct::IntoHand,
        crate::behaviour::paused::selected::<V>(taken).as_slice(),
    );
}

/// How many cards `NeowsFury` takes back: its `Cards`, or as many as the
/// hand has room for. `None` where that is none.
fn neows_fury_room<V: crate::Build, F: Fight<V>>(fight: &F) -> Option<nonmax::NonMaxUsize> {
    let cards: usize =
        num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of cards is not negative");
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    nonmax::NonMaxUsize::new(cards.min(room)).filter(|count| count.get() > 0)
}

/// The Attacks and Powers of the hand.
fn attacks_and_powers_in_hand<V: crate::Build, F: Fight<V>>(fight: &F) -> Cards<V> {
    fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&at| {
            matches!(
                V::card_kind(fight.card_ident(at).card),
                Some(sts2_core::CardKind::Attack | sts2_core::CardKind::Power)
            )
        })
        .collect()
}

/// A Mad Science attack's rider, once its hits are over: Sapping's Weak and
/// Vulnerable, or Choking's Strangle, on the target.
fn mad_science_rider<V: crate::Build, F: Fight<V>>(fight: &mut F, card: V::Card) {
    let target = fight.target();
    let counted = |fight: &F, key| PowerAmount::Counted(fight.var(key));
    match card {
        __c0 if __c0 == V::Card::MAD_SCIENCE_SAPPING => {
            let weak = counted(fight, V::CardVar::SAPPING_WEAK);
            fight.apply_power(target, V::Power::WEAK_POWER, weak);
            let vulnerable = counted(fight, V::CardVar::SAPPING_VULNERABLE);
            fight.apply_power(target, V::Power::VULNERABLE_POWER, vulnerable);
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_CHOKING => {
            let strangle = counted(fight, V::CardVar::CHOKING_DAMAGE);
            fight.apply_power(target, V::Power::STRANGLE_POWER, strangle);
        }
        _ => {}
    }
}

/// `Metamorphosis`: its `Cards` attacks from the character's pool, each
/// `NextItem` off `CombatCardGeneration`, free this fight and into the draw
/// pile at a random place.
fn metamorphosis<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let attacks = crate::behaviour::character_cards(fight, sts2_core::CardKind::Attack);
    for _ in 0..fight.var(V::CardVar::CARDS) {
        let Some(card) = fight.generate_card(&attacks) else {
            unimplemented!("a character pool with no attack")
        };
        if let Some(at) = fight.add_at_random(card, Pile::Draw) {
            for pool in [CostPool::Energy, CostPool::Star] {
                fight.modify_card(
                    at,
                    Change::SetCost(pool, num_traits::zero()),
                    Lasting::ThisFight,
                );
            }
        }
    }
    Played::Done
}

/// Its damage to `target`, `hits` times, as one attack.
pub(crate) fn hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    target: sts2_core::CombatId,
    hits: usize,
) {
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
}

/// `MadScience`, a skill or a power: what its type does, then its rider.
/// The attacks are in [`paused`].
///
/// A skill gains its block. A power applies what its rider names to its
/// owner. Energized gives energy, Wisdom draws, and Chaos adds a card of the
/// character's pool, free this turn.
fn mad_science<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V, F::Kind> {
    use sts2_core::PowerAmount;

    let counted = |fight: &F, key| PowerAmount::Counted(fight.var(key));
    match card {
        __c0 if __c0 == V::Card::MAD_SCIENCE_ENERGIZED
            || __c0 == V::Card::MAD_SCIENCE_WISDOM
            || __c0 == V::Card::MAD_SCIENCE_CHAOS =>
        {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            match card {
                __c0 if __c0 == V::Card::MAD_SCIENCE_ENERGIZED => {
                    fight.modify_energy(fight.var(V::CardVar::ENERGIZED_ENERGY))
                }
                __c0 if __c0 == V::Card::MAD_SCIENCE_WISDOM => {
                    fight.draw(fight.var(V::CardVar::WISDOM_CARDS));
                }
                _ => crate::behaviour::free_card_into_hand(fight, |_| true, false),
            }
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_EXPERTISE => {
            let strength = counted(fight, V::CardVar::EXPERTISE_STRENGTH);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, strength);
            let dexterity = counted(fight, V::CardVar::EXPERTISE_DEXTERITY);
            fight.apply_to_actor(V::Power::DEXTERITY_POWER, dexterity);
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_CURIOUS => {
            let curious = counted(fight, V::CardVar::CURIOUS_REDUCTION);
            fight.apply_to_actor(V::Power::CURIOUS_POWER, curious);
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_IMPROVEMENT => {
            fight.apply_to_actor(V::Power::IMPROVEMENT_POWER, PowerAmount::Counted(1));
        }
        card => unreachable!("{card:?} is not a Mad Science"),
    }
    Played::Done
}

/// `Whistle`: its damage to the target, then `CreatureCmd.Stun` on it with
/// no move named after.
fn whistle<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
    fight.stun(
        target,
        <V as crate::build::Lists>::powers_stunned(),
        sts2_core::StunResume::Interrupted,
    );
    Played::Done
}

/// The rest of `Maul`, once its attack is over: every Maul's damage raised.
fn maul_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let raise = Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(
        fight.var(V::CardVar::INCREASE),
    ));
    // `AllCards`: the hand, the draw, discard and exhaust piles, then the
    // play pile. That holds the Maul being played, and any other an
    // auto-play from the draw pile took there to play after it.
    let this = fight.playing();
    for pile in [
        Pile::Hand,
        Pile::Draw,
        Pile::Discard,
        Pile::Exhaust,
        Pile::Play,
    ] {
        for card in fight.cards_in(pile) {
            if card.id() != this && fight.card_ident(card).card == V::Card::MAUL {
                fight.modify_card(card, raise, Lasting::ThisFight);
            }
        }
    }
    fight.modify_this(raise, Lasting::ThisFight);
}

/// `Apotheosis`: every other card its owner holds in the fight upgraded,
/// where it can be.
fn apotheosis<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    // `AllCards` less itself: the hand, the draw, discard, exhaust and play
    // piles, where a card that auto-played it stands. `CardCmd.Upgrade`
    // leaves a card at its highest alone.
    let this = fight.playing();
    for pile in [
        Pile::Hand,
        Pile::Draw,
        Pile::Discard,
        Pile::Exhaust,
        Pile::Play,
    ] {
        for card in fight.cards_in(pile) {
            if card.id() != this {
                fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
            }
        }
    }
    Played::Done
}
