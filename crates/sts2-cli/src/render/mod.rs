//! Human-readable rendering of an observation.
//! A formatting choice, which is why it is here and not in `sts2-headless`. The transport reports what the game said. How that reads at a terminal is this front end's opinion, and a second front end is entitled to a different one.
//! `Debug` on the response stays exhaustive and mechanical. This is the one meant to be read, and it reaches into the untyped `state` by key, the one place that assumes the observation's shape.
//! It degrades rather than failing: a section whose keys are missing is skipped. That is right for a display and is wrong anywhere else, nothing here feeds a comparison. Therefore a field this cannot find is a gap in the rendering rather than a divergence being hidden.

mod combat;
mod next;
mod state;

use state::state_of;

use serde_json::Value;
use std::fmt;
use sts2_headless::{Question, Response, RewardKind};

/// A response, rendered for a person.
///
/// A wrapper because `Response` belongs to `sts2-headless` and `Display` to the standard library, so this crate cannot implement one for the other. That is the orphan rule doing something useful for once: the rendering has to say whose choice it is.
pub struct Readable<'a>(pub &'a Response);

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

impl fmt::Display for Readable<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Response::Observation {
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
                state_of(f, state)?;
                next_of(f, self.0)
            }

            Response::Pending {
                seq,
                cmd,
                digest,
                state,
                question,
            } => {
                rule(f, *seq, cmd, digest)?;
                question_of(f, question)?;
                state_of(f, state)?;
                next_of(f, self.0)
            }

            Response::Bye => write!(f, "bye"),
        }
    }
}

fn rule(f: &mut fmt::Formatter<'_>, seq: u64, cmd: &str, digest: &str) -> fmt::Result {
    let head = format!("── #{seq} {cmd} ");
    // 60 columns, so two of these side by side still fit a normal terminal.
    let pad = 60usize.saturating_sub(head.chars().count() + digest.len() + 1);
    writeln!(f, "{head}{} {digest}", "─".repeat(pad))
}

/// A pending question, and the command that answers it.
///
/// The command is the point. A pending blocks everything except answering it, so a reader who cannot tell which of `answer cards`, `answer take` and `answer skip` applies is stuck. The two questions look alike until you know that one is a card selection and the other a reward screen. Naming the command inline is cheaper than expecting anyone to remember.
fn question_of(f: &mut fmt::Formatter<'_>, question: &Question) -> fmt::Result {
    match question {
        Question::SelectCards {
            min,
            max,
            asker,
            options,
        } => {
            let who = match asker.as_deref() {
                Some(asker) => format!(" for {asker}"),
                None => String::new(),
            };
            let count = match (min, max) {
                (a, b) if a == b => format!("exactly {a}"),
                (a, b) => format!("{a} to {b}"),
            };
            writeln!(f, "asks    pick {count}{who}:")?;
            for o in options {
                writeln!(f, "          {:<3} {}", o.index, o.card)?;
            }
        }
        Question::Rewards { rewards, .. } => {
            writeln!(f, "asks    take rewards:")?;

            // A card reward is two levels deep, which reward, then which card
            // inside it, and the game refuses a card reward taken without a
            // card rather than choosing one for you. So the inner options are
            // listed with the pair that names them, and the hint is one of
            // those pairs rather than a list of reward indices that is
            // refused.
            for r in rewards {
                let unpopulated = if r.populated { "" } else { "  [empty]" };
                match &r.reward {
                    RewardKind::Card { cards } => {
                        writeln!(f, "          {:<5} pick one card:{unpopulated}", r.index)?;
                        for (at, card) in cards.iter().enumerate() {
                            writeln!(f, "          {:<5} {card}", format!("{}.{at}", r.index))?;
                        }
                    }
                    other => {
                        writeln!(f, "          {:<5} {}{unpopulated}", r.index, plain(other))?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// A reward that is not a card, in one line.
fn plain(reward: &RewardKind) -> String {
    match reward {
        RewardKind::Gold { amount } => format!("{amount} gold"),
        RewardKind::Relic { relic, rarity } => match relic {
            Some(relic) => format!("relic {relic} ({rarity})"),
            None => format!("a {rarity} relic"),
        },
        RewardKind::Potion { potion } => match potion {
            Some(potion) => format!("potion {potion}"),
            None => "a potion".into(),
        },
        RewardKind::Other { kind } => format!("{kind} (not modelled here)"),
        // Listed with its cards by the caller, which is the only useful way to
        // show one.
        RewardKind::Card { cards } => format!("{} cards", cards.len()),
    }
}

/// Consecutive runs collapsed, order preserved.
///
/// Order is the point for a draw pile, `STRIKE · BASH · 3x STRIKE` says something a global tally will destroy.
pub(super) fn cards(list: &[Value]) -> String {
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

pub(super) fn render_run(name: &str, count: usize) -> String {
    if count == 1 {
        name.to_string()
    } else {
        format!("{count}x {name}")
    }
}

pub(super) fn names(list: &[Value]) -> String {
    list.iter()
        .map(|v| v.as_str().unwrap_or("?"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The one line that says what to do next.
fn next_of(f: &mut fmt::Formatter<'_>, response: &Response) -> fmt::Result {
    match next::next(response) {
        Some(command) => writeln!(f, "next    {command}"),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::Readable;
    use serde_json::json;
    use sts2_headless::Response;

    /// Rendered, with runs of spaces collapsed so a test is about the words rather than the column widths.
    fn rendered(pending: serde_json::Value) -> String {
        let response: Response =
            serde_json::from_value(pending).expect("a pending this crate can read");

        format!("{}", Readable(&response))
            .lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A card reward is two levels, and the hint has to say so.
    ///
    /// `sts2 choose 0 1` was the old hint and it was not merely unclear, it was wrong. The protocol refuses a card reward taken without a card rather than choosing one. Therefore it has sent two rewards with no card and been rejected. The dotted pair is the only form the game accepts.
    #[test]
    fn a_card_reward_lists_its_cards_and_hints_a_pair() {
        let out = rendered(json!({
            "type": "pending", "seq": 7, "cmd": "end_turn", "digest": "AA",
            "kind": "rewards", "skippable": true,
            "rewards": [
                {"index": 0, "type": "CardReward", "populated": true,
                 "cards": ["POISONED_STAB", "FTL", "SPOILS_OF_BATTLE"]},
                {"index": 1, "type": "GoldReward", "populated": true, "amount": 25}
            ],
            "state": {}
        }));

        assert!(out.contains("0.0 POISONED_STAB"), "{out}");
        assert!(out.contains("0.1 FTL"), "{out}");
        assert!(out.contains("0.2 SPOILS_OF_BATTLE"), "{out}");
        assert!(out.contains("25 gold"), "{out}");
        assert!(out.contains("next sts2 choose 0.0"), "{out}");
        assert!(out.contains("or: sts2 answer skip"), "{out}");
    }

    /// A reward with nothing inside it still says which it is.
    #[test]
    fn a_reward_that_is_not_a_card_needs_no_pair() {
        let out = rendered(json!({
            "type": "pending", "seq": 7, "cmd": "end_turn", "digest": "AA",
            "kind": "rewards", "skippable": false,
            "rewards": [{"index": 0, "type": "PotionReward", "populated": false}],
            "state": {}
        }));

        assert!(out.contains("a potion"), "{out}");
        assert!(out.contains("[empty]"), "{out}");
        assert!(out.lines().any(|l| l == "next sts2 choose 0"), "{out}");
        assert!(
            !out.contains("answer skip"),
            "unskippable, so do not offer it: {out}"
        );
    }
}
