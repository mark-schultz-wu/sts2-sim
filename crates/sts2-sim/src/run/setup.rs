//! What a run draws before a player has done anything.
//!
//! Starting a run takes 413 draws from the `UpFront` stream. The relic bags are
//! shuffled, the shared ancients are dealt to the later acts, and every act's
//! rooms are rolled. None of it shows in an observation except as that one
//! number, so a simulator 412 draws in has landed somewhere else entirely.
//!
//! `tests/setup.txt` is where the phases stand, taken from the game by
//! `cargo xtask dump setupdump`. Each line is a place this has to reach.

use sts2_core::rng::GameRng;
use sts2_core::rng::{Bound, Span};
use sts2_core::{
    ActTable, ContentVersion, EncounterTable, Rarities, RelicPools, RelicTable, Rooms as _, Rules,
    Table,
};

/// How often a relic turns up, as this build spells it.
pub type Rarity<V> = <<V as ContentVersion>::Relic as RelicTable>::Rarity;

/// The pool a relic is drafted from, as this build spells it.
pub type Pool<V> = <<V as ContentVersion>::Relic as RelicTable>::Pool;

/// One rarity's relics, in the order the bag deals them.
///
/// The rarity is derived from the first relic. Further relics must have that
/// same rarity, so the label and contents cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deque<V: Rules> {
    rarity: Rarity<V>,
    relics: Vec<V::Relic>,
}

impl<V: Rules> Deque<V> {
    fn new(relic: V::Relic) -> Self {
        Deque {
            rarity: relic.rarity(),
            relics: vec![relic],
        }
    }

    fn push(&mut self, relic: V::Relic) {
        assert_eq!(
            relic.rarity(),
            self.rarity,
            "a relic deque holds one rarity"
        );
        self.relics.push(relic);
    }

    /// The rarity shared by every relic in this deque.
    pub fn rarity(&self) -> Rarity<V> {
        self.rarity
    }

    /// The relics in the order the bag deals them.
    pub fn relics(&self) -> &[V::Relic] {
        &self.relics
    }
}

/// One run's relic bag, by rarity, in the order it deals from.
///
/// The game keeps a dictionary of rarity to list and shuffles each list. That
/// dictionary is in the order a rarity was first seen. So this is a list and
/// not a map, and the order the relics come in decides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelicBag<V: Rules> {
    deques: Vec<Deque<V>>,
}

impl<V: Rules> RelicBag<V> {
    /// Each rarity's relics, in the order the bag first saw that rarity.
    pub fn deques(&self) -> &[Deque<V>] {
        &self.deques
    }

    /// The shared bag, which is `RelicGrabBag.Populate(relics, rng)`.
    ///
    /// Every relic the shared pool holds, at any rarity. The overload that
    /// takes a list applies no rarity filter, which is why an ancient and an
    /// event relic are in here.
    pub fn shared<R: GameRng>(rng: &mut R) -> Self {
        Self::populate(
            rng,
            Self::pool(<V::Relic as RelicTable>::Pool::SHARED),
            false,
        )
    }

    /// A player's own bag, which is `RelicGrabBag.Populate(player, rng)`.
    ///
    /// The shared pool and then the character's, filtered to the four rarities
    /// a bag deals. That filter is the difference between the two overloads.
    pub fn player<R: GameRng>(rng: &mut R, character: V::Character) -> Self {
        let mut relics = Self::pool(Pool::<V>::SHARED);
        relics.extend(Self::pool(V::relic_pool(character)));
        Self::populate(rng, relics, true)
    }

    /// Every relic a pool holds, in the game's own order.
    fn pool(pool: Pool<V>) -> Vec<V::Relic> {
        V::Relic::ALL
            .iter()
            .copied()
            .filter(|relic| relic.pools().contains(&pool))
            .collect()
    }

    /// Group by rarity and shuffle each group, which is the body both
    /// overloads share.
    fn populate<R: GameRng>(rng: &mut R, relics: Vec<V::Relic>, drafted_only: bool) -> Self {
        let mut deques: Vec<Deque<V>> = Vec::new();
        for relic in relics {
            let rarity = relic.rarity();
            if drafted_only && !Rarity::<V>::DRAFTED.contains(&rarity) {
                continue;
            }
            match deques.iter_mut().find(|deque| deque.rarity() == rarity) {
                Some(deque) => deque.push(relic),
                None => deques.push(Deque::new(relic)),
            }
        }
        for deque in &mut deques {
            rng.unstable_shuffle(&mut deque.relics);
        }
        RelicBag { deques }
    }
}

/// A weighted bag that draws without replacement.
///
/// A port of `GrabBag<T>`. Every entry carries a weight. A draw scales a double
/// by the total, then walks the entries until it passes it. Taking an entry out
/// takes its weight with it.
///
/// Room generation adds everything at weight one. It stays weighted here,
/// because the weights decide which entry a draw lands on.
#[derive(Debug, Clone, Default)]
pub struct GrabBag<T> {
    entries: Vec<(T, f64)>,
    total: f64,
}

impl<T: Copy> GrabBag<T> {
    pub fn new() -> Self {
        GrabBag {
            entries: Vec::new(),
            total: 0.0,
        }
    }

    pub fn add(&mut self, item: T, weight: f64) {
        self.entries.push((item, weight));
        self.total += weight;
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// One entry's index, which is `GrabIndex(rng)`.
    ///
    /// `None` where the weights do not reach the draw, which the game spells
    /// as -1. Floating point makes that reachable on a bag whose weights sum to
    /// slightly under what they were added as.
    fn index<R: GameRng>(&self, rng: &mut R) -> Option<usize> {
        let past = rng.next_double() * self.total;
        let mut so_far = 0.0;
        for (at, (_, weight)) in self.entries.iter().enumerate() {
            so_far += weight;
            if past < so_far {
                return Some(at);
            }
        }
        None
    }

    /// Take one entry the predicate accepts, and remove it.
    ///
    /// The game rejects and redraws rather than reweighting. A bag holding one
    /// acceptable entry in twenty takes about twenty draws to find it, and how
    /// many it took has to be reproduced.
    ///
    /// It checks first whether anything is acceptable, and that check draws
    /// nothing. So a bag with no acceptable entry costs nothing.
    pub fn take<R: GameRng>(
        &mut self,
        rng: &mut R,
        accepts: Option<&dyn Fn(T) -> bool>,
    ) -> Option<T> {
        if let Some(accepts) = accepts
            && !self.entries.iter().any(|&(item, _)| accepts(item))
        {
            return None;
        }

        let mut at = self.index(rng);
        while let (Some(accepts), Some(found)) = (accepts, at) {
            if accepts(self.entries[found].0) {
                break;
            }
            at = self.index(rng);
        }

        let at = at?;
        let (item, weight) = self.entries.remove(at);
        self.total -= weight;
        Some(item)
    }
}

/// One act's rooms, rolled before the run reaches it.
///
/// A port of `ActModel.GenerateRooms`. Every act's rooms are rolled at the
/// start of the run, off one stream and in act order. So this is setup, and not
/// something an act does on arrival.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rooms<V: Rules> {
    /// Every event the act offers, shuffled. Ancients are not among them.
    pub events: Vec<V::Event>,
    /// The weak fights first, then the rest, in the order they are met.
    pub normal: Vec<V::Encounter>,
    /// Fifteen, which is more elites than a map ever holds.
    pub elite: Vec<V::Encounter>,
    pub boss: V::Encounter,
    pub ancient: V::Event,
}

impl<V: Rules> Rooms<V> {
    /// Roll one act's rooms.
    ///
    /// `ancients` is the act's own list with whatever share of the run's
    /// ancients it was dealt already on the end. Only acts after the first get
    /// a share, so for the first act it is the act's own list alone.
    pub fn roll<R: GameRng>(rng: &mut R, act: V::Act, ancients: &[V::Event]) -> Self {
        let events = Self::events(act);
        let mut events: Vec<V::Event> = events;
        rng.unstable_shuffle(&mut events);

        let mut normal: Vec<V::Encounter> = Vec::new();
        let weak = act.weak_encounters() as usize;
        Self::deal(
            rng,
            &mut normal,
            weak,
            &Self::of_room(act, V::RoomType::MONSTER, true),
        );
        Self::deal(
            rng,
            &mut normal,
            act.rooms() as usize - weak,
            &Self::of_room(act, V::RoomType::MONSTER, false),
        );

        let mut elite: Vec<V::Encounter> = Vec::new();
        Self::deal(
            rng,
            &mut elite,
            15,
            &Self::of_room(act, V::RoomType::ELITE, false),
        );

        let bosses = Self::of_room(act, V::RoomType::BOSS, false);
        let boss = next_item(rng, &bosses).expect("an act fields a boss");
        let ancient = next_item(rng, ancients).expect("an act offers an ancient");

        Rooms {
            events,
            normal,
            elite,
            boss,
            ancient,
        }
    }

    /// The act's own events and the shared pool, ancients left out.
    ///
    /// `AllEvents.Concat(ModelDb.AllSharedEvents)`. The epoch filters that
    /// follow it drop nothing, because a run is set up with everything
    /// unlocked.
    fn events(act: V::Act) -> Vec<V::Event> {
        V::events(act)
            .iter()
            .chain(V::shared_events())
            .copied()
            .collect()
    }

    /// The act's encounters of one room kind, in the act's own order.
    ///
    /// `weak` splits the monster rooms into the two lists an act deals from.
    /// It is meaningless for the other kinds, which pass false.
    fn of_room(act: V::Act, room: V::RoomType, weak: bool) -> Vec<V::Encounter> {
        V::encounters(act)
            .iter()
            .copied()
            .filter(|e| e.room() == room && (room != V::RoomType::MONSTER || e.weak() == weak))
            .collect()
    }

    /// Deal `count` encounters, refilling the bag whenever it runs dry.
    ///
    /// One bag across the whole run of deals, so a fight is not offered twice
    /// until every other has been. Fifteen elites out of three is that bag
    /// refilling five times.
    fn deal<R: GameRng>(
        rng: &mut R,
        into: &mut Vec<V::Encounter>,
        count: usize,
        from: &[V::Encounter],
    ) {
        let mut bag: GrabBag<V::Encounter> = GrabBag::new();
        for _ in 0..count {
            if bag.is_empty() {
                for &encounter in from {
                    bag.add(encounter, 1.0);
                }
            }
            Self::without_repeating(rng, into, &mut bag);
        }
    }

    /// One encounter that does not repeat the last, which is
    /// `AddWithoutRepeatingTags`.
    ///
    /// It asks for one sharing no family with the last dealt, and takes
    /// whatever comes if there is none. A bag with nothing acceptable costs no
    /// draw on the first ask, so the fallback is where the draw happens.
    fn without_repeating<R: GameRng>(
        rng: &mut R,
        into: &mut Vec<V::Encounter>,
        bag: &mut GrabBag<V::Encounter>,
    ) {
        let last = into.last().copied();
        let fresh = |candidate: V::Encounter| match last {
            None => true,
            Some(last) => !candidate.shares_family_with(last) && candidate != last,
        };
        let taken = bag.take(rng, Some(&fresh)).or_else(|| bag.take(rng, None));
        if let Some(encounter) = taken {
            into.push(encounter);
        }
    }
}

/// One item of a list, which is `Rng.NextItem`.
///
/// A two-argument `NextInt`, which is a different draw from the one-argument
/// one even over the same range.
fn next_item<T: Copy, R: GameRng>(rng: &mut R, items: &[T]) -> Option<T> {
    let span = Span::new(0, i32::try_from(items.len()).ok()?).ok()?;
    items.get(rng.next_int_range(span) as usize).copied()
}

/// Everything a run draws before the player does anything.
///
/// The phases in the order `RunManager` runs them. The shared relic bag, each
/// player's own, the shared ancients dealt out to the later acts, then every
/// act's rooms. One stream throughout, because where it stands when a phase
/// starts decides what that phase draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening<V: Rules> {
    pub shared_relics: RelicBag<V>,
    pub player_relics: RelicBag<V>,
    /// One entry per act, in act order.
    pub acts: Vec<(V::Act, Rooms<V>)>,
}

impl<V: Rules> Opening<V> {
    /// Draw the whole of a run's setup.
    ///
    /// `rng` is the run's `UpFront` stream at its start. `acts` is the run's
    /// act list in order, which is the default list for an ordinary run.
    pub fn draw<R: GameRng>(rng: &mut R, character: V::Character, acts: &[V::Act]) -> Self {
        let shared_relics = RelicBag::shared(rng);
        let player_relics = RelicBag::player(rng, character);

        // The run's shared ancients, shuffled once and then dealt out. Each act
        // after the first takes a share, and how big a share is itself a draw.
        let mut pool: Vec<V::Event> = V::shared_ancients().to_vec();
        rng.unstable_shuffle(&mut pool);

        let mut dealt: Vec<Vec<V::Event>> = vec![Vec::new()];
        for _ in acts.iter().skip(1) {
            let bound = Bound::new(pool.len() as i32 + 1).expect("a count plus one is positive");
            let taken = rng.next_int(bound) as usize;
            dealt.push(pool.drain(..taken).collect());
        }

        let rooms = acts
            .iter()
            .zip(&dealt)
            .map(|(&act, share)| {
                let mut ancients: Vec<V::Event> = V::ancients(act).to_vec();
                ancients.extend(share);
                (act, Rooms::roll(rng, act, &ancients))
            })
            .collect();

        Opening {
            shared_relics,
            player_relics,
            acts: rooms,
        }
    }
}
