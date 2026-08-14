//! Human-readable rendering of a response.
//!
//! `Debug` stays exhaustive and mechanical; this is the one meant to be read.
//! It reaches into the untyped `state` by key, which is the one place in this
//! crate that knows the observation's shape.
//!
//! It degrades rather than failing: a section whose keys are missing is skipped.
//! That is right for a display and would be wrong anywhere else — nothing here
//! feeds a comparison, so a field this renderer cannot find is a gap in the
//! rendering rather than a divergence being hidden.

use crate::{Question, Response};
use serde_json::Value;
use std::fmt;

/// Fields a renderer asks for by name, returning nothing rather than panicking.
trait Field {
    fn str_at(&self, key: &str) -> Option<&str>;
    fn int_at(&self, key: &str) -> Option<i64>;
    fn list_at(&self, key: &str) -> &[Value];
    fn is_true(&self, key: &str) -> bool;
}

impl Field for Value {
    fn str_at(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    fn int_at(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_i64()
    }

    fn list_at(&self, key: &str) -> &[Value] {
        self.get(key).and_then(Value::as_array).map_or(&[], |v| v)
    }

    fn is_true(&self, key: &str) -> bool {
        self.get(key).and_then(Value::as_bool).unwrap_or(false)
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hello {
                protocol,
                game,
                commands,
                ..
            } => write!(
                f,
                "oracle {game}, protocol v{protocol}, {} commands",
                commands.len()
            ),

            Self::Observation {
                seq,
                cmd,
                resolved,
                digest,
                state,
                game_errors,
            } => {
                rule(f, *seq, cmd, digest)?;
                if !resolved {
                    writeln!(f, "note    unresolved: combat ended mid-decision")?;
                }
                if let Some(errors) = game_errors {
                    for e in errors {
                        writeln!(f, "error   {e}")?;
                    }
                }
                state_of(f, state)
            }

            Self::Pending {
                seq,
                cmd,
                digest,
                state,
                question,
            } => {
                rule(f, *seq, cmd, digest)?;
                question_of(f, question)?;
                state_of(f, state)
            }

            Self::Error { cmd, message } => write!(f, "refused {cmd}: {message}"),

            Self::Bye => write!(f, "bye"),
        }
    }
}

fn rule(f: &mut fmt::Formatter<'_>, seq: u64, cmd: &str, digest: &str) -> fmt::Result {
    let head = format!("── #{seq} {cmd} ");
    // 60 columns, so two of these side by side still fit a normal terminal.
    let pad = 60usize.saturating_sub(head.chars().count() + digest.len() + 1);
    writeln!(f, "{head}{} {digest}", "─".repeat(pad))
}

fn question_of(f: &mut fmt::Formatter<'_>, question: &Question) -> fmt::Result {
    match question {
        Question::SelectCards {
            min,
            max,
            asker,
            options,
        } => {
            let who = asker.as_deref().unwrap_or("something");
            writeln!(f, "asks    {who} wants {min}..={max} of:")?;
            for o in options {
                writeln!(f, "          {:<3} {}", o.index, o.card)?;
            }
        }
        Question::Rewards { skippable, rewards } => {
            let skip = if *skippable { "skippable" } else { "forced" };
            writeln!(f, "asks    rewards ({skip}):")?;
            for r in rewards {
                let unpopulated = if r.populated { "" } else { "  [empty]" };
                writeln!(f, "          {:<3} {:?}{unpopulated}", r.index, r.reward)?;
            }
        }
    }
    Ok(())
}

fn state_of(f: &mut fmt::Formatter<'_>, state: &Value) -> fmt::Result {
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

fn combat_of(f: &mut fmt::Formatter<'_>, combat: &Value) -> fmt::Result {
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
                    if !targets.is_empty() {
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
        // An option is described by whichever of these it carries; the key is
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

/// Only the streams that have moved. Fifteen counters of which one is non-zero
/// is noise; the one that moved is the signal.
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

/// Consecutive runs collapsed, order preserved.
///
/// Order is the point for a draw pile — `STRIKE · BASH · 3x STRIKE` says
/// something a global tally would destroy.
fn cards(list: &[Value]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut run: Option<(&str, usize)> = None;

    for item in list {
        let name = item.as_str().unwrap_or("?");
        match run {
            Some((prev, count)) if prev == name => run = Some((prev, count + 1)),
            Some((prev, count)) => {
                out.push(render_run(prev, count));
                run = Some((name, 1));
            }
            None => run = Some((name, 1)),
        }
    }
    if let Some((name, count)) = run {
        out.push(render_run(name, count));
    }
    out.join(" · ")
}

fn render_run(name: &str, count: usize) -> String {
    if count == 1 {
        name.to_string()
    } else {
        format!("{count}x {name}")
    }
}

fn names(list: &[Value]) -> String {
    list.iter()
        .map(|v| v.as_str().unwrap_or("?"))
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn observation(state: Value) -> Response {
        Response::Observation {
            seq: 1,
            cmd: "start".into(),
            resolved: true,
            digest: "7F46CA454F1CEE19".into(),
            state,
            game_errors: None,
        }
    }

    #[test]
    fn a_refusal_reads_as_a_sentence() {
        let r = Response::Error {
            cmd: "travel".into(),
            message: "(2,1) is not travelable from here".into(),
        };
        assert_eq!(
            r.to_string(),
            "refused travel: (2,1) is not travelable from here"
        );
    }

    #[test]
    fn consecutive_cards_collapse_but_keep_their_order() {
        let list = [
            json!("STRIKE"),
            json!("BASH"),
            json!("STRIKE"),
            json!("STRIKE"),
            json!("STRIKE"),
        ];
        assert_eq!(cards(&list), "STRIKE · BASH · 3x STRIKE");
    }

    #[test]
    fn a_single_card_is_not_counted() {
        assert_eq!(cards(&[json!("BASH")]), "BASH");
    }

    #[test]
    fn only_moved_rng_streams_are_named() {
        let out = observation(json!({
            "rng": {"UpFront": 411, "Shuffle": 0, "MonsterAi": 0, "Niche": 2}
        }))
        .to_string();
        assert!(out.contains("Niche 2"), "{out}");
        assert!(out.contains("UpFront 411"), "{out}");
        assert!(out.contains("(2 others at 0)"), "{out}");
        assert!(
            !out.contains("Shuffle"),
            "idle streams should stay quiet: {out}"
        );
    }

    /// The renderer reaches into an untyped value, so the thing worth pinning is
    /// that a state it does not recognise still produces a header instead of a
    /// panic.
    #[test]
    fn an_unfamiliar_state_still_renders() {
        let out = observation(json!({"something": "else"})).to_string();
        assert!(out.starts_with("── #1 start"), "{out}");
        assert!(out.contains("7F46CA454F1CEE19"), "{out}");
    }

    #[test]
    fn an_empty_state_does_not_panic() {
        assert!(observation(json!({})).to_string().contains("start"));
    }

    #[test]
    fn a_finished_run_says_how_it_ended() {
        let out = observation(json!({
            "hp": 0, "maxHp": 80, "floor": 12, "act": 1, "gold": 250,
            "alive": false, "runOver": true, "victory": false
        }))
        .to_string();
        assert!(out.contains("RUN OVER (defeat)"), "{out}");
    }

    #[test]
    fn combat_shows_who_is_in_it_and_what_they_intend() {
        let out = observation(json!({
            "combat": {
                "turn": 1, "round": 1, "phase": "Play", "energy": 3,
                "creatures": [
                    {"id": 0, "side": "Player", "model": "CHARACTER/IRONCLAD",
                     "hp": 80, "maxHp": 80, "block": 5, "alive": true,
                     "intent": null, "powers": []},
                    {"id": 1, "side": "Enemy", "model": "MONSTER/BOWLBUG_ROCK",
                     "hp": 48, "maxHp": 48, "block": 0, "alive": true,
                     "intent": "HEADBUTT_MOVE",
                     "powers": [{"id": "IMBALANCED_POWER", "amount": 1}]}
                ],
                "hand": ["STRIKE_IRONCLAD"],
                "legal": [{"card": "STRIKE_IRONCLAD", "cost": 1, "playable": true,
                           "target": "AnyEnemy", "targets": [1], "reason": null,
                           "preventer": null}],
                "draw": [], "discard": [], "exhaust": [], "orbs": null
            }
        }))
        .to_string();

        assert!(out.contains("turn 1 · round 1 · Play · 3 energy"), "{out}");
        assert!(out.contains("+5 block"), "{out}");
        assert!(out.contains("→ HEADBUTT_MOVE"), "{out}");
        assert!(out.contains("IMBALANCED_POWER"), "{out}");
        // The category prefix is dropped; the side already says as much.
        assert!(out.contains("BOWLBUG_ROCK"), "{out}");
        assert!(!out.contains("MONSTER/"), "{out}");
        // The hand carries its playable target ids.
        assert!(out.contains("→ 1"), "{out}");
    }

    #[test]
    fn an_unplayable_card_says_why_and_what_blocked_it() {
        let out = observation(json!({
            "combat": {
                "turn": 1, "round": 1, "phase": "Play", "energy": 0,
                "creatures": [],
                "hand": ["STRIKE_IRONCLAD"],
                "legal": [{"card": "STRIKE_IRONCLAD", "cost": 1, "playable": false,
                           "reason": "NotEnoughEnergy", "preventer": "ENTANGLED_POWER",
                           "target": "AnyEnemy", "targets": []}],
                "draw": [], "discard": [], "exhaust": []
            }
        }))
        .to_string();
        assert!(out.contains("✗ NotEnoughEnergy"), "{out}");
        assert!(out.contains("(ENTANGLED_POWER)"), "{out}");
    }

    #[test]
    fn an_event_lists_what_it_offers() {
        let out = observation(json!({
            "room": {
                "type": "Event", "id": "NEOW", "finished": false,
                "options": [
                    {"index": 0, "relic": "GOLDEN_PEARL", "chosen": false,
                     "locked": false, "proceed": false,
                     "key": "NEOW.pages.INITIAL.options.GOLDEN_PEARL"},
                    {"index": 1, "relic": "ARCANE_SCROLL", "chosen": false,
                     "locked": true, "proceed": false,
                     "key": "NEOW.pages.INITIAL.options.ARCANE_SCROLL"}
                ]
            }
        }))
        .to_string();
        assert!(out.contains("room    Event NEOW"), "{out}");
        assert!(out.contains("0   GOLDEN_PEARL"), "{out}");
        assert!(out.contains("[locked]"), "{out}");
    }

    #[test]
    fn an_unresolved_observation_says_so() {
        let Response::Observation { seq, cmd, .. } = observation(json!({})) else {
            unreachable!()
        };
        let r = Response::Observation {
            seq,
            cmd,
            resolved: false,
            digest: "X".into(),
            state: json!({}),
            game_errors: Some(vec!["boom".into()]),
        };
        let out = r.to_string();
        assert!(out.contains("unresolved"), "{out}");
        assert!(out.contains("error   boom"), "{out}");
    }
}
