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

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use sts2_core::rng::GameRng;
use sts2_core::{MapPointKinds, Rules};

use super::{ActMapBuilder, Error, Id};

/// Prune, repair, and go round again while the repair changed anything.
pub(super) fn prune_and_repair<V: Rules, R: GameRng>(
    map: &mut ActMapBuilder<V>,
    rng: &mut R,
) -> Result<(), Error> {
    let mut segments = SegmentMap::default();
    for _ in 0..3 {
        prune_duplicates(map, rng, &mut segments)?;
        if !repair(map, rng) {
            return Ok(());
        }
    }
    Ok(())
}

/// Prune until no duplicate stretch is left, which is
/// `PruneDuplicateSegments`.
///
/// Each pass changes one thing and then looks again, because a cut can join two
/// stretches that were separate.
fn prune_duplicates<V: Rules, R: GameRng>(
    map: &mut ActMapBuilder<V>,
    rng: &mut R,
    segments: &mut SegmentMap,
) -> Result<(), Error> {
    let mut passes = 0;
    let mut duplicates = matching_segments(map, segments)?;
    while prune_pass(map, rng, duplicates) {
        passes += 1;
        assert!(
            passes <= 50,
            "the map still has duplicate stretches after 50 prunes"
        );
        duplicates = matching_segments(map, segments)?;
    }
    Ok(())
}

/// The game's textual ordering, represented without its redundant characters.
///
/// The header uses ordered four-bit character codes. Variable point kinds use
/// three bits each. Kinds fixed by their row are omitted.
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct SegmentKey {
    head: u64,
    kinds: u64,
}

#[derive(Default)]
struct SegmentHasher(u64);

/// A cheap hasher for the two already-packed integers in `SegmentKey`.
///
/// This table never sees attacker-controlled keys, and its iteration order is
/// discarded: duplicate groups are sorted by their full key before use.
impl Hasher for SegmentHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_ne_bytes(word));
        }
    }

    fn write_u64(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

type SegmentMap = HashMap<SegmentKey, SegmentGroup, BuildHasherDefault<SegmentHasher>>;

#[derive(Clone, Copy)]
struct PackedPath {
    ids: u128,
    kinds: u64,
    head: u64,
    head_len: u32,
    len: u8,
}

impl PackedPath {
    fn start<V: Rules>(map: &ActMapBuilder<V>, id: Id) -> Result<Self, Error> {
        if id >= 128 {
            return Err(Error::PointIdTooLarge { id });
        }
        let point = &map.points[id];
        let mut head = 0;
        let mut head_len = 0;
        if point.row == 0 {
            push_number(&mut head, &mut head_len, point.row);
        } else {
            push_number(&mut head, &mut head_len, point.col);
            push_head(&mut head, &mut head_len, b',');
            push_number(&mut head, &mut head_len, point.row);
        }
        push_head(&mut head, &mut head_len, b'-');
        Ok(PackedPath {
            ids: id as u128,
            kinds: variable_kind::<V>(point).unwrap_or(0),
            head,
            head_len,
            len: 1,
        })
    }

    fn push<V: Rules>(self, map: &ActMapBuilder<V>, id: Id) -> Result<Self, Error> {
        if self.len >= 17 || id >= 128 {
            if self.len >= 17 {
                return Err(Error::SegmentTooLong {
                    length: usize::from(self.len) + 1,
                });
            }
            return Err(Error::PointIdTooLarge { id });
        }
        let kinds = match variable_kind::<V>(&map.points[id]) {
            Some(kind) => (self.kinds << 3) | kind,
            None => self.kinds,
        };
        Ok(PackedPath {
            ids: (self.ids << 7) | id as u128,
            kinds,
            head: self.head,
            head_len: self.head_len,
            len: self.len + 1,
        })
    }
}

fn variable_kind<V: Rules>(point: &super::Point<V>) -> Option<u64> {
    if !point.modifiable
        || point.kind == V::MapPointType::ANCIENT
        || point.kind == V::MapPointType::BOSS
    {
        None
    } else {
        Some(kind_index::<V>(point.kind) as u64)
    }
}

struct Segment {
    ids: u128,
    len: u8,
}

impl Segment {
    fn len(&self) -> usize {
        usize::from(self.len)
    }

    fn get(&self, at: usize) -> Option<Id> {
        if at < self.len() {
            let shift = (self.len() - at - 1) * 7;
            Some(((self.ids >> shift) & 0x7f) as Id)
        } else {
            None
        }
    }

    fn overlaps(&self, other: &[Id]) -> bool {
        if self.len() < 3 || other.len() < 3 {
            return false;
        }
        (1..=self.len() - 2).any(|at| other.get(at).copied() == self.get(at))
    }

    fn into_vec(self) -> Vec<Id> {
        let len = usize::from(self.len);
        (0..len)
            .map(|at| ((self.ids >> ((len - at - 1) * 7)) & 0x7f) as Id)
            .collect()
    }
}

enum SegmentGroup {
    One(Segment),
    Many(Vec<Segment>),
}

fn push_head(head: &mut u64, len: &mut u32, byte: u8) {
    let encoded = match byte {
        b',' => 1,
        b'-' => 2,
        b'0'..=b'9' => byte - b'0' + 3,
        _ => unreachable!("segment headers contain only digits and separators"),
    };
    *head = (*head << 4) | u64::from(encoded);
    *len += 1;
}

fn push_number(head: &mut u64, len: &mut u32, number: usize) {
    if number >= 10 {
        push_number(head, len, number / 10);
    }
    push_head(head, len, b'0' + (number % 10) as u8);
}

fn endpoint<V: Rules>(point: &super::Point<V>) -> (u64, u32) {
    let mut head = 0;
    let mut len = 0;
    push_number(&mut head, &mut len, point.col);
    push_head(&mut head, &mut len, b',');
    push_number(&mut head, &mut len, point.row);
    push_head(&mut head, &mut len, b'-');
    (head, len)
}

/// Every group of stretches that read the same, which is
/// `FindMatchingSegments`.
///
/// Keyed by where the stretch starts and ends and the types along it. The map
/// is ordered by that key, as the game's `SortedDictionary` is, so the groups
/// come back in the same order on every machine.
fn matching_segments<V: Rules>(
    map: &ActMapBuilder<V>,
    segments: &mut SegmentMap,
) -> Result<Vec<Vec<Vec<Id>>>, Error> {
    // Pruning rebuilds this table after every cut. Retaining its allocation
    // avoids growing the same table repeatedly.
    segments.clear();
    let endpoints: Vec<_> = map.points.iter().map(endpoint::<V>).collect();
    let mut segment = Vec::with_capacity(map.rows + 2);
    for start in map.live().chain([map.ancient]) {
        if starts_a_segment(map, start) {
            segment.push(start);
            collect_segments_from(
                map,
                start,
                &mut segment,
                PackedPath::start(map, start)?,
                &endpoints,
                segments,
            )?;
            segment.clear();
        }
    }
    let mut duplicates: Vec<_> = segments
        .drain()
        .filter_map(|(key, group)| match group {
            SegmentGroup::One(_) => None,
            SegmentGroup::Many(segments) => {
                Some((key, segments.into_iter().map(Segment::into_vec).collect()))
            }
        })
        .collect();
    duplicates.sort_unstable_by_key(|(key, _)| *key);
    Ok(duplicates.into_iter().map(|(_, group)| group).collect())
}

/// Visit every stretch from one junction in child insertion order.
///
/// A stretch is independent of the route used to reach its first point and of
/// every route after its last point. Walking from each eligible start therefore
/// finds it once instead of once per complete ancient-to-boss route.
fn collect_segments_from<V: Rules>(
    map: &ActMapBuilder<V>,
    from: Id,
    segment: &mut Vec<Id>,
    packed: PackedPath,
    endpoints: &[(u64, u32)],
    segments: &mut SegmentMap,
) -> Result<(), Error> {
    for child in map.points[from].children.iter() {
        segment.push(child);
        let packed = packed.push(map, child)?;
        if segment.len() >= 3 && map.points[child].parents.len() >= 2 {
            collect_segment(segment, packed, endpoints, segments);
        }
        collect_segments_from(map, child, segment, packed, endpoints, segments)?;
        segment.pop();
    }
    Ok(())
}

fn stored_segment(packed: PackedPath) -> Segment {
    Segment {
        ids: packed.ids,
        len: packed.len,
    }
}

/// File one candidate stretch unless it overlaps one already under its key.
fn collect_segment(
    segment: &[Id],
    packed: PackedPath,
    endpoints: &[(u64, u32)],
    segments: &mut SegmentMap,
) {
    let key = segment_key(segment, packed, endpoints);
    match segments.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(SegmentGroup::One(stored_segment(packed)));
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let group = entry.get_mut();
            match group {
                SegmentGroup::One(held) if !held.overlaps(segment) => {
                    let first = std::mem::replace(held, Segment { ids: 0, len: 0 });
                    *group = SegmentGroup::Many(vec![first, stored_segment(packed)]);
                }
                SegmentGroup::Many(held) if !held.iter().any(|prior| prior.overlaps(segment)) => {
                    held.push(stored_segment(packed));
                }
                _ => {}
            }
        }
    }
}

/// Whether a stretch is allowed to start here, which is
/// `IsValidSegmentStartMapPoint`.
fn starts_a_segment<V: Rules>(map: &ActMapBuilder<V>, id: Id) -> bool {
    map.points[id].children.len() > 1 || map.points[id].row == 0
}

/// What makes two stretches the same: their ends and the types along them.
///
/// A stretch from the first node is keyed by its row alone, so two openings
/// into the same node compare equal wherever they started.
fn segment_key(segment: &[Id], packed: PackedPath, endpoints: &[(u64, u32)]) -> SegmentKey {
    let (mut head, mut head_len) = (packed.head, packed.head_len);
    let (end, end_len) = endpoints[segment[segment.len() - 1]];
    head = (head << (end_len * 4)) | end;
    head_len += end_len;
    assert!(head_len <= 16, "a segment header must fit in 64 bits");
    head <<= (16 - head_len) * 4;

    let kinds = packed.kinds;
    SegmentKey { head, kinds }
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

/// One pass over the duplicate groups, which is `PrunePaths`.
///
/// Returns as soon as anything changed, so the groups are found again against
/// the map that change left behind.
fn prune_pass<V: Rules, R: GameRng>(
    map: &mut ActMapBuilder<V>,
    rng: &mut R,
    duplicates: Vec<Vec<Vec<Id>>>,
) -> bool {
    for mut group in duplicates {
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
fn all_but_last<V: Rules>(map: &mut ActMapBuilder<V>, group: &[Vec<Id>]) -> usize {
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
fn prune_segment<V: Rules>(map: &mut ActMapBuilder<V>, segment: &[Id]) -> bool {
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
fn break_a_link<V: Rules>(map: &mut ActMapBuilder<V>, segment: &[Id]) -> bool {
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
fn repair<V: Rules, R: GameRng>(map: &mut ActMapBuilder<V>, rng: &mut R) -> bool {
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
    map: &mut ActMapBuilder<V>,
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
