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

use super::{ActMap, COLS, Id};

/// Shift the map one column towards the empty side, which is `CenterGrid`.
///
/// Only when two columns at one end are empty and two at the other are not.
/// Anything shifted off the end is dropped, which the emptiness check makes
/// unreachable.
pub(super) fn center<V: Rules>(map: &mut ActMap<V>) {
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
fn column_empty<V: Rules>(map: &ActMap<V>, col: usize) -> bool {
    (0..map.rows).all(|row| map.cell(col, row).is_none())
}

/// Push the nodes in each row as far apart as their edges allow, which is
/// `SpreadAdjacentMapPoints`.
///
/// A node has to stay within one column of every parent and every child, so
/// the room it has is the overlap of those. It takes the free column that
/// leaves the widest gap to its neighbours, and the row settles when nothing
/// moves.
pub(super) fn spread<V: Rules>(map: &mut ActMap<V>) {
    for row in 0..map.rows {
        let nodes = map.row(row);
        loop {
            let mut moved = false;
            for &id in &nodes {
                let from = map.points[id].col;
                let here = gap(map, from, &nodes, id);
                let mut best = (from, here);
                for col in allowed(map, id) {
                    if col == from {
                        continue;
                    }
                    if map.cell(col, row).is_some_and(|held| held != id) {
                        continue;
                    }
                    let widened = gap(map, col, &nodes, id);
                    if widened > best.1 {
                        best = (col, widened);
                    }
                }
                if best.0 != from {
                    map.put(from, row, None);
                    map.put(best.0, row, Some(id));
                    map.points[id].col = best.0;
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
fn allowed<V: Rules>(map: &ActMap<V>, id: Id) -> Vec<usize> {
    let point = &map.points[id];
    let linked: Vec<usize> = point
        .parents
        .iter()
        .chain(point.children.iter())
        .map(|other| map.points[other].col)
        .collect();
    (0..COLS)
        .filter(|&col| linked.iter().all(|&at| col + 1 >= at && at + 1 >= col))
        .collect()
}

/// How far a candidate column is from the nearest other node in the row.
///
/// `usize::MAX` for a row holding nothing else, which is the game's own
/// `int.MaxValue` and makes a lone node stay put.
fn gap<V: Rules>(map: &ActMap<V>, candidate: usize, nodes: &[Id], id: Id) -> usize {
    nodes
        .iter()
        .filter(|&&other| other != id)
        .map(|&other| candidate.abs_diff(map.points[other].col))
        .min()
        .unwrap_or(usize::MAX)
}

/// Pull a lone node back in line, which is `StraightenPaths`.
///
/// Only a node with exactly one parent and one child, sitting outside both. It
/// moves one column towards them if that column is free.
pub(super) fn straighten<V: Rules>(map: &mut ActMap<V>) {
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
