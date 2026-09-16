//! The loop: draw, wait for a key, turn it into a move, send it, draw again.
//!
//! Everything the game is asked is a `Decision<V>`, and everything drawn comes
//! out of the `Applied<V>` that came back. So this module is the only one that
//! knows both. The screens read, and this decides.
//!
//! Two rules it holds to. A key that cannot mean anything here does nothing and
//! says nothing, because a beep on every stray press is worse than silence. And
//! a move the game refuses is a *line in the log* rather than an error. A
//! refusal is the game answering, and the run carries on.

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use sts2_core::{
    Answer, BeltIndex, CombatId, Cost, Decided, Decision, HandIndex, Observation, Play, Question,
    Room, Snapshot, TakeReward,
};

use super::keys::{self, Action};
use super::screen;
use super::state::{Aim, Options, Overlay, Pile, Tone, Ui};
use crate::cli::{choose, moves};
use crate::render::{self, Amount, Pool};

/// What kind of thing the game is showing.
///
/// Only ever compared, never drawn, because the screens ask the observation
/// directly. Its job is to notice when the game moved on to a different kind of
/// screen. A cursor into the last one's list means nothing in the next one's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Before the first observation, or after the run ended.
    Nothing,
    Over,
    AskingCards,
    AskingRewards,
    AskingDivine,
    Fight,
    RestSite,
    Treasure,
    Event,
    Shop,
    Travel,
    /// A room that has been answered and is now only a way out. Its own stage
    /// because the list under the cursor is a different list.
    Leaving,
}

/// What a key press asks of the game, if it asks anything.
///
/// The handlers below decide. None of them send. One place does that. So a
/// keyboard can be driven with no game behind it, and a fault part way through
/// a handler cannot leave the screen half-changed.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent<V: sts2_content_wire::WireContentVersion> {
    /// The key moved a cursor, opened a panel, or meant nothing here.
    Nothing,
    /// A fresh look, which advances nothing.
    Look,
    /// One decision, sent as a move in the run.
    Send(Decision<V>),
}

/// What a card costs, spelled out for a panel with room for words.
///
/// The compact form belongs on a card box, where the position says which pool
/// is meant. This is read on its own, so it names them.
fn priced(energy: Cost, stars: Cost) -> String {
    let words: Vec<String> = render::pools(energy, stars)
        .map(|(amount, pool)| {
            // Each pool carries both forms. Energy is uncountable and stars are
            // not, so one name with an `s` on the end reads as "3 energys".
            let (one, many) = match pool {
                Pool::Energy => ("energy", "energy"),
                Pool::Stars => ("star", "stars"),
            };
            match amount {
                Amount::All => format!("all your {many}"),
                Amount::Of(1) => format!("1 {one}"),
                Amount::Of(n) => format!("{n} {many}"),
            }
        })
        .collect();

    if words.is_empty() {
        "costs nothing".to_string()
    } else {
        format!("costs {}", words.join(" and "))
    }
}

/// Why the screen stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The player left.
    Quit,
    /// The game says it is holding no run. The daemon's can idle out from
    /// under a live screen, and what wants drawing then is the setup screen.
    NoRun,
}

pub struct App<V: sts2_content_wire::WireContentVersion, G: Play<V>> {
    game: G,
    /// Why the loop will stop, read once it has.
    ended: Ended,
    /// The most recent thing the game said. `None` only before the first look.
    seen: Option<Snapshot<V>>,
    stage: Stage,
    /// The run being set up, when there is no run.
    ///
    /// Nothing else on the screen is drawn while this is `Some`: there is no
    /// observation behind it to draw.
    pub ui: Ui,
    pub options: Options,
}

impl<V, G> App<V, G>
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    /// A screen in front of a run.
    ///
    /// The only kind there is. Starting one is [`super::setup`]'s, and it has
    /// happened by the time this exists.
    pub fn playing(game: G, options: Options) -> App<V, G> {
        App {
            game,
            ended: Ended::Quit,
            seen: None,
            stage: Stage::Nothing,
            ui: Ui::default(),
            options,
        }
    }

    /// What the game last said, for the screens to draw.
    pub fn seen(&self) -> Option<&Snapshot<V>> {
        self.seen.as_ref()
    }

    pub fn observation(&self) -> Option<&Observation<V>> {
        self.seen.as_ref().map(|a| &a.observation)
    }

    /// One look, for a caller that draws its own frames.
    ///
    /// Public because `check` drives the front end without a terminal under it.
    /// A fault is a panic here rather than a `Result`. The callers are tests,
    /// and a game that cannot answer `observe` has nothing for them to check.
    pub fn look_now(&mut self) {
        self.look().expect("the game answered `observe`");
    }

    /// One action, for a caller that has no keyboard.
    pub fn press(&mut self, action: Action) {
        let intent = self.act(action);
        self.perform(intent)
            .expect("the game answered the decision");
    }

    /// One key press.
    ///
    /// Raw rather than an [`Action`], for a caller that has one.
    pub fn key(&mut self, key: KeyEvent) -> Result<()> {
        let intent = self.decide(key);
        self.perform(intent)
    }

    /// What a key press changes on the screen, and what it leaves to ask.
    ///
    /// Every handler it reaches is a state transition, and none of them can
    /// fail. Asking the game is [`App::perform`]'s, and happens after the
    /// screen has finished changing.
    pub fn decide(&mut self, key: KeyEvent) -> Intent<V> {
        match keys::map(key) {
            Some(action) => self.act(action),
            None => Intent::Nothing,
        }
    }

    /// Asks the game for what the keyboard decided to ask.
    ///
    /// The one place in the front end that talks to the game while it is being
    /// played.
    fn perform(&mut self, intent: Intent<V>) -> Result<()> {
        match intent {
            Intent::Nothing => Ok(()),
            Intent::Look => self.look(),
            Intent::Send(decision) => self.send(decision).map(drop),
        }
    }

    /// Draws and reads keys until the player leaves, or the run goes away.
    ///
    /// The daemon's game can idle out from under a live screen. What comes back
    /// says which happened, because a run that is gone wants the setup screen
    /// rather than a window onto nothing.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<Ended> {
        // One look before the first frame, so the screen is the run rather than
        // an empty shape waiting for a key.
        self.look()?;

        while !self.ui.leaving {
            terminal
                .draw(|frame| screen::draw(frame, &self))
                .context("drawing the screen")?;

            // Blocked on, not polled. A player thinking about a card sends
            // nothing, and nothing is what should happen: the daemon keeps the
            // game alive on its own, so the screen has no clock to answer to.
            match event::read().context("reading a key")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => self.key(key)?,
                // A resize repaints on the next pass round the loop, which is
                // the next statement. Nothing else to do.
                _ => {}
            }
        }
        Ok(self.ended)
    }

    // -----------------------------------------------------------------------
    // Acting.
    // -----------------------------------------------------------------------

    fn act(&mut self, action: Action) -> Intent<V> {
        match action {
            Action::Quit => {
                self.ui.leaving = true;
                return Intent::Nothing;
            }
            Action::Help => {
                self.ui.overlay = match self.ui.overlay {
                    Some(Overlay::Help) => None,
                    _ => Some(Overlay::Help),
                };
                return Intent::Nothing;
            }
            Action::Look => return Intent::Look,
            _ => {}
        }

        // An overlay eats the keys while it is up, because a card played by a
        // keystroke aimed at a panel is a move nobody meant to make. The belt
        // is the exception: it is where a potion is drunk from, so it has keys
        // of its own rather than a key that dismisses it.
        match self.ui.overlay {
            Some(Overlay::Belt) => return self.in_belt(action),
            Some(_) => {
                self.in_overlay(action);
                return Intent::Nothing;
            }
            None => {}
        }

        match action {
            Action::ShowDeck => self.ui.overlay = Some(Overlay::Pile(Pile::Deck)),
            Action::ShowDraw => self.ui.overlay = Some(Overlay::Pile(Pile::Draw)),
            Action::ShowDiscard => self.ui.overlay = Some(Overlay::Pile(Pile::Discard)),
            Action::ShowExhaust => self.ui.overlay = Some(Overlay::Pile(Pile::Exhaust)),
            Action::ShowMap => self.ui.overlay = Some(Overlay::Map),
            Action::ShowPotions => {
                self.ui.at = 0;
                self.ui.overlay = Some(Overlay::Belt);
            }
            Action::ShowRelics => {
                self.ui.at = 0;
                self.ui.overlay = Some(Overlay::Relics);
            }
            Action::ScrollBack => self.ui.scrolled = self.ui.scrolled.saturating_add(1),
            Action::ScrollOn => self.ui.scrolled = self.ui.scrolled.saturating_sub(1),
            other => return self.in_stage(other),
        }
        Intent::Nothing
    }

    /// The belt, which is acted in rather than read.
    ///
    /// A potion that cannot be drunk is refused here off the game's own
    /// `reason`, exactly as an unplayable card is. The belt already carries the
    /// answer, and asking again is a round trip for a line of log nobody needs.
    /// This is not a second opinion about drinkability. No rule about when a
    /// potion can be drunk lives in this crate.
    fn in_belt(&mut self, action: Action) -> Intent<V> {
        let belt = self.observation().map(|o| o.potions.len()).unwrap_or(0);

        match action {
            Action::Cancel | Action::ShowPotions => {
                self.ui.overlay = None;
                self.ui.at = 0;
            }
            Action::Up | Action::Left => self.ui.step(-1, belt),
            Action::Down | Action::Right => self.ui.step(1, belt),
            Action::Nth(n) if n < belt => {
                self.ui.at = n;
                return self.aim_or_drink(n);
            }
            Action::Confirm => {
                let at = self.ui.at;
                return self.aim_or_drink(at);
            }
            // Anything else closes it, the way the read-only panels do.
            _ => {
                self.ui.overlay = None;
                self.ui.at = 0;
            }
        }
        Intent::Nothing
    }

    /// Drinks the nth potion, or starts picking a creature for it.
    ///
    /// The same rule cards get, and for the same reason. A potion with one
    /// legal target still goes through aiming, because aiming is when the
    /// screen shows which creature it is about to land on.
    fn aim_or_drink(&mut self, at: usize) -> Intent<V> {
        let Some(potion) = self.observation().and_then(|o| o.potions.get(at)) else {
            return Intent::Nothing;
        };

        if let Some(why) = potion.reason {
            let why = super::names::undrinkable(why);
            self.ui
                .say(Tone::Cannot, format!("that potion cannot be drunk: {why}"));
            return Intent::Nothing;
        }

        if potion.targets.is_empty() {
            self.ui.overlay = None;
            self.ui.at = 0;
            return drink(at, None);
        }

        // The belt closes so the fight underneath is what the aim is drawn
        // over: pointing at a creature hidden behind a panel is pointing at
        // nothing.
        self.ui.overlay = None;
        self.ui.at = 0;
        self.ui.aim = Aim::Drinking { potion: at, at: 0 };
        Intent::Nothing
    }

    // -----------------------------------------------------------------------
    // Before there is a run.
    // -----------------------------------------------------------------------

    fn in_overlay(&mut self, action: Action) {
        match action {
            Action::Up => self.ui.at = self.ui.at.saturating_sub(1),
            Action::Down => self.ui.at = self.ui.at.saturating_add(1),
            // Any other key closes it. A panel that needs to be dismissed the
            // one right way is a panel that gets in the way.
            _ => {
                self.ui.overlay = None;
                self.ui.at = 0;
            }
        }
    }

    fn in_stage(&mut self, action: Action) -> Intent<V> {
        match self.stage {
            Stage::Fight => self.in_fight(action),
            Stage::AskingCards => self.in_card_select(action),
            Stage::AskingRewards => self.in_rewards(action),
            Stage::RestSite
            | Stage::Treasure
            | Stage::Event
            | Stage::Shop
            | Stage::Travel
            | Stage::Leaving => self.in_list(action),
            Stage::AskingDivine => {
                self.ui.say(
                    Tone::Cannot,
                    "the Crystal Sphere's grid is not drawn here yet — \
                     answer it with `sts2 act answer divine <x> <y>`",
                );
                Intent::Nothing
            }
            Stage::Over | Stage::Nothing => Intent::Nothing,
        }
    }

    // -----------------------------------------------------------------------
    // A fight.
    // -----------------------------------------------------------------------

    fn in_fight(&mut self, action: Action) -> Intent<V> {
        let Some(combat) = self.observation().and_then(|o| o.fight()) else {
            return Intent::Nothing;
        };
        let hand = combat.legal.len();

        // Aiming: the cursor is walking the legal targets of one card or one
        // potion rather than the hand, so the same keys mean something else.
        // The two aims move identically and differ only in what confirming
        // sends, which is the one branch below that tells them apart.
        let aiming = match self.ui.aim {
            Aim::Targeting { card, at } => Some((
                combat
                    .legal
                    .get(card)
                    .map(|c| c.targets.clone())
                    .unwrap_or_default(),
                at,
            )),
            Aim::Drinking { potion, at } => Some((
                self.observation()
                    .and_then(|o| o.potions.get(potion))
                    .map(|p| p.targets.clone())
                    .unwrap_or_default(),
                at,
            )),
            _ => None,
        };

        if let Some((targets, at)) = aiming {
            // Whatever was being aimed is gone — a card was exhausted, a potion
            // was drunk — so there is nothing to point.
            if targets.is_empty() {
                self.ui.aim = Aim::Idle;
                return Intent::Nothing;
            }

            let moved = |aim: Aim, to: usize| match aim {
                Aim::Targeting { card, .. } => Aim::Targeting { card, at: to },
                Aim::Drinking { potion, .. } => Aim::Drinking { potion, at: to },
                other => other,
            };

            return match action {
                Action::Cancel => {
                    self.ui.aim = Aim::Idle;
                    Intent::Nothing
                }
                Action::Left | Action::Up => {
                    let to = (at + targets.len() - 1) % targets.len();
                    self.ui.aim = moved(self.ui.aim.clone(), to);
                    Intent::Nothing
                }
                Action::Right | Action::Down => {
                    let to = (at + 1) % targets.len();
                    self.ui.aim = moved(self.ui.aim.clone(), to);
                    Intent::Nothing
                }
                Action::Nth(n) if n < targets.len() => {
                    self.ui.aim = moved(self.ui.aim.clone(), n);
                    Intent::Nothing
                }
                Action::Confirm => {
                    let target = targets[at];
                    let aim = std::mem::take(&mut self.ui.aim);
                    match aim {
                        Aim::Targeting { card, .. } => play_card(card, Some(target)),
                        Aim::Drinking { potion, .. } => drink(potion, Some(target)),
                        _ => Intent::Nothing,
                    }
                }
                _ => Intent::Nothing,
            };
        }

        match action {
            Action::Left | Action::Up => self.ui.step(-1, hand),
            Action::Right | Action::Down => self.ui.step(1, hand),
            Action::Nth(n) if n < hand => self.ui.at = n,
            Action::EndTurn => return Intent::Send(Decision::EndTurn),
            Action::Inspect => self.inspect_card(),
            Action::Confirm => return self.aim_or_play(),
            _ => {}
        }
        Intent::Nothing
    }

    /// Plays the card under the cursor, or starts picking a creature for it.
    ///
    /// A card with one legal target still enters aiming. That view shows its
    /// computed effect on the creature, including changes from Strength or
    /// Vulnerable. Only a card with no target plays immediately.
    fn aim_or_play(&mut self) -> Intent<V> {
        let Some(combat) = self.observation().and_then(|o| o.fight()) else {
            return Intent::Nothing;
        };
        let at = self.ui.at;
        let Some(card) = combat.legal.get(at) else {
            return Intent::Nothing;
        };

        // The game has already said why, on the card's own face. Sending it to
        // be told again is a round trip for a line of log nobody needs — and
        // this is not a second opinion about legality, it is the game's own
        // `reason` field being believed.
        if let Some(reason) = card.reason {
            let why = crate::render::flags(reason);
            self.ui
                .say(Tone::Cannot, format!("that card cannot be played: {why}"));
            return Intent::Nothing;
        }

        match card.targets.is_empty() {
            true => play_card(at, None),
            false => {
                self.ui.aim = Aim::Targeting { card: at, at: 0 };
                Intent::Nothing
            }
        }
    }

    // -----------------------------------------------------------------------
    // Rooms, shops, the map.
    // -----------------------------------------------------------------------

    fn in_list(&mut self, action: Action) -> Intent<V> {
        let moves = self.moves();
        match action {
            Action::Up | Action::Left => self.ui.step(-1, moves.len()),
            Action::Down | Action::Right => self.ui.step(1, moves.len()),
            // On a numbered list the digit *is* the choice: that is what the
            // number beside each row means. In the hand it only moves the
            // cursor, because a card still has to be aimed.
            Action::Nth(n) if n < moves.len() => {
                self.ui.at = n;
                return Intent::Send(moves[n].clone());
            }
            Action::Confirm => {
                return match moves.get(self.ui.at) {
                    Some(one) => Intent::Send(one.clone()),
                    None => Intent::Nothing,
                };
            }
            _ => {}
        }
        Intent::Nothing
    }

    // -----------------------------------------------------------------------
    // Questions.
    // -----------------------------------------------------------------------

    fn in_card_select(&mut self, action: Action) -> Intent<V> {
        let Some(Question::SelectCards {
            min, max, options, ..
        }) = self.seen.as_ref().and_then(|a| a.asking.as_ref())
        else {
            return Intent::Nothing;
        };
        let (min, max, len) = (*min as usize, *max as usize, options.len());

        let mut marked = match &self.ui.aim {
            Aim::Selecting(marked) => marked.clone(),
            _ => Vec::new(),
        };

        match action {
            Action::Up | Action::Left => self.ui.step(-1, len),
            Action::Down | Action::Right => self.ui.step(1, len),
            Action::Nth(n) if n < len => self.ui.at = n,
            Action::Cancel => marked.clear(),
            Action::Mark => {
                let at = self.ui.at;
                match marked.iter().position(|&m| m == at) {
                    Some(already) => {
                        marked.remove(already);
                    }
                    // The order they were marked in is the order they are sent,
                    // because the game takes a list and not a set.
                    None if marked.len() < max => marked.push(at),
                    None => self.ui.say(
                        Tone::Cannot,
                        format!("this asks for at most {max}; unmark one first"),
                    ),
                }
            }
            Action::Confirm => {
                // Enter on nothing marked is the common case — one card wanted,
                // one card under the cursor — so it means that card.
                if marked.is_empty() && max >= 1 {
                    marked.push(self.ui.at);
                }
                if marked.len() < min {
                    let short = min - marked.len();
                    self.ui.say(
                        Tone::Cannot,
                        format!("this asks for {min}; mark {short} more with space"),
                    );
                } else {
                    self.ui.aim = Aim::Idle;
                    return Intent::Send(Decision::Answer(Answer::Cards(marked)));
                }
            }
            // Declining, where declining is allowed.
            Action::Skip if min == 0 => {
                self.ui.aim = Aim::Idle;
                return Intent::Send(Decision::Answer(Answer::Cards(Vec::new())));
            }
            _ => {}
        }

        self.ui.aim = Aim::Selecting(marked);
        Intent::Nothing
    }

    fn in_rewards(&mut self, action: Action) -> Intent<V> {
        let Some(Question::Rewards { skippable, .. }) =
            self.seen.as_ref().and_then(|a| a.asking.as_ref())
        else {
            return Intent::Nothing;
        };
        let skippable = *skippable;
        let takeable = self.takeable();

        match action {
            Action::Up | Action::Left => self.ui.step(-1, takeable.len()),
            Action::Down | Action::Right => self.ui.step(1, takeable.len()),
            Action::Nth(n) if n < takeable.len() => {
                self.ui.at = n;
                return Intent::Send(Decision::Answer(Answer::Take(vec![takeable[n].clone()])));
            }
            Action::Confirm => {
                if let Some(take) = takeable.get(self.ui.at) {
                    return Intent::Send(Decision::Answer(Answer::Take(vec![take.clone()])));
                }
            }
            Action::Skip => {
                return match skippable {
                    true => Intent::Send(Decision::Answer(Answer::Skip)),
                    false => {
                        self.ui
                            .say(Tone::Cannot, "this reward set cannot be skipped");
                        Intent::Nothing
                    }
                };
            }
            _ => {}
        }
        Intent::Nothing
    }

    // -----------------------------------------------------------------------
    // What the screens need to know about the lists they draw.
    // -----------------------------------------------------------------------

    /// Everything the player can do on the screen in front of them, in the
    /// order it is drawn and numbered.
    ///
    /// Asked of `cli::choose` for the room's own half, which is what the
    /// subcommand runs. So the TUI cannot offer an option the other front end
    /// refuses.
    pub fn moves(&self) -> Vec<Decision<V>> {
        self.seen
            .as_ref()
            .map(crate::cli::moves::navigation)
            .unwrap_or_default()
    }

    /// Every individual thing a reward screen will still hand over.
    ///
    /// Flattens reward sets and their card lists for both input and rendering.
    /// Sharing one list keeps displayed digits aligned after each taken reward.
    /// Entries already taken are omitted.
    pub fn takeable(&self) -> Vec<TakeReward> {
        let Some(question) = self.seen.as_ref().and_then(|a| a.asking.as_ref()) else {
            return Vec::new();
        };
        moves::takeable(question)
    }

    /// Whether the room still has anything to answer.
    ///
    /// What decides which screen is drawn: a room with business is a room, and
    /// a room without is a picture of where to go next.
    pub fn room_has_moves(&self) -> bool {
        self.moves()
            .iter()
            .any(|m| matches!(m, Decision::ChooseOption { .. } | Decision::Buy { .. }))
    }

    /// Which creature the thing being aimed is currently pointed at.
    ///
    /// One answer for a card and for a potion. Every screen that asks wants the
    /// same thing, which is the creature to draw the aim on. None of them cares
    /// which of the two is in flight.
    pub fn aimed_at(&self) -> Option<CombatId> {
        let (targets, at) = match self.ui.aim {
            Aim::Targeting { card, at } => (
                self.observation()
                    .and_then(|o| o.fight())
                    .and_then(|c| c.legal.get(card))
                    .map(|c| c.targets.as_slice())?,
                at,
            ),
            Aim::Drinking { potion, at } => (
                self.observation()
                    .and_then(|o| o.potions.get(potion))
                    .map(|p| p.targets.as_slice())?,
                at,
            ),
            _ => return None,
        };
        targets.get(at).copied()
    }

    /// The potion being aimed, for a screen that has to say what it is.
    pub fn drinking(&self) -> Option<usize> {
        match self.ui.aim {
            Aim::Drinking { potion, .. } => Some(potion),
            _ => None,
        }
    }

    /// The cards marked in an open selection.
    pub fn marked(&self) -> &[usize] {
        match &self.ui.aim {
            Aim::Selecting(marked) => marked,
            _ => &[],
        }
    }

    // -----------------------------------------------------------------------
    // Talking to the game.
    // -----------------------------------------------------------------------

    fn look(&mut self) -> Result<()> {
        let standing = self.game.observe().context("asking the game where it is")?;
        self.arrive(standing);
        Ok(())
    }

    /// Sends one decision, and reports whether the game accepted it.
    fn send(&mut self, decision: Decision<V>) -> Result<bool> {
        let said = moves::describe(&decision);
        let answered = self
            .game
            .play(&decision)
            .with_context(|| format!("sending {said}"))?;
        Ok(self.accept(answered, Some(said)))
    }

    /// Takes what the game said, whichever half it said.
    ///
    /// A refusal is a value here rather than an error. The game answered, the
    /// run is where it was, and the reason belongs on the screen where the
    /// player can act on it.
    /// Takes where the run now stands, and resets the log when the screen
    /// behind it changes.
    fn arrive(&mut self, standing: Snapshot<V>) {
        let stage = stage_of(&standing);
        if stage != self.stage {
            self.ui.reset();
            self.stage = stage;
        }
        self.seen = Some(standing);
    }

    fn accept(&mut self, answered: sts2_core::Answered<V>, said: Option<String>) -> bool {
        let accepted = answered.is_ok();
        match answered {
            Ok(applied) => {
                if let Some(said) = said {
                    self.ui.say(Tone::Did, said);
                }
                let (decided, snapshot) = applied.split();
                // A parked decision is not resolved either, and that is
                // ordinary — the question is what tells the two apart. Without
                // the second test this fired on every card selection in the
                // game, including Neow's.
                if decided == Decided::Abandoned && snapshot.asking.is_none() {
                    self.ui.say(
                        Tone::Warned,
                        "the game abandoned that mid-cascade — the fight ended under it",
                    );
                }
                self.arrive(snapshot);
            }
            Err(refusal) => {
                // "No run" is not a refusal to report, it is a run waiting to
                // be started. Anything else is the game saying no to something
                // that was asked, which belongs in the log.
                if matches!(refusal.reason, sts2_core::Reason::NoRun) {
                    self.ended = Ended::NoRun;
                    self.ui.leaving = true;
                } else {
                    self.ui
                        .say(Tone::Refused, crate::render::refusal::refusal(&refusal));
                }
            }
        }
        accepted
    }

    /// Puts the card under the cursor into the detail panel.
    fn inspect_card(&mut self) {
        let Some(combat) = self.observation().and_then(|o| o.fight()) else {
            return;
        };
        let Some(card) = combat.legal.get(self.ui.at) else {
            return;
        };

        let mut lines = vec![
            crate::render::ident(&card.card),
            String::new(),
            priced(card.cost, card.star_cost),
            format!("targets {}", crate::render::spelling(&card.target)),
        ];
        if let Some(reason) = &card.reason {
            lines.push(format!(
                "cannot be played: {}",
                crate::render::flags(*reason)
            ));
        }
        for preview in &card.effect {
            lines.push(String::new());
            if let Some(id) = preview.target {
                lines.push(format!("against {id}:"));
            }
            if let Some(text) = &preview.text {
                lines.push(crate::render::prose::plain(text));
            }
        }
        self.ui.overlay = Some(Overlay::Detail(lines));
    }
}

/// The card at an index in the hand, played at a creature or at nothing.
fn play_card<V: sts2_content_wire::WireContentVersion>(
    index: usize,
    target: Option<CombatId>,
) -> Intent<V> {
    // A hand that long is no hand the game deals, so the key meant nothing.
    let Some(index) = hand_index(index) else {
        return Intent::Nothing;
    };
    Intent::Send(Decision::PlayCard { index, target })
}

/// The place in the hand a screen position names, for a hand the game deals.
fn hand_index(index: usize) -> Option<HandIndex> {
    u8::try_from(index)
        .ok()
        .and_then(|index| HandIndex::try_new(index).ok())
}

/// The slot in the belt a screen position names, for a belt the game gives.
fn belt_index(index: usize) -> Option<BeltIndex> {
    u8::try_from(index)
        .ok()
        .and_then(|index| BeltIndex::try_new(index).ok())
}

/// The potion at an index in the belt, drunk at a creature or at nothing.
fn drink<V: sts2_content_wire::WireContentVersion>(
    index: usize,
    target: Option<CombatId>,
) -> Intent<V> {
    // As in [`play_card`]: a belt that long is no belt the game gives.
    let Some(index) = belt_index(index) else {
        return Intent::Nothing;
    };
    Intent::Send(Decision::UsePotion { index, target })
}

/// Which kind of screen this is.
fn stage_of<V: sts2_content_wire::WireContentVersion>(standing: &Snapshot<V>) -> Stage {
    if let Some(question) = &standing.asking {
        return match question {
            Question::SelectCards { .. } => Stage::AskingCards,
            Question::Rewards { .. } => Stage::AskingRewards,
            Question::Divine { .. } => Stage::AskingDivine,
        };
    }

    let observed = &standing.observation;
    if observed.run_over {
        return Stage::Over;
    }
    if observed.fight().is_some() {
        return Stage::Fight;
    }
    let spent = observed
        .room
        .as_ref()
        .is_some_and(|room| choose::choosable(room).is_empty() && choose::buyable(room).is_empty());

    match &observed.room {
        Some(Room::Shop { .. }) => Stage::Shop,
        Some(_) if spent => Stage::Leaving,
        Some(Room::RestSite { .. }) => Stage::RestSite,
        Some(Room::Treasure { .. }) => Stage::Treasure,
        Some(Room::Event { .. }) => Stage::Event,
        None => Stage::Travel,
    }
}

#[cfg(test)]
mod tests {
    use sts2_core::fixture::{at, hand};

    use super::{App, Intent, Stage, priced};
    use crate::fixture::{self, playable, unplayable};
    use crate::scripted::Scripted;
    use crate::tui::keys::Action;
    use crate::tui::state::{Aim, Options};

    use serde_json::json;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{Applied, Decision};

    type V = V0_107_1;

    /// Each pool is named in the number it is in, and a pool the card is not
    /// priced in is not named at all.
    ///
    /// Energy is uncountable and stars are not. One name with an `s` appended
    /// covered both for a while, which read as "3 energys".
    #[test]
    fn a_price_is_spelled_in_the_pools_it_is_paid_from() {
        use sts2_core::Cost;
        use sts2_core::Cost::{None as Unpriced, X};

        assert_eq!(priced(Unpriced, Unpriced), "costs nothing");
        assert_eq!(priced(Cost::fixed(1), Unpriced), "costs 1 energy");
        assert_eq!(priced(Cost::fixed(3), Unpriced), "costs 3 energy");
        assert_eq!(priced(X, Unpriced), "costs all your energy");
        assert_eq!(priced(Unpriced, Cost::fixed(1)), "costs 1 star");
        assert_eq!(priced(Unpriced, Cost::fixed(2)), "costs 2 stars");
        assert_eq!(priced(Unpriced, X), "costs all your stars");
        assert_eq!(
            priced(Cost::fixed(1), Cost::fixed(2)),
            "costs 1 energy and 2 stars"
        );
    }

    /// A pool priced at zero is priced, and says so. `Cost::None` is the pool
    /// the card is not priced in, and the two must not read alike.
    #[test]
    fn a_zero_price_is_not_the_same_as_no_price() {
        use sts2_core::Cost::{self, None as Unpriced};

        assert_eq!(priced(Cost::fixed(0), Unpriced), "costs 0 energy");
        assert_eq!(priced(Unpriced, Unpriced), "costs nothing");
    }

    /// The front end, standing at what the game last said.
    ///
    /// The game under it fails the test if it is asked anything. Deciding what
    /// a key press means must not reach it, and that is what holds it to that.
    fn showing(applied: Applied<V>) -> App<V, Scripted<V>> {
        let mut app = App::playing(Scripted::untouchable(), Options::default());
        app.accept(Ok(applied), None);
        app
    }

    /// A room with one numbered option in it.
    fn in_an_event() -> App<V, Scripted<V>> {
        showing(fixture::applied(json!({
            "room": {"type": "Event", "id": "WELLSPRING",
                     "options": [{"index": 6,
                                  "key": "WELLSPRING.pages.INITIAL.options.DRINK"}]}
        })))
    }

    /// A fight, with one card that needs aiming and one that cannot be played.
    fn in_a_fight() -> App<V, Scripted<V>> {
        showing(fixture::applied(json!({
            "inCombat": true,
            "combat": fixture::combat(json!({
                "hand": ["STRIKE_IRONCLAD", "CLASH"],
                "legal": [
                    playable("STRIKE_IRONCLAD", &[1, 2]),
                    unplayable("CLASH", "BlockedByCardLogic")
                ],
                "creatures": [
                    fixture::creature(0, "Player", Some("CHARACTER/IRONCLAD"), &[]),
                    fixture::creature(1, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                    fixture::creature(2, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[])
                ]
            }))
        })))
    }

    /// Moving the cursor is the screen's own business, and asks nothing.
    #[test]
    fn walking_a_list_asks_the_game_nothing() {
        let mut app = in_a_fight();

        let intent = app.act(Action::Right);

        assert_eq!(intent, Intent::Nothing);
        assert_eq!(app.ui.at, 1);
    }

    /// On a numbered list the digit is the choice, and moves the cursor too.
    #[test]
    fn a_digit_on_a_list_decides_that_row() {
        let mut app = in_an_event();

        let intent = app.act(Action::Nth(0));

        assert_eq!(intent, Intent::Send(Decision::ChooseOption { index: 6 }));
        assert_eq!(app.ui.at, 0);
    }

    /// The game's own `reason` is believed rather than tested by sending.
    #[test]
    fn an_unplayable_card_says_why_and_asks_nothing() {
        let mut app = in_a_fight();
        app.ui.at = 1;

        let intent = app.act(Action::Confirm);

        assert_eq!(intent, Intent::Nothing);
        assert!(
            app.ui
                .log
                .iter()
                .any(|line| line.text.contains("cannot be played")),
            "{:?}",
            app.ui.log
        );
    }

    /// Aiming is two presses: the first points, and only the second sends.
    #[test]
    fn a_targeted_card_is_aimed_before_it_is_played() {
        let mut app = in_a_fight();

        let pointing = app.act(Action::Confirm);
        assert_eq!(pointing, Intent::Nothing);
        assert_eq!(app.ui.aim, Aim::Targeting { card: 0, at: 0 });

        let moved = app.act(Action::Right);
        assert_eq!(moved, Intent::Nothing);

        let played = app.act(Action::Confirm);
        assert_eq!(
            played,
            Intent::Send(Decision::PlayCard {
                index: hand(0),
                target: Some(at(2))
            })
        );
        assert_eq!(app.ui.aim, Aim::Idle, "the aim outlived the card");
    }

    /// Cancelling an aim puts the card back down and asks nothing.
    #[test]
    fn cancelling_an_aim_sends_nothing() {
        let mut app = in_a_fight();
        app.act(Action::Confirm);

        let intent = app.act(Action::Cancel);

        assert_eq!(intent, Intent::Nothing);
        assert_eq!(app.ui.aim, Aim::Idle);
    }

    /// A key that means nothing here means nothing, quietly.
    #[test]
    fn a_key_with_no_meaning_here_does_nothing() {
        let mut app = showing(fixture::applied(json!({})));
        app.stage = Stage::Nothing;

        let intent = app.act(Action::EndTurn);

        assert_eq!(intent, Intent::Nothing);
        assert!(app.ui.log.is_empty(), "{:?}", app.ui.log);
    }
}
