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

use sts2_core::rng::GameRng;
use sts2_core::{MapPointKinds, Rules};

use smallvec::SmallVec;

use super::{ActMapBuilder, COLS, Error, Id};

/// Prune, repair, and go round again while the repair changed anything.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn prune_and_repair<V: Rules, R: GameRng>(
    map: &mut ActMapBuilder<V>,
    rng: &mut R,
) -> Result<(), Error> {
    let mut segments = Scratch::default();
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
/// Each pass changes one thing and looks again, since a cut can join two
/// stretches. The game searches the whole map each time. But a pass only
/// takes points and links away, which makes no new stretch. So the map is
/// searched once a round, and what still stands is grouped after each pass.
#[tracing::instrument(level = "trace", skip_all)]
fn prune_duplicates<V: Rules, R: GameRng>(
    map: &mut ActMapBuilder<V>,
    rng: &mut R,
    segments: &mut Scratch,
) -> Result<(), Error> {
    let mut passes = 0;
    let mut found = candidates(map, segments)?;
    while prune_pass(map, rng, &found) {
        passes += 1;
        assert!(
            passes <= 50,
            "the map still has duplicate stretches after 50 prunes"
        );
        still_standing(map, &mut found);
    }
    Ok(())
}

/// Every stretch under a key that holds two that overlap nowhere, as they
/// were found.
type Candidates = Vec<(SegmentKey, Vec<Segment>)>;

/// Keep the stretches the map still holds, and the keys that still hold two.
///
/// A stretch stands while three things hold. Its start still opens more than
/// one way, or sits on row zero. Every link along it is still there. Two
/// links still lead into its end. The search asks the same of one it finds.
#[tracing::instrument(level = "trace", skip_all)]
fn still_standing<V: Rules>(map: &ActMapBuilder<V>, found: &mut Candidates) {
    found.retain_mut(|(_, stretches)| {
        stretches.retain(|stretch| {
            let points = stretch.points();
            let start = points[0];
            let end = points[points.len() - 1];
            starts_a_segment(map, start)
                && map.points[end].parents.len() >= 2
                && points
                    .windows(2)
                    .all(|link| map.points[link[0]].children.contains(link[1]))
        });
        stretches.len() >= 2
    });
}

/// The group one key's stretches make, which a pass prunes when it holds two
/// or more: each stretch that overlaps none found before it.
fn group(stretches: &[Segment]) -> SmallVec<[Points; 4]> {
    let mut group: SmallVec<[&Segment; 4]> = SmallVec::new();
    for stretch in stretches {
        if !group.iter().any(|prior| prior.overlaps(stretch)) {
            group.push(stretch);
        }
    }
    group.into_iter().map(Segment::points).collect()
}

/// A stretch's points, inline to the most a stretch holds.
type Points = SmallVec<[Id; 18]>;

/// The game's textual ordering, represented without its redundant characters.
///
/// The header uses ordered four-bit character codes. Variable point kinds use
/// three bits each. Kinds fixed by their row are omitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SegmentKey {
    head: u64,
    kinds: u64,
}

#[derive(Clone, Copy)]
struct PackedPath {
    ids: u128,
    kinds: u64,
    /// How many types [`PackedPath::kinds`] holds.
    count: u8,
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
        let kind = variable_kind::<V>(point);
        Ok(PackedPath {
            ids: id as u128,
            kinds: kind.unwrap_or(0),
            count: u8::from(kind.is_some()),
            head,
            head_len,
            len: 1,
        })
    }

    /// The path on to `id`, whose type spells `kind` into a key.
    fn push(self, id: Id, kind: Option<u64>) -> Result<Self, Error> {
        if self.len >= 17 || id >= 128 {
            if self.len >= 17 {
                return Err(Error::SegmentTooLong {
                    length: usize::from(self.len) + 1,
                });
            }
            return Err(Error::PointIdTooLarge { id });
        }
        let (kinds, count) = match kind {
            Some(kind) => ((self.kinds << 3) | kind, self.count + 1),
            None => (self.kinds, self.count),
        };
        Ok(PackedPath {
            ids: (self.ids << 7) | id as u128,
            kinds,
            count,
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

#[derive(Clone, Copy)]
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

    fn overlaps(&self, other: &Segment) -> bool {
        if self.len() < 3 || other.len() < 3 {
            return false;
        }
        (1..=self.len() - 2).any(|at| other.get(at) == self.get(at))
    }

    fn points(&self) -> Points {
        let len = usize::from(self.len);
        (0..len)
            .map(|at| ((self.ids >> ((len - at - 1) * 7)) & 0x7f) as Id)
            .collect()
    }
}

/// Every stretch filed from one start, or from every start on row zero, in
/// groups that read the same.
///
/// A key spells where a stretch starts, or only the row for row zero. So a
/// group is found here by the stretch's end and its types, without the key.
/// Only a group that gathers a second stretch gets its key.
#[derive(Default)]
struct Groups {
    /// An open table of [`Groups::groups`], by end and types: each slot is a
    /// group plus one, or zero for none. Twice as many slots as groups at
    /// least, and a power of two.
    slots: Vec<u16>,
    groups: Vec<Group>,
}

/// Stretches that end at one point and read the same, in the order the walk
/// filed them.
struct Group {
    end: Id,
    /// Where the group sits in [`Groups::slots`].
    slot: usize,
    /// The first stretch filed, which also carries the start of the key.
    first: PackedPath,
    /// Every later stretch under the same key, in the order filed.
    more: Vec<Segment>,
}

/// How many slots a table starts with.
const SLOTS: usize = 256;

impl Groups {
    /// Nothing filed, with the room the last search needed.
    fn reset(&mut self) {
        if self.slots.is_empty() {
            self.slots.resize(SLOTS, 0);
        }
        for group in &self.groups {
            self.slots[group.slot] = 0;
        }
        self.groups.clear();
    }

    /// Where a group of `end` and `kinds` sits, or the empty slot it goes in.
    fn find(&self, end: Id, kinds: u64) -> usize {
        let mask = self.slots.len() - 1;
        let hash = (kinds ^ ((end as u64) << 57)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut slot = (hash >> 32) as usize & mask;
        loop {
            match self.slots[slot] {
                0 => return slot,
                at => {
                    let group = &self.groups[usize::from(at) - 1];
                    if group.end == end && group.first.kinds == kinds {
                        return slot;
                    }
                }
            }
            slot = (slot + 1) & mask;
        }
    }

    /// File one stretch, which ends at `end`.
    fn file(&mut self, end: Id, packed: PackedPath) {
        let slot = self.find(end, packed.kinds);
        let at = self.slots[slot];
        if at != 0 {
            self.groups[usize::from(at) - 1]
                .more
                .push(stored_segment(packed));
            return;
        }
        self.groups.push(Group {
            end,
            slot,
            first: packed,
            more: Vec::new(),
        });
        self.slots[slot] =
            u16::try_from(self.groups.len()).expect("a search files fewer than 65,535 groups");
        if self.groups.len() * 2 > self.slots.len() {
            self.grow();
        }
    }

    /// Twice the slots, with every group placed again.
    #[cold]
    fn grow(&mut self) {
        let slots = self.slots.len() * 2;
        self.slots.clear();
        self.slots.resize(slots, 0);
        for at in 0..self.groups.len() {
            let (end, kinds) = (self.groups[at].end, self.groups[at].first.kinds);
            let slot = self.find(end, kinds);
            self.slots[slot] =
                u16::try_from(at + 1).expect("a search files fewer than 65,535 groups");
            self.groups[at].slot = slot;
        }
    }

    /// Hand every group of two stretches or more to `found` under its key,
    /// and file nothing more.
    fn harvest<V: Rules>(&mut self, map: &ActMapBuilder<V>, found: &mut Candidates) {
        for group in self.groups.drain(..) {
            self.slots[group.slot] = 0;
            if group.more.is_empty() {
                continue;
            }
            let key = segment_key(endpoint(&map.points[group.end]), group.first);
            let stretches: Vec<Segment> = std::iter::once(stored_segment(group.first))
                .chain(group.more)
                .collect();
            // A group is stretches that overlap none of each other. So a key
            // with no two apart makes no group, and taking stretches away
            // never makes two apart that were not.
            let apart = stretches
                .iter()
                .enumerate()
                .any(|(at, one)| stretches[at + 1..].iter().any(|other| !one.overlaps(other)));
            if apart {
                found.push((key, stretches));
            }
        }
    }
}

/// What a search keeps between one and the next, so it grows nothing twice.
#[derive(Default)]
struct Scratch {
    /// For every start on row zero together, whose stretches share keys.
    opening: Groups,
    /// For one other start at a time.
    local: Groups,
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

/// Every stretch under a key that holds two or more, which is the search
/// `FindMatchingSegments` makes.
///
/// Keyed by where the stretch starts and ends and the types along it, and in
/// that key's order, as the game's `SortedDictionary` is. Only a key two
/// stretches fill apart is kept. [`wanted`] finds those keys without walking
/// every path, and the walk from a start follows only paths toward them.
#[tracing::instrument(level = "trace", skip_all)]
fn candidates<V: Rules>(
    map: &ActMapBuilder<V>,
    segments: &mut Scratch,
) -> Result<Candidates, Error> {
    let mut flat = Flat::of(map)?;
    segments.opening.reset();
    segments.local.reset();
    let starts: Vec<Id> = map
        .live()
        .chain([map.ancient])
        .filter(|&start| starts_a_segment(map, start))
        .collect();
    let opening: Vec<Id> = starts
        .iter()
        .copied()
        .filter(|&start| map.points[start].row == 0)
        .collect();
    let from_opening = wanted(&mut flat, &opening);
    let mut found = Vec::new();
    for &start in &starts {
        let packed = PackedPath::start(map, start)?;
        match map.points[start].row {
            0 => collect_wanted(&flat, start, packed, &from_opening, &mut segments.opening)?,
            _ => {
                let from_start = wanted(&mut flat, &[start]);
                collect_wanted(&flat, start, packed, &from_start, &mut segments.local)?;
                segments.local.harvest(map, &mut found);
            }
        }
    }
    segments.opening.harvest(map, &mut found);
    // No two groups share a key, since each came from its own start or from
    // row zero's.
    found.sort_unstable_by_key(|(key, _)| *key);
    Ok(found)
}

/// [`candidates`] by walking every path from every start, which is the
/// search as the game makes it.
#[cfg(test)]
fn walked<V: Rules>(map: &ActMapBuilder<V>, segments: &mut Scratch) -> Result<Candidates, Error> {
    let flat = Flat::of(map)?;
    segments.opening.reset();
    segments.local.reset();
    let mut found = Vec::new();
    for start in map.live().chain([map.ancient]) {
        if !starts_a_segment(map, start) {
            continue;
        }
        let packed = PackedPath::start(map, start)?;
        match map.points[start].row {
            0 => collect_segments_from(&flat, start, packed, &mut segments.opening)?,
            _ => {
                collect_segments_from(&flat, start, packed, &mut segments.local)?;
                segments.local.harvest(map, &mut found);
            }
        }
    }
    segments.opening.harvest(map, &mut found);
    found.sort_unstable_by_key(|(key, _)| *key);
    Ok(found)
}

/// A key that two stretches from a start can fill apart: where they end, and
/// the types along them, which number `count`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Wanted {
    end: Id,
    kinds: u64,
    count: u8,
}

/// The types after `kinds` with one more point's, and how many there are.
fn read_on(kinds: u64, count: u8, kind: Option<u64>) -> (u64, u8) {
    match kind {
        Some(kind) => ((kinds << 3) | kind, count + 1),
        None => (kinds, count),
    }
}

/// Every key the stretches from `starts` can fill with two apart.
///
/// Two stretches are apart when they share no point but their ends. Every
/// link runs one row on, so they are two walks side by side. They leave by
/// different first steps, stand on different points of each row, read
/// alike, and step to one point. [`Flat::meet`] says which pairs can still
/// get there, and only those are followed.
#[tracing::instrument(level = "trace", skip_all)]
fn wanted(flat: &mut Flat, starts: &[Id]) -> Vec<Wanted> {
    let mut wanted = Vec::new();
    let firsts: Vec<(Id, Id)> = starts
        .iter()
        .flat_map(|&start| {
            flat.children(start)
                .iter()
                .map(move |&child| (start, child))
        })
        .collect();
    for (at, &(start, one)) in firsts.iter().enumerate() {
        for &(other_start, other) in &firsts[at + 1..] {
            if one == other
                || flat.kinds[start] != flat.kinds[other_start]
                || flat.kinds[one] != flat.kinds[other]
                || !flat.meet(one, other)
            {
                continue;
            }
            let (kinds, count) = read_on(0, 0, flat.kinds[start]);
            let (kinds, count) = read_on(kinds, count, flat.kinds[one]);
            side_by_side(flat, one, other, kinds, count, &mut wanted);
        }
    }
    wanted
}

/// Follow two walks from `one` and `other`, which read alike and can meet,
/// and note each key where they do.
fn side_by_side(
    flat: &mut Flat,
    one: Id,
    other: Id,
    kinds: u64,
    count: u8,
    wanted: &mut Vec<Wanted>,
) {
    for &child in flat.children(one) {
        if flat.children(other).contains(&child) {
            let (kinds, count) = read_on(kinds, count, flat.kinds[child]);
            let key = Wanted {
                end: child,
                kinds,
                count,
            };
            if !wanted.contains(&key) {
                wanted.push(key);
            }
        }
    }
    for at in flat.from[one]..flat.from[one + 1] {
        let next = flat.children[at];
        for other_at in flat.from[other]..flat.from[other + 1] {
            let other_next = flat.children[other_at];
            if next != other_next
                && flat.kinds[next] == flat.kinds[other_next]
                && flat.meet(next, other_next)
            {
                let (kinds, count) = read_on(kinds, count, flat.kinds[next]);
                side_by_side(flat, next, other_next, kinds, count, wanted);
            }
        }
    }
}

/// File every stretch from `from` under one of `wanted`, in the order a walk
/// of every path files them.
///
/// The walk steps only where some wanted key can still be filled beyond. Its
/// end lies ahead or is this point, and the types so far begin its types. A
/// path that fails that files nothing.
#[tracing::instrument(level = "trace", skip_all)]
fn collect_wanted(
    flat: &Flat,
    from: Id,
    packed: PackedPath,
    wanted: &[Wanted],
    groups: &mut Groups,
) -> Result<(), Error> {
    if wanted.is_empty() {
        return Ok(());
    }
    let every: u64 = match wanted.len() {
        64 => u64::MAX,
        len if len < 64 => (1 << len) - 1,
        len => unimplemented!("a start with {len} keys two stretches fill apart"),
    };
    let mut path: smallvec::SmallVec<[_; 24]> = smallvec::SmallVec::new();
    path.push((flat.children(from).iter(), packed, every));
    while let Some((children, packed, alive)) = path.last_mut() {
        let (packed, alive) = (*packed, *alive);
        let Some(&child) = children.next() else {
            path.pop();
            continue;
        };
        let packed = packed.push(child, flat.kinds[child])?;
        let (mut left, mut still, mut ends) = (alive, 0u64, false);
        while left != 0 {
            let at = left.trailing_zeros() as usize;
            left &= left - 1;
            let key = wanted[at];
            let ahead = key.end == child || flat.below[child] & 1 << key.end != 0;
            let read = packed.count <= key.count
                && key.kinds >> (3 * u32::from(key.count - packed.count)) == packed.kinds;
            if ahead && read {
                still |= 1 << at;
                ends |= key.end == child;
            }
        }
        if still == 0 {
            continue;
        }
        if ends && packed.len >= 3 && flat.joins & (1 << child) != 0 {
            groups.file(child, packed);
        }
        path.push((flat.children(child).iter(), packed, still));
    }
    Ok(())
}

/// What the walk reads of each point, worked out once for a whole search
/// rather than at every visit. A point is visited once per path through it.
struct Flat {
    /// Each point's children, in insertion order, one point after another.
    children: Vec<Id>,
    /// Where each point's children start in [`Flat::children`], and one past
    /// the last point's.
    from: Vec<usize>,
    /// What each point's type spells into a key, if anything.
    kinds: Vec<Option<u64>>,
    /// The points two links or more lead into, one bit a point. A stretch
    /// ends at one of these.
    joins: u128,
    /// Every point a path from each point reaches, one bit a point.
    below: Vec<u128>,
    /// For each point, the points on its row it can meet. Two meet when they
    /// read alike, and a step from each, apart and alike again, leads to a
    /// point both step to. [`Flat::meet`] works a pair out when first asked,
    /// and [`Flat::known`] says which pairs it has.
    meets: Vec<u128>,
    known: Vec<u128>,
}

impl Flat {
    #[tracing::instrument(level = "trace", skip_all)]
    fn of<V: Rules>(map: &ActMapBuilder<V>) -> Result<Self, Error> {
        let mut flat = Flat {
            children: Vec::new(),
            from: Vec::with_capacity(map.points.len() + 1),
            kinds: map.points.iter().map(variable_kind::<V>).collect(),
            joins: 0,
            below: vec![0; map.points.len()],
            meets: vec![0; map.points.len()],
            known: vec![0; map.points.len()],
        };
        for (id, point) in map.points.iter().enumerate() {
            flat.from.push(flat.children.len());
            flat.children.extend(point.children.iter());
            if point.parents.len() >= 2 && id < 128 {
                flat.joins |= 1 << id;
            }
        }
        flat.from.push(flat.children.len());

        // Every point a path can stand on, from the last row up. A link runs
        // from one row to a later one, so a point's children are settled
        // before it. The boss is past the last row, and the first node on
        // the first.
        let rows = (0..map.rows)
            .rev()
            .flat_map(|row| (0..COLS).filter_map(move |col| map.cell(col, row)));
        for id in std::iter::once(map.boss).chain(rows).chain([map.ancient]) {
            if id >= 128 {
                return Err(Error::PointIdTooLarge { id });
            }
            flat.below[id] = flat
                .children(id)
                .iter()
                .fold(0, |below, &child| below | 1 << child | flat.below[child]);
        }
        Ok(flat)
    }

    fn children(&self, id: Id) -> &[Id] {
        &self.children[self.from[id]..self.from[id + 1]]
    }

    /// Whether `one` and `other`, on one row and reading alike, can meet.
    fn meet(&mut self, one: Id, other: Id) -> bool {
        if self.known[one] & 1 << other != 0 {
            return self.meets[one] & 1 << other != 0;
        }
        let mut meet = false;
        for at in self.from[one]..self.from[one + 1] {
            let child = self.children[at];
            for next_at in self.from[other]..self.from[other + 1] {
                let next = self.children[next_at];
                meet = next == child
                    || (self.kinds[next] == self.kinds[child] && self.meet(child, next));
                if meet {
                    break;
                }
            }
            if meet {
                break;
            }
        }
        self.known[one] |= 1 << other;
        self.known[other] |= 1 << one;
        if meet {
            self.meets[one] |= 1 << other;
            self.meets[other] |= 1 << one;
        }
        meet
    }
}

/// Visit every stretch from one junction in child insertion order, which is
/// the reference [`collect_wanted`] is held to.
///
/// A walk from each start finds each stretch once. It runs depth first, on
/// a stack of its own. Each entry is a point on the path, the children of it
/// still to visit, and the path up to it.
#[cfg(test)]
fn collect_segments_from(
    flat: &Flat,
    from: Id,
    packed: PackedPath,
    groups: &mut Groups,
) -> Result<(), Error> {
    let mut path: smallvec::SmallVec<[_; 24]> = smallvec::SmallVec::new();
    path.push((flat.children(from).iter(), packed));
    while let Some((children, packed)) = path.last_mut() {
        let packed = *packed;
        let Some(&child) = children.next() else {
            path.pop();
            continue;
        };
        let packed = packed.push(child, flat.kinds[child])?;
        if packed.len >= 3 && flat.joins & (1 << child) != 0 {
            groups.file(child, packed);
        }
        path.push((flat.children(child).iter(), packed));
    }
    Ok(())
}

fn stored_segment(packed: PackedPath) -> Segment {
    Segment {
        ids: packed.ids,
        len: packed.len,
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
fn segment_key((end, end_len): (u64, u32), packed: PackedPath) -> SegmentKey {
    let (mut head, mut head_len) = (packed.head, packed.head_len);
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
///
/// Each key's group is worked out as the pass reaches it, since the pass
/// often ends at the first.
fn prune_pass<V: Rules, R: GameRng>(
    map: &mut ActMapBuilder<V>,
    rng: &mut R,
    found: &Candidates,
) -> bool {
    for (_, stretches) in found {
        let mut group = group(stretches);
        if group.len() < 2 {
            continue;
        }
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
fn all_but_last<V: Rules>(map: &mut ActMapBuilder<V>, group: &[Points]) -> usize {
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

#[cfg(test)]
mod tests {
    use crate::rng::MegaRng;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::acts::Act;
    use sts2_content::v0_107_1::ascension::AscensionLevel;

    use super::*;

    type V = V0_107_1;

    /// Every group of two or more a pass reaches, as points.
    fn groups(found: &Candidates) -> Vec<Vec<Vec<Id>>> {
        found
            .iter()
            .map(|(_, stretches)| group(stretches))
            .filter(|group| group.len() >= 2)
            .map(|group| group.iter().map(|points| points.to_vec()).collect())
            .collect()
    }

    /// Every key and every stretch under it, as points.
    fn listed(found: &Candidates) -> Vec<(SegmentKey, Vec<Vec<Id>>)> {
        found
            .iter()
            .map(|(key, stretches)| {
                let points = stretches.iter().map(|stretch| stretch.points().to_vec());
                (*key, points.collect())
            })
            .collect()
    }

    /// Keeping what a pass leaves standing finds what a fresh search finds,
    /// after every pass. This holds at both elite counts, in every act, over
    /// many seeds. The search also finds what walking every path finds.
    #[test]
    fn what_stands_after_a_pass_is_what_a_search_finds() {
        let mut compared = 0;
        for act in [Act::Overgrowth, Act::Hive, Act::Glory] {
            let layout = <V as Rules>::act_layout(act);
            for ascension in [AscensionLevel::None, AscensionLevel::SwarmingElites] {
                for seed in 0..50u64 {
                    let mut rng: MegaRng = sts2_core::rng::named::<V, _>(seed, "act_1_map");
                    let mut map = ActMapBuilder::<V>::dealt(&mut rng, layout, ascension, None);
                    let mut segments = Scratch::default();
                    for _ in 0..3 {
                        let mut found =
                            candidates(&map, &mut segments).expect("a dealt map searches");
                        loop {
                            let fresh =
                                candidates(&map, &mut segments).expect("a pruned map searches");
                            let every = walked(&map, &mut segments).expect("a pruned map walks");
                            assert_eq!(
                                listed(&fresh),
                                listed(&every),
                                "seed {seed} of {act:?} at {ascension:?}"
                            );
                            assert_eq!(
                                groups(&found),
                                groups(&fresh),
                                "seed {seed} of {act:?} at {ascension:?}"
                            );
                            compared += 1;
                            if !prune_pass(&mut map, &mut rng, &found) {
                                break;
                            }
                            still_standing(&map, &mut found);
                        }
                        if !repair(&mut map, &mut rng) {
                            break;
                        }
                    }
                }
            }
        }
        assert!(compared > 1_500, "{compared} searches compared");
    }
}
