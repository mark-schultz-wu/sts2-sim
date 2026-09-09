//! Cutting duplicate stretches out of the map.
//!
//! Seven paths through a narrow grid produce runs of nodes that read the same:
//! the same shape between the same two junctions, over the same types. A port
//! of `MapPathPruning`, which finds those, keeps one of each, and removes or
//! disconnects the rest.
//!
//! Removing nodes takes types off the map, so a prune is followed by a repair
//! that puts the missing ones back over monsters. Repairing makes new
//! duplicates, so the pair runs up to three times.
//!
//! Both halves draw from the map's stream, so this is not tidying that a
//! simulator can skip. What it removes decides where every later draw lands.

use std::collections::BTreeMap;

use sts2_core::rng::GameRng;
use sts2_core::{MapPoints, Rules};

use super::{ActMap, Id};

/// Prune, repair, and go round again while the repair changed anything.
pub(super) fn prune_and_repair<V: Rules, R: GameRng>(map: &mut ActMap<V>, rng: &mut R) {
    for _ in 0..3 {
        prune_duplicates(map, rng);
        if !repair(map, rng) {
            break;
        }
    }
}

/// Prune until no duplicate stretch is left, which is
/// `PruneDuplicateSegments`.
///
/// Each pass changes one thing and then looks again, because a cut can join two
/// stretches that were separate.
fn prune_duplicates<V: Rules, R: GameRng>(map: &mut ActMap<V>, rng: &mut R) {
    let mut passes = 0;
    let mut duplicates = matching_segments(map);
    while prune_pass(map, rng, &duplicates) {
        passes += 1;
        assert!(
            passes <= 50,
            "the map still has duplicate stretches after 50 prunes"
        );
        duplicates = matching_segments(map);
    }
}

/// Every group of stretches that read the same, which is
/// `FindMatchingSegments`.
///
/// Keyed by where the stretch starts and ends and the types along it. The map
/// is ordered by that key, as the game's `SortedDictionary` is, so the groups
/// come back in the same order on every machine.
fn matching_segments<V: Rules>(map: &ActMap<V>) -> Vec<Vec<Vec<Id>>> {
    let mut segments: BTreeMap<String, Vec<Vec<Id>>> = BTreeMap::new();
    for path in all_paths(map, map.ancient) {
        collect_segments(map, &path, &mut segments);
    }
    segments
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

/// Every route from here to the boss, which is `FindAllPaths`.
///
/// In the order the children were linked, which is the order the game walks
/// its `HashSet` in. That order decides which stretch of a duplicate pair is
/// found first.
fn all_paths<V: Rules>(map: &ActMap<V>, from: Id) -> Vec<Vec<Id>> {
    if map.points[from].kind == V::MapPointType::BOSS {
        return vec![vec![from]];
    }
    let mut paths = Vec::new();
    for child in map.points[from].children.iter() {
        for tail in all_paths(map, child) {
            let mut path = vec![from];
            path.extend(tail);
            paths.push(path);
        }
    }
    paths
}

/// Every stretch of one route worth comparing, which is
/// `AddSegmentsToDictionary`.
///
/// A stretch starts at a junction or at the first node, ends where two routes
/// meet, and is at least three nodes long. A stretch overlapping one already
/// filed under the same key is dropped rather than filed beside it.
fn collect_segments<V: Rules>(
    map: &ActMap<V>,
    path: &[Id],
    segments: &mut BTreeMap<String, Vec<Vec<Id>>>,
) {
    for start in 0..path.len().saturating_sub(1) {
        if !starts_a_segment(map, path[start]) {
            continue;
        }
        for span in 2..path.len() - start {
            if map.points[path[start + span]].parents.len() < 2 {
                continue;
            }
            let segment: Vec<Id> = path[start..=start + span].to_vec();
            let key = segment_key(map, &segment);
            match segments.get_mut(&key) {
                None => {
                    segments.insert(key, vec![segment]);
                }
                Some(group) => {
                    if !group.iter().any(|held| overlaps(held, &segment)) {
                        group.push(segment);
                    }
                }
            }
        }
    }
}

/// Whether a stretch is allowed to start here, which is
/// `IsValidSegmentStartMapPoint`.
fn starts_a_segment<V: Rules>(map: &ActMap<V>, id: Id) -> bool {
    map.points[id].children.len() > 1 || map.points[id].row == 0
}

/// What makes two stretches the same: their ends and the types along them.
///
/// A stretch from the first node is keyed by its row alone, so two openings
/// into the same node compare equal wherever they started.
fn segment_key<V: Rules>(map: &ActMap<V>, segment: &[Id]) -> String {
    let first = &map.points[segment[0]];
    let last = &map.points[segment[segment.len() - 1]];
    let head = if first.row == 0 {
        format!("{}-{},{}-", first.row, last.col, last.row)
    } else {
        format!("{},{}-{},{}-", first.col, first.row, last.col, last.row)
    };
    let types: Vec<String> = segment
        .iter()
        .map(|&id| kind_index::<V>(map.points[id].kind).to_string())
        .collect();
    head + &types.join(",")
}

/// A node type as the number the game writes into a key.
///
/// The game casts its enum, and the cast is what goes into the key. Here the
/// nine types are found by name, because a build's own numbering is not
/// something a generic caller can reach.
fn kind_index<V: Rules>(kind: V::MapPointType) -> usize {
    const ORDER: usize = 9;
    let named = [
        V::MapPointType::UNASSIGNED,
        V::MapPointType::UNKNOWN,
        V::MapPointType::SHOP,
        V::MapPointType::TREASURE,
        V::MapPointType::REST_SITE,
        V::MapPointType::MONSTER,
        V::MapPointType::ELITE,
        V::MapPointType::BOSS,
        V::MapPointType::ANCIENT,
    ];
    debug_assert_eq!(named.len(), ORDER);
    named
        .iter()
        .position(|&of| of == kind)
        .expect("every node carries one of the nine types")
}

/// Whether two stretches share an inner node, which is `OverlappingSegment`.
fn overlaps(a: &[Id], b: &[Id]) -> bool {
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    (1..=a.len() - 2).any(|at| b.get(at) == Some(&a[at]))
}

/// One pass over the duplicate groups, which is `PrunePaths`.
///
/// Returns as soon as anything changed, so the groups are found again against
/// the map that change left behind.
fn prune_pass<V: Rules, R: GameRng>(
    map: &mut ActMap<V>,
    rng: &mut R,
    duplicates: &[Vec<Vec<Id>>],
) -> bool {
    for group in duplicates {
        let mut group = group.clone();
        rng.unstable_shuffle(&mut group);
        if all_but_last(map, &group) != 0 {
            return true;
        }
        if group.iter().any(|segment| break_a_link(map, segment)) {
            return true;
        }
    }
    false
}

/// Try to remove every stretch in a group but the last, which is
/// `PruneAllButLast`.
fn all_but_last<V: Rules>(map: &mut ActMap<V>, group: &[Vec<Id>]) -> usize {
    let mut removed = 0;
    for segment in group {
        // Counted prunes, not stretches looked at: the last one of a group
        // stays, so the count stops one short of the group's size.
        if removed == group.len() - 1 {
            return removed;
        }
        if prune_segment(map, segment) {
            removed += 1;
        }
    }
    removed
}

/// Remove what can go of one stretch, which is `PruneSegment`.
///
/// A node goes only where nothing depends on it. Not a junction, not a
/// meeting point, and not the only child of a parent left with none.
fn prune_segment<V: Rules>(map: &mut ActMap<V>, segment: &[Id]) -> bool {
    let mut removed = false;
    for at in 0..segment.len() - 1 {
        let id = segment[at];
        if !map.in_map(id) {
            return true;
        }
        let point = &map.points[id];
        let orphans_a_parent = point
            .parents
            .iter()
            .any(|parent| map.points[parent].children.len() == 1 && map.on_grid(parent));
        if point.children.len() > 1 || point.parents.len() > 1 || orphans_a_parent {
            continue;
        }

        let opens_later = segment[at..].iter().any(|&later| {
            map.points[later].children.len() > 1 && map.points[later].parents.len() == 1
        });
        if opens_later {
            continue;
        }
        if map.points[segment[segment.len() - 1]].parents.len() == 1 {
            return false;
        }
        let strands_a_child = map.points[id]
            .children
            .iter()
            .filter(|child| !segment.contains(child))
            .any(|child| map.points[child].parents.len() == 1);
        if !strands_a_child {
            map.cut(id);
            removed = true;
        }
    }
    removed
}

/// Cut one parent to child link in a stretch, which is
/// `BreakAParentChildRelationshipInSegment`.
///
/// The fallback for a duplicate whose nodes are all load bearing. Both nodes
/// stay and the route between them goes.
fn break_a_link<V: Rules>(map: &mut ActMap<V>, segment: &[Id]) -> bool {
    let mut broken = false;
    for at in 0..segment.len() - 1 {
        let (parent, child) = (segment[at], segment[at + 1]);
        if map.points[parent].children.len() >= 2 && map.points[child].parents.len() != 1 {
            map.unlink(parent, child);
            broken = true;
        }
    }
    broken
}

/// Put back the types pruning took off the map, which is
/// `RepairPrunedPointTypes`.
///
/// Shops, elites, rest sites and question marks, in that order, each replacing
/// monsters the rules allow it over. Returns whether anything changed, which is
/// what makes the caller prune again.
fn repair<V: Rules, R: GameRng>(map: &mut ActMap<V>, rng: &mut R) -> bool {
    let mut changed = false;
    let wanted = [
        (V::MapPointType::SHOP, map.counts.shops),
        (V::MapPointType::ELITE, map.counts.elites),
        (V::MapPointType::REST_SITE, map.counts.rests),
        (V::MapPointType::UNKNOWN, map.counts.unknowns),
    ];
    for (kind, count) in wanted {
        changed |= repair_kind(map, rng, kind, count);
    }
    changed
}

/// Replace monsters with one missing type, which is `RepairPointType`.
fn repair_kind<V: Rules, R: GameRng>(
    map: &mut ActMap<V>,
    rng: &mut R,
    kind: V::MapPointType,
    wanted: usize,
) -> bool {
    let have = map.live().filter(|&id| map.points[id].kind == kind).count();
    if have >= wanted {
        return false;
    }
    let mut missing = wanted - have;

    let mut monsters: Vec<Id> = map
        .live()
        .filter(|&id| map.points[id].kind == V::MapPointType::MONSTER && map.points[id].modifiable)
        .collect();
    map.shuffle_points(rng, &mut monsters);

    let mut changed = false;
    for id in monsters {
        if missing == 0 {
            break;
        }
        if map.allows(kind, id) {
            map.points[id].kind = kind;
            missing -= 1;
            changed = true;
        }
    }
    changed
}
