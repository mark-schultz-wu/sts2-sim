//! The fight: creatures, piles, energy, and what is legal right now.

use super::{Field, cards};
use serde_json::Value;
use std::fmt;

pub(super) fn combat_of(f: &mut fmt::Formatter<'_>, combat: &Value) -> fmt::Result {
    let turn = combat.int_at("turn").unwrap_or(0);
    let round = combat.int_at("round").unwrap_or(0);
    let phase = combat.str_at("phase").unwrap_or("?");
    let energy = combat.int_at("energy").unwrap_or(0);
    writeln!(
        f,
        "combat  turn {turn} · round {round} · {phase} · {energy} energy"
    )?;

    for c in combat.list_at("creatures") {
        // The category prefix is redundant next to the side, which says the same
        // thing more usefully.
        let model = c
            .str_at("model")
            .map(|m| m.rsplit('/').next().unwrap_or(m))
            .unwrap_or("?");
        let side = c.str_at("side").unwrap_or("?");
        let marker = if side == "Player" { "you" } else { "   " };
        let id = c.int_at("id").unwrap_or(-1);

        let mut line = format!(
            "        {marker} {id:<2} {model:<20} {}/{}",
            c.int_at("hp").unwrap_or(0),
            c.int_at("maxHp").unwrap_or(0)
        );
        match c.int_at("block").unwrap_or(0) {
            0 => {}
            block => line.push_str(&format!(" +{block} block")),
        }
        if !c.is_true("alive") {
            line.push_str(" DEAD");
        }
        if let Some(intent) = c.str_at("intent") {
            line.push_str(&format!(" → {intent}"));
            // A move carries a list of intents, so an attack that also blocks
            // reports both. Only attacks have numbers. The rest name themselves,
            // because the game has no amount to report for them either.
            let detail: Vec<String> = c
                .list_at("intents")
                .iter()
                .map(|i| {
                    let kind = i.str_at("type").unwrap_or("?").to_lowercase();
                    match (i.int_at("damage"), i.int_at("repeats")) {
                        (Some(d), Some(r)) if r > 1 => format!("{kind} {d}×{r}"),
                        (Some(d), _) => format!("{kind} {d}"),
                        _ => kind,
                    }
                })
                .collect();
            if !detail.is_empty() {
                line.push_str(&format!(" ({})", detail.join(", ")));
            }
        }
        let powers: Vec<String> = c
            .list_at("powers")
            .iter()
            .map(|p| {
                let id = p.str_at("id").unwrap_or("?");
                match p.int_at("amount") {
                    Some(n) if n != 1 => format!("{id} {n}"),
                    _ => id.to_string(),
                }
            })
            .collect();
        if !powers.is_empty() {
            line.push_str(&format!("  [{}]", powers.join(", ")));
        }
        writeln!(f, "{line}")?;
    }

    // The hand and the legality list are parallel arrays over the same cards, so
    // they are shown as one thing: an index you can play, and why you cannot.
    let hand = combat.list_at("hand");
    let legal = combat.list_at("legal");
    if !hand.is_empty() {
        writeln!(f, "hand")?;
        for (i, card) in hand.iter().enumerate() {
            let name = card.as_str().unwrap_or("?");
            let mut line = format!("        {i:<3} {name:<20}");
            if let Some(l) = legal.get(i) {
                line.push_str(&format!(" {}e", l.int_at("cost").unwrap_or(0)));
                if l.is_true("playable") {
                    let targets: Vec<String> = l
                        .list_at("targets")
                        .iter()
                        .filter_map(Value::as_i64)
                        .map(|t| t.to_string())
                        .collect();
                    // Both halves are needed. Ids alone leave an empty list
                    // ambiguous between "targets itself, nothing to pick" and
                    // "targets enemies, none are alive", which are not at all
                    // the same thing to a caller deciding what to play.
                    let kind = l.str_at("target").unwrap_or("?");
                    if targets.is_empty() {
                        line.push_str(&format!("  ({kind})"));
                    } else {
                        line.push_str(&format!("  → {}", targets.join(",")));
                    }
                } else {
                    let why = l.str_at("reason").unwrap_or("unplayable");
                    line.push_str(&format!("  ✗ {why}"));
                    if let Some(by) = l.str_at("preventer") {
                        line.push_str(&format!(" ({by})"));
                    }
                }
            }
            writeln!(f, "{line}")?;
        }
    }

    let piles = [
        ("draw", "draw"),
        ("discard", "discard"),
        ("exhaust", "exhaust"),
    ];
    for (key, label) in piles {
        let pile = combat.list_at(key);
        if !pile.is_empty() {
            writeln!(f, "{label:<7} {}", cards(pile))?;
        }
    }

    if let Some(orbs) = combat.get("orbs").filter(|o| !o.is_null()) {
        writeln!(f, "orbs    {orbs}")?;
    }
    Ok(())
}
