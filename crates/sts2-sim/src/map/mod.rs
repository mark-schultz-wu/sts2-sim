//! The act's map, generated the way the game generates it.
//!
//! A port of `StandardActMap`, which every act but the Spoils uses. Seven
//! paths are walked from the second row to the last. The rows a run cannot
//! choose are stamped, and the rest of the node types are dealt out. Duplicate
//! stretches are pruned away, and the survivors are nudged apart for looks.
//!
//! Every stage draws from one stream named `act_<n>_map`, seeded from the run's
//! seed. So the whole map follows from the seed, and a draw taken in the wrong
//! order moves every node after it.
//!
//! `sts2-core`'s `Coord` is what the observation speaks in. Inside here a point
//! is an index into `points`, because the game holds references: a node keeps
//! its identity while post-processing moves it between columns.

mod prune;
mod set;
mod tidy;
mod types;

use sts2_core::rng::{GameRng, Span};
use sts2_core::{Coord, MapPointKinds, Rules};

use set::Order;
pub use types::{Layout, Rests};

/// Why the simulator failed to represent an act map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A point identity needs more than the seven bits reserved for it.
    #[error("map point {id} does not fit in seven bits")]
    PointIdTooLarge { id: usize },
    /// A segment needs more than the 17 identities that fit in its packed path.
    #[error("map segment of length {length} does not fit in 128 bits")]
    SegmentTooLong { length: usize },
    /// Nothing stands where a caller asked. A map's own coordinates all
    /// answer, so this is a run reading a coordinate off some other map.
    #[error("this act's map has no point at {at}")]
    NoPointAt { at: Coord },
}

/// The grid is this wide in every act.
pub(crate) const COLS: usize = 7;

/// How many paths are walked, which is also how wide the grid is.
const PATHS: usize = 7;

/// Which point this is. An index into [`ActMap::points`], never reused.
pub(crate) type Id = usize;

/// One node: where it sits, what it is, and what it leads to.
///
/// `col` moves during post-processing and `row` never does. Both are held as
/// numbers rather than as a `Coord`, because the generator does arithmetic on
/// them and a negative intermediate is normal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Point<V: Rules> {
    pub(crate) col: usize,
    pub(crate) row: usize,
    pub(crate) kind: V::MapPointType,
    /// False for the rows the generator stamped, which nothing later rewrites.
    pub(crate) modifiable: bool,
    pub(crate) parents: Order<Id>,
    pub(crate) children: Order<Id>,
}

impl<V: Rules> Point<V> {
    fn new(col: usize, row: usize) -> Self {
        Point {
            col,
            row,
            kind: V::MapPointType::UNASSIGNED,
            modifiable: true,
            parents: Order::new(),
            children: Order::new(),
        }
    }

    pub(crate) fn at(&self) -> Coord {
        Coord::new(self.col as u8, self.row as u8)
    }
}

/// A generated act map.
///
/// The grid holds the rows a path can pass through. The first node and the
/// boss sit outside it. The first is above the grid's row zero, and the boss a
/// row past its end. Both are still points, and the rest of the map links to
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActMap<V: Rules> {
    pub(crate) points: Vec<Point<V>>,
    /// `COLS` columns by [`ActMap::rows`] rows, column major, as the game's is.
    grid: Vec<Option<Id>>,
    /// One more than the act's room count, which is the grid's height.
    rows: usize,
    starts: Order<Id>,
    boss: Id,
    ancient: Id,
    counts: types::Counts,
}

impl<V: Rules> ActMap<V> {
    /// Generate one act's map.
    ///
    /// `rng` is the act's own `act_<n>_map` stream, already seeded and at its
    /// start. It is drawn from throughout, and the caller keeps it.
    ///
    /// `ascension` reaches only the elite count. Every other rung of the ladder
    /// leaves a map alone, which `tests/maps.txt` checks rather than assumes.
    pub fn generate<R: GameRng>(
        rng: &mut R,
        layout: Layout,
        ascension: V::AscensionLevel,
    ) -> Result<Self, Error> {
        let rows = layout.rooms + 1;
        let counts = types::Counts::roll(rng, layout, ascension);
        let mut map = ActMap {
            points: Vec::new(),
            grid: vec![None; COLS * rows],
            rows,
            starts: Order::new(),
            boss: 0,
            ancient: 0,
            counts,
        };

        // Both sit outside the grid, so they are made rather than placed. The
        // boss is a row past the last, and the first node a row before the
        // first one a path reaches.
        map.boss = map.add(COLS / 2, rows);
        map.ancient = map.add(COLS / 2, 0);

        map.walk_paths(rng);
        map.assign_types(rng);
        prune::prune_and_repair(&mut map, rng)?;
        tidy::center(&mut map);
        tidy::spread(&mut map);
        tidy::straighten(&mut map);
        Ok(map)
    }

    /// Every node the run can stand on, in the observation's own order.
    ///
    /// Row then column, which is how the game's own reporting sorts them. The
    /// first node and the boss are in here, unlike in `GetAllMapPoints`.
    pub fn nodes(&self) -> Vec<(Coord, V::MapPointType, Vec<Coord>)> {
        let mut nodes: Vec<(Coord, V::MapPointType, Vec<Coord>)> = self
            .live()
            .chain([self.ancient, self.boss])
            .map(|id| {
                let point = &self.points[id];
                let mut to: Vec<Coord> = point
                    .children
                    .iter()
                    .map(|child| self.points[child].at())
                    .collect();
                to.sort_by_key(|at| (at.row, at.col));
                (point.at(), point.kind, to)
            })
            .collect();
        nodes.sort_by_key(|(at, _, _)| (at.row, at.col));
        nodes
    }

    /// Where the run stands before it has moved anywhere.
    pub fn ancient(&self) -> Coord {
        self.points[self.ancient].at()
    }

    /// The point standing on a coordinate, which is the map's index.
    ///
    /// The grid answers for every point a path runs through. The first node
    /// and the boss sit outside it, so they answer for themselves. Row zero
    /// holds the first node alone, because a path starts at row one.
    ///
    /// A coordinate this map does not hold came off some other map. That is
    /// an error, not an empty answer.
    pub(crate) fn at(&self, at: Coord) -> Result<&Point<V>, Error> {
        for id in [self.ancient, self.boss] {
            if self.points[id].at() == at {
                return Ok(&self.points[id]);
            }
        }
        self.cell(at.col as usize, at.row as usize)
            .map(|id| &self.points[id])
            .ok_or(Error::NoPointAt { at })
    }

    /// A new point, off the grid. The caller places it if it belongs on one.
    fn add(&mut self, col: usize, row: usize) -> Id {
        self.points.push(Point::new(col, row));
        self.points.len() - 1
    }

    /// The point at a cell, or a new one placed there.
    fn get_or_create(&mut self, col: usize, row: usize) -> Id {
        if let Some(id) = self.cell(col, row) {
            return id;
        }
        let id = self.add(col, row);
        self.grid[col * self.rows + row] = Some(id);
        id
    }

    /// What the grid holds at a cell, which is `Grid[col, row]`.
    ///
    /// `None` off the grid as well as at an empty cell. The game indexes an
    /// array and never asks out of range. The two callers that come close
    /// check whether a point was pruned away.
    pub(crate) fn cell(&self, col: usize, row: usize) -> Option<Id> {
        if col >= COLS || row >= self.rows {
            return None;
        }
        self.grid[col * self.rows + row]
    }

    pub(crate) fn put(&mut self, col: usize, row: usize, id: Option<Id>) {
        self.grid[col * self.rows + row] = id;
    }

    /// Every point still on the grid, column major, which is
    /// `GetAllMapPoints`.
    ///
    /// Neither the first node nor the boss is among them, and neither is a
    /// point that pruning removed.
    pub(crate) fn live(&self) -> impl Iterator<Item = Id> + '_ {
        (0..COLS).flat_map(move |col| (0..self.rows).filter_map(move |row| self.cell(col, row)))
    }

    /// Every point in one row, left to right, which is `ForEachInRow`.
    pub(crate) fn row(&self, row: usize) -> Vec<Id> {
        (0..COLS).filter_map(|col| self.cell(col, row)).collect()
    }

    pub(crate) fn link(&mut self, parent: Id, child: Id) {
        self.points[parent].children.insert(child);
        self.points[child].parents.insert(parent);
    }

    pub(crate) fn unlink(&mut self, parent: Id, child: Id) {
        self.points[parent].children.remove(child);
        self.points[child].parents.remove(parent);
    }

    /// Cut a point out of the map, which is `RemovePoint`.
    pub(crate) fn cut(&mut self, id: Id) {
        let (col, row) = (self.points[id].col, self.points[id].row);
        self.put(col, row, None);
        self.starts.remove(id);
        for child in self.points[id].children.iter().collect::<Vec<_>>() {
            self.unlink(id, child);
        }
        for parent in self.points[id].parents.iter().collect::<Vec<_>>() {
            self.unlink(parent, id);
        }
    }

    /// Whether a point still holds its cell, which is pruning's `IsRemoved`
    /// read the other way up.
    ///
    /// False for the first node and for the boss, which never held one.
    pub(crate) fn on_grid(&self, id: Id) -> bool {
        let point = &self.points[id];
        self.cell(point.col, point.row) == Some(id)
    }

    /// Whether a point is still reachable, which is pruning's `IsInMap`.
    ///
    /// The first node and the boss are never on the grid and are always in the
    /// map.
    pub(crate) fn in_map(&self, id: Id) -> bool {
        let point = &self.points[id];
        self.cell(point.col, point.row).is_some()
            || point.kind == V::MapPointType::ANCIENT
            || point.kind == V::MapPointType::BOSS
    }

    /// Seven paths from the second row to the last, and the ends joined on.
    ///
    /// The second of the seven is redrawn until it starts somewhere the first
    /// did not, so a map always offers at least two openings.
    fn walk_paths<R: GameRng>(&mut self, rng: &mut R) {
        let span = Span::new(0, COLS as i32).expect("seven columns is not an empty span");
        for path in 0..PATHS {
            let mut start = self.get_or_create(rng.next_int_range(span) as usize, 1);
            if path == 1 {
                while self.starts.contains(start) {
                    start = self.get_or_create(rng.next_int_range(span) as usize, 1);
                }
            }
            self.starts.insert(start);
            self.walk(rng, start);
        }

        for id in self.row(self.rows - 1) {
            self.link(id, self.boss);
        }
        for id in self.row(1) {
            self.link(self.ancient, id);
        }
    }

    /// One path, a row at a time, which is `PathGenerate`.
    fn walk<R: GameRng>(&mut self, rng: &mut R, from: Id) {
        let mut at = from;
        while self.points[at].row < self.rows - 1 {
            let (col, row) = self.next_coord(rng, at);
            let next = self.get_or_create(col, row);
            self.link(at, next);
            at = next;
        }
    }

    /// Where a path goes next, which is `GenerateNextCoord`.
    ///
    /// The three sideways steps are shuffled after being put in order, so the
    /// order they were built in cannot matter. The first that does not cross an
    /// existing path wins.
    fn next_coord<R: GameRng>(&self, rng: &mut R, at: Id) -> (usize, usize) {
        let col = self.points[at].col;
        let left = col.saturating_sub(1);
        let right = (col + 1).min(COLS - 1);

        let mut steps = [-1i32, 0, 1];
        rng.stable_shuffle(&mut steps);

        for step in steps {
            let target = match step {
                -1 => left,
                0 => col,
                _ => right,
            };
            if !self.crosses(at, target) {
                return (target, self.points[at].row + 1);
            }
        }
        unreachable!("a path always has somewhere to go: one of three steps stays put")
    }

    /// Whether stepping sideways to `target` crosses a path going the other
    /// way, which is `HasInvalidCrossover`.
    ///
    /// Two paths that swap columns between the same pair of rows draw as an X.
    /// The game forbids that, and lets the straight-ahead step through without
    /// looking.
    fn crosses(&self, at: Id, target: usize) -> bool {
        let point = &self.points[at];
        let step = target as i32 - point.col as i32;
        if step == 0 {
            return false;
        }
        let Some(neighbour) = self.cell(target, point.row) else {
            return false;
        };
        self.points[neighbour]
            .children
            .iter()
            .any(|child| self.points[child].col as i32 - self.points[neighbour].col as i32 == -step)
    }
}

#[cfg(test)]
mod tests {
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::ascension::AscensionLevel;
    use sts2_core::rng::{MegaRng, stream_seed};
    use sts2_core::{ContentVersion, Seed};

    use super::*;

    type V = V0_107_1;

    fn map() -> ActMap<V> {
        let seed = Seed::new("0RACLE1").expect("the fixture's seed is a seed");
        let mut rng = MegaRng::named(stream_seed(&seed), "act_1_map");
        ActMap::<V>::generate(&mut rng, Layout::ACT_ONE, AscensionLevel::None)
            .expect("the fixture's map generates")
    }

    /// Every node the map reports is a node the map can find again.
    ///
    /// The first node and the boss are the cases worth the test. Neither sits
    /// on the grid, so an index that only reads cells misses both.
    #[test]
    fn the_index_finds_every_node_the_walk_built() {
        let map = map();
        let nodes = map.nodes();
        assert!(!nodes.is_empty(), "the fixture's map has nodes");
        for (at, kind, _) in nodes {
            let point = map.at(at).unwrap_or_else(|_| panic!("no point at {at}"));
            assert_eq!(point.kind, kind, "at {at}");
            assert_eq!(point.at(), at);
        }
        assert_eq!(
            map.at(map.ancient()).expect("an act opens somewhere").kind,
            <V as ContentVersion>::MapPointType::ANCIENT
        );
    }

    /// A coordinate off this map is an error, not an empty answer.
    #[test]
    fn the_index_refuses_a_coordinate_it_does_not_hold() {
        let map = map();
        let off = Coord::new(0, 0);
        assert_eq!(map.at(off), Err(Error::NoPointAt { at: off }));
    }
}
