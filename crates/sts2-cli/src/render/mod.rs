//! Human-readable rendering of an observation.
//!
//! A formatting choice, which is why it is here and not in `sts2-headless`. The
//! transport reports what the game said. How that reads at a terminal is this
//! front end's opinion, and a second front end is entitled to a different one.
//!
//! `Debug` on the response stays exhaustive and mechanical. This is the one
//! meant to be read, and it reaches into the untyped `state` by key. That makes
//! it the one place assuming the observation's shape.
//!
//! It degrades rather than failing, and a section whose keys are missing is
//! skipped. That is right for a display and wrong anywhere else. Nothing here
//! feeds a comparison, so a field this cannot find is a gap in the rendering
//! rather than a divergence being hidden.

mod combat;
pub mod coverage;
pub mod map;
mod next;
pub mod prose;
pub mod refusal;
mod state;

use prose::{asker as who_asked, one_line};
use state::state_of;

use std::fmt;
use sts2_core::{CardIdent, Cost, Decided, FlagSet, ModelId, Question, RewardKind, Snapshot};

/// What one step produced, rendered for a person.
///
/// Wraps [`Applied`], which any implementation produces. This keeps rendering
/// independent of the oracle.
///
/// The wrapper permits this crate to implement `Display` without violating the
/// orphan rule. It also carries whose choice is being rendered.
pub struct Readable<'a, V: sts2_content_wire::WireContentVersion> {
    snapshot: &'a Snapshot<V>,
    /// Absent for a look, which decided nothing to report on.
    decided: Option<Decided>,
    rule: Option<Rule<'a>>,
    errors: &'a [String],
    rng: bool,
}

/// The line at the top of a step, for an implementation that has one.
///
/// Holds only the protocol envelope: sequence, command name, and game digest. A
/// simulator has none. Supplying these values keeps the renderer usable by
/// either implementation.
struct Rule<'a> {
    seq: u64,
    cmd: &'a str,
    digest: &'a str,
}

impl<'a, V: sts2_content_wire::WireContentVersion> Readable<'a, V> {
    /// The player's view: what a person sitting down to play can see.
    pub fn new(snapshot: &'a Snapshot<V>) -> Self {
        Self {
            snapshot,
            decided: None,
            rule: None,
            errors: &[],
            rng: false,
        }
    }

    /// The envelope the oracle's protocol wraps a reply in.
    pub fn in_reply(mut self, seq: u64, cmd: &'a str, digest: &'a str) -> Self {
        self.rule = Some(Rule { seq, cmd, digest });
        self
    }

    /// What the game logged while running the command.
    ///
    /// Never suppressed. The command can have half-happened, and only a reader
    /// decides whether to trust the state that came back.
    pub fn logging(mut self, errors: &'a [String]) -> Self {
        self.errors = errors;
        self
    }

    /// Also the RNG stream counters.
    ///
    /// Off by default because a player cannot see them. They remain available
    /// for divergence diagnosis, where an unexpected counter movement is useful
    /// evidence.
    pub fn with_rng(mut self) -> Self {
        self.rng = true;
        self
    }
}

impl<V: sts2_content_wire::WireContentVersion> fmt::Display for Readable<'_, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snapshot = self.snapshot;

        if let Some(Rule { seq, cmd, digest }) = self.rule {
            rule(f, seq, cmd, digest)?;
        }

        // A pending is unresolved by construction — the game stopped to ask
        // something — so saying so there would be noise on every question.
        if self.decided == Some(Decided::Abandoned) && snapshot.asking.is_none() {
            writeln!(f, "note    unresolved: combat ended mid-decision")?;
        }
        for e in self.errors {
            writeln!(f, "error   {e}")?;
        }

        if let Some(question) = snapshot.asking.as_ref() {
            question_of(f, question)?;
        }
        state_of(f, &snapshot.observation, self.rng)?;
        next_of(f, snapshot)
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
/// The command is the point. A pending blocks everything except answering it,
/// so a reader who cannot tell which of `answer cards`, `answer take` and
/// `answer skip` applies is stuck. The two questions look alike until you know
/// that one is a card selection and the other a reward screen. Naming the
/// command inline is cheaper than expecting anyone to remember.
fn question_of<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    question: &Question<V>,
) -> fmt::Result {
    match question {
        Question::SelectCards {
            min,
            max,
            asker,
            options,
        } => {
            let who = match asker.as_deref() {
                Some(asker) => format!(" for {}", who_asked(asker)),
                None => String::new(),
            };
            let count = match (min, max) {
                (a, b) if a == b => format!("exactly {a}"),
                (a, b) => format!("{a} to {b}"),
            };
            writeln!(f, "asks    pick {count}{who}:")?;
            for o in options {
                writeln!(
                    f,
                    "          {:<3} {:<22}{}",
                    o.index,
                    ident(&o.card),
                    says(&o.text)
                )?;
            }
        }
        Question::Divine {
            left,
            width,
            height,
            cleared,
            revealed,
            ..
        } => {
            writeln!(
                f,
                "asks    divine {left} more time{}, on a {width}x{height} grid:",
                if *left == 1 { "" } else { "s" }
            )?;
            writeln!(
                f,
                "          {} of {} cells uncovered",
                cleared.len(),
                width * height
            )?;
            for item in revealed {
                writeln!(
                    f,
                    "          {} at ({},{}) {}",
                    item.item,
                    item.x,
                    item.y,
                    if item.good { "" } else { "-- bad" }
                )?;
            }
        }
        Question::Rewards { rewards, .. } => {
            writeln!(f, "asks    take rewards:")?;

            // A card reward is two levels deep — which reward, then which card
            // inside it — and the game refuses a card reward taken without a
            // card rather than choosing one for you. So the inner options are
            // listed with the pair that names them, and the hint is one of
            // those pairs rather than a list of reward indices that would be
            // refused.
            //
            // Taking one no longer forfeits the rest, so the same screen comes
            // back with one more reward marked taken each time. Those stay in
            // the list with their numbers: the indices are the game's, they do
            // not renumber, and a reward vanishing between two questions would
            // read as one having been lost.
            for r in rewards {
                let mut note = String::new();
                if !r.populated {
                    note.push_str("  [empty]");
                }
                if r.taken {
                    note.push_str("  [taken]");
                }
                match &r.reward {
                    RewardKind::Card { cards } => {
                        writeln!(f, "          {:<5} pick one card:{note}", r.index)?;
                        for (at, card) in cards.iter().enumerate() {
                            writeln!(
                                f,
                                "          {:<5} {:<22}{}",
                                format!("{}.{at}", r.index),
                                ident(&card.card),
                                says(&card.text)
                            )?;
                        }
                    }
                    other => {
                        writeln!(f, "          {:<5} {}{note}", r.index, plain(other))?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// A power and how much of it, as a person reads one.
///
/// An endless power has no number. `NPower` shows an amount label for a
/// counter and nothing for a single instance. A number here is the game's own
/// spelling of "endless" on a screen a person reads.
pub fn stacked<N: std::fmt::Display>(name: &str, amount: &sts2_core::PowerAmount<N>) -> String {
    match amount {
        sts2_core::PowerAmount::Counted(amount) => format!("{name} {amount}"),
        sts2_core::PowerAmount::Endless => name.to_string(),
    }
}

/// A pool a card is priced in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pool {
    Energy,
    Stars,
}

/// How much of a pool a card spends.
///
/// There is no variant for a pool the card is not priced in. `pools` drops that
/// case, so no screen has to spell it, and none can forget to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Amount {
    /// Everything left in the pool.
    All,
    Of(u8),
}

impl fmt::Display for Amount {
    /// The number, or `X` for a cost that takes the pool.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Amount::All => f.write_str("X"),
            Amount::Of(n) => write!(f, "{n}"),
        }
    }
}

/// Each pool a card is priced in, and what it spends there.
///
/// A pool the card is not priced in is left out rather than reported as a zero.
/// The two are not the same thing. `Cost::None` makes Confused skip the card,
/// where `Fixed(0)` does not. Three screens show a cost, and each used to decide
/// this again. This is the one place that decides it now.
pub fn pools(energy: Cost, stars: Cost) -> impl Iterator<Item = (Amount, Pool)> {
    [(energy, Pool::Energy), (stars, Pool::Stars)]
        .into_iter()
        .filter_map(|(cost, pool)| match cost {
            Cost::None => None,
            Cost::X => Some((Amount::All, pool)),
            Cost::Fixed(n) => Some((Amount::Of(n.get()), pool)),
        })
}

/// What a card costs, in the pools it is actually priced in.
///
/// `1e`, `1e 2★`, `Xe`, or `free`. Read in a line of text, so the energy
/// amount carries a mark. Nothing else there says which pool it means.
pub fn costs(energy: Cost, stars: Cost) -> String {
    let spelled: Vec<String> = pools(energy, stars)
        .map(|(amount, pool)| match pool {
            Pool::Energy => format!("{amount}e"),
            Pool::Stars => format!("{amount}★"),
        })
        .collect();

    if spelled.is_empty() {
        "free".to_string()
    } else {
        spelled.join(" ")
    }
}

/// A card's own words, beside its id, or nothing when the game sent none.
fn says(text: &Option<String>) -> String {
    match text.as_deref().map(one_line) {
        Some(text) if !text.is_empty() => text,
        _ => String::new(),
    }
}

/// A reward that is not a card, in one line.
fn plain<V: sts2_content_wire::WireContentVersion>(reward: &RewardKind<V>) -> String {
    match reward {
        RewardKind::Gold { amount } => format!("{amount} gold"),
        RewardKind::Relic { relic, rarity } => {
            let rarity = spelling(rarity);
            match relic {
                Some(relic) => format!("relic {} ({rarity})", spelling(relic)),
                None => format!("a {rarity} relic"),
            }
        }
        RewardKind::Potion { potion } => match potion {
            Some(potion) => format!("potion {}", spelling(potion)),
            None => "a potion".into(),
        },
        RewardKind::SpecialCard { card } => match card {
            Some(card) => format!("card {} returned", ident(&card.card)),
            None => "a returned card".into(),
        },
        RewardKind::Other { kind } => format!("{kind} (not modelled here)"),
        // Listed with its cards by the caller, which is the only useful way to
        // show one.
        RewardKind::Card { cards } => format!("{} cards", cards.len()),
    }
}

/// Consecutive runs collapsed, order preserved.
///
/// Order is the point for a draw pile. `STRIKE · BASH · 3x STRIKE` says
/// something a global tally destroys.
pub(super) fn cards<V: sts2_content_wire::WireContentVersion>(list: &[CardIdent<V>]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut run: Option<(String, usize)> = None;

    for card in list {
        // The packed spelling: two Strikes are one run only if they are the
        // same card, and an upgrade is what makes them not.
        let name = ident(card);
        match run.take() {
            Some((prev, count)) if prev == name => run = Some((prev, count + 1)),
            Some((prev, count)) => {
                out.push(render_run(&prev, count));
                run = Some((name, 1));
            }
            None => run = Some((name, 1)),
        }
    }
    if let Some((name, count)) = run {
        out.push(render_run(&name, count));
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

// The display-safe spelling remains under `render` for existing rendering callers.
pub use crate::id::display as spelling;

/// An event option's key, spelled the way the game writes it.
///
/// The parts are typed, so this is the one place that puts the sentence back
/// together. A front end is where a name becomes words.
pub fn option_key<V: sts2_content_wire::WireContentVersion>(
    option: &sts2_core::EventOption<V>,
) -> String {
    use sts2_core::EventOptionKinds;
    let name = V::event_option_name(option.key.option, option.locked)
        .map(str::to_owned)
        .or_else(|| option.key.option.relic().map(|relic| spelling(&relic)))
        .expect("a comparable event option has a boundary spelling");
    format!(
        "{}.pages.INITIAL.options.{name}",
        spelling(&option.key.event)
    )
}

/// A card identity, the way the observation spells one.
///
/// `BASH+1/SMOG*ADROIT`: the card, then what it is carrying. A person reading a
/// selection beside a deck listing sees the same card written the same way in
/// both.
pub fn ident<V: sts2_content_wire::WireContentVersion>(card: &CardIdent<V>) -> String {
    let mut out = spelling(&card.card);
    if card.upgrade > 0 {
        out.push_str(&format!("+{}", card.upgrade));
    }
    if let Some(affliction) = &card.affliction {
        out.push('/');
        out.push_str(&spelling(affliction));
    }
    if let Some(enchantment) = &card.enchantment {
        out.push('*');
        out.push_str(&spelling(enchantment));
    }
    out
}

/// The entry name of a model id, without its category.
///
/// Half an identity and not one, because `BYRDPIP` is a monster *and* a relic.
/// That is fine for a reader who has the category beside it.
pub fn entry<V: sts2_content_wire::WireContentVersion>(model: &ModelId<V>) -> String {
    match model {
        ModelId::Act(id) => spelling(id),
        ModelId::Affliction(id) => spelling(id),
        ModelId::Card(id) => spelling(id),
        ModelId::Character(id) => spelling(id),
        ModelId::Enchantment(id) => spelling(id),
        ModelId::Encounter(id) => spelling(id),
        ModelId::Event(id) => spelling(id),
        ModelId::Modifier(id) => spelling(id),
        ModelId::Monster(id) => spelling(id),
        ModelId::Orb(id) => spelling(id),
        ModelId::Potion(id) => spelling(id),
        ModelId::Power(id) => spelling(id),
        ModelId::Relic(id) => spelling(id),
    }
}

/// A flag set, by the names its build gives the members it holds.
///
/// The game's own spelling: the members joined with `", "`, which is what
/// `[Flags].ToString()` writes and what the oracle put on the wire.
pub fn flags<S: FlagSet<Member: serde::Serialize + std::fmt::Debug>>(set: S) -> String {
    S::MEMBERS
        .iter()
        .filter(|member| set.contains(**member))
        .map(spelling)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A list of ids, by the names their build gives them.
pub(super) fn names<T: serde::Serialize + std::fmt::Debug>(list: &[T]) -> String {
    list.iter().map(spelling).collect::<Vec<_>>().join(" · ")
}

/// The one line that says what to do next.
fn next_of<V: sts2_content_wire::WireContentVersion>(
    f: &mut fmt::Formatter<'_>,
    snapshot: &Snapshot<V>,
) -> fmt::Result {
    match next::next(snapshot) {
        Some(command) => writeln!(f, "next    {command}"),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::Readable;
    use serde_json::json;

    /// Rendered, with runs of spaces collapsed so a test is about the words
    /// rather than the column widths.
    fn rendered(pending: serde_json::Value) -> String {
        shown(pending, false)
    }

    fn shown(value: serde_json::Value, rng: bool) -> String {
        let snapshot = crate::fixture::answered(value).snapshot();
        let readable = Readable::new(&snapshot).in_reply(1, "observe", "AA");
        let readable = if rng { readable.with_rng() } else { readable };

        format!("{readable}")
            .lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// An observation, for the tests that are about state rather than a question.
    ///
    /// The state is completed by the fixture, so what each test writes is the
    /// part it is about.
    fn observation(state: serde_json::Value) -> serde_json::Value {
        json!({
            "type": "observation", "seq": 1, "cmd": "observe",
            "resolved": true, "digest": "AA", "state": crate::fixture::state(state),
        })
    }

    /// A card reward is two levels, and the hint has to say so.
    ///
    /// `sts2 act choose 0 1` was the old hint, and it was wrong rather than
    /// merely unclear. The protocol refuses a card reward taken without a card
    /// rather than choosing one. That hint sent two rewards with no card, and
    /// was rejected. The dotted pair is the only form the game accepts.
    #[test]
    fn a_card_reward_lists_its_cards_and_hints_a_pair() {
        let out = rendered(json!({
            "type": "pending", "seq": 7, "cmd": "end_turn", "digest": "AA",
            "kind": "rewards", "skippable": true,
            "rewards": [
                {"index": 0, "type": "CardReward", "populated": true, "taken": false,
                 "cards": [
                     {"card": "POISONED_STAB", "text": "Deal 6 damage.\nApply [green]4[/green] Poison."},
                     {"card": "FTL"},
                     {"card": "SPOILS_OF_BATTLE"}
                 ]},
                {"index": 1, "type": "GoldReward", "populated": true, "amount": 25}
            ],
            "state": {}
        }));

        assert!(out.contains("0.0 POISONED_STAB"), "{out}");
        assert!(out.contains("0.1 FTL"), "{out}");
        assert!(out.contains("0.2 SPOILS_OF_BATTLE"), "{out}");
        assert!(out.contains("25 gold"), "{out}");
        assert!(out.contains("next sts2 act choose 0.0"), "{out}");
        assert!(out.contains("or: sts2 act answer skip"), "{out}");

        // Choosing between three ids is choosing blind. The card's own words go
        // beside it, on one row, with the markup gone.
        assert!(
            out.contains("POISONED_STAB Deal 6 damage. Apply 4 Poison."),
            "{out}"
        );
    }

    /// A reward already taken stays listed, under the number it was offered as.
    ///
    /// The screen comes back after one is taken, so what it has to say is which
    /// rewards are gone. Without the mark the second question is unanswerable.
    #[test]
    fn a_taken_reward_says_so_and_keeps_its_number() {
        let out = rendered(json!({
            "type": "pending", "seq": 8, "cmd": "answer", "digest": "AA",
            "kind": "rewards", "skippable": true,
            "rewards": [
                {"index": 0, "type": "GoldReward", "populated": true, "taken": true,
                 "amount": 25},
                {"index": 1, "type": "PotionReward", "populated": true, "taken": false,
                 "potion": "BLOCK_POTION"}
            ],
            "state": {}
        }));

        assert!(out.contains("0 25 gold [taken]"), "{out}");
        assert!(out.contains("1 potion BLOCK_POTION"), "{out}");
        assert!(!out.contains("1 potion BLOCK_POTION [taken]"), "{out}");

        // And the hint points at one still on offer. Naming a taken reward
        // would be advising a command the game refuses.
        assert!(out.contains("next sts2 act choose 1"), "{out}");
    }

    /// The complaint that produced all of this: what will the card actually do?
    ///
    /// The name and the cost say nothing about the number, and the number is
    /// not the printed one. Shrink took two off this Strike. The arrow is how
    /// the reader can tell that something did.
    #[test]
    fn a_card_in_hand_says_what_playing_it_does() {
        let out = rendered(observation(json!({
            "inCombat": true,
            "combat": {
                "turn": 3, "round": 3, "phase": "Play", "energy": 2,
                "hand": ["STRIKE_IRONCLAD", "DEFEND_IRONCLAD"],
                "legal": [
                    {"card": "STRIKE_IRONCLAD", "cost": 1, "starCost": null, "target": "AnyEnemy",
                     "playable": true, "targets": [1],
                     "effect": [{"target": 1, "text": "Deal [red]4[/red] damage.",
                                 "vars": {"Damage": 4}, "base": {"Damage": 6}}]},
                    {"card": "DEFEND_IRONCLAD", "cost": 1, "starCost": null, "target": "Self",
                     "playable": true, "targets": [],
                     "effect": [{"target": null, "text": "Gain 5 [gold]Block[/gold].",
                                 "vars": {"Block": 5}, "base": {"Block": 5}}]}
                ],
                "creatures": []
            }
        })));

        assert!(out.contains("Deal 4 damage."), "{out}");
        assert!(out.contains("Gain 5 Block."), "{out}");

        // The text says what will happen; this says something changed it, which
        // a 4 sitting on its own cannot.
        assert!(out.contains("Deal 4 damage. (Damage 6→4)"), "{out}");
        assert!(
            !out.contains("Block 5→5"),
            "an unchanged value is not news: {out}"
        );
        // Energy was on the `combat` line, four fields along from anything that
        // used it.
        assert!(out.contains("hand 2 energy"), "{out}");
    }

    /// A card that hits harder into one enemy than another says which is which.
    #[test]
    fn an_effect_that_varies_by_target_is_listed_per_target() {
        let out = rendered(observation(json!({
            "inCombat": true,
            "combat": {
                "turn": 1, "round": 1, "phase": "Play", "energy": 3,
                "hand": ["BASH"],
                "legal": [{"card": "BASH", "cost": 2, "starCost": null, "target": "AnyEnemy",
                           "playable": true, "targets": [1, 2],
                           "effect": [
                               {"target": 1, "text": "Deal 8 damage."},
                               {"target": 2, "text": "Deal [green]12[/green] damage."}
                           ]}],
                "creatures": []
            }
        })));

        assert!(out.contains("vs 1: Deal 8 damage."), "{out}");
        assert!(out.contains("vs 2: Deal 12 damage."), "{out}");
    }

    /// Four Strikes are four indices and one decision.
    ///
    /// The indices all have to survive, because playing one needs one of them.
    #[test]
    fn cards_that_would_do_the_same_thing_are_listed_once() {
        let strike = json!({"card": "STRIKE_IRONCLAD", "cost": 1, "starCost": null, "target": "AnyEnemy",
                            "playable": true, "targets": [1],
                            "effect": [{"text": "Deal 6 damage."}]});
        let out = rendered(observation(json!({
            "inCombat": true,
            "combat": {
                "turn": 1, "round": 1, "phase": "Play", "energy": 3,
                "hand": ["STRIKE_IRONCLAD", "BASH", "STRIKE_IRONCLAD", "STRIKE_IRONCLAD"],
                "legal": [
                    strike,
                    {"card": "BASH", "cost": 2, "starCost": null, "target": "AnyEnemy",
                     "playable": true, "targets": [1],
                     "effect": [{"text": "Deal 8 damage."}]},
                    strike,
                    strike
                ],
                "creatures": []
            }
        })));

        assert!(out.contains("0,2,3 STRIKE_IRONCLAD 1e → 1"), "{out}");
        assert!(out.contains("1 BASH 2e → 1"), "{out}");
        // One row, and so one copy of what it does.
        assert_eq!(out.matches("Deal 6 damage.").count(), 1, "{out}");
    }

    /// Sameness is what the card does, not what it is called.
    ///
    /// Two Strikes with one Vulnerable target between them deal different
    /// damage, and collapsing them hides exactly the number the list exists to
    /// show. The same goes for one that cannot be played at all.
    #[test]
    fn cards_that_share_a_name_but_not_an_outcome_stay_apart() {
        let out = rendered(observation(json!({
            "inCombat": true,
            "combat": {
                "turn": 1, "round": 1, "phase": "Play", "energy": 3,
                "hand": ["STRIKE_IRONCLAD", "STRIKE_IRONCLAD", "STRIKE_IRONCLAD"],
                "legal": [
                    {"card": "STRIKE_IRONCLAD", "cost": 1, "starCost": null, "target": "AnyEnemy",
                     "playable": true, "targets": [1],
                     "effect": [{"text": "Deal 6 damage.", "base": {"Damage": 6},
                                 "vars": {"Damage": 6}}]},
                    {"card": "STRIKE_IRONCLAD", "cost": 1, "starCost": null, "target": "AnyEnemy",
                     "playable": true, "targets": [1],
                     "effect": [{"text": "Deal 9 damage.", "base": {"Damage": 6},
                                 "vars": {"Damage": 9}}]},
                    {"card": "STRIKE_IRONCLAD", "cost": 1, "starCost": null, "target": "AnyEnemy",
                     "playable": false, "reason": "EnergyCostTooHigh",
                     "effect": [{"text": "Deal 6 damage."}]}
                ],
                "creatures": []
            }
        })));

        assert!(out.contains("0 STRIKE_IRONCLAD 1e → 1"), "{out}");
        assert!(out.contains("1 STRIKE_IRONCLAD 1e → 1"), "{out}");
        assert!(
            out.contains("2 STRIKE_IRONCLAD 1e ✗ EnergyCostTooHigh"),
            "{out}"
        );
        assert!(out.contains("Deal 9 damage. (Damage 6→9)"), "{out}");
    }

    /// A power's amount is the question being asked when anyone looks at one.
    ///
    /// Unless it has none. The game writes an endless power down as `-1`, and
    /// a reader who sees that reads it as a count of minus one.
    #[test]
    fn a_power_is_shown_with_its_amount() {
        let out = rendered(observation(json!({
            "inCombat": true,
            "combat": {
                "turn": 1, "round": 1, "phase": "Play", "energy": 3,
                "hand": [], "legal": [],
                "creatures": [{
                    "id": 0, "model": "CHARACTER/IRONCLAD", "side": "Player",
                    "hp": 60, "maxHp": 80, "block": 0, "alive": true,
                    "powers": [{"id": "VULNERABLE_POWER", "amount": 1},
                               {"id": "SHRINK_POWER", "amount": -1}]
                }]
            }
        })));

        assert!(out.contains("[VULNERABLE 1, SHRINK]"), "{out}");
    }

    /// An event is prose, and the keys the protocol carries are not it.
    #[test]
    fn an_event_shows_its_words_rather_than_its_loc_keys() {
        let out = rendered(observation(json!({
            "room": {
                "type": "Event", "id": "BYRDONIS_NEST", "finished": false,
                "text": "A nest of squalling byrdlings.",
                "options": [
                    {"index": 0, "key": "BYRDONIS_NEST.pages.INITIAL.options.EAT",
                     "title": "Eat one", "text": "Gain 10 Max HP. Take 5 damage."},
                    {"index": 1, "key": "BYRDONIS_NEST.pages.INITIAL.options.LEAVE",
                     "title": "Leave", "proceed": true}
                ]
            }
        })));

        assert!(out.contains("A nest of squalling byrdlings."), "{out}");
        assert!(out.contains("0 Eat one"), "{out}");
        // An event option carries no `enabled` at all, and absent must not read
        // as disabled — which is what "not true" would have made it.
        assert!(!out.contains("disabled"), "{out}");
        assert!(out.contains("next sts2 act choose 0"), "{out}");
        assert!(out.contains("Gain 10 Max HP. Take 5 damage."), "{out}");
        assert!(!out.contains("pages.INITIAL"), "{out}");
    }

    /// A shop used to print its own name and nothing else.
    ///
    /// Its stock is `entries`, not `options`, and nothing here read that key.
    /// So there was no stock, no prices, and no way to learn the index that
    /// `buy` wants from the tool's own output.
    #[test]
    fn a_shop_lists_its_stock_and_what_each_costs() {
        let out = rendered(observation(json!({
            "room": {
                "type": "Shop",
                "entries": [
                    {"index": 0, "kind": "MerchantCardEntry", "id": "POMMEL_STRIKE",
                     "cost": 75, "stocked": true, "affordable": true},
                    {"index": 1, "kind": "MerchantRelicEntry", "id": "VAJRA",
                     "cost": 300, "stocked": true, "affordable": false},
                    {"index": 2, "kind": "MerchantPotionEntry", "id": "BLOCK_POTION",
                     "cost": 50, "stocked": false, "affordable": true},
                    {"index": 3, "kind": "MerchantCardRemovalEntry", "id": null,
                     "cost": 75, "stocked": true, "affordable": true}
                ]
            }
        })));

        assert!(out.contains("0 card POMMEL_STRIKE 75g"), "{out}");
        assert!(out.contains("1 relic VAJRA 300g [too dear]"), "{out}");
        assert!(out.contains("2 potion BLOCK_POTION 50g [sold]"), "{out}");

        // Card removal is a service, so it has no id — and its class name is
        // two words. Both of these read as bugs when they are not.
        assert!(out.contains("3 card removal 75g"), "{out}");
        assert!(!out.contains('?'), "{out}");

        // And the one it can actually buy is the one it offers to.
        assert!(out.contains("next sts2 act choose 0"), "{out}");
    }

    /// Nothing buyable is not the same as nothing to say.
    #[test]
    fn a_shop_nobody_can_afford_advises_no_purchase() {
        let out = rendered(observation(json!({
            "room": {
                "type": "Shop",
                "entries": [
                    {"index": 0, "kind": "MerchantRelicEntry", "id": "VAJRA",
                     "cost": 300, "stocked": true, "affordable": false}
                ]
            }
        })));

        assert!(out.contains("VAJRA 300g [too dear]"), "{out}");
        assert!(!out.contains("next"), "nothing here is buyable: {out}");
    }

    /// The game refuses a disabled option, so advising one advises a refusal.
    #[test]
    fn a_disabled_rest_site_option_is_marked_and_not_advised() {
        let out = rendered(observation(json!({
            "room": {
                "type": "RestSite",
                "options": [
                    {"index": 0, "id": "SMITH", "enabled": false},
                    {"index": 1, "id": "HEAL", "enabled": true}
                ]
            }
        })));

        assert!(out.contains("0 SMITH [disabled]"), "{out}");
        assert!(out.contains("1 HEAL"), "{out}");
        assert!(!out.contains("1 HEAL [disabled]"), "{out}");
        assert!(out.contains("next sts2 act choose 1"), "{out}");
    }

    /// The hint has to be an answer the game takes.
    ///
    /// Deriving it from `max` alone does not: "discard up to 3" with two cards
    /// in hand suggests `choose 0 1 2`, and index 2 is refused.
    #[test]
    fn the_card_hint_never_names_an_option_that_is_not_there() {
        let out = rendered(json!({
            "type": "pending", "seq": 5, "cmd": "play_card", "digest": "AA", "state": {},
            "kind": "select_cards", "min": 0, "max": 3, "asker": null,
            "options": [{"index": 0, "card": "STRIKE_IRONCLAD"}, {"index": 1, "card": "DEFEND_IRONCLAD"}]
        }));

        assert!(out.contains("next sts2 act choose 0 1"), "{out}");
        assert!(!out.contains("choose 0 1 2"), "{out}");
    }

    /// And it has to name enough of them: three indices answer a minimum of
    /// five with a refusal too.
    #[test]
    fn the_card_hint_names_at_least_the_minimum() {
        let options: Vec<_> = (0..6)
            .map(|i| json!({"index": i, "card": "STRIKE_IRONCLAD"}))
            .collect();
        let out = rendered(json!({
            "type": "pending", "seq": 5, "cmd": "play_card", "digest": "AA", "state": {},
            "kind": "select_cards", "min": 5, "max": 5, "asker": null, "options": options
        }));

        assert!(out.contains("next sts2 act choose 0 1 2 3 4"), "{out}");
    }

    /// The counters are not part of the game, and are one flag away.
    #[test]
    fn the_rng_counters_are_hidden_unless_asked_for() {
        let state = observation(json!({"rng": {"Shuffle": 44, "Niche": 0}}));

        let hidden = shown(state.clone(), false);
        assert!(!hidden.contains("Shuffle"), "{hidden}");

        let asked = shown(state, true);
        assert!(asked.contains("rng Shuffle 44"), "{asked}");
    }

    /// A reward with nothing inside it still says which it is.
    #[test]
    fn a_reward_that_is_not_a_card_needs_no_pair() {
        let out = rendered(json!({
            "type": "pending", "seq": 7, "cmd": "end_turn", "digest": "AA",
            "kind": "rewards", "skippable": false,
            "rewards": [
                {"index": 0, "type": "PotionReward", "populated": false, "taken": false}
            ],
            "state": {}
        }));

        assert!(out.contains("a potion"), "{out}");
        assert!(out.contains("[empty]"), "{out}");
        assert!(out.lines().any(|l| l == "next sts2 act choose 0"), "{out}");
        assert!(
            !out.contains("answer skip"),
            "unskippable, so do not offer it: {out}"
        );
    }
}
