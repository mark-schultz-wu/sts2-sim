//! The TUI on a local terminal: the only part that knows crossterm.
//!
//! Each loop here draws a frame, then blocks on the next key. It does not
//! poll. A player thinking about a card sends nothing, and nothing happens:
//! the daemon keeps the game alive on its own, so the screen has no clock to
//! answer to. A resize repaints on the next pass round the loop.
//!
//! Everything else reads [`Key`] and draws into any ratatui backend. So a
//! front end over another transport writes its own version of this module
//! and nothing more.

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::KeyCode as Crossterm;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind, KeyModifiers};
use sts2_core::{Decision, RunTable, SeatIndex, Settled, Start, Verdict};

use super::app::{App, Ended, Intent};
use super::input::{Key, KeyCode};
use super::screen;
use super::setup::{Screen, Typed};
use super::{Error, Options};

/// A crossterm key press as the TUI reads it, or nothing for a release, a
/// repeat, or a key no screen reads.
pub fn key(event: KeyEvent) -> Option<Key> {
    if event.kind != KeyEventKind::Press {
        return None;
    }
    let code = match event.code {
        Crossterm::Char(c) => KeyCode::Char(c),
        Crossterm::Up => KeyCode::Up,
        Crossterm::Down => KeyCode::Down,
        Crossterm::Left => KeyCode::Left,
        Crossterm::Right => KeyCode::Right,
        Crossterm::Enter => KeyCode::Enter,
        Crossterm::Esc => KeyCode::Esc,
        Crossterm::Backspace => KeyCode::Backspace,
        Crossterm::Tab => KeyCode::Tab,
        _ => return None,
    };
    Some(Key {
        code,
        ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
    })
}

/// The next key a screen reads, blocking until one comes.
fn next_key() -> Result<Key, Error> {
    loop {
        if let Event::Key(event) = event::read().map_err(Error::Key)?
            && let Some(key) = key(event)
        {
            return Ok(key);
        }
    }
}

/// A person at a terminal, as a player: the screen, drawn and read until a
/// key asks for a move.
///
/// A fault of the terminal's own stops the run, as the player leaving does,
/// and is kept for [`Human::fault`].
pub struct Human<'t, V: crate::tui::Build> {
    app: App<V>,
    terminal: &'t mut DefaultTerminal,
    fault: Option<Error>,
}

impl<'t, V: crate::tui::Build> Human<'t, V> {
    /// A player at `terminal`.
    pub fn at(terminal: &'t mut DefaultTerminal, options: Options) -> Self {
        Self {
            app: App::new(options),
            terminal,
            fault: None,
        }
    }

    /// Why the screen stopped: the player left, or the run went away.
    pub fn ended_as(&self) -> Ended {
        self.app.ended()
    }

    /// What went wrong with the terminal, if anything did.
    pub fn fault(&mut self) -> Option<Error> {
        self.fault.take()
    }

    /// Draws the screen and blocks on the next key, or keeps the fault.
    fn next(&mut self) -> Option<Key> {
        let app = &self.app;
        let read = self
            .terminal
            .draw(|frame| screen::draw(frame, app))
            .map_err(|source| Error::Draw {
                screen: "screen",
                source,
            })
            .and_then(|_| next_key());
        match read {
            Ok(key) => Some(key),
            Err(fault) => {
                self.fault = Some(fault);
                None
            }
        }
    }
}

impl<V: crate::tui::Build> sts2_run::Player<V> for Human<'_, V> {
    /// Shows where the run stands, then reads keys until one asks for a move.
    /// The screen reads the moves off the run's view, so `_moves` goes
    /// unread.
    fn choose<T: RunTable<V>>(
        &mut self,
        seat: SeatIndex,
        table: &mut T,
        _moves: &[Decision<V>],
    ) -> Result<Option<Decision<V>>, T::Error> {
        if self.app.leaving() {
            return Ok(None);
        }
        self.app.show(table.snapshot(seat)?);
        loop {
            let Some(key) = self.next() else {
                return Ok(None);
            };
            match self.app.key(key) {
                Intent::Nothing if self.app.leaving() => return Ok(None),
                Intent::Nothing => {}
                Intent::Look => self.app.show(table.snapshot(seat)?),
                Intent::Send(decision) => return Ok(Some(decision)),
            }
        }
    }

    fn answered(&mut self, _: SeatIndex, decision: &Decision<V>, settled: &Settled<V>) {
        self.app.answered(decision, settled);
    }

    /// Shows the run as it ended, and reads keys until the player leaves.
    fn ended<T: RunTable<V>>(&mut self, table: &mut T, verdict: &Verdict) {
        if self.app.leaving() || self.fault.is_some() {
            return;
        }
        if let Ok(standing) = table.snapshot(SeatIndex(0)) {
            self.app.show(standing);
        }
        if !matches!(verdict.end, sts2_core::End::Won | sts2_core::End::Lost) {
            self.app
                .ui
                .say(super::state::Tone::Refused, verdict.to_string());
        }
        while !self.app.leaving() {
            let Some(key) = self.next() else {
                return;
            };
            if let Intent::Look = self.app.key(key)
                && let Ok(standing) = table.snapshot(SeatIndex(0))
            {
                self.app.show(standing);
            }
        }
    }
}

impl<V: crate::tui::Build> Screen<V> {
    /// Draws and reads keys until this describes a run, or the player leaves.
    ///
    /// `None` when they left.
    pub fn collect(&mut self, terminal: &mut DefaultTerminal) -> Result<Option<Start<V>>, Error> {
        loop {
            terminal
                .draw(|frame| self.draw(frame))
                .map_err(|source| Error::Draw {
                    screen: "setup screen",
                    source,
                })?;
            match self.key(next_key()?) {
                Typed::Nothing => {}
                Typed::Leaving => return Ok(None),
                Typed::Start(start) => return Ok(Some(start)),
            }
        }
    }
}

/// Plays the run at `table`, which is under way, with the person at
/// `terminal` taking every seat, until they leave or the run goes away.
///
/// A window onto a run that has already started. What run that is, and what to
/// do once this returns, belong to the caller: this plays one and reports how
/// it ended.
pub fn play<V, T>(
    terminal: &mut DefaultTerminal,
    table: &mut T,
    options: Options,
) -> Result<Ended, Error>
where
    V: crate::tui::Build,
    T: RunTable<V>,
    sts2_run::Fault: From<T::Error>,
{
    let mut human = Human::at(terminal, options);
    let verdict = sts2_run::play_at(table, &mut human, usize::MAX);
    if let Some(fault) = human.fault() {
        return Err(fault);
    }
    Ok(match (human.ended_as(), verdict.end) {
        (Ended::NoRun, _) => Ended::NoRun,
        // A run that stopped short, on a fault of its table's, wants the
        // setup screen rather than a window onto nothing.
        (_, sts2_core::End::Failed | sts2_core::End::GameGone) => Ended::NoRun,
        _ => Ended::Quit,
    })
}
