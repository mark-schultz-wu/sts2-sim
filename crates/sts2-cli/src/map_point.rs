//! Shared presentation facts about the map-point names a build reports.

/// The compact mark and human description for one map-point spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub glyph: char,
    pub description: &'static str,
}

/// Looks up presentation metadata without assuming every build has one enum.
pub fn mark(kind: &str) -> Mark {
    match kind {
        "Monster" => Mark {
            glyph: 'M',
            description: "a fight",
        },
        "Elite" => Mark {
            glyph: 'E',
            description: "an elite",
        },
        "Shop" => Mark {
            glyph: '$',
            description: "a shop",
        },
        "Treasure" => Mark {
            glyph: 'T',
            description: "a chest",
        },
        "RestSite" => Mark {
            glyph: 'R',
            description: "a fire",
        },
        "Unknown" => Mark {
            glyph: '?',
            description: "unknown",
        },
        "Boss" => Mark {
            glyph: 'B',
            description: "the boss",
        },
        "Ancient" => Mark {
            glyph: 'A',
            description: "an ancient",
        },
        _ => Mark {
            glyph: '.',
            description: "somewhere",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_map_point_degrades_in_one_defined_way() {
        assert_eq!(
            mark("A_FUTURE_ROOM"),
            Mark {
                glyph: '.',
                description: "somewhere"
            }
        );
    }
}
