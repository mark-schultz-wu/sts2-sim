//! Keys, as the TUI reads them, whatever delivered them.
//!
//! The screens and the app read these and never a terminal library's events.
//! `term` turns a crossterm event into one. An ssh or browser front end turns
//! its own input into one, and the rest of the TUI cannot tell.

/// Which key.
///
/// Only the keys some screen reads. A key outside this list means nothing to
/// any of them, so a driver drops it rather than passing it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Backspace,
    Tab,
}

/// One key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    /// Held with control. Only `ctrl-c` means anything, and it means leave.
    pub ctrl: bool,
}

impl From<KeyCode> for Key {
    fn from(code: KeyCode) -> Key {
        Key { code, ctrl: false }
    }
}
