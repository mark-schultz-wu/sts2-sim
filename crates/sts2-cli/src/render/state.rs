//! The run outside a fight: the map point, the deck, and the RNG counters.

use super::combat::combat_of;
use super::{Field, cards, names};
use serde_json::Value;
use std::fmt;

pub(super) fn state_of(f: &mut fmt::Formatter<'_>, state: &Value) -> fmt::Result {
    if let (Some(hp), Some(max), Some(floor)) = (
        state.int_at("hp"),
        state.int_at("maxHp"),
        state.int_at("floor"),
    ) {
        let act = state.int_at("act").unwrap_or(0);
        let gold = state.int_at("gold").unwrap_or(0);
        let mut line = format!("run     floor {floor} · act {act} · hp {hp}/{max} · gold {gold}");
        if state.is_true("runOver") {
            let outcome = if state.is_true("victory") {
                "victory"
            } else {
                "defeat"
            };
            line.push_str(&format!(" · RUN OVER ({outcome})"));
        } else if !state.is_true("alive") {
            line.push_str(" · dead");
        }
        writeln!(f, "{line}")?;
    }

    let deck = state.list_at("deck");
    if !deck.is_empty() {
        writeln!(f, "deck    {}", cards(deck))?;
    }
    let relics = state.list_at("relics");
    if !relics.is_empty() {
        writeln!(f, "relics  {}", names(relics))?;
    }
    let potions = state.list_at("potions");
    if !potions.is_empty() {
        writeln!(f, "potions {}", names(potions))?;
    }

    if let Some(map) = state.get("map") {
        let at = map.str_at("at").unwrap_or("?");
        let to: Vec<String> = map
            .list_at("travelable")
            .iter()
            .map(|p| {
                format!(
                    "{} ({})",
                    p.str_at("at").unwrap_or("?"),
                    p.str_at("type").unwrap_or("?")
                )
            })
            .collect();
        let to = if to.is_empty() {
            "nowhere".to_string()
        } else {
            to.join(" · ")
        };
        writeln!(f, "map     at {at} → {to}")?;
    }

    if let Some(combat) = state.get("combat").filter(|c| !c.is_null()) {
        combat_of(f, combat)?;
    } else if let Some(room) = state.get("room").filter(|r| !r.is_null()) {
        room_of(f, room)?;
    }

    rng_of(f, state.get("rng"))
}

fn room_of(f: &mut fmt::Formatter<'_>, room: &Value) -> fmt::Result {
    let kind = room.str_at("type").unwrap_or("?");
    let id = room.str_at("id").unwrap_or("");
    let done = if room.is_true("finished") {
        " (finished)"
    } else {
        ""
    };
    writeln!(f, "room    {kind} {id}{done}")?;

    for o in room.list_at("options") {
        let index = o.int_at("index").unwrap_or(-1);
        // An option is described by whichever of these it carries. The key is
        // the fallback because every option has one.
        let what = o
            .str_at("relic")
            .or_else(|| o.str_at("card"))
            .or_else(|| o.str_at("id"))
            .or_else(|| o.str_at("key"))
            .unwrap_or("?");
        let mut flags = Vec::new();
        if o.is_true("chosen") {
            flags.push("chosen");
        }
        if o.is_true("locked") {
            flags.push("locked");
        }
        if o.is_true("proceed") {
            flags.push("proceed");
        }
        let suffix = if flags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", flags.join(", "))
        };
        writeln!(f, "          {index:<3} {what}{suffix}")?;
    }

    Ok(())
}

/// Only the streams that have moved. Fifteen counters of which one is non-zero is noise. The one that moved is the signal.
fn rng_of(f: &mut fmt::Formatter<'_>, rng: Option<&Value>) -> fmt::Result {
    let Some(map) = rng.and_then(Value::as_object) else {
        return Ok(());
    };
    let mut moved: Vec<String> = Vec::new();
    let mut idle = 0;
    for (name, value) in map {
        match value.as_i64() {
            Some(0) | None => idle += 1,
            Some(n) => moved.push(format!("{name} {n}")),
        }
    }
    if moved.is_empty() && idle == 0 {
        return Ok(());
    }
    let rest = match idle {
        0 => String::new(),
        n => format!("  ({n} others at 0)"),
    };
    writeln!(f, "rng     {}{rest}", moved.join(" · "))
}
