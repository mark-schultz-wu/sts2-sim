//! Complete observations, for tests to bend one corner of.
//!
//! Everything here goes through the same conversion the game's own replies do.
//! A fixture has to be a state the mod itself sends: every key present, every
//! list element whole. That is stricter than the tests used to be, and it is the
//! point. The old fixtures wrote a creature with no `id`, and a monster model of
//! `"BOWLBUG"` where the wire says `"MONSTER/BOWLBUG"`. They agreed with code
//! that disagreed with the game.
//!
//! [`state`] merges an overlay into a whole observation, so each test's JSON is
//! about its own subject. Objects merge key by key, and arrays and scalars
//! replace. A test naming three cards means those three, not those three plus
//! whatever was here.

use serde_json::{Value, json};
use sts2_core::Applied;

type V = sts2_content::v0_107_1::V0_107_1;

/// A whole observation, with `overlay` merged into it.
pub fn state(overlay: Value) -> Value {
    let mut base = json!({
        "act": 0, "floor": 1, "hp": 72, "maxHp": 80, "alive": true,
        "runOver": false, "victory": null, "gold": 99,
        "relics": [], "potions": [], "potionSlots": 3,
        "deck": [], "inCombat": false,
        "map": {
            "at": "3,11",
            "visited": ["3,11"],
            "travelable": [],
            "points": [{"at": "3,11", "type": "Ancient", "to": []}],
            "proceed": false
        },
        "rng": {}
    });
    merge(&mut base, overlay);
    fill(&mut base);
    base
}

/// The parts a test writes one corner of.
///
/// A combat block, an event option and a preview each carry a dozen keys that
/// no test is about. Filled here rather than in every fixture, so what a test
/// writes stays the part it means. The whole thing still has to be a state the
/// mod itself sends.
fn fill(state: &mut Value) {
    if let Some(block) = state.get_mut("combat")
        && !block.is_null()
    {
        let mut whole = combat(json!({}));
        merge(&mut whole, block.take());
        for card in whole["legal"].as_array_mut().into_iter().flatten() {
            let mut base = playable("STRIKE_IRONCLAD", &[]);
            merge(&mut base, card.take());
            for preview in base["effect"].as_array_mut().into_iter().flatten() {
                let mut shape = json!({"target": null, "text": "", "vars": {}, "base": {}});
                merge(&mut shape, preview.take());
                *preview = shape;
            }
            *card = base;
        }
        for c in whole["creatures"].as_array_mut().into_iter().flatten() {
            let mut base = creature(1, "Enemy", None, &[]);
            merge(&mut base, c.take());
            *c = base;
        }
        *block = whole;
    }

    // A relic or a potion may be written as its bare id, because most fixtures
    // are not about what it says or whether it can be drunk. Both are widened
    // here into the shape the mod sends; a fixture that *is* about one writes
    // the object and keeps whatever it wrote.
    for relic in state["relics"].as_array_mut().into_iter().flatten() {
        let mut base = json!({"id": null, "title": null, "text": null});
        merge(&mut base, named(relic.take()));
        *relic = base;
    }
    for potion in state["potions"].as_array_mut().into_iter().flatten() {
        let mut base = json!({"id": null, "title": null, "text": null,
                              "usable": true, "reason": null, "targets": []});
        merge(&mut base, named(potion.take()));
        *potion = base;
    }

    if state.get("room").and_then(|r| r.get("type")) == Some(&json!("Event")) {
        let room = &mut state["room"];
        let mut whole = json!({"type": "Event", "id": "WELLSPRING", "finished": false,
                               "title": null, "text": null, "options": []});
        merge(&mut whole, room.take());
        for option in whole["options"].as_array_mut().into_iter().flatten() {
            let mut base = json!({"index": 0, "key": "K", "title": null, "text": null,
                                  "locked": false, "proceed": false, "chosen": false,
                                  "relic": null});
            merge(&mut base, option.take());
            *option = base;
        }
        *room = whole;
    }
}

/// What that observation says.
///
/// What almost every reader wants. Everything but the renderer works from
/// [`sts2_core::Applied`], because the sequence number and the game's own digest
/// are not facts about the run.
pub fn applied(overlay: Value) -> Applied<V> {
    answered(json!({
        "type": "observation", "seq": 1, "cmd": "observe",
        "resolved": true, "digest": "AA", "state": state(overlay),
    }))
}

/// A whole reply, a pending for instance, with a complete state put under it.
///
/// Built as a wire message and read back through `sts2-headless`. A fixture then
/// exercises the same parser a real reply does, rather than a second shape that
/// only the tests believe in.
pub fn answered(mut value: Value) -> Applied<V> {
    // Whatever state the reply carries is completed rather than replaced, so a
    // test that writes one corner of it keeps that corner.
    let carried = value.get("state").cloned().unwrap_or(json!({}));
    value["state"] = state(carried);
    let response: sts2_headless::Response =
        serde_json::from_value(value).expect("a response this crate can read");
    response
        .applied::<V>()
        .expect("a state this crate can read")
        .expect("a fixture is never a goodbye")
}

/// A combat block, with `overlay` merged into it.
pub fn combat(overlay: Value) -> Value {
    let mut base = json!({
        "turn": 1, "phase": "Play", "energy": 3, "round": 1,
        "encounter": null, "hand": [], "draw": [], "discard": [], "exhaust": [],
        "creatures": [], "legal": [], "orbs": null
    });
    merge(&mut base, overlay);
    base
}

/// One card in hand, playable, against these targets.
pub fn playable(card: &str, targets: &[u32]) -> Value {
    json!({
        "card": card, "cost": 1, "target": "AnyEnemy", "playable": true,
        "reason": null, "preventer": null, "targets": targets, "effect": []
    })
}

/// One card in hand that cannot be played, for that reason.
pub fn unplayable(card: &str, reason: &str) -> Value {
    json!({
        "card": card, "cost": 1, "target": "None", "playable": false,
        "reason": reason, "preventer": null, "targets": [], "effect": []
    })
}

/// One creature, with the model spelled the way the wire spells it.
pub fn creature(id: u32, side: &str, model: Option<&str>, powers: &[(&str, i32)]) -> Value {
    json!({
        "id": id, "model": model, "side": side,
        "hp": 20, "maxHp": 20, "block": 0, "alive": true,
        "powers": powers.iter()
            .map(|(id, amount)| json!({"id": id, "amount": amount}))
            .collect::<Vec<_>>(),
        "intent": null, "intents": null
    })
}

/// A bare id read as the object it is shorthand for.
fn named(value: Value) -> Value {
    match value {
        Value::String(id) => json!({"id": id}),
        other => other,
    }
}

/// Objects merge, and anything else replaces.
fn merge(into: &mut Value, from: Value) {
    match (into, from) {
        (Value::Object(into), Value::Object(from)) => {
            for (key, value) in from {
                merge(into.entry(key).or_insert(Value::Null), value);
            }
        }
        (into, from) => *into = from,
    }
}
