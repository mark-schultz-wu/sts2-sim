//! The run outside a fight: the map point, the deck, and the RNG counters.

use super::combat::combat_of;
use super::prose::one_line;
use super::{cards, ident, names, spelling};
use std::fmt;
use sts2_core::{
    Blocked, ChestOption, Counters, Entry, EventOption, Observation, RestOption, Room, Stock, Take,
};

pub(super) fn state_of<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    obs: &Observation<V>,
    rng: bool,
) -> fmt::Result {
    let mut line = format!(
        "run     floor {} · act {} · hp {}/{} · gold {}",
        obs.floor, obs.act, obs.hp, obs.max_hp, obs.gold
    );
    if obs.run_over {
        // `victory` and not `alive`: winning kills every player too, so `alive`
        // cannot tell a win from a loss and this is the field that does.
        let outcome = match obs.victory {
            Some(true) => "victory",
            _ => "defeat",
        };
        line.push_str(&format!(" · RUN OVER ({outcome})"));
    } else if !obs.alive {
        line.push_str(" · dead");
    }
    writeln!(f, "{line}")?;

    if !obs.deck.is_empty() {
        writeln!(f, "deck    {}", cards(&obs.deck))?;
    }
    if !obs.relics.is_empty() {
        let relics: Vec<_> = obs.relics.iter().map(|r| r.relic).collect();
        writeln!(f, "relics  {}", names(&relics))?;
    }
    if !obs.potions.is_empty() {
        let potions: Vec<_> = obs.potions.iter().map(|p| p.potion).collect();
        writeln!(f, "potions {}", names(&potions))?;
    }

    let at = match obs.map.at {
        Some(at) => at.to_string(),
        None => "?".to_string(),
    };
    let to = match obs.map.travelable.as_slice() {
        [] => "nowhere".to_string(),
        list => list
            .iter()
            .map(|p| format!("{} ({})", p.at, spelling(&p.kind)))
            .collect::<Vec<_>>()
            .join(" · "),
    };
    writeln!(f, "map     at {at} → {to}")?;

    // A fight and a room are exclusive in practice, and the fight is what a
    // reader is looking at while there is one. `fight()` is the pair of fields
    // read as one question, so a stale combat block cannot show through.
    if let Some(combat) = obs.fight() {
        combat_of(f, combat)?;
    } else if let Some(room) = &obs.room {
        room_of(f, room)?;
    }

    if rng {
        rng_of(f, &obs.rng)?;
    }
    Ok(())
}

/// The row of flags after an option, or nothing.
fn flagged(flags: &[&str]) -> String {
    match flags.is_empty() {
        true => String::new(),
        false => format!("  [{}]", flags.join(", ")),
    }
}

/// One option's line: its index, what it is, and what is wrong with it.
fn option(f: &mut fmt::Formatter<'_>, index: u8, what: &str, flags: &[&str]) -> fmt::Result {
    writeln!(f, "          {index:<3} {what}{}", flagged(flags))
}

/// A sentence under the option it belongs to.
///
/// Indented rather than run onto the same line: an event option's description
/// is a sentence, not a label.
fn detail(f: &mut fmt::Formatter<'_>, text: Option<&String>) -> fmt::Result {
    match text.map(|t| one_line(t)).filter(|t| !t.is_empty()) {
        Some(text) => writeln!(f, "              {text}"),
        None => Ok(()),
    }
}

fn room_of<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    room: &Room<V>,
) -> fmt::Result {
    // The room's own name, and the thing it is a room *of* where there is one.
    // Only an event has an id; a shop and a rest site are the whole line, and a
    // blank id would leave a space hanging off the end of it.
    let (kind, finished) = match room {
        Room::RestSite { .. } => ("RestSite".to_string(), false),
        Room::Treasure { finished, .. } => ("Treasure".to_string(), *finished),
        Room::Event {
            event, finished, ..
        } => (format!("Event {}", spelling(event)), *finished),
        Room::Shop { .. } => ("Shop".to_string(), false),
    };
    writeln!(
        f,
        "room    {kind}{}",
        if finished { " (finished)" } else { "" }
    )?;

    match room {
        Room::RestSite { options } => rest_options(f, options),
        Room::Treasure { options, .. } => chest_options(f, options),
        Room::Event { text, options, .. } => {
            // The prose, and not the title beside it: the room line above
            // already names the event, and an event's title is that name in
            // words — `Event NEOW` then `Neow` is one fact twice.
            //
            // The game resolves the localisation keys; the oracle passes the
            // words along; this prints them.
            if let Some(text) = text.as_ref().map(|t| one_line(t)).filter(|t| !t.is_empty()) {
                writeln!(f, "        {text}")?;
            }
            event_options(f, options)
        }
        Room::Shop { entries } => shop(f, entries),
    }
}

fn rest_options<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    options: &[RestOption<V>],
) -> fmt::Result {
    for o in options {
        // The game refuses a disabled option, so a listing that shows it
        // exactly like the others is inviting a refusal.
        let flags: &[&str] = if o.enabled { &[] } else { &["disabled"] };
        option(f, o.index, &spelling(&o.id), flags)?;
    }
    Ok(())
}

fn chest_options<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    options: &[ChestOption<V>],
) -> fmt::Result {
    for o in options {
        let what = match &o.take {
            Take::Relic(relic) => spelling(relic),
            // Skipping is one of the options rather than a separate command, so
            // it needs a name in the list like the rest of them.
            Take::Skip => "skip".to_string(),
        };
        let flags: &[&str] = if o.chosen { &["chosen"] } else { &[] };
        option(f, o.index, &what, flags)?;
    }
    Ok(())
}

fn event_options<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    options: &[EventOption<V>],
) -> fmt::Result {
    for o in options {
        // Described by whichever of these it carries. The title is the words a
        // player would read; the relic and then the key are the fallbacks,
        // because every option has a key and some have nothing else.
        let what = o
            .title
            .as_ref()
            .map(|t| one_line(t))
            .filter(|t| !t.is_empty())
            .or_else(|| o.relic.as_ref().map(spelling))
            .unwrap_or_else(|| super::option_key(o));

        let mut flags = Vec::new();
        if o.chosen {
            flags.push("chosen");
        }
        if o.locked {
            flags.push("locked");
        }
        if o.proceed {
            flags.push("proceed");
        }
        option(f, o.index, &what, &flags)?;
        detail(f, o.text.as_ref())?;
    }
    Ok(())
}

/// A shop's stock: bought rather than chosen, so it is priced rather than
/// described.
fn shop<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    entries: &[Entry<V>],
) -> fmt::Result {
    for e in entries {
        // Card removal is a service rather than a thing, so it has nothing to
        // name. The kind beside it already says what it is, and a `?` would
        // read as a field the oracle failed to report.
        let (sells, what) = match &e.stock {
            Stock::Card(card) => (
                "card",
                card.as_ref().map(|c| ident(&c.card)).unwrap_or_default(),
            ),
            Stock::Relic(relic) => ("relic", relic.as_ref().map(spelling).unwrap_or_default()),
            Stock::Potion(potion) => ("potion", potion.as_ref().map(spelling).unwrap_or_default()),
            Stock::CardRemoval => ("card removal", String::new()),
        };

        let mut flags = Vec::new();
        if !e.stocked {
            flags.push("sold");
        } else if !e.affordable {
            // Only worth saying while there is still something to buy.
            flags.push("too dear");
        } else if let Some(Blocked::NoPotionSlot) = e.blocked {
            // Stocked and affordable and still refused — a potion with nowhere
            // to go. Without this the row looks buyable and is not.
            flags.push("no potion slot");
        }

        writeln!(
            f,
            "          {:<3} {sells:<13}{what:<24} {}g{}",
            e.index,
            e.cost,
            flagged(&flags)
        )?;
    }
    Ok(())
}

/// Only the streams that have moved. Fifteen counters of which one is non-zero
/// is noise, and the one that moved is the signal.
///
/// The two sets are printed as one list. The game keeps them apart because one
/// is per run and the other per player. A reader looking for the stream that
/// moved does not care which map it came out of.
fn rng_of<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    rng: &Counters<V>,
) -> fmt::Result {
    let mut moved: Vec<String> = Vec::new();
    let mut idle = 0;

    for (name, draws) in rng
        .run
        .iter()
        .map(|(k, n)| (spelling(k), *n))
        .chain(rng.player.iter().map(|(k, n)| (spelling(k), *n)))
    {
        match draws {
            0 => idle += 1,
            n => moved.push(format!("{name} {n}")),
        }
    }

    if moved.is_empty() && idle == 0 {
        return Ok(());
    }

    // Sorted, because the counters live in hash maps and an order that moves
    // between two prints of the same state is a diff that is not one.
    moved.sort();
    let rest = match idle {
        0 => String::new(),
        n => format!("  ({n} others at 0)"),
    };
    writeln!(f, "rng     {}{rest}", moved.join(" · "))
}
