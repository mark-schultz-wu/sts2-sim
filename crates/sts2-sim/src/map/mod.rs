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
use sts2_core::{Coord, MapPoints, Rules};

use set::Order;
pub use types::{Layout, Rests};

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
    pub fn generate<R: GameRng>(rng: &mut R, layout: Layout, ascension: V::AscensionLevel) -> Self {
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
        prune::prune_and_repair(&mut map, rng);
        tidy::center(&mut map);
        tidy::spread(&mut map);
        tidy::straighten(&mut map);
        map
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
