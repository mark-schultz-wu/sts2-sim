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
use sts2_core::{Play, Start};

use super::app::{App, Ended};
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

impl<V, G> App<V, G>
where
    V: crate::tui::Build,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    /// Draws and reads keys until the player leaves, or the run goes away.
    ///
    /// The daemon's game can idle out from under a live screen. What comes back
    /// says which happened, because a run that is gone wants the setup screen
    /// rather than a window onto nothing.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<Ended, Error> {
        // One look before the first frame, so the screen is the run rather than
        // an empty shape waiting for a key.
        self.look()?;
        while !self.ui.leaving {
            terminal
                .draw(|frame| screen::draw(frame, &self))
                .map_err(|source| Error::Draw {
                    screen: "screen",
                    source,
                })?;
            let key = next_key()?;
            self.key(key)?;
        }
        Ok(self.ended())
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

/// Draws a run and reads keys, until the player leaves or the run goes away.
///
/// A window onto a run that has already started. What run that is, and what to
/// do once this returns, belong to the caller: this draws one and reports how
/// it ended.
pub fn play<V, G>(terminal: &mut DefaultTerminal, game: G, options: Options) -> Result<Ended, Error>
where
    V: crate::tui::Build,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    App::playing(game, options).run(terminal)
}
