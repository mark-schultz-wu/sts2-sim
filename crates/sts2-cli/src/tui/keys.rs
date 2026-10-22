//! Keys to intentions.
//!
//! One table, so what a key does can be read off in one place and shown to the
//! player from the same list. [`HELP`] and [`map`] are written beside each
//! other, and a key that appears in one and not the other is visible.
//!
//! The intention is not the move. `Confirm` on a card is a card being played,
//! and on a shop entry it is a purchase. What a key *means* is here. What it
//! does is `app`, which is the only place that knows what the game is showing.

use super::input::{Key, KeyCode};

/// What a key asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Help,
    /// Ask the game where it is, without moving it. The repair for a screen
    /// that got out of step with the run, which does not happen and is cheap to
    /// offer anyway.
    Look,
    Up,
    Down,
    Left,
    Right,
    /// Play the card, take the option, buy the thing.
    Confirm,
    /// Put the card down, close the overlay, leave the pile.
    Cancel,
    /// Add the thing under the cursor to a multiple selection.
    Mark,
    EndTurn,
    /// Skip a reward set, or take the rest site's "leave" option.
    Skip,
    /// Jump the cursor to the nth thing in front of it.
    Nth(usize),
    ShowDeck,
    ShowDraw,
    ShowDiscard,
    ShowExhaust,
    ShowMap,
    /// The belt, which is also the only way to drink from it.
    ShowPotions,
    /// What every relic being carried does.
    ShowRelics,
    /// The thing under the cursor, in full.
    Inspect,
    ScrollBack,
    ScrollOn,
}

/// The keys, as the help panel lists them.
///
/// Grouped the way a player learns them rather than alphabetically: moving
/// about, then acting, then looking at things.
pub const HELP: &[(&str, &str)] = &[
    ("←/→  h/l", "move the cursor"),
    ("↑/↓  k/j", "move the cursor, and scroll a list"),
    ("1-9", "take the nth option — or point at the nth card"),
    ("enter", "play the card, take the option"),
    ("esc", "put it down, close the panel"),
    ("space", "mark a card, where several are wanted"),
    ("e", "end the turn"),
    ("s", "skip — a reward set, or a rest"),
    ("i", "look at the thing under the cursor"),
    ("d / w / f / x", "deck, draw pile, discard, exhaust"),
    ("m", "the act's map"),
    ("p", "your potions — and drink one"),
    ("v", "your relics, and what they do"),
    ("[ / ]", "scroll the log"),
    ("r", "ask the game where it is"),
    ("?", "this"),
    ("q", "leave — the run keeps going"),
];

/// What a key press asked for, or nothing for a key with no meaning here.
///
/// `ctrl-c` is quit as well as `q`, because a terminal program that ignores it
/// is a terminal program somebody has to kill.
pub fn map(key: Key) -> Option<Action> {
    if key.ctrl {
        return match key.code {
            KeyCode::Char('c') => Some(Action::Quit),
            _ => None,
        };
    }

    Some(match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('?') => Action::Help,
        KeyCode::Char('r') => Action::Look,
        KeyCode::Up | KeyCode::Char('k') => Action::Up,
        KeyCode::Down | KeyCode::Char('j') => Action::Down,
        KeyCode::Left | KeyCode::Char('h') => Action::Left,
        KeyCode::Right | KeyCode::Char('l') => Action::Right,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Esc => Action::Cancel,
        KeyCode::Char(' ') => Action::Mark,
        KeyCode::Char('e') => Action::EndTurn,
        KeyCode::Char('s') => Action::Skip,
        KeyCode::Char('i') => Action::Inspect,
        KeyCode::Char('d') => Action::ShowDeck,
        KeyCode::Char('w') => Action::ShowDraw,
        KeyCode::Char('f') => Action::ShowDiscard,
        KeyCode::Char('x') => Action::ShowExhaust,
        KeyCode::Char('m') => Action::ShowMap,
        KeyCode::Char('p') => Action::ShowPotions,
        KeyCode::Char('v') => Action::ShowRelics,
        KeyCode::Char('[') => Action::ScrollBack,
        KeyCode::Char(']') => Action::ScrollOn,
        // `1` is the first, so the digits line up with what is drawn beside
        // each card rather than with the index the command takes.
        KeyCode::Char(c @ '1'..='9') => Action::Nth(c as usize - '1' as usize),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> Option<Action> {
        map(Key::from(code))
    }

    #[test]
    fn digits_are_one_based_because_the_screen_is() {
        assert_eq!(press(KeyCode::Char('1')), Some(Action::Nth(0)));
        assert_eq!(press(KeyCode::Char('9')), Some(Action::Nth(8)));
    }

    #[test]
    fn a_key_with_no_meaning_is_no_action() {
        assert_eq!(press(KeyCode::Char('§')), None);
        assert_eq!(press(KeyCode::Backspace), None);
    }

    #[test]
    fn control_c_leaves() {
        let key = Key {
            code: KeyCode::Char('c'),
            ctrl: true,
        };
        assert_eq!(map(key), Some(Action::Quit));
    }

    /// The help panel is the only documentation of the keymap, so a key that
    /// exists and is not listed is a key nobody will find.
    #[test]
    fn every_listed_key_is_a_key() {
        for (keys, _) in HELP {
            assert!(!keys.is_empty());
        }
        assert!(HELP.iter().any(|(k, _)| k.contains("enter")));
    }
}
