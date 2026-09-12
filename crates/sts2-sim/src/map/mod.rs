//! The act's map, generated the way the game generates it.
//!
//! A port of `ActMapBuilder`, which every act but the Spoils uses. Seven
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

use std::sync::Arc;

use sts2_core::rng::{GameRng, RngInteger, Span};
use sts2_core::{Coord, MapPointKinds, NoNode, Node, Rules};

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
}

/// The grid is this wide in every act.
pub(crate) const COLS: usize = 7;

/// How many paths are walked, which is also how wide the grid is.
const PATHS: usize = 7;

/// Which point this is. An index into `ActMapBuilder::points`, never reused.
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

/// One act's map, as a run holds it.
///
/// The act's nodes, settled when it was generated and unchanged while it is
/// played. Cloning shares them, so every branch and every report reads one
/// copy.
///
/// Held as the shape a report asks for. That is convenience, not a promise.
/// A run needing what a [`Node`] lacks changes what is behind these
/// accessors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActMap<V: Rules> {
    /// In the observation's order, which is what lets [`ActMap::at`] search.
    nodes: Arc<[Node<V>]>,
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
        Ok(ActMapBuilder::<V>::generate(rng, layout, ascension)?.into())
    }

    /// The act's nodes, to hand to a report that outlives this borrow.
    pub fn shared(&self) -> Arc<[Node<V>]> {
        Arc::clone(&self.nodes)
    }
}

/// A map is what a finished builder holds, once its order is settled.
impl<V: Rules> From<ActMapBuilder<V>> for ActMap<V> {
    fn from(built: ActMapBuilder<V>) -> Self {
        ActMap {
            nodes: built.nodes().into(),
        }
    }
}

impl<V: Rules> sts2_core::ActMap<V> for ActMap<V> {
    fn nodes(&self) -> &[Node<V>] {
        &self.nodes
    }

    /// The nodes are in the observation's order, so this searches them. The
    /// trait walks them instead.
    fn at(&self, at: Coord) -> Result<&Node<V>, NoNode> {
        self.nodes
            .binary_search_by_key(&(at.row, at.col), |node| (node.at.row, node.at.col))
            .map(|found| &self.nodes[found])
            .map_err(|_| NoNode { at })
    }
}

/// A map being generated, which is the port of the game's own class.
///
/// The grid holds the rows a path can pass through. The first node and the
/// boss sit outside it. The first is above the grid's row zero, and the boss a
/// row past its end. Both are still points, and the map links to them.
///
/// Generation's own working shape. A run holds the [`ActMap`] this turns into.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActMapBuilder<V: Rules> {
    pub(crate) points: Vec<Point<V>>,
    /// `COLS` columns by `ActMapBuilder::rows` rows, column major, as the game's is.
    grid: Vec<Option<Id>>,
    /// One more than the act's room count, which is the grid's height.
    rows: usize,
    starts: Order<Id>,
    boss: Id,
    ancient: Id,
    counts: types::Counts,
}

impl<V: Rules> ActMapBuilder<V> {
    /// Walk, deal, prune and tidy one act's map.
    fn generate<R: GameRng>(
        rng: &mut R,
        layout: Layout,
        ascension: V::AscensionLevel,
    ) -> Result<Self, Error> {
        let rows = layout.rooms + 1;
        let counts = types::Counts::roll(rng, layout, ascension);
        let mut map = ActMapBuilder {
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
    /// first node and the boss are in here, unlike in `GetAllMapPoints`. This
    /// is the only place that order is decided.
    fn nodes(&self) -> Vec<Node<V>> {
        let mut nodes: Vec<Node<V>> = self
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
                Node {
                    at: point.at(),
                    kind: point.kind,
                    to,
                }
            })
            .collect();
        nodes.sort_by_key(|node| (node.at.row, node.at.col));
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
        let columns =
            num_traits::cast(COLS).unwrap_or_else(|| panic!("the map width fits the RNG integer"));
        let span = Span::new(R::Int::ZERO, columns).expect("seven columns is not an empty span");
        for path in 0..PATHS {
            let column = num_traits::cast(rng.next_int_range(span))
                .unwrap_or_else(|| unreachable!("a map-column draw is a valid index"));
            let mut start = self.get_or_create(column, 1);
            if path == 1 {
                while self.starts.contains(start) {
                    let column = num_traits::cast(rng.next_int_range(span))
                        .unwrap_or_else(|| unreachable!("a map-column draw is a valid index"));
                    start = self.get_or_create(column, 1);
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
    use crate::rng::MegaRng;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::ascension::AscensionLevel;
    use sts2_core::ActMap as _;
    use sts2_core::Seed;
    use sts2_core::rng::stream_seed;

    use super::*;

    type V = V0_107_1;
    type MapPointType = <V as sts2_core::ContentVersion>::MapPointType;

    fn map() -> ActMap<V> {
        let seed = Seed::new("0RACLE1").expect("the fixture's seed is a seed");
        let mut rng = MegaRng::named(stream_seed(&seed), "act_1_map");
        ActMap::<V>::generate(&mut rng, Layout::ACT_ONE, AscensionLevel::None)
            .expect("the fixture's map generates")
    }

    /// The nodes are in the order a report lists them.
    ///
    /// [`ActMap::at`] searches rather than scans, which needs that order. A
    /// search over an unsorted slice finds nothing, and this map then reports
    /// no such node.
    #[test]
    fn the_nodes_are_in_the_order_a_report_lists_them() {
        let map = map();
        let nodes = map.nodes();
        assert!(nodes.len() > 1, "an act map has nodes");
        assert!(
            nodes
                .windows(2)
                .all(|pair| (pair[0].at.row, pair[0].at.col) < (pair[1].at.row, pair[1].at.col)),
            "the nodes are not sorted by row and then column"
        );
    }

    /// Every node the map reports is one it can find again.
    ///
    /// The first node and the boss are the cases worth the test. Neither sits
    /// on the grid, so a map built from grid cells alone loses both.
    #[test]
    fn the_map_finds_every_node_it_holds() {
        let map = map();
        for node in map.nodes() {
            let found = map
                .at(node.at)
                .unwrap_or_else(|_| panic!("no node at {}", node.at));
            assert_eq!(found, node);
        }
        let kinds: Vec<_> = map.nodes().iter().map(|node| node.kind).collect();
        for kind in [MapPointType::ANCIENT, MapPointType::BOSS] {
            assert!(kinds.contains(&kind), "no {kind:?} among the act's nodes");
        }
    }

    #[test]
    fn a_map_opens_on_an_ancient() {
        let map = map();
        let first = map.ancient().expect("an act opens somewhere");
        assert_eq!(first.kind, MapPointType::ANCIENT);
        assert_eq!(
            map.at(first.at).expect("the first node is on the map"),
            first
        );
    }

    /// A coordinate off the map is an error. The search says so, and so does
    /// the walk the trait writes.
    #[test]
    fn a_coordinate_off_the_map_is_an_error() {
        let off = Coord::new(0, 0);
        let map = map();
        assert_eq!(map.at(off), Err(NoNode { at: off }));
        assert_eq!(map.nodes().at(off), Err(NoNode { at: off }));
    }
}
