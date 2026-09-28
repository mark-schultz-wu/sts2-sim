//! Mad Science, as the nine cards Tinker Time makes of it.
//!
//! The game has one class, `MadScience`, whose type, target and play all read
//! two saved fields: `TinkerTimeType` and `TinkerTimeRider`. Every copy is
//! made by `TinkerTime.RiderChosen`, which sets both, and the rider fixes the
//! type. So a copy is one of nine cards, and the tables hold nine rows where
//! the dump holds one. The mod names a copy the same way, rider and all.

use super::Dump;

/// The dump's id for the one class.
const CLASS: &str = "MAD_SCIENCE";

/// `TinkerTime.RiderEffect` in its own order, less `None`, with the card type
/// `ChooseRiderEffect` offers it under, the `TargetType` that type gives, and
/// `GainsBlock`.
const RIDERS: [(&str, &str, &str, bool); 9] = [
    ("SAPPING", "Attack", "AnyEnemy", false),
    ("VIOLENCE", "Attack", "AnyEnemy", false),
    ("CHOKING", "Attack", "AnyEnemy", false),
    ("ENERGIZED", "Skill", "Self", true),
    ("WISDOM", "Skill", "Self", true),
    ("CHAOS", "Skill", "Self", true),
    ("EXPERTISE", "Power", "Self", false),
    ("CURIOUS", "Power", "Self", false),
    ("IMPROVEMENT", "Power", "Self", false),
];

/// The id the game gives a card, which is the class's for every rider.
pub(super) fn game_id(id: &str) -> &str {
    match id.strip_prefix(CLASS) {
        Some(rider) if rider.starts_with('_') => CLASS,
        _ => id,
    }
}

/// Each row naming the class written once for each rider, in rider order.
///
/// A card row takes the rider's type, target and `GainsBlock`. A pool
/// listing's index moves the rows after it along, so each pool stays
/// numbered without a gap. Any other row is copied with the id alone
/// changed.
pub(super) fn expand(dump: &mut Dump) {
    // Each pool listing the class, by the index it stands at. A row of the
    // pool after it moves along past the eight added.
    let shifted: Vec<(String, String, usize)> = dump
        .rows
        .iter()
        .filter(|row| matches!(row[0].as_str(), "pool" | "shoppool") && row[3] == CLASS)
        .map(|row| {
            let at = row[2].parse().expect("a pool index is a number");
            (row[0].clone(), row[1].clone(), at)
        })
        .collect();
    let mut rows = Vec::with_capacity(dump.rows.len() + RIDERS.len());
    for row in std::mem::take(&mut dump.rows) {
        let kind = row[0].as_str();
        let pooled = matches!(kind, "pool" | "shoppool");
        if pooled && row[3] == CLASS {
            let at: usize = row[2].parse().expect("a pool index is a number");
            for (offset, (rider, ..)) in RIDERS.iter().enumerate() {
                let mut copy = row.clone();
                copy[2] = (at + offset).to_string();
                copy[3] = format!("{CLASS}_{rider}");
                rows.push(copy);
            }
            continue;
        }
        if pooled {
            let mut row = row;
            if let Some(&(.., at)) = shifted
                .iter()
                .find(|(listing, pool, _)| listing == &row[0] && pool == &row[1])
            {
                let index: usize = row[2].parse().expect("a pool index is a number");
                if index > at {
                    row[2] = (index + RIDERS.len() - 1).to_string();
                }
            }
            rows.push(row);
            continue;
        }
        if row.get(1).map(String::as_str) != Some(CLASS) {
            rows.push(row);
            continue;
        }
        for &(rider, card_type, target, gains_block) in &RIDERS {
            let mut copy = row.clone();
            copy[1] = format!("{CLASS}_{rider}");
            // The cards dump's own row. The plays dump names a card with its
            // id alone.
            if kind == "card" && copy.len() > 13 {
                copy[3] = card_type.to_owned();
                copy[5] = target.to_owned();
                copy[13] = if gains_block { "1" } else { "0" }.to_owned();
            }
            rows.push(copy);
        }
    }
    dump.rows = rows;
}
