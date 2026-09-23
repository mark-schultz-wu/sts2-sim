//! What a comparison reads from a run, lent rather than built.
//!
//! [`Observation`] is the owned form: what a front end holds, and what the
//! game's reply is parsed into. A comparison only reads it, one field after
//! another, into a digest. So the digest walks this instead, and anything that
//! can lend these facts can be compared without building an observation.
//!
//! An item with no heap of its own is lent as `impl Borrow<Item>`. The owned
//! form hands out a reference, and an implementation that works a fact out
//! hands out the value. Prose travels inside those items, so a digest that
//! reads it and one that skips it are the same walk.
//!
//! An item that holds lists of its own is a trait here too, so neither side
//! collects one to lend it.

use std::borrow::Borrow;

use crate::{
    CardIdent, Carried, Combat, CombatId, ContentVersion, Coord, Cost, Creature, Digest, Held,
    ModelId, NextMove, Node, Observation, Orbs, Playable, Position, Preview, Reachable, Room,
    Stack,
};

/// Everything a digest reads from a run at one decision point.
///
/// Field for field what [`Observation`] holds, in its order.
pub trait Observed<V: ContentVersion> {
    fn act(&self) -> V::Count;
    fn floor(&self) -> V::Count;
    fn hp(&self) -> V::Health;
    fn max_hp(&self) -> V::Health;
    fn alive(&self) -> bool;
    fn run_over(&self) -> bool;
    fn victory(&self) -> Option<bool>;
    fn gold(&self) -> V::Number;
    fn relics(&self) -> impl ExactSizeIterator<Item = impl Borrow<Carried<V>>> + '_;
    fn potions(&self) -> impl ExactSizeIterator<Item = impl Borrow<Held<V>>> + '_;
    fn potion_slots(&self) -> V::Count;
    fn deck(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_;
    fn in_combat(&self) -> bool;

    /// Where the run stands on its map, which is [`crate::Map::at`].
    fn at(&self) -> Option<Coord>;
    fn visited(&self) -> impl ExactSizeIterator<Item = Coord> + '_;
    fn travelable(&self) -> impl ExactSizeIterator<Item = Reachable<V>> + '_;
    fn points(&self) -> &[Node<V>];

    /// [`Observed::points`], as the only thing a digest reads of them.
    ///
    /// The points are settled when an act's map is generated, so a run that
    /// keeps this beside them answers without walking them again.
    fn points_digest(&self) -> Digest {
        Digest::of_points(self.points())
    }

    fn proceed(&self) -> bool;

    /// Where each run stream stands, in the order of the streams' own type.
    fn run_streams(&self) -> impl ExactSizeIterator<Item = (V::RunRngType, Position)> + '_;
    fn player_streams(&self) -> impl ExactSizeIterator<Item = (V::PlayerRngType, Position)> + '_;

    fn room(&self) -> Option<impl Borrow<Room<V>> + '_>;
    fn combat(&self) -> Option<impl ObservedFight<V> + '_>;
}

/// A fight, as a digest reads it. What [`Combat`] holds.
pub trait ObservedFight<V: ContentVersion> {
    fn turn(&self) -> V::Count;
    fn phase(&self) -> V::PlayerTurnPhase;
    fn energy(&self) -> V::Count;
    fn stars(&self) -> V::Count;
    fn round(&self) -> V::Count;
    fn encounter(&self) -> Option<V::Encounter>;
    fn hand(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_;
    fn draw(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_;
    fn discard(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_;
    fn exhaust(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_;
    fn creatures(&self) -> impl ExactSizeIterator<Item = impl ObservedCreature<V>> + '_;
    fn legal(&self) -> impl ExactSizeIterator<Item = impl ObservedPlayable<V>> + '_;
    fn orbs(&self) -> Option<&Orbs<V>>;
}

/// One creature in a fight, as a digest reads it. What [`Creature`] holds.
pub trait ObservedCreature<V: ContentVersion> {
    fn id(&self) -> CombatId;
    fn model(&self) -> Option<ModelId<V>>;
    fn side(&self) -> V::CombatSide;
    fn hp(&self) -> V::Health;
    fn max_hp(&self) -> V::Health;
    fn block(&self) -> V::Number;
    fn alive(&self) -> bool;
    fn powers(&self) -> impl ExactSizeIterator<Item = Stack<V>> + '_;
    fn next_move(&self) -> Option<impl Borrow<NextMove<V>> + '_>;
}

/// One card in hand and what playing it does. What [`Playable`] holds.
pub trait ObservedPlayable<V: ContentVersion> {
    fn card(&self) -> CardIdent<V>;
    fn cost(&self) -> Cost;
    fn star_cost(&self) -> Cost;
    fn target(&self) -> V::TargetType;
    fn reason(&self) -> Option<V::UnplayableReasons>;
    fn preventer(&self) -> Option<ModelId<V>>;
    fn targets(&self) -> &[CombatId];

    /// The previews, handed to `read` as one list.
    ///
    /// Lent through a call rather than returned, so an implementation that
    /// works them out keeps them where it built them.
    fn effect<R>(&self, read: impl FnOnce(&[Preview<V>]) -> R) -> R;
}

impl<V: ContentVersion> Observed<V> for Observation<V> {
    fn act(&self) -> V::Count {
        self.act
    }

    fn floor(&self) -> V::Count {
        self.floor
    }

    fn hp(&self) -> V::Health {
        self.hp
    }

    fn max_hp(&self) -> V::Health {
        self.max_hp
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn run_over(&self) -> bool {
        self.run_over
    }

    fn victory(&self) -> Option<bool> {
        self.victory
    }

    fn gold(&self) -> V::Number {
        self.gold
    }

    fn relics(&self) -> impl ExactSizeIterator<Item = impl Borrow<Carried<V>>> + '_ {
        self.relics.iter()
    }

    fn potions(&self) -> impl ExactSizeIterator<Item = impl Borrow<Held<V>>> + '_ {
        self.potions.iter()
    }

    fn potion_slots(&self) -> V::Count {
        self.potion_slots
    }

    fn deck(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.deck.iter().copied()
    }

    fn in_combat(&self) -> bool {
        self.in_combat
    }

    fn at(&self) -> Option<Coord> {
        self.map.at
    }

    fn visited(&self) -> impl ExactSizeIterator<Item = Coord> + '_ {
        self.map.visited.iter().copied()
    }

    fn travelable(&self) -> impl ExactSizeIterator<Item = Reachable<V>> + '_ {
        self.map.travelable.iter().copied()
    }

    fn points(&self) -> &[Node<V>] {
        &self.map.points
    }

    fn proceed(&self) -> bool {
        self.map.proceed
    }

    fn run_streams(&self) -> impl ExactSizeIterator<Item = (V::RunRngType, Position)> + '_ {
        self.rng.run.iter().map(|(&stream, &at)| (stream, at))
    }

    fn player_streams(&self) -> impl ExactSizeIterator<Item = (V::PlayerRngType, Position)> + '_ {
        self.rng.player.iter().map(|(&stream, &at)| (stream, at))
    }

    fn room(&self) -> Option<impl Borrow<Room<V>> + '_> {
        self.room.as_ref()
    }

    fn combat(&self) -> Option<impl ObservedFight<V> + '_> {
        self.combat.as_ref()
    }
}

impl<V: ContentVersion> ObservedFight<V> for &Combat<V> {
    fn turn(&self) -> V::Count {
        self.turn
    }

    fn phase(&self) -> V::PlayerTurnPhase {
        self.phase
    }

    fn energy(&self) -> V::Count {
        self.energy
    }

    fn stars(&self) -> V::Count {
        self.stars
    }

    fn round(&self) -> V::Count {
        self.round
    }

    fn encounter(&self) -> Option<V::Encounter> {
        self.encounter
    }

    fn hand(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.hand.iter().copied()
    }

    fn draw(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.draw.iter().copied()
    }

    fn discard(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.discard.iter().copied()
    }

    fn exhaust(&self) -> impl ExactSizeIterator<Item = CardIdent<V>> + '_ {
        self.exhaust.iter().copied()
    }

    fn creatures(&self) -> impl ExactSizeIterator<Item = impl ObservedCreature<V>> + '_ {
        self.creatures.iter()
    }

    fn legal(&self) -> impl ExactSizeIterator<Item = impl ObservedPlayable<V>> + '_ {
        self.legal.iter()
    }

    fn orbs(&self) -> Option<&Orbs<V>> {
        self.orbs.as_ref()
    }
}

impl<V: ContentVersion> ObservedCreature<V> for &Creature<V> {
    fn id(&self) -> CombatId {
        self.id
    }

    fn model(&self) -> Option<ModelId<V>> {
        self.model
    }

    fn side(&self) -> V::CombatSide {
        self.side
    }

    fn hp(&self) -> V::Health {
        self.hp
    }

    fn max_hp(&self) -> V::Health {
        self.max_hp
    }

    fn block(&self) -> V::Number {
        self.block
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn powers(&self) -> impl ExactSizeIterator<Item = Stack<V>> + '_ {
        self.powers.iter().copied()
    }

    fn next_move(&self) -> Option<impl Borrow<NextMove<V>> + '_> {
        self.next_move.as_ref()
    }
}

impl<V: ContentVersion> ObservedPlayable<V> for &Playable<V> {
    fn card(&self) -> CardIdent<V> {
        self.card
    }

    fn cost(&self) -> Cost {
        self.cost
    }

    fn star_cost(&self) -> Cost {
        self.star_cost
    }

    fn target(&self) -> V::TargetType {
        self.target
    }

    fn reason(&self) -> Option<V::UnplayableReasons> {
        self.reason
    }

    fn preventer(&self) -> Option<ModelId<V>> {
        self.preventer
    }

    fn targets(&self) -> &[CombatId] {
        &self.targets
    }

    fn effect<R>(&self, read: impl FnOnce(&[Preview<V>]) -> R) -> R {
        read(&self.effect)
    }
}

/// Every field the owned side lends, named against the method that lends it.
///
/// Never called. Each destructuring is exhaustive, so a field added to one of
/// these structs fails to compile here. It compiles again once the field is
/// named, which is the moment to give its trait a method and the walk a read.
#[allow(dead_code)]
fn every_field_is_lent<V: ContentVersion>(
    observation: &Observation<V>,
    combat: &Combat<V>,
    creature: &Creature<V>,
    playable: &Playable<V>,
) {
    let Observation {
        act: _,
        floor: _,
        hp: _,
        max_hp: _,
        alive: _,
        run_over: _,
        victory: _,
        gold: _,
        relics: _,
        potions: _,
        potion_slots: _,
        deck: _,
        in_combat: _,
        map:
            crate::Map {
                at: _,
                visited: _,
                travelable: _,
                points: _,
                proceed: _,
            },
        rng:
            crate::Positions {
                run: _,    // run_streams
                player: _, // player_streams
            },
        room: _,
        combat: _,
    } = observation;
    let Combat {
        turn: _,
        phase: _,
        energy: _,
        stars: _,
        round: _,
        encounter: _,
        hand: _,
        draw: _,
        discard: _,
        exhaust: _,
        creatures: _,
        legal: _,
        orbs: _,
    } = combat;
    let Creature {
        id: _,
        model: _,
        side: _,
        hp: _,
        max_hp: _,
        block: _,
        alive: _,
        powers: _,
        next_move: _,
    } = creature;
    let Playable {
        card: _,
        cost: _,
        star_cost: _,
        target: _,
        reason: _,
        preventer: _,
        targets: _,
        effect: _,
    } = playable;
}
