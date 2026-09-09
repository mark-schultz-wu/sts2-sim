//! Which node type each point gets.
//!
//! Three rows are stamped and never rewritten. The row a path starts on is all
//! monsters. The seventh from the end is all treasure, and the last is all rest
//! sites. The remaining rest sites, shops, elites and question marks are dealt
//! out over three passes. Every point still bare at the end becomes a monster.
//!
//! A type is only dealt where five rules allow it. They keep rest sites out of
//! the lower map and off its top rows. They also stop a type sitting twice in a
//! row or side by side.

use std::collections::VecDeque;

use sts2_core::rng::{GameRng, Interval, Span};

use sts2_core::{AscensionKinds, MapPointKinds, Rules};

use super::{ActMapBuilder, Id};

/// How many of each type an act's map holds.
///
/// Elites and shops are the same in every act. The other two are rolled from
/// the map's own stream, which is why the numbers are taken before any path is
/// walked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) unknowns: usize,
    pub(crate) rests: usize,
    pub(crate) elites: usize,
    pub(crate) shops: usize,
}

/// How one act rolls the rest sites on its map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rests {
    /// `NextGaussianInt(mean, 1, min, max)`, which three of the four acts use.
    Around { mean: i32, min: i32, max: i32 },
    /// `NextInt(min, max)`, which the last act uses.
    Between { min: i32, max: i32 },
}

/// What one act's map is made of, before any of it is rolled.
///
/// The four acts differ in three numbers and in nothing else, so the generator
/// takes them rather than knowing about acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// `BaseNumberOfRooms`, which is one less than the grid's height.
    pub rooms: usize,
    pub rests: Rests,
    /// Taken off the standard question mark roll. Zero in the first act.
    pub fewer_unknowns: i32,
}

impl Layout {
    /// The first act, which is the Underdocks or the Overgrowth.
    ///
    /// The two are the same map: fifteen rooms, rest sites around seven, and
    /// the standard question mark roll untouched.
    pub const ACT_ONE: Layout = Layout {
        rooms: 15,
        rests: Rests::Around {
            mean: 7,
            min: 6,
            max: 7,
        },
        fewer_unknowns: 0,
    };
}

impl Counts {
    /// Roll an act's numbers, which is `ActModel.GetMapPointTypes` together
    /// with `MapPointTypeCounts`.
    ///
    /// Three shops always. Five elites, or eight from the first rung of the
    /// ascension ladder upwards. Nothing else on the ladder reaches a map, and
    /// `tests/maps.txt` holds every rung for ten seeds to say so.
    pub(crate) fn roll<R: GameRng, A: AscensionKinds>(
        rng: &mut R,
        layout: Layout,
        ascension: A,
    ) -> Self {
        let rests = match layout.rests {
            Rests::Around { mean, min, max } => {
                let accept = Interval::new(min, max).expect("an act's rest range is ordered");
                rng.next_gaussian_int(mean, 1, accept)
            }
            Rests::Between { min, max } => {
                let span = Span::new(min, max).expect("an act's rest range holds something");
                rng.next_int_range(span)
            }
        };
        let unknowns = standard_unknowns(rng) - layout.fewer_unknowns;
        Counts {
            unknowns: unknowns.max(0) as usize,
            rests: rests.max(0) as usize,
            elites: elites(ascension),
            shops: 3,
        }
    }
}

/// How many elites a map holds, which is `MapPointTypeCounts.NumOfElites`.
///
/// The game multiplies in `f32` and rounds the result, and this does the same
/// rather than writing the eight down. The product is a hair over eight, so
/// which way the rounding goes is not something to leave to a comment.
fn elites<A: AscensionKinds>(ascension: A) -> usize {
    let scale: f32 = if ascension.has(A::SWARMING_ELITES) {
        1.6
    } else {
        1.0
    };
    (f64::from(5.0f32 * scale)).round_ties_even() as usize
}

/// `MapPointTypeCounts.StandardRandomUnknownCount`, which every act starts
/// from.
fn standard_unknowns<R: GameRng>(rng: &mut R) -> i32 {
    let accept = Interval::new(10, 14).expect("ten is below fourteen");
    rng.next_gaussian_int(12, 1, accept)
}

impl<V: Rules> ActMapBuilder<V> {
    /// Stamp the fixed rows and deal out the rest, which is
    /// `AssignPointTypes`.
    ///
    /// The treasure row becomes elites under the Warden ascension, which this
    /// does not offer yet.
    pub(super) fn assign_types<R: GameRng>(&mut self, rng: &mut R) {
        self.stamp(self.rows - 1, V::MapPointType::REST_SITE);
        self.stamp(self.rows - 7, V::MapPointType::TREASURE);
        self.stamp(1, V::MapPointType::MONSTER);

        let mut queue: VecDeque<V::MapPointType> = VecDeque::new();
        for (count, kind) in [
            (self.counts.rests, V::MapPointType::REST_SITE),
            (self.counts.shops, V::MapPointType::SHOP),
            (self.counts.elites, V::MapPointType::ELITE),
            (self.counts.unknowns, V::MapPointType::UNKNOWN),
        ] {
            queue.extend(std::iter::repeat_n(kind, count));
        }
        self.deal(rng, &mut queue);

        for id in self.live().collect::<Vec<_>>() {
            if self.points[id].kind == V::MapPointType::UNASSIGNED {
                self.points[id].kind = V::MapPointType::MONSTER;
            }
        }
        self.points[self.boss].kind = V::MapPointType::BOSS;
        self.points[self.ancient].kind = V::MapPointType::ANCIENT;
    }

    /// One row given one type, and closed to anything later.
    fn stamp(&mut self, row: usize, kind: V::MapPointType) {
        for id in self.row(row) {
            self.points[id].kind = kind;
            self.points[id].modifiable = false;
        }
    }

    /// Three passes over the bare points, which is
    /// `AssignRemainingTypesToRandomPoints`.
    ///
    /// A pass takes what is left of the queue in a shuffled order. Three
    /// because a type the rules refuse everywhere on one pass finds room once
    /// its neighbours are settled.
    fn deal<R: GameRng>(&mut self, rng: &mut R, queue: &mut VecDeque<V::MapPointType>) {
        for _ in 0..3 {
            if queue.is_empty() {
                break;
            }
            let mut bare: Vec<Id> = self
                .live()
                .filter(|&id| self.points[id].kind == V::MapPointType::UNASSIGNED)
                .collect();
            self.shuffle_points(rng, &mut bare);
            for id in bare {
                if queue.is_empty() {
                    break;
                }
                self.points[id].kind = self.next_valid(queue, id);
            }
        }
    }

    /// A stable shuffle of points, which sorts by column then row first.
    ///
    /// The game sorts by `MapPoint.CompareTo`, which compares the coordinate.
    /// Sorting the indices themselves gives another order, so the sort is
    /// spelled out here rather than left to `stable_shuffle`.
    pub(crate) fn shuffle_points<R: GameRng>(&self, rng: &mut R, points: &mut [Id]) {
        points.sort_by_key(|&id| (self.points[id].col, self.points[id].row));
        rng.unstable_shuffle(points);
    }

    /// The first type in the queue the rules allow here, which is
    /// `GetNextValidPointType`.
    ///
    /// One rotation of the queue. A type the rules refuse goes to the back, and
    /// a queue with nothing that fits leaves the point bare for a later pass.
    fn next_valid(&self, queue: &mut VecDeque<V::MapPointType>, id: Id) -> V::MapPointType {
        for _ in 0..queue.len() {
            let Some(kind) = queue.pop_front() else {
                break;
            };
            if self.allows(kind, id) {
                return kind;
            }
            queue.push_back(kind);
        }
        V::MapPointType::UNASSIGNED
    }

    /// Whether the five placement rules allow a type here, which is
    /// `IsValidPointType`.
    pub(crate) fn allows(&self, kind: V::MapPointType, id: Id) -> bool {
        let point = &self.points[id];

        // No rest sites on the last three rows, and neither rest sites nor
        // elites below row six.
        if point.row + 3 >= self.rows && kind == V::MapPointType::REST_SITE {
            return false;
        }
        if point.row < 6 && [V::MapPointType::REST_SITE, V::MapPointType::ELITE].contains(&kind) {
            return false;
        }

        // Never twice in a row. The game keeps two lists here and they hold
        // the same four types, so it checks the children twice.
        let never_twice = [
            V::MapPointType::ELITE,
            V::MapPointType::REST_SITE,
            V::MapPointType::TREASURE,
            V::MapPointType::SHOP,
        ];
        if never_twice.contains(&kind) {
            let touching = point.parents.iter().chain(point.children.iter());
            if touching
                .map(|other| self.points[other].kind)
                .any(|k| k == kind)
            {
                return false;
            }
        }

        // Never side by side. Treasure is not in this list, which is how a
        // whole row of it is allowed.
        let never_beside = [
            V::MapPointType::REST_SITE,
            V::MapPointType::MONSTER,
            V::MapPointType::UNKNOWN,
            V::MapPointType::ELITE,
            V::MapPointType::SHOP,
        ];
        if never_beside.contains(&kind) {
            return !self
                .siblings(id)
                .into_iter()
                .any(|other| self.points[other].kind == kind);
        }
        true
    }

    /// Every other child of this point's parents, which is `GetSiblings`.
    fn siblings(&self, id: Id) -> Vec<Id> {
        self.points[id]
            .parents
            .iter()
            .flat_map(|parent| self.points[parent].children.iter().collect::<Vec<_>>())
            .filter(|&other| other != id)
            .collect()
    }
}
