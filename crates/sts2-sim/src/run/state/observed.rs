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

use super::{Phase, State};
use crate::combat::Fight;

impl<V: Rules> Observed<V> for State<V> {
    fn act(&self) -> V::Count {
        num_traits::cast(self.act).expect("an act index is a game number")
    }

    fn floor(&self) -> V::Count {
        self.floor
    }

    fn hp(&self) -> V::Health {
        self.standing().hp
    }

    fn max_hp(&self) -> V::Health {
        self.standing().max_hp
    }

    fn alive(&self) -> bool {
        self.standing().alive
    }

    fn run_over(&self) -> bool {
        !self.standing().alive
    }

    fn victory(&self) -> Option<bool> {
        (!self.standing().alive).then_some(matches!(self.phase, Phase::Won))
    }

    fn gold(&self) -> V::Number {
        self.player.gold
    }

    fn relics(&self) -> impl ExactSizeIterator<Item = impl Borrow<Carried<V>>> + '_ {
        self.carried()
    }

    fn potions(&self) -> impl ExactSizeIterator<Item = impl Borrow<Held<V>>> + '_ {
        crate::combat::belt::<V>(
            &self.player.potions,
            self.fight(),
            self.standing().alive,
            self.potions_held(),
        )
    }

    fn potion_slots(&self) -> V::Count {
        self.player.potion_slots
    }

    fn deck(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.player.deck.iter().map(|held| held.ident)
    }

    fn in_combat(&self) -> bool {
        State::in_combat(self)
    }

    fn at(&self) -> Option<Coord> {
        self.visited.last().copied()
    }

    fn visited(&self) -> impl ExactSizeIterator<Item = Coord> + '_ {
        self.visited.iter().copied()
    }

    fn travelable(&self) -> impl ExactSizeIterator<Item = Reachable<V>> + '_ {
        self.reachable().into_iter()
    }

    fn points(&self) -> &[Node<V>] {
        sts2_core::ActMap::nodes(&*self.map)
    }

    fn points_digest(&self) -> Digest {
        self.map.digest()
    }

    fn proceed(&self) -> bool {
        self.can_proceed()
    }

    fn run_streams(&self) -> impl ExactSizeIterator<Item = (V::RunRngType, Position)> + '_ {
        self.streams.run_positions()
    }

    fn player_streams(&self) -> impl ExactSizeIterator<Item = (V::PlayerRngType, Position)> + '_ {
        self.streams.player_positions()
    }

    fn room(&self) -> Option<impl Borrow<Room<V>> + '_> {
        State::room(self)
    }

    fn combat(&self) -> Option<impl ObservedFight<V> + '_> {
        match &self.phase {
            Phase::Combat(live) => Some(Fight::Live {
                live,
                relics: &self.player.relics,
            }),
            Phase::Lost { combat } | Phase::Rewards { combat, .. } => Some(Fight::Over(combat)),
            Phase::Map
            | Phase::Won
            | Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => None,
        }
    }
}
