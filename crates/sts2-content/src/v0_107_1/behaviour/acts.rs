//! What an act's own model says about the map it generates.
//!
//! `ActModel.GetMapPointTypes`, which every act overrides with the two numbers
//! it rolls. The room count sits in the table beside them, so this reads it
//! from there rather than print it twice.

use sts2_core::{Layout, Rests};

use super::super::generated::acts::Act;

/// `MapPointTypeCounts.StandardRandomUnknownCount`, and what each act takes
/// off it.
///
/// The two first acts take nothing. The two after them take one each.
const FEWER_LATE: usize = 1;

/// The map one act generates.
///
/// # Panics
///
/// An act nobody has ported. The deprecated act is one no run reaches.
pub(crate) fn layout(act: Act) -> Layout {
    let rooms = usize::from(sts2_core::Table::data(act).rooms);
    let (rests, fewer_unknowns) = match act {
        // `NextGaussianInt(7, 1, 6, 7)`, and the standard question marks.
        Act::Overgrowth | Act::Underdocks => (
            Rests::Around {
                mean: 7,
                min: 6,
                max: 7,
            },
            0,
        ),
        // One fewer of each than the act before it.
        Act::Hive => (
            Rests::Around {
                mean: 6,
                min: 6,
                max: 7,
            },
            FEWER_LATE,
        ),
        // The one act that rolls its rest sites flat.
        Act::Glory => (Rests::Between { min: 5, max: 7 }, FEWER_LATE),
        act => unimplemented!("the map {act:?} generates"),
    };
    Layout {
        rooms,
        rests,
        fewer_unknowns,
    }
}
