//! The fight: creatures, piles, energy, and what is legal right now.

use super::prose::one_line;
use super::{cards, entry, flags, ident, spelling};
use num_traits::Zero;
use std::fmt;
use sts2_core::{CardIdent, CardVars, Combat, ContentVersion, Creature, Orbs, Playable, Preview};

pub(super) fn combat_of<V: ContentVersion>(
    f: &mut fmt::Formatter<'_>,
    combat: &Combat<V>,
) -> fmt::Result {
    // Round and turn are the same number in a singleplayer run and diverge only
    // when there is a second player to take one, so printing both every turn
    // spends a column on a value nobody reads.
    let rounds = if combat.round == combat.turn {
        String::new()
    } else {
        format!(" · round {}", combat.round)
    };
    writeln!(
        f,
        "combat  turn {}{rounds} · {}",
        combat.turn,
        spelling(&combat.phase)
    )?;

    for c in &combat.creatures {
        creature(f, c)?;
    }

    // The hand and the legality list are parallel arrays over the same cards, so
    // they are shown as one thing: an index you can play, what it will do, and
    // why you cannot.
    //
    // Energy belongs on this header and nowhere else. It was on the `combat`
    // line, four fields along, where it read as trivia about the turn — and the
    // one moment anybody wants it is while looking down this list deciding what
    // is affordable.
    if !combat.hand.is_empty() {
        writeln!(
            f,
            "hand    {} energy · {} stars",
            combat.energy, combat.stars
        )?;

        // Four Strikes are four indices and one decision. Printed a card at a
        // time they were four identical rows, each carrying its own copy of the
        // per-target damage — a dozen lines saying one thing, which is how the
        // one card that *is* different stops being noticed.
        let rows = same_cards(&combat.hand, &combat.legal);
        let width = rows.iter().map(|r| r.at.len()).max().unwrap_or(1).max(3);
        for row in rows {
            writeln!(f, "        {:<width$} {}", row.at, row.card)?;
            for effect in row.effects {
                writeln!(f, "        {:<width$} {effect}", "")?;
            }
        }
    }

    for (label, pile) in [
        ("draw", &combat.draw),
        ("discard", &combat.discard),
        ("exhaust", &combat.exhaust),
    ] {
        if !pile.is_empty() {
            writeln!(f, "{label:<7} {}", cards(pile))?;
        }
    }

    if let Some(orbs) = &combat.orbs {
        orbs_of(f, orbs)?;
    }
    Ok(())
}

fn creature<V: ContentVersion>(f: &mut fmt::Formatter<'_>, c: &Creature<V>) -> fmt::Result {
    // The category prefix is redundant next to the side, which says the same
    // thing more usefully — so the entry name alone.
    let model = match &c.model {
        Some(model) => entry(model),
        None => "?".to_string(),
    };
    let side = spelling(&c.side);
    let marker = if side == "Player" { "you" } else { "   " };

    let mut line = format!(
        "        {marker} {:<2} {model:<20} {}/{}",
        c.id, c.hp, c.max_hp
    );
    if !c.block.is_zero() {
        line.push_str(&format!(" +{} block", c.block));
    }
    if !c.alive {
        line.push_str(" DEAD");
    }

    if let Some(next) = &c.next_move {
        line.push_str(&format!(" → {}", spelling(&next.id)));
        // A move carries a list of intents, so an attack that also blocks
        // reports both. Only attacks have numbers; the rest name themselves,
        // because the game has no amount to report for them either.
        let detail: Vec<String> = next
            .intents
            .iter()
            .map(|i| {
                let kind = spelling(&i.kind).to_lowercase();
                match i.attack {
                    Some(a) if a.repeats > 1 => format!("{kind} {}×{}", a.damage, a.repeats),
                    Some(a) => format!("{kind} {}", a.damage),
                    None => kind,
                }
            })
            .collect();
        if !detail.is_empty() {
            line.push_str(&format!(" ({})", detail.join(", ")));
        }
    }

    // Always the amount. Hiding it at 1 saved four characters and cost the
    // difference between one turn of Vulnerable and two, which is the question a
    // player is actually asking when they look.
    //
    // `_POWER` is on every id and distinguishes none of them.
    let powers: Vec<String> = c
        .powers
        .iter()
        .map(|p| {
            let id = spelling(&p.power);
            let id = id.strip_suffix("_POWER").unwrap_or(&id).to_string();
            format!("{id} {}", p.amount)
        })
        .collect();
    if !powers.is_empty() {
        line.push_str(&format!("  [{}]", powers.join(", ")));
    }

    writeln!(f, "{line}")
}

/// The orb queue, which is a queue. The same orbs in another order evoke
/// differently, so this is printed in order and empty slots are shown.
fn orbs_of<V: ContentVersion>(f: &mut fmt::Formatter<'_>, orbs: &Orbs<V>) -> fmt::Result {
    let mut slots: Vec<String> = orbs
        .queue
        .iter()
        .map(|o| format!("{} {}/{}", spelling(&o.orb), o.passive, o.evoke))
        .collect();

    // The empty ones matter: channelling into a full queue evokes the front of
    // it, and how many are free is what says whether that is about to happen.
    slots.extend(std::iter::repeat_n("—".to_string(), orbs.free()));

    writeln!(f, "orbs    {}", slots.join(" · "))
}

/// One row of the hand: the cards that are the same card, and what they do.
///
/// `at` is a list because a hand holds duplicates and they are one choice.
struct Row {
    at: String,
    card: String,
    effects: Vec<String>,
}

/// The hand, with cards that do exactly the same thing shown once.
///
/// Sameness follows the rendered card rather than its id. Upgrades,
/// unplayability, and power effects already change that text. Thus cards grouped
/// here cannot differ in play through a displayed property.
///
/// Order is first appearance, so the indices stay roughly ascending and a hand
/// does not reshuffle itself between turns for cosmetic reasons.
fn same_cards<V: ContentVersion>(hand: &[CardIdent<V>], legal: &[Playable<V>]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();

    for (i, card) in hand.iter().enumerate() {
        let name = ident(card);
        let (card, effects) = match legal.get(i) {
            Some(l) => (format!("{name:<22}{}", playing(l)), effects_of(&l.effect)),
            None => (name, Vec::new()),
        };

        match rows
            .iter_mut()
            .find(|r| r.card == card && r.effects == effects)
        {
            Some(row) => row.at.push_str(&format!(",{i}")),
            None => rows.push(Row {
                at: i.to_string(),
                card,
                effects,
            }),
        }
    }

    rows
}

/// The cost and what it can be aimed at, or why it cannot be played.
fn playing<V: ContentVersion>(l: &Playable<V>) -> String {
    let mut line = format!(" {}", super::costs(l.cost, l.star_cost));

    // `reason` present is the whole of "unplayable" — `CardModel.CanPlay`
    // returns `reason == None` for a card that can be played — so there is no
    // second field to consult.
    if let Some(why) = &l.reason {
        line.push_str(&format!("  ✗ {}", flags(*why)));
        if let Some(by) = &l.preventer {
            line.push_str(&format!(" ({})", entry(by)));
        }
        return line;
    }

    // Both halves are needed. Ids alone leave an empty list ambiguous between
    // "targets itself, nothing to pick" and "targets enemies, none are alive" —
    // which are not at all the same thing to a caller deciding what to play.
    match l.targets.as_slice() {
        [] => line.push_str(&format!("  ({})", spelling(&l.target))),
        targets => {
            let ids: Vec<String> = targets.iter().map(ToString::to_string).collect();
            line.push_str(&format!("  → {}", ids.join(",")));
        }
    }
    line
}

/// What playing the card does, in the numbers it does it with.
///
/// Printed values need not be the outcome. Strength, Shrink, and Vulnerable can
/// all change Strike.s damage without changing its name, cost, or targets.
///
/// The oracle reports the game.s computed values. Distinct target outcomes get
/// separate prefixed lines. A single outcome has no prefix.
fn effects_of<V: ContentVersion>(previews: &[Preview<V>]) -> Vec<String> {
    let many = previews.len() > 1;

    previews
        .iter()
        .filter_map(|preview| {
            let text = preview.text.as_deref().map(one_line).unwrap_or_default();
            if text.is_empty() {
                return None;
            }
            let moved = moved_in(preview);
            Some(match preview.target.filter(|_| many) {
                Some(target) => format!("vs {target}: {text}{moved}"),
                None => format!("{text}{moved}"),
            })
        })
        .collect()
}

/// Which of the card's numbers are not the printed ones, and what they were.
///
/// The outcome text says what happens. This identifies changed printed values.
/// A Strike reading 4 otherwise looks like a card printed with 4.
///
/// This compares the oracle.s two maps. Colour is insufficient because `[red]`
/// also styles ordinary event prose.
fn moved_in<V: ContentVersion>(preview: &Preview<V>) -> String {
    let changed: Vec<String> = preview
        .base
        .iter()
        .filter_map(|(name, was)| {
            let is = preview.vars.get(name)?;
            (was != is).then(|| format!("{} {was}→{is}", name.name()))
        })
        .collect();

    match changed.is_empty() {
        true => String::new(),
        false => format!("   ({})", changed.join(", ")),
    }
}
