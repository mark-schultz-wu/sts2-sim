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
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use sts2_core::{
    Answer, Applied, ContentVersion, Decision, Observation, Play, Question, Room, TakeReward,
};

use super::keys::{self, Action};
use super::screen;
use super::state::{Aim, NewRun, Options, Overlay, Pile, SEED_MAX, Tone, Ui};
use crate::cli::{choose, moves};

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

pub struct App<V: ContentVersion, G: Play<V>> {
    game: G,
    /// The most recent thing the game said. `None` only before the first look.
    seen: Option<Applied<V>>,
    stage: Stage,
    /// The run being set up, when there is no run.
    ///
    /// Nothing else on the screen is drawn while this is `Some`: there is no
    /// observation behind it to draw.
    starting: Option<NewRun>,
    pub ui: Ui,
    pub options: Options,
}

impl<V, G> App<V, G>
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn new(game: G, options: Options) -> App<V, G> {
        App {
            game,
            seen: None,
            stage: Stage::Nothing,
            starting: options.fresh.then(NewRun::default),
            ui: Ui::default(),
            options,
        }
    }

    /// What the game last said, for the screens to draw.
    pub fn seen(&self) -> Option<&Applied<V>> {
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
        self.act(action).expect("the game answered the decision");
    }

    /// One key press.
    ///
    /// Raw rather than an [`Action`], because the setup screen is the one place
    /// a letter means a letter. `k` is a seed character there and "move up"
    /// everywhere else, and nothing in the key table tells those apart.
    pub fn key(&mut self, key: KeyEvent) -> Result<()> {
        if self.starting.is_some() {
            return self.in_new_run(key);
        }
        match keys::map(key) {
            Some(action) => self.act(action),
            None => Ok(()),
        }
    }

    /// The run being set up, for the screen that draws it.
    pub fn starting(&self) -> Option<&NewRun> {
        self.starting.as_ref()
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // One look before the first frame, so the screen is the run rather than
        // an empty shape waiting for a key. Skipped while setting one up: it
        // would launch the game to ask about a run nobody has started, and the
        // twenty seconds that costs would be spent before the first frame
        // rather than while a character is being picked.
        if self.starting.is_none() {
            self.look()?;
        }

        while !self.ui.leaving {
            terminal
                .draw(|frame| screen::draw(frame, &self))
                .context("drawing the screen")?;

            match event::read().context("reading a key")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => self.key(key)?,
                // A resize repaints on the next pass round the loop, which is
                // the next statement. Nothing else to do.
                _ => {}
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Acting.
    // -----------------------------------------------------------------------

    fn act(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => {
                self.ui.leaving = true;
                return Ok(());
            }
            Action::Help => {
                self.ui.overlay = match self.ui.overlay {
                    Some(Overlay::Help) => None,
                    _ => Some(Overlay::Help),
                };
                return Ok(());
            }
            Action::Look => return self.look(),
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
                return Ok(());
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
        Ok(())
    }

    /// The belt, which is acted in rather than read.
    ///
    /// A potion that cannot be drunk is refused here off the game's own
    /// `reason`, exactly as an unplayable card is. The belt already carries the
    /// answer, and asking again is a round trip for a line of log nobody needs.
    /// This is not a second opinion about drinkability. No rule about when a
    /// potion can be drunk lives in this crate.
    fn in_belt(&mut self, action: Action) -> Result<()> {
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
        Ok(())
    }

    /// Drinks the nth potion, or starts picking a creature for it.
    ///
    /// The same rule cards get, and for the same reason. A potion with one
    /// legal target still goes through aiming, because aiming is when the
    /// screen shows which creature it is about to land on.
    fn aim_or_drink(&mut self, at: usize) -> Result<()> {
        let Some(potion) = self.observation().and_then(|o| o.potions.get(at)) else {
            return Ok(());
        };

        if let Some(why) = potion.reason {
            let why = super::names::undrinkable(why);
            self.ui
                .say(Tone::Cannot, format!("that potion cannot be drunk: {why}"));
            return Ok(());
        }

        if potion.targets.is_empty() {
            self.ui.overlay = None;
            self.ui.at = 0;
            return self.drink(at, None);
        }

        // The belt closes so the fight underneath is what the aim is drawn
        // over: pointing at a creature hidden behind a panel is pointing at
        // nothing.
        self.ui.overlay = None;
        self.ui.at = 0;
        self.ui.aim = Aim::Drinking { potion: at, at: 0 };
        Ok(())
    }

    fn drink(&mut self, index: usize, target: Option<u32>) -> Result<()> {
        let index = u8::try_from(index).unwrap_or(u8::MAX);
        self.send(Decision::UsePotion {
            index,
            target: target.and_then(nonmax::NonMaxU32::new),
        })
    }

    // -----------------------------------------------------------------------
    // Before there is a run.
    // -----------------------------------------------------------------------

    /// Every character this build offers, in the table's own order.
    ///
    /// Filtered by the build's own `playable`, not by a list written here.
    /// Three of this build's eight are a deprecated placeholder, the Deprived
    /// and a `random` pseudo-character. Which those are is a fact about the
    /// build rather than about this screen.
    pub fn roster(&self) -> Vec<V::Character> {
        Self::characters()
    }

    pub fn characters() -> Vec<V::Character> {
        <V::Character as sts2_core::Table>::ALL
            .iter()
            .copied()
            .filter(|c| sts2_core::CharacterTable::playable(*c))
            .collect()
    }

    /// The setup screen's keys.
    ///
    /// Only the arrows move the cursor. `j` and `k` are seed characters here,
    /// which is the whole reason this screen reads keys rather than actions.
    fn in_new_run(&mut self, key: KeyEvent) -> Result<()> {
        let roster = Self::characters();
        let Some(setting_up) = self.starting.as_mut() else {
            return Ok(());
        };

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('c') {
                self.ui.leaving = true;
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.ui.leaving = true,
            KeyCode::Up | KeyCode::Left => {
                setting_up.at = (setting_up.at + roster.len().saturating_sub(1))
                    .checked_rem(roster.len())
                    .unwrap_or(0);
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                setting_up.at = (setting_up.at + 1).checked_rem(roster.len()).unwrap_or(0);
            }
            KeyCode::Backspace => {
                setting_up.seed.pop();
                setting_up.why = None;
            }
            KeyCode::Enter => return self.begin(),
            // Anything the seed alphabet could hold once it is folded. Filtered
            // as it is typed so the field never shows what the game will not
            // take; the folding itself is `Seed::fold`'s and happens on Enter.
            KeyCode::Char(c)
                if c.is_ascii_alphanumeric() && setting_up.seed.chars().count() < SEED_MAX =>
            {
                setting_up.seed.push(c.to_ascii_uppercase());
                setting_up.why = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Starts the run the setup screen describes.
    ///
    /// A refusal lands back on the setup screen rather than in the log, because
    /// the log is not on the screen yet. See `accept`.
    fn begin(&mut self) -> Result<()> {
        let roster = Self::characters();
        let Some(setting_up) = self.starting.as_mut() else {
            return Ok(());
        };
        let Some(character) = roster.get(setting_up.at).copied() else {
            setting_up.why = Some("this build has no playable character".into());
            return Ok(());
        };

        let typed = match setting_up.seed.trim().is_empty() {
            true => made_up(),
            false => setting_up.seed.clone(),
        };
        let seed = match sts2_core::Seed::fold(&typed) {
            Ok(seed) => seed,
            Err(why) => {
                setting_up.why = Some(format!("{typed:?} is not a seed: {why}"));
                return Ok(());
            }
        };

        // Ascension `None` and no modifiers. Both are real choices this screen
        // does not offer yet, and `sts2 run start` is where they are made.
        let ascension = match crate::cli::ascension::<V>("None") {
            Ok(level) => level,
            Err(why) => {
                setting_up.why = Some(why.to_string());
                return Ok(());
            }
        };

        // The seed is shown back, because folding changes what was typed:
        // there is no `O` in the alphabet, so `ORACLE1` starts `0RACLE1`.
        let said = format!("started {}", seed.as_str());
        let answered = self
            .game
            .play(&Decision::Start {
                seed,
                character,
                ascension,
                modifiers: Vec::new(),
            })
            .context("starting the run")?;

        let began = answered.is_ok();
        self.accept(answered, Some(said));
        if began {
            self.starting = None;
            self.ui.reset();
        }
        Ok(())
    }

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

    fn in_stage(&mut self, action: Action) -> Result<()> {
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
                Ok(())
            }
            Stage::Over | Stage::Nothing => Ok(()),
        }
    }

    // -----------------------------------------------------------------------
    // A fight.
    // -----------------------------------------------------------------------

    fn in_fight(&mut self, action: Action) -> Result<()> {
        let Some(combat) = self.observation().and_then(|o| o.fight()) else {
            return Ok(());
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
                return Ok(());
            }

            let moved = |aim: Aim, to: usize| match aim {
                Aim::Targeting { card, .. } => Aim::Targeting { card, at: to },
                Aim::Drinking { potion, .. } => Aim::Drinking { potion, at: to },
                other => other,
            };

            return match action {
                Action::Cancel => {
                    self.ui.aim = Aim::Idle;
                    Ok(())
                }
                Action::Left | Action::Up => {
                    let to = (at + targets.len() - 1) % targets.len();
                    self.ui.aim = moved(self.ui.aim.clone(), to);
                    Ok(())
                }
                Action::Right | Action::Down => {
                    let to = (at + 1) % targets.len();
                    self.ui.aim = moved(self.ui.aim.clone(), to);
                    Ok(())
                }
                Action::Nth(n) if n < targets.len() => {
                    self.ui.aim = moved(self.ui.aim.clone(), n);
                    Ok(())
                }
                Action::Confirm => {
                    let target = targets[at];
                    let aim = std::mem::take(&mut self.ui.aim);
                    match aim {
                        Aim::Targeting { card, .. } => self.play_card(card, Some(target)),
                        Aim::Drinking { potion, .. } => self.drink(potion, Some(target)),
                        _ => Ok(()),
                    }
                }
                _ => Ok(()),
            };
        }

        match action {
            Action::Left | Action::Up => self.ui.step(-1, hand),
            Action::Right | Action::Down => self.ui.step(1, hand),
            Action::Nth(n) if n < hand => self.ui.at = n,
            Action::EndTurn => return self.send(Decision::EndTurn),
            Action::Inspect => self.inspect_card(),
            Action::Confirm => return self.aim_or_play(),
            _ => {}
        }
        Ok(())
    }

    /// Plays the card under the cursor, or starts picking a creature for it.
    ///
    /// A card with one legal target still enters aiming. That view shows its
    /// computed effect on the creature, including changes from Strength or
    /// Vulnerable. Only a card with no target plays immediately.
    fn aim_or_play(&mut self) -> Result<()> {
        let Some(combat) = self.observation().and_then(|o| o.fight()) else {
            return Ok(());
        };
        let at = self.ui.at;
        let Some(card) = combat.legal.get(at) else {
            return Ok(());
        };

        // The game has already said why, on the card's own face. Sending it to
        // be told again is a round trip for a line of log nobody needs — and
        // this is not a second opinion about legality, it is the game's own
        // `reason` field being believed.
        if let Some(reason) = card.reason {
            let why = crate::render::flags(reason);
            self.ui
                .say(Tone::Cannot, format!("that card cannot be played: {why}"));
            return Ok(());
        }

        match card.targets.is_empty() {
            true => self.play_card(at, None),
            false => {
                self.ui.aim = Aim::Targeting { card: at, at: 0 };
                Ok(())
            }
        }
    }

    fn play_card(&mut self, index: usize, target: Option<u32>) -> Result<()> {
        let index = u8::try_from(index).unwrap_or(u8::MAX);
        self.send(Decision::PlayCard {
            index,
            target: target.and_then(nonmax::NonMaxU32::new),
        })
    }

    // -----------------------------------------------------------------------
    // Rooms, shops, the map.
    // -----------------------------------------------------------------------

    fn in_list(&mut self, action: Action) -> Result<()> {
        let moves = self.moves();
        match action {
            Action::Up | Action::Left => self.ui.step(-1, moves.len()),
            Action::Down | Action::Right => self.ui.step(1, moves.len()),
            // On a numbered list the digit *is* the choice: that is what the
            // number beside each row means. In the hand it only moves the
            // cursor, because a card still has to be aimed.
            Action::Nth(n) if n < moves.len() => {
                self.ui.at = n;
                return self.send(moves[n].clone());
            }
            Action::Confirm => {
                return match moves.get(self.ui.at) {
                    Some(one) => self.send(one.clone()),
                    None => Ok(()),
                };
            }
            _ => {}
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Questions.
    // -----------------------------------------------------------------------

    fn in_card_select(&mut self, action: Action) -> Result<()> {
        let Some(Question::SelectCards {
            min, max, options, ..
        }) = self.seen.as_ref().and_then(|a| a.asking.as_ref())
        else {
            return Ok(());
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
                    return self.send(Decision::Answer(Answer::Cards(marked)));
                }
            }
            // Declining, where declining is allowed.
            Action::Skip if min == 0 => {
                self.ui.aim = Aim::Idle;
                return self.send(Decision::Answer(Answer::Cards(Vec::new())));
            }
            _ => {}
        }

        self.ui.aim = Aim::Selecting(marked);
        Ok(())
    }

    fn in_rewards(&mut self, action: Action) -> Result<()> {
        let Some(Question::Rewards { skippable, .. }) =
            self.seen.as_ref().and_then(|a| a.asking.as_ref())
        else {
            return Ok(());
        };
        let skippable = *skippable;
        let takeable = self.takeable();

        match action {
            Action::Up | Action::Left => self.ui.step(-1, takeable.len()),
            Action::Down | Action::Right => self.ui.step(1, takeable.len()),
            Action::Nth(n) if n < takeable.len() => {
                self.ui.at = n;
                return self.send(Decision::Answer(Answer::Take(vec![takeable[n].clone()])));
            }
            Action::Confirm => {
                if let Some(take) = takeable.get(self.ui.at) {
                    return self.send(Decision::Answer(Answer::Take(vec![take.clone()])));
                }
            }
            Action::Skip => {
                return match skippable {
                    true => self.send(Decision::Answer(Answer::Skip)),
                    false => {
                        self.ui
                            .say(Tone::Cannot, "this reward set cannot be skipped");
                        Ok(())
                    }
                };
            }
            _ => {}
        }
        Ok(())
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
    pub fn aimed_at(&self) -> Option<u32> {
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
        let answered = self.game.observe().context("asking the game where it is")?;
        self.accept(answered, None);
        Ok(())
    }

    fn send(&mut self, decision: Decision<V>) -> Result<()> {
        let said = moves::describe(&decision);
        let answered = self
            .game
            .play(&decision)
            .with_context(|| format!("sending {said}"))?;
        self.accept(answered, Some(said));
        Ok(())
    }

    /// Takes what the game said, whichever half it said.
    ///
    /// A refusal is a value here rather than an error. The game answered, the
    /// run is where it was, and the reason belongs on the screen where the
    /// player can act on it.
    fn accept(&mut self, answered: sts2_core::Answered<V>, said: Option<String>) {
        match answered {
            Ok(applied) => {
                if let Some(said) = said {
                    self.ui.say(Tone::Did, said);
                }
                // A parked decision is not resolved either, and that is
                // ordinary — the question is what tells the two apart. Without
                // the second test this fired on every card selection in the
                // game, including Neow's.
                if !applied.resolved && applied.asking.is_none() {
                    self.ui.say(
                        Tone::Warned,
                        "the game abandoned that mid-cascade — the fight ended under it",
                    );
                }
                let stage = stage_of(&applied);
                if stage != self.stage {
                    self.ui.reset();
                    self.stage = stage;
                }
                self.seen = Some(applied);
            }
            Err(refusal) => {
                // "No run" is not a refusal to report, it is a run waiting to
                // be started. Anything else is the game saying no to something
                // that was asked, which belongs in the log.
                if matches!(refusal.reason, sts2_core::Reason::NoRun) {
                    self.starting.get_or_insert_with(NewRun::default).why = None;
                    return;
                }
                if let Some(setting_up) = self.starting.as_mut() {
                    setting_up.why = Some(crate::render::refusal::refusal(&refusal));
                    return;
                }
                self.ui
                    .say(Tone::Refused, crate::render::refusal::refusal(&refusal));
            }
        }
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
            format!("costs {}", card.cost),
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
            lines.push(crate::render::prose::plain(&preview.text));
        }
        self.ui.overlay = Some(Overlay::Detail(lines));
    }
}

/// A seed for a player who did not want to pick one.
///
/// Off the clock rather than a PRNG, because this crate has no random number
/// generator and does not want one for this. It has to be *a* seed nobody has
/// played, not an unguessable one. The run is recorded with the seed in it
/// either way, so reproducing it never depends on how it was chosen.
fn made_up() -> String {
    let mut n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);

    let alphabet: Vec<char> = sts2_core::Seed::ALPHABET.chars().collect();
    let base = alphabet.len() as u128;
    (0..sts2_core::Seed::DEFAULT_LENGTH)
        .map(|_| {
            let at = (n % base) as usize;
            n /= base;
            alphabet[at]
        })
        .collect()
}

/// Which kind of screen this is.
fn stage_of<V: ContentVersion>(applied: &Applied<V>) -> Stage {
    if let Some(question) = &applied.asking {
        return match question {
            Question::SelectCards { .. } => Stage::AskingCards,
            Question::Rewards { .. } => Stage::AskingRewards,
            Question::Divine { .. } => Stage::AskingDivine,
        };
    }

    let observed = &applied.observation;
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
