//! The run's facts, lent to a comparison without building its report.
//!
//! Field for field what [`State::report`] puts in an observation, read the same
//! way. A fact the report works out is worked out here when the digest asks
//! for it, and nothing is collected to be lent.

use std::borrow::Borrow;

use sts2_core::{
    CardIdent, Carried, Coord, Digest, Held, Node, Observed, ObservedFight, Position, Reachable,
    Room, Rules,
};

use super::{AtSeat, Phase, State};
use crate::combat::Fight;

impl<V: Rules> Observed<V> for AtSeat<'_, V> {
    fn act(&self) -> V::Count {
        num_traits::cast(self.state.act).expect("an act index is a game number")
    }

    fn floor(&self) -> V::Count {
        self.state.floor
    }

    fn hp(&self) -> V::Health {
        self.state.standing(self.seat).hp
    }

    fn max_hp(&self) -> V::Health {
        self.state.standing(self.seat).max_hp
    }

    fn alive(&self) -> bool {
        self.state.standing(self.seat).alive
    }

    fn run_over(&self) -> bool {
        !self.state.standing(self.seat).alive
    }

    fn victory(&self) -> Option<bool> {
        (!self.state.standing(self.seat).alive)
            .then_some(matches!(self.state.phase, Phase::Won { .. }))
    }

    fn gold(&self) -> V::Number {
        self.state.players[self.seat].gold
    }

    fn relics(&self) -> impl ExactSizeIterator<Item = impl Borrow<Carried<V>>> + '_ {
        self.state.carried(self.seat)
    }

    fn potions(&self) -> impl ExactSizeIterator<Item = impl Borrow<Held<V>>> + '_ {
        crate::combat::belt::<V>(
            self.seat,
            &self.state.players[self.seat].potions,
            self.state.fight(),
            self.state.merchant(self.seat),
            self.state.standing(self.seat).alive,
            self.state.potions_held(self.seat),
        )
    }

    fn potion_slots(&self) -> V::Count {
        self.state.players[self.seat].potion_slots
    }

    fn deck(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.state.players[self.seat]
            .deck
            .iter()
            .map(|held| held.ident)
    }

    fn in_combat(&self) -> bool {
        State::in_combat(self.state)
    }

    fn at(&self) -> Option<Coord> {
        self.state.visited.last().copied()
    }

    fn visited(&self) -> impl ExactSizeIterator<Item = Coord> + '_ {
        self.state.visited.iter().copied()
    }

    fn travelable(&self) -> impl ExactSizeIterator<Item = Reachable<V>> + '_ {
        State::travelable(self.state, self.seat).into_iter()
    }

    fn points(&self) -> &[Node<V>] {
        sts2_core::ActMap::nodes(&*self.state.map)
    }

    fn points_digest(&self) -> Digest {
        self.state.map.digest()
    }

    fn proceed(&self) -> bool {
        self.state.can_proceed()
    }

    fn run_streams(&self) -> impl ExactSizeIterator<Item = (V::RunRngType, Position)> + '_ {
        self.state.streams.run_positions()
    }

    fn player_streams(&self) -> impl ExactSizeIterator<Item = (V::PlayerRngType, Position)> + '_ {
        self.state.streams.player_positions(self.seat)
    }

    fn room(&self) -> Option<impl Borrow<Room<V>> + '_> {
        State::room(self.state, self.seat)
    }

    fn combat(&self) -> Option<impl ObservedFight<V> + '_> {
        match &self.state.phase {
            Phase::Combat(live) => Some(Fight::Live {
                live,
                belts: &self.state.belts,
                seat: self.seat,
            }),
            Phase::Lost { combats, .. } => Some(Fight::Over(&combats[self.seat])),
            Phase::Rewards { seats, .. } => Some(Fight::Over(&seats[self.seat].combat)),
            Phase::Map
            | Phase::Won { .. }
            | Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => None,
        }
    }

    fn moves(&self) -> impl ExactSizeIterator<Item = sts2_core::Decision<V>> + '_ {
        self.state.legal(self.seat).into_iter()
    }
}
