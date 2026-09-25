//! Moving nodes sideways so the drawn map reads clearly.
//!
//! A port of `MapPostProcessing`, in three passes. Shift everything one column
//! if two on one side are empty. Push the nodes in a row apart. Straighten a
//! lone node that juts out of a straight run.
//!
//! None of it draws a random number, and none of it adds or removes an edge.
//! It still matters to a simulator, because a node's column is what the
//! observation reports and what the run travels by.

use sts2_core::Rules;

use super::{ActMapBuilder, COLS, Id};

/// Shift the map one column towards the empty side, which is `CenterGrid`.
///
/// Only when two columns at one end are empty and two at the other are not.
/// Anything shifted off the end is dropped, which the emptiness check makes
/// unreachable.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn center<V: Rules>(map: &mut ActMapBuilder<V>) {
    let left_clear = column_empty(map, 0) && column_empty(map, 1);
    let right_clear = column_empty(map, COLS - 1) && column_empty(map, COLS - 2);
    let step: i32 = match (left_clear, right_clear) {
        (true, false) => -1,
        (false, true) => 1,
        _ => return,
    };

    for row in 0..map.rows {
        // Away from the direction of travel, so a cell is read before the
        // node moving into it overwrites it.
        let columns: Vec<usize> = if step > 0 {
            (0..COLS).rev().collect()
        } else {
            (0..COLS).collect()
        };
        for col in columns {
            let held = map.cell(col, row);
            map.put(col, row, None);
            let to = col as i32 + step;
            if to < 0 || to >= COLS as i32 {
                continue;
            }
            map.put(to as usize, row, held);
            if let Some(id) = held {
                map.points[id].col = to as usize;
            }
        }
    }
}

/// Whether a column holds nothing at all.
fn column_empty<V: Rules>(map: &ActMapBuilder<V>, col: usize) -> bool {
    (0..map.rows).all(|row| map.cell(col, row).is_none())
}

/// Push the nodes in each row as far apart as their edges allow, which is
/// `SpreadAdjacentMapPoints`.
///
/// A node has to stay within one column of every parent and every child, so
/// the room it has is the overlap of those. It takes the free column that
/// leaves the widest gap to its neighbours, and the row settles when nothing
/// moves.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn spread<V: Rules>(map: &mut ActMapBuilder<V>) {
    for row in 0..map.rows {
        let nodes: smallvec::SmallVec<[Id; COLS]> =
            (0..COLS).filter_map(|col| map.cell(col, row)).collect();
        // Each node's column, beside it in `nodes`, kept as nodes move.
        let mut cols: smallvec::SmallVec<[usize; COLS]> =
            nodes.iter().map(|&id| map.points[id].col).collect();
        loop {
            let mut moved = false;
            for (at, &id) in nodes.iter().enumerate() {
                let from = cols[at];
                let here = gap(&cols, at, from);
                let mut best = (from, here);
                for col in allowed(map, id) {
                    if col == from {
                        continue;
                    }
                    if map.cell(col, row).is_some_and(|held| held != id) {
                        continue;
                    }
                    let widened = gap(&cols, at, col);
                    if widened > best.1 {
                        best = (col, widened);
                    }
                }
                if best.0 != from {
                    map.put(from, row, None);
                    map.put(best.0, row, Some(id));
                    map.points[id].col = best.0;
                    cols[at] = best.0;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
    }
}

/// The columns a node can sit in and still reach everything it links to.
///
/// Ascending, which is the order the game's own set comes back in: it starts
/// at every column and only ever removes.
///
/// Within one column of each linked node is one run of columns: from one
/// left of the rightmost to one right of the leftmost.
fn allowed<V: Rules>(map: &ActMapBuilder<V>, id: Id) -> std::ops::RangeInclusive<usize> {
    let point = &map.points[id];
    let (mut low, mut high) = (0, COLS - 1);
    for other in point.parents.iter().chain(point.children.iter()) {
        let at = map.points[other].col;
        low = low.max(at.saturating_sub(1));
        high = high.min(at + 1);
    }
    low..=high
}

/// How far a candidate column is from the nearest other node in the row.
///
/// `usize::MAX` for a row holding nothing else, which is the game's own
/// `int.MaxValue` and makes a lone node stay put.
///
/// `cols` holds the row's columns, and the node itself stands at `at`.
fn gap(cols: &[usize], at: usize, candidate: usize) -> usize {
    cols.iter()
        .enumerate()
        .filter(|&(other, _)| other != at)
        .map(|(_, &col)| candidate.abs_diff(col))
        .min()
        .unwrap_or(usize::MAX)
}

/// Pull a lone node back in line, which is `StraightenPaths`.
///
/// Only a node with exactly one parent and one child, sitting outside both. It
/// moves one column towards them if that column is free.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn straighten<V: Rules>(map: &mut ActMapBuilder<V>) {
    for row in 0..map.rows {
        for col in 0..COLS {
            let Some(id) = map.cell(col, row) else {
                continue;
            };
            let point = &map.points[id];
            if point.parents.len() != 1 || point.children.len() != 1 {
                continue;
            }
            let (Some(parent), Some(child)) = (point.parents.first(), point.children.first())
            else {
                continue;
            };
            let (above, below) = (map.points[parent].col, map.points[child].col);
            // Both read before anything moves, and they cannot both hold.
            let left_of_both = col < below && col < above;
            let right_of_both = col > below && col > above;

            if left_of_both && col + 1 < COLS {
                if map.cell(col + 1, row).is_some() {
                    continue;
                }
                map.points[id].col = col + 1;
                map.put(col, row, None);
                map.put(col + 1, row, Some(id));
            }
            if right_of_both && col > 0 && map.cell(col - 1, row).is_none() {
                map.points[id].col = col - 1;
                map.put(col, row, None);
                map.put(col - 1, row, Some(id));
            }
        }
    }
}
