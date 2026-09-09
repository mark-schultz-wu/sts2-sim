//! The act, drawn.
//!
//! Where to go next is the decision above every other one in a run, and it is
//! made by looking several rows ahead. Whether the elite comes before the shop,
//! whether this branch reaches a campfire at all. The one-line summary in an
//! observation answers "where am I and what is adjacent", which is the wrong
//! question for that. This draws the graph the observation carries.
//!
//! It is a command of its own rather than part of every reply. The map does not
//! change while a fight is being played, and fifteen rows printed after every
//! card bury the fight.

use super::spelling;
use std::fmt::Write as _;
use sts2_core::{ContentVersion, Coord, Map};

/// The act as a picture, or nothing if the observation carried no graph.
///
/// Returns a `String` rather than writing through a formatter, because the whole
/// grid has to be laid out before any of it can be printed. A row's connectors
/// are decided by the row below it.
pub fn drawn<V: ContentVersion>(map: &Map<V>) -> Option<String> {
    if map.points.is_empty() {
        return None;
    }

    let at = map.at;
    let travelable: Vec<Coord> = map.travelable.iter().map(|p| p.at).collect();

    let cols = i32::from(map.points.iter().map(|p| p.at.col).max().unwrap_or(0)) + 1;
    let top = map.points.iter().map(|p| p.at.row).max().unwrap_or(0);
    let bottom = map.points.iter().map(|p| p.at.row).min().unwrap_or(0);
    let width = (cols * 3) as usize;

    let mut out = String::new();
    for row in (bottom..=top).rev() {
        let mut line = vec![' '; width];
        for p in map.points.iter().filter(|p| p.at.row == row) {
            let mut mark = crate::map_point::mark(&spelling(&p.kind)).glyph;
            // A node already walked through is shown lowercase rather than
            // hidden: the route taken is what makes the rest of the map
            // readable, and on a second act the row numbers alone do not say
            // which way the run came.
            if map.visited.contains(&p.at) && at != Some(p.at) {
                mark = mark.to_ascii_lowercase();
            }
            let (open, close) = match (at == Some(p.at), travelable.contains(&p.at)) {
                (true, _) => ('(', ')'),
                (_, true) => ('[', ']'),
                _ => (' ', ' '),
            };
            let cell = usize::from(p.at.col) * 3;
            line[cell] = open;
            line[cell + 1] = mark;
            line[cell + 2] = close;
        }
        let _ = writeln!(
            out,
            "{row:>3} {}",
            line.iter().collect::<String>().trim_end()
        );

        if row == bottom {
            continue;
        }

        // Every edge from the row below into this one. Drawn from `Children`,
        // so this is the same set of lines the game's own map screen draws.
        let mut links = vec![' '; width + 1];
        for p in map.points.iter().filter(|p| p.at.row + 1 == row) {
            for to in p.to.iter().filter(|c| c.row == row) {
                let (offset, mark) = match i32::from(to.col) - i32::from(p.at.col) {
                    0 => (1, '|'),
                    n if n > 0 => (2, '/'),
                    _ => (0, '\\'),
                };
                links[usize::from(p.at.col) * 3 + offset] = mark;
            }
        }
        let _ = writeln!(out, "    {}", links.iter().collect::<String>().trim_end());
    }

    let ruler: String = (0..cols).map(|c| format!(" {c} ")).collect();
    let _ = writeln!(out, "    {}", ruler.trim_end());
    let _ = writeln!(
        out,
        "\n    (here) [reachable] visited  ·  M monster  E elite  $ shop  T treasure  R rest  ? unknown  B boss  A ancient"
    );
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::drawn;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{ContentVersion, Coord, Map, Node, Reachable};

    type V = V0_107_1;
    type MapPointType = <V as ContentVersion>::MapPointType;

    /// A point type by the game's own name for it.
    ///
    /// Through serde, because that is where a build's names live, and because
    /// the names are what these tests are about. The glyph table is keyed on
    /// them, so a test that named variants directly checks something other than
    /// the thing that can go wrong.
    fn kind(name: &str) -> MapPointType {
        serde_json::from_value(serde_json::json!(name)).expect("a point type this build has")
    }

    fn at(spelling: &str) -> Coord {
        let (col, row) = spelling.split_once(',').expect("col,row");
        Coord {
            col: col.parse().expect("a column"),
            row: row.parse().expect("a row"),
        }
    }

    /// A map, written the way the picture reads: a node, its type, its children.
    fn map(
        here: Option<&str>,
        visited: &[&str],
        travelable: &[(&str, &str)],
        points: &[(&str, &str, &[&str])],
    ) -> Map<V> {
        Map {
            at: here.map(at),
            visited: visited.iter().map(|c| at(c)).collect(),
            travelable: travelable
                .iter()
                .map(|(c, k)| Reachable {
                    at: at(c),
                    kind: kind(k),
                })
                .collect(),
            points: points
                .iter()
                .map(|(c, k, to)| Node {
                    at: at(c),
                    kind: kind(k),
                    to: to.iter().map(|c| at(c)).collect(),
                })
                .collect(),
            proceed: false,
        }
    }

    /// A map with one junction in it, drawn.
    ///
    /// Coordinates are what `sts2 act travel` takes, so a node has to sit under
    /// its own column number. The ruler is not decoration. It is how the picture
    /// is read back into a command.
    #[test]
    fn a_node_sits_in_its_own_column_and_its_edges_point_at_its_children() {
        let out = drawn(&map(
            Some("0,0"),
            &[],
            &[("1,1", "Shop")],
            &[
                ("0,0", "Ancient", &["0,1", "1,1"]),
                ("0,1", "Monster", &[]),
                ("1,1", "Shop", &[]),
            ],
        ))
        .expect("a map");

        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "  1  M [$]", "{out}");
        //                        ^ a straight edge to 0,1 and a right one to 1,1
        assert_eq!(lines[1], "     |/", "{out}");
        assert_eq!(lines[2], "  0 (A)", "{out}");
        assert_eq!(lines[3], "     0  1", "{out}");
    }

    /// Where the run has already been, and where it can go next, are different
    /// marks. The node it is standing on is neither.
    #[test]
    fn the_route_taken_is_told_apart_from_the_route_offered() {
        let out = drawn(&map(
            Some("1,1"),
            &["0,0", "1,1"],
            &[("2,2", "Elite")],
            &[
                ("0,0", "Monster", &["1,1"]),
                ("1,1", "RestSite", &["2,2"]),
                ("2,2", "Elite", &[]),
                ("0,2", "Treasure", &[]),
            ],
        ))
        .expect("a map");

        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "  2  T    [E]", "{out}");
        // Snapshot on a visited node marks it as here, not as walked.
        assert_eq!(lines[2], "  1    (R)", "{out}");
        assert_eq!(lines[4], "  0  m", "{out}");
    }

    /// The observation from a game with no map at all is not a broken map.
    #[test]
    fn a_map_with_no_points_draws_nothing() {
        assert!(drawn(&map(None, &[], &[], &[])).is_none());
    }
}
