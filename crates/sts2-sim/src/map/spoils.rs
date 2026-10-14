//! `SpoilsActMap`, the map the Spoils Map card puts in place of its act's.
//!
//! Seven paths, as the standard map walks, but every path is drawn to one
//! treasure in the middle column, seven rows from the top. The rows around it
//! narrow to meet it, so the map reads as an hourglass. Types are dealt the
//! standard way, and pruning runs as it does there. Nothing is tidied after.
//!
//! Everything draws from one stream named `spoils_map`, seeded from the run's
//! seed alone.

use sts2_core::rng::{GameRng, RngInteger, Span};
use sts2_core::{Layout, MapPointKinds, Rules};

use super::set::Order;
use super::{ActMapBuilder, COLS, Error, Id, PATHS, prune};

/// The middle column, which the treasure sits in.
const MIDDLE: usize = COLS / 2;

impl<V: Rules> ActMapBuilder<V> {
    /// Walk, deal and prune a Spoils map, which is `SpoilsActMap`'s
    /// constructor.
    pub(super) fn spoils<R: GameRng>(
        rng: &mut R,
        layout: Layout,
        ascension: V::AscensionLevel,
        recount: Option<super::Recount>,
    ) -> Result<Self, Error> {
        let rows = layout.rooms + 1;
        let counts = super::counts(rng, layout, ascension, recount);
        let mut map = ActMapBuilder {
            points: Vec::new(),
            grid: vec![None; COLS * rows],
            rows,
            starts: Order::new(),
            boss: 0,
            ancient: 0,
            second_boss: None,
            counts,
        };
        map.boss = map.add(MIDDLE, rows);
        map.ancient = map.add(MIDDLE, 0);

        map.hourglass(rng);
        map.assign_spoils_types(rng);
        prune::prune_and_repair(&mut map, rng)?;
        Ok(map)
    }

    /// The row the treasure sits on, which is `_treasureRow`.
    fn treasure_row(&self) -> usize {
        self.rows - 7
    }

    /// `GenerateHourglassMap`: seven paths, then every point on the treasure
    /// row folded into the one in the middle, then the ends joined on.
    fn hourglass<R: GameRng>(&mut self, rng: &mut R) {
        let columns = num_traits::cast(COLS).expect("the map width fits the RNG integer");
        let span = Span::new(R::Int::ZERO, columns).expect("seven columns is not an empty span");
        let draw = |rng: &mut R| -> usize {
            num_traits::cast(rng.next_int_range(span)).expect("a map-column draw is a valid index")
        };
        for path in 0..PATHS {
            let mut start = self.get_or_create(draw(rng), 1);
            if path == 1 {
                while self.starts.contains(start) {
                    start = self.get_or_create(draw(rng), 1);
                }
            }
            self.starts.insert(start);
            self.walk_to_treasure(rng, start);
        }

        let treasure = self.get_or_create(MIDDLE, self.treasure_row());
        self.points[treasure].kind = V::MapPointType::TREASURE;
        self.points[treasure].modifiable = false;
        for stray in self.row(self.treasure_row()) {
            if stray != treasure {
                self.redirect(stray, treasure);
            }
        }

        for id in self.row(self.rows - 1) {
            self.link(id, self.boss);
        }
        for id in self.row(1) {
            self.link(self.ancient, id);
        }
    }

    /// One path, a row at a time, which is `PathGenerate`.
    fn walk_to_treasure<R: GameRng>(&mut self, rng: &mut R, from: Id) {
        let mut at = from;
        while self.points[at].row < self.rows - 1 {
            let (col, row) = self.next_toward_treasure(rng, at);
            let next = self.get_or_create(col, row);
            self.link(at, next);
            at = next;
        }
    }

    /// Where a path goes next, which is `GenerateNextCoord`.
    ///
    /// Far from the treasure row the three steps are shuffled. Within three
    /// rows above it, the step toward the middle comes first. A step must
    /// stay inside the row's allowed columns, cross nothing, and join a point
    /// that is not full. A path that finds none heads for the middle.
    ///
    /// # Panics
    ///
    /// A fallback step of more than one column, which the game throws on.
    fn next_toward_treasure<R: GameRng>(&self, rng: &mut R, at: Id) -> (usize, usize) {
        let point = &self.points[at];
        let (col, row) = (point.col, point.row + 1);
        let (min, max) = self.allowed_columns(row);
        let ahead = self.treasure_row() as i32 - point.row as i32;
        let steps: smallvec::SmallVec<[i32; 3]> = match ahead {
            ahead if ahead > 3 || ahead <= 0 => {
                let mut steps = [-1i32, 0, 1];
                rng.stable_shuffle(&mut steps);
                steps.into_iter().collect()
            }
            _ => centered_priority(col, MIDDLE),
        };
        for step in steps {
            let next = match step {
                -1 => col.saturating_sub(1),
                0 => col,
                _ => (col + 1).min(COLS - 1),
            };
            if next < min || next > max || self.crosses(at, next) {
                continue;
            }
            let there = self.cell(next, row);
            let joins = there.is_none_or(|there| {
                self.points[there].parents.contains(at) || self.points[there].parents.len() < 3
            });
            let opens = at == self.ancient
                || point.children.len() < 3
                || there.is_some_and(|there| point.children.contains(there));
            if joins && opens {
                return (next, row);
            }
        }
        let mut fallback = MIDDLE.clamp(min, max);
        if fallback.abs_diff(col) > 1 {
            let toward = if fallback > col { col + 1 } else { col - 1 };
            fallback = toward.clamp(min, max);
        }
        if self.crosses(at, fallback) {
            fallback = col.clamp(min, max);
        }
        if fallback.abs_diff(col) > 1 {
            unimplemented!("a Spoils path stepping from column {col} to {fallback}");
        }
        (fallback, row)
    }

    /// `GetAllowedColumnsForRow`: the columns open to a row, narrowing to the
    /// middle toward the treasure row and toward the top.
    fn allowed_columns(&self, row: usize) -> (usize, usize) {
        let from_treasure = row.abs_diff(self.treasure_row());
        let from_top = (self.rows - 1).saturating_sub(row);
        let top = MIDDLE.min(from_top + 1);
        let half = MIDDLE.min(from_treasure.min(top));
        (MIDDLE.saturating_sub(half), (MIDDLE + half).min(COLS - 1))
    }

    /// `RedirectToTreasure`: every link into and out of `stray` moved onto
    /// the treasure, and `stray` taken off the grid.
    fn redirect(&mut self, stray: Id, treasure: Id) {
        for parent in self.points[stray].parents.iter().collect::<Vec<_>>() {
            self.unlink(parent, stray);
            self.link(parent, treasure);
        }
        for child in self.points[stray].children.iter().collect::<Vec<_>>() {
            self.unlink(stray, child);
            self.link(treasure, child);
        }
        let (col, row) = (self.points[stray].col, self.points[stray].row);
        self.put(col, row, None);
    }
}

/// `BuildCenteredPriorityList`: the step toward the middle, then straight on,
/// then away, then whichever of the two sideways steps is still missing.
fn centered_priority(col: usize, middle: usize) -> smallvec::SmallVec<[i32; 3]> {
    let toward = (middle as i32 - col as i32).signum();
    let mut steps = smallvec::SmallVec::<[i32; 3]>::new();
    if toward != 0 {
        steps.push(toward);
    }
    steps.push(0);
    if toward != 0 {
        steps.push(-toward);
    }
    for side in [-1, 1] {
        if !steps.contains(&side) {
            steps.push(side);
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use crate::rng::MegaRng;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::acts::Act;
    use sts2_content::v0_107_1::ascension::AscensionLevel;
    use sts2_core::ActMap as _;

    use super::super::ActMap;
    use super::*;

    type V = V0_107_1;
    type MapPointType = <V as sts2_core::ContentVersion>::MapPointType;

    /// Every seed walks to one treasure in the middle column, seven rows from
    /// the top, and no path needs a step wider than one column.
    #[test]
    fn every_spoils_map_narrows_to_one_treasure() {
        let layout = <V as Rules>::act_layout(Act::Hive);
        for seed in 0..500u64 {
            let mut rng: MegaRng = sts2_core::rng::named::<V, _>(seed, "spoils_map");
            let map = ActMap::<V>::spoils(&mut rng, layout, AscensionLevel::None, None)
                .expect("a Spoils map generates");
            let treasures: Vec<_> = (map.nodes().iter())
                .filter(|node| node.kind == MapPointType::TREASURE)
                .map(|node| node.at)
                .collect();
            let row = u8::try_from(layout.rooms + 1 - 7).expect("a row fits a byte");
            assert_eq!(
                treasures,
                [sts2_core::Coord::new(MIDDLE as u8, row)],
                "seed {seed}"
            );
        }
    }
}
