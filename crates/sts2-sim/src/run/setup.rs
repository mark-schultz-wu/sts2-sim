//! What a run draws before a player has done anything.
//!
//! Starting a run takes 413 draws from the `UpFront` stream. The relic bags are
//! shuffled, the shared ancients are dealt to the later acts, and every act's
//! rooms are rolled. None of it shows in an observation except as that one
//! number, so a simulator 412 draws in has landed somewhere else entirely.
//!
//! `tests/setup.txt` is where the phases stand, taken from the game by
//! `cargo xtask dump setupdump`. Each line is a place this has to reach.

use indexmap::IndexMap;
use sts2_core::rng::{Bound, GameRng};
use sts2_core::{
    ActTable, ContentVersion, EncounterTable, RarityKinds, RelicPoolKinds, RelicTable,
    RoomKinds as _, Rules, Table,
};

/// How often a relic turns up, as this build spells it.
pub type Rarity<V> = <<V as ContentVersion>::Relic as RelicTable>::Rarity;

/// The pool a relic is drafted from, as this build spells it.
pub type Pool<V> = <<V as ContentVersion>::Relic as RelicTable>::Pool;

/// One run's relic bag, by rarity, in the order it deals from.
///
/// The game keeps a dictionary of rarity to list and shuffles each list. That
/// dictionary is in the order a rarity was first seen, which `IndexMap`
/// preserves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelicBag<V: Rules> {
    deques: IndexMap<Rarity<V>, Vec<V::Relic>>,
}

/// `RelicModel.IsAllowed` for a run of one player, `floor` floors in.
fn allowed_now<V: Rules>(relic: V::Relic, floor: usize) -> bool {
    V::relic_allowed(relic, floor, 1)
}

impl<V: Rules> RelicBag<V> {
    /// Each rarity's relics, in the order the bag first saw that rarity.
    pub fn deques(&self) -> impl Iterator<Item = (Rarity<V>, &[V::Relic])> {
        self.deques
            .iter()
            .map(|(&rarity, relics)| (rarity, relics.as_slice()))
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

    /// `RelicGrabBag.PullFromFront`, as a read rather than a write.
    ///
    /// The game removes what it deals, and what `RelicModel.IsAllowed` fails
    /// as it pulls. The shuffle never moves and a floor only climbs, so the
    /// deque less `dealt` and less what fails at `floor` is the same sequence.
    ///
    /// Answers the deque for `rarity`, or the next up when it is spent: shop,
    /// common, uncommon, rare.
    ///
    /// # Panics
    ///
    /// A bag with nothing left, which the game refreshes. That is not reached.
    pub fn front(&self, rarity: Rarity<V>, dealt: &[V::Relic], floor: usize) -> V::Relic {
        self.pull(rarity, dealt, floor, |_| true, false)
    }

    /// [`Self::front`] through a filter, which a rarity's deque is walked
    /// past for when nothing in it passes.
    pub fn front_where(
        &self,
        rarity: Rarity<V>,
        dealt: &[V::Relic],
        floor: usize,
        allowed: impl Fn(V::Relic) -> bool,
    ) -> V::Relic {
        self.pull(rarity, dealt, floor, allowed, false)
    }

    /// `RelicGrabBag.PullFromBack`, which is what a merchant stocks from.
    ///
    /// The same deque and the same walk, read from the other end and through a
    /// filter. `IsAllowedInShops` is that filter, and a rarity whose deque
    /// holds nothing passing it is walked past rather than drawn from.
    pub fn back(
        &self,
        rarity: Rarity<V>,
        dealt: &[V::Relic],
        floor: usize,
        allowed: impl Fn(V::Relic) -> bool,
    ) -> V::Relic {
        self.pull(rarity, dealt, floor, allowed, true)
    }

    /// `RelicGrabBag.HasAvailableRelics`: whether a deal from the front finds
    /// a relic at any of the rarities a bag deals.
    ///
    /// Every drafted rarity is walked up to rare, so any undealt relic at any
    /// of the four that `IsAllowed` lets through answers yes.
    ///
    /// # Panics
    ///
    /// A bag with nothing left, where the game refreshes a spent deque or
    /// falls back to a relic of its own. Neither is reached.
    pub fn has_left(&self, dealt: &[V::Relic], floor: usize) -> bool {
        let left = self
            .deques
            .iter()
            .filter(|(rarity, _)| Rarity::<V>::DRAFTED.contains(rarity))
            .any(|(_, relics)| {
                relics
                    .iter()
                    .any(|&relic| !dealt.contains(&relic) && allowed_now::<V>(relic, floor))
            });
        if !left {
            unimplemented!("a relic bag with nothing left to deal");
        }
        left
    }

    /// The body both ends share: walk the rarities, then read one deque.
    fn pull(
        &self,
        rarity: Rarity<V>,
        dealt: &[V::Relic],
        floor: usize,
        allowed: impl Fn(V::Relic) -> bool,
        from_back: bool,
    ) -> V::Relic {
        let mut rarity = rarity;
        loop {
            let held = self.deques.get(&rarity).map(|relics| {
                let left = relics.iter().copied().filter(|&relic| {
                    !dealt.contains(&relic) && allowed_now::<V>(relic, floor) && allowed(relic)
                });
                match from_back {
                    true => left.last(),
                    false => left.take(1).last(),
                }
            });
            if let Some(Some(relic)) = held {
                return relic;
            }
            rarity = match rarity {
                r if r == <Rarity<V> as RarityKinds>::SHOP => <Rarity<V> as RarityKinds>::COMMON,
                r if r == <Rarity<V> as RarityKinds>::COMMON => {
                    <Rarity<V> as RarityKinds>::UNCOMMON
                }
                r if r == <Rarity<V> as RarityKinds>::UNCOMMON => <Rarity<V> as RarityKinds>::RARE,
                _ => unimplemented!("a relic bag with nothing left to deal"),
            };
        }
    }

    /// Every relic a pool holds, in the pool's own order.
    fn pool(pool: Pool<V>) -> Vec<V::Relic> {
        V::Relic::members(pool).to_vec()
    }

    /// Group by rarity and shuffle each group, which is the body both
    /// overloads share.
    fn populate<R: GameRng>(rng: &mut R, relics: Vec<V::Relic>, drafted_only: bool) -> Self {
        let mut deques: IndexMap<Rarity<V>, Vec<V::Relic>> = IndexMap::new();
        for relic in relics {
            let rarity = relic.rarity();
            if !drafted_only || Rarity::<V>::DRAFTED.contains(&rarity) {
                deques.entry(rarity).or_default().push(relic);
            }
        }

        for relics in deques.values_mut() {
            rng.unstable_shuffle(relics);
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
    /// `RoomSet.SecondBoss`, which the last act holds at Double Boss and
    /// fights once its first boss is done.
    pub second_boss: Option<V::Encounter>,
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
            second_boss: None,
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
    rng.choose(items).ok().copied()
}

/// The acts a run is played through, in order: `ActModel.GetRandomList`
/// over `UnlockState.all`, off the `act_selection` stream
/// `StartRunLobby.BeginRunLocally` seeds from the run's seed.
///
/// Each place takes one of the acts that can fill it, in table order, so the
/// first is Overgrowth or Underdocks. Outside `TestMode` the game first sends
/// a player to an act their profile has not met, and an unlocked run has met
/// them all.
pub fn run_acts<V: Rules>(raw_seed: u64) -> Vec<V::Act> {
    let mut rng: crate::rng::MegaRng = sts2_core::rng::named::<V, _>(raw_seed, "act_selection");
    let mut acts = Vec::new();
    for place in 0.. {
        let fill: Vec<V::Act> = (V::Act::ALL.iter().copied())
            .filter(|act| act.index() == Some(place))
            .collect();
        if fill.is_empty() {
            break;
        }
        acts.push(next_item(&mut rng, &fill).expect("a place some act fills"));
    }
    acts
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
    /// act list in order, which [`run_acts`] draws.
    pub fn draw<R: GameRng>(
        rng: &mut R,
        character: V::Character,
        acts: &[V::Act],
        ascension: V::AscensionLevel,
    ) -> Self {
        let shared_relics = RelicBag::shared(rng);
        let player_relics = RelicBag::player(rng, character);

        // The run's shared ancients, shuffled once and then dealt out. Each act
        // after the first takes a share, and how big a share is itself a draw.
        let mut pool: Vec<V::Event> = V::shared_ancients().to_vec();
        rng.unstable_shuffle(&mut pool);

        let mut dealt: Vec<Vec<V::Event>> = vec![Vec::new()];
        for _ in acts.iter().skip(1) {
            let count =
                num_traits::cast(pool.len() + 1).expect("an ancient pool fits the RNG integer");
            let bound = Bound::try_new(count).expect("a count plus one is positive");
            let taken: usize = num_traits::cast(rng.next_int(bound))
                .expect("a bounded draw is a valid ancient index");
            dealt.push(pool.drain(..taken).collect());
        }

        let last = acts.len() - 1;
        let double_boss = sts2_core::AscensionKinds::has(
            ascension,
            <V::AscensionLevel as sts2_core::AscensionKinds>::DOUBLE_BOSS,
        );
        let rooms = acts
            .iter()
            .zip(&dealt)
            .enumerate()
            .map(|(at, (&act, share))| {
                let mut ancients: Vec<V::Event> = V::ancients(act).to_vec();
                ancients.extend(share);
                let mut rooms = Rooms::roll(rng, act, &ancients);
                // `SetSecondBossEncounter`, off the same stream once the last
                // act's rooms are rolled: another of its bosses.
                if at == last && double_boss {
                    let others: Vec<V::Encounter> =
                        Rooms::<V>::of_room(act, V::RoomType::BOSS, false)
                            .into_iter()
                            .filter(|&boss| boss != rooms.boss)
                            .collect();
                    rooms.second_boss =
                        Some(next_item(rng, &others).expect("a last act fields a second boss"));
                }
                (act, rooms)
            })
            .collect();

        Opening {
            shared_relics,
            player_relics,
            acts: rooms,
        }
    }
}
