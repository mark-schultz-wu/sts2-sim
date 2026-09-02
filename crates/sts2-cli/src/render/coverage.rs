//! A coverage report, for reading.
//!
//! Two questions, in this order: how much of the game has play reached, and
//! what is left. The first is a table of one line per axis. The second is the
//! slices, which is where a number becomes a direction. "Cards played 38%" is a
//! fact, and "no Rare played" is somewhere to go next.
//!
//! The missing ids themselves are behind a flag rather than in the report. 578
//! cards means hundreds of lines, and a batch that is going well prints a
//! screenful either way.

use crate::coverage::{Axis, Slice};
use std::fmt;

/// A report, laid out.
pub struct Report<'a> {
    axes: &'a [Axis],
    /// Name what is missing, per axis, rather than only counting it.
    listing: bool,
}

impl<'a> Report<'a> {
    /// The summary, with the slices under each axis.
    pub fn new(axes: &'a [Axis]) -> Self {
        Self {
            axes,
            listing: false,
        }
    }

    /// The same, plus every id nobody reached.
    pub fn listing(mut self) -> Self {
        self.listing = true;
        self
    }
}

impl fmt::Display for Report<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self
            .axes
            .iter()
            .map(|axis| axis.name.len())
            .max()
            .unwrap_or(0);

        for axis in self.axes {
            let count = match axis.reachable {
                // Reachable first, because it is the number that means
                // something; the whole table is in brackets behind it.
                Some((seen, total)) => format!("{seen}/{total} reachable, of {}", axis.total),
                None => format!("{}/{}", axis.seen, axis.total),
            };
            writeln!(
                f,
                "{:<width$}  {:>5.1}%  {count}",
                axis.name,
                axis.percent()
            )?;

            for grouping in &axis.groupings {
                let cells: Vec<String> = grouping.slices.iter().map(cell).collect();
                for line in wrapped(&cells, 4) {
                    writeln!(f, "{:<width$}    by {}: {line}", "", grouping.by)?;
                }
            }

            if !axis.unlisted.is_empty() {
                writeln!(
                    f,
                    "{:<width$}    not in the tables: {}",
                    "",
                    axis.unlisted.join(" · ")
                )?;
            }

            if self.listing && !axis.missing.is_empty() {
                for line in wrapped(&axis.missing, 4) {
                    writeln!(f, "{:<width$}    missing {line}", "")?;
                }
            }
        }

        Ok(())
    }
}

/// One slice, as `HIVE 14/17`.
fn cell(slice: &Slice) -> String {
    format!("{} {}/{}", slice.key, slice.seen, slice.total)
}

/// Cells packed into lines of a readable width.
fn wrapped(cells: &[String], per_line_min: usize) -> Vec<String> {
    const WIDTH: usize = 88;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut count = 0;

    for cell in cells {
        let would = line.len() + cell.len() + 3;
        if count >= per_line_min && would > WIDTH {
            lines.push(std::mem::take(&mut line));
            count = 0;
        }
        if !line.is_empty() {
            line.push_str(" · ");
        }
        line.push_str(cell);
        count += 1;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::Grouping;

    fn axis() -> Axis {
        Axis {
            name: "encounters",
            seen: 2,
            total: 96,
            reachable: Some((2, 85)),
            missing: vec!["AEONGLASS_BOSS".into()],
            unlisted: vec![],
            groupings: vec![Grouping {
                by: "act",
                slices: vec![
                    Slice {
                        key: "HIVE".into(),
                        seen: 2,
                        total: 17,
                    },
                    Slice {
                        key: "GLORY".into(),
                        seen: 0,
                        total: 16,
                    },
                ],
            }],
        }
    }

    #[test]
    fn the_reachable_count_is_the_one_reported() {
        let axes = [axis()];
        let shown = Report::new(&axes).to_string();
        let first = shown.lines().next().unwrap();

        assert!(first.contains("2/85 reachable, of 96"), "{first}");
        // 2 of 85, not 2 of 96.
        assert!(first.contains("2.4%"), "{first}");
    }

    #[test]
    fn slices_are_named_under_their_axis() {
        let axes = [axis()];
        let shown = Report::new(&axes).to_string();

        assert!(shown.contains("by act: HIVE 2/17 · GLORY 0/16"), "{shown}");
        assert!(!shown.contains("AEONGLASS_BOSS"));
    }

    #[test]
    fn listing_names_what_was_never_reached() {
        let axes = [axis()];
        let shown = Report::new(&axes).listing().to_string();

        assert!(shown.contains("missing AEONGLASS_BOSS"), "{shown}");
    }
}
