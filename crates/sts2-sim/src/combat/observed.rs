//! A fight's facts, lent to a comparison without building its report.
//!
//! A run holds a fight in one of two shapes. A live one is a [`Live`], which
//! works each reported fact out when it is asked. One that is over is the
//! [`Combat`] its last report built, kept because the run has not left the
//! room. Each is lent the same way, so a digest walks either.

use std::borrow::Cow;

use sts2_core::{
    CardIdent, Combat, CombatId, Cost, ModelId, NextMove, ObservedCreature, ObservedFight,
    ObservedPlayable, Orbs, Pile, Playable, Preview, Rules, Stack,
};

use super::legal::InHand;
use super::live::{Fighter, HeldPower, Live};

/// The fight a run is standing in, live or over.
pub(crate) enum Fight<'a, V: Rules> {
    Live(&'a Live<V>),
    /// Won or lost, as its last report left it.
    Over(&'a Combat<V>),
}

impl<V: Rules> ObservedFight<V> for Fight<'_, V> {
    fn turn(&self) -> V::Count {
        match self {
            Fight::Live(live) => live.turn,
            Fight::Over(combat) => combat.turn,
        }
    }

    fn phase(&self) -> V::PlayerTurnPhase {
        match self {
            Fight::Live(live) => live.phase,
            Fight::Over(combat) => combat.phase,
        }
    }

    fn energy(&self) -> V::Count {
        match self {
            Fight::Live(live) => live.energy,
            Fight::Over(combat) => combat.energy,
        }
    }

    fn stars(&self) -> V::Count {
        match self {
            Fight::Live(live) => live.stars,
            Fight::Over(combat) => combat.stars,
        }
    }

    fn round(&self) -> V::Count {
        match self {
            Fight::Live(live) => live.round,
            Fight::Over(combat) => combat.round,
        }
    }

    fn encounter(&self) -> Option<V::Encounter> {
        match self {
            Fight::Live(live) => Some(live.encounter),
            Fight::Over(combat) => combat.encounter,
        }
    }

    fn hand(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        match self {
            Fight::Live(live) => Either::Left(live.cards.idents(Pile::Hand)),
            Fight::Over(combat) => Either::Right(combat.hand.iter().copied()),
        }
    }

    fn draw(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        match self {
            Fight::Live(live) => Either::Left(live.cards.idents(Pile::Draw)),
            Fight::Over(combat) => Either::Right(combat.draw.iter().copied()),
        }
    }

    fn discard(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        match self {
            Fight::Live(live) => Either::Left(live.cards.idents(Pile::Discard)),
            Fight::Over(combat) => Either::Right(combat.discard.iter().copied()),
        }
    }

    fn exhaust(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        match self {
            Fight::Live(live) => Either::Left(live.cards.idents(Pile::Exhaust)),
            Fight::Over(combat) => Either::Right(combat.exhaust.iter().copied()),
        }
    }

    fn creatures(&self) -> impl ExactSizeIterator<Item = impl ObservedCreature<V>> + '_ {
        match self {
            Fight::Live(live) => Either::Left(
                live.creatures
                    .iter()
                    .map(|fighter| Creature::Live { fighter, live }),
            ),
            Fight::Over(combat) => Either::Right(combat.creatures.iter().map(Creature::Over)),
        }
    }

    fn legal(&self) -> impl ExactSizeIterator<Item = impl ObservedPlayable<V>> + '_ {
        match self {
            Fight::Live(live) => Either::Left(live.cards.iter(Pile::Hand).map(|card| {
                Card::Live(InHand::new(card, &live.creatures, live.energy, live.stars))
            })),
            Fight::Over(combat) => Either::Right(combat.legal.iter().map(Card::Over)),
        }
    }

    fn orbs(&self) -> Option<&Orbs<V>> {
        match self {
            Fight::Live(live) => live.orbs.as_ref(),
            Fight::Over(combat) => combat.orbs.as_ref(),
        }
    }
}

/// One creature, in a fight live or over.
enum Creature<'a, V: Rules> {
    Live {
        fighter: &'a Fighter<V>,
        live: &'a Live<V>,
    },
    Over(&'a sts2_core::Creature<V>),
}

impl<V: Rules> ObservedCreature<V> for Creature<'_, V> {
    fn id(&self) -> CombatId {
        match self {
            Creature::Live { fighter, .. } => fighter.id,
            Creature::Over(creature) => creature.id,
        }
    }

    fn model(&self) -> Option<ModelId<V>> {
        match self {
            Creature::Live { fighter, .. } => fighter.model,
            Creature::Over(creature) => creature.model,
        }
    }

    fn side(&self) -> V::CombatSide {
        match self {
            Creature::Live { fighter, .. } => fighter.side,
            Creature::Over(creature) => creature.side,
        }
    }

    fn hp(&self) -> V::Health {
        match self {
            Creature::Live { fighter, .. } => fighter.hp,
            Creature::Over(creature) => creature.hp,
        }
    }

    fn max_hp(&self) -> V::Health {
        match self {
            Creature::Live { fighter, .. } => fighter.max_hp,
            Creature::Over(creature) => creature.max_hp,
        }
    }

    fn block(&self) -> V::Number {
        match self {
            Creature::Live { fighter, .. } => fighter.block,
            Creature::Over(creature) => creature.block,
        }
    }

    fn alive(&self) -> bool {
        match self {
            Creature::Live { fighter, .. } => fighter.alive,
            Creature::Over(creature) => creature.alive,
        }
    }

    fn powers(&self) -> impl ExactSizeIterator<Item = Stack<V>> + '_ {
        match self {
            Creature::Live { fighter, .. } => {
                Either::Left(fighter.powers.iter().map(HeldPower::stack))
            }
            Creature::Over(creature) => Either::Right(creature.powers.iter().copied()),
        }
    }

    fn next_move(&self) -> Option<impl std::borrow::Borrow<NextMove<V>> + '_> {
        match self {
            Creature::Live { fighter, live } => live.telegraphed(fighter).map(Cow::Owned),
            Creature::Over(creature) => creature.next_move.as_ref().map(Cow::Borrowed),
        }
    }
}

/// One card in hand, in a fight live or over.
enum Card<'a, V: Rules> {
    Live(InHand<'a, V>),
    Over(&'a Playable<V>),
}

impl<V: Rules> ObservedPlayable<V> for Card<'_, V> {
    fn card(&self) -> CardIdent<V> {
        match self {
            Card::Live(held) => held.card(),
            Card::Over(playable) => playable.card,
        }
    }

    fn cost(&self) -> Cost {
        match self {
            Card::Live(held) => held.cost(),
            Card::Over(playable) => playable.cost,
        }
    }

    fn star_cost(&self) -> Cost {
        match self {
            Card::Live(held) => held.star_cost(),
            Card::Over(playable) => playable.star_cost,
        }
    }

    fn target(&self) -> V::TargetType {
        match self {
            Card::Live(held) => held.target(),
            Card::Over(playable) => playable.target,
        }
    }

    fn reason(&self) -> Option<V::UnplayableReasons> {
        match self {
            Card::Live(held) => held.reason(),
            Card::Over(playable) => playable.reason,
        }
    }

    fn preventer(&self) -> Option<ModelId<V>> {
        match self {
            Card::Live(held) => held.preventer(),
            Card::Over(playable) => playable.preventer,
        }
    }

    fn targets(&self) -> &[CombatId] {
        match self {
            Card::Live(held) => held.targets(),
            Card::Over(playable) => &playable.targets,
        }
    }

    fn effect<R>(&self, read: impl FnOnce(&[Preview<V>]) -> R) -> R {
        match self {
            Card::Live(held) => held.effect(read),
            Card::Over(playable) => read(&playable.effect),
        }
    }
}

/// One of two iterators over the same items.
///
/// What a live fight and a finished one lend through, so each method returns
/// one type whichever the run holds.
enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<T, L: Iterator<Item = T>, R: Iterator<Item = T>> Iterator for Either<L, R> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            Either::Left(left) => left.next(),
            Either::Right(right) => right.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Either::Left(left) => left.size_hint(),
            Either::Right(right) => right.size_hint(),
        }
    }
}

impl<T, L: ExactSizeIterator<Item = T>, R: ExactSizeIterator<Item = T>> ExactSizeIterator
    for Either<L, R>
{
}
