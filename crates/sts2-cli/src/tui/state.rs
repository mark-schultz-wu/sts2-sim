//! What the front end knows that the game does not.
//!
//! A cursor, a card being aimed, a scrollback, an open overlay. None of it is
//! run state, because the game is the only thing that holds that, and none of
//! it survives the process. Kept apart from the observation for the same reason
//! the observation is kept whole. Anything mixed into it is a number on the
//! screen that no implementation reported.

use std::collections::VecDeque;
use std::path::PathBuf;

/// What the front end was told at startup.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Where the run is being written, for the header to say so.
    pub recording: Option<PathBuf>,
}

/// Which pile the pile viewer is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pile {
    Deck,
    Draw,
    Discard,
    Exhaust,
}

impl Pile {
    pub const fn title(self) -> &'static str {
        match self {
            Pile::Deck => "Deck",
            Pile::Draw => "Draw pile",
            Pile::Discard => "Discard",
            Pile::Exhaust => "Exhaust",
        }
    }
}

/// A panel over the screen, which the next key closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    /// The keys, on `?`.
    Help,
    /// A pile, listed and counted.
    Pile(Pile),
    /// The act, drawn, when a fight is covering it.
    Map,
    /// The belt. Unlike the others this one is acted in rather than read: it is
    /// where a potion is chosen, so its keys are its own.
    Belt,
    /// Every relic being carried, and what each one does.
    Relics,
    /// One thing, in full: its id, its numbers and its words.
    Detail(Vec<String>),
}

/// What the player is in the middle of doing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Aim {
    /// Nothing. The cursor is just sitting on a card.
    #[default]
    Idle,
    /// A card is chosen and a creature is being picked for it.
    Targeting {
        /// The card's index in hand.
        card: usize,
        /// Which of that card's legal targets is under the cursor.
        at: usize,
    },
    /// A potion is chosen and a creature is being picked for it.
    ///
    /// Separate from [`Aim::Targeting`] rather than a flag on it, because what
    /// confirming sends is a different decision. A screen drawing the aim has to
    /// say which of the two it is showing.
    Drinking {
        /// The potion's index in the belt.
        potion: usize,
        /// Which of that potion's legal targets is under the cursor.
        at: usize,
    },
    /// A select-cards question is open and these options are chosen.
    Selecting(Vec<usize>),
}

/// A run being set up, before there is one.
///
/// Its own state rather than a screen driven by the observation, because there
/// is no observation. The game has refused to say where it is standing, on the
/// grounds that it is not standing anywhere.
#[derive(Debug, Clone, Default)]
pub struct NewRun {
    /// Which playable character is under the cursor.
    pub at: usize,
    /// What has been typed. Empty means "pick one for me".
    pub seed: String,
    /// Why the last attempt did not start a run, for the screen to show.
    pub why: Option<String>,
}

/// The longest seed the field will take.
///
/// The game generates ten and accepts other lengths, so this is a field width
/// rather than a rule. Long enough that nothing a person types is cut.
pub const SEED_MAX: usize = 20;

/// One line of the scrollback.
#[derive(Debug, Clone)]
pub struct Note {
    pub text: String,
    pub tone: Tone,
}

/// How a note reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A move that happened.
    Did,
    /// The game saying no, which is information and not a fault.
    Refused,
    /// The front end saying it cannot express something. Distinct from a
    /// refusal, because the game did not object. This did.
    Cannot,
    /// Something the game logged beside a move that still worked.
    Warned,
}

/// The cursor, the aim, the log and the overlay.
#[derive(Debug, Clone, Default)]
pub struct Ui {
    /// The cursor in whatever list is in front of the player. The hand in a
    /// fight, the options in a room, the destinations on a map.
    pub at: usize,
    pub aim: Aim,
    pub overlay: Option<Overlay>,
    pub log: VecDeque<Note>,
    /// How far back the log is scrolled, in lines from the newest.
    pub scrolled: usize,
    pub leaving: bool,
    /// The last thing that went wrong, shown over the screen until the next
    /// key. A screen too short for a log shows nothing else of it.
    pub toast: Option<Note>,
}

/// How much scrollback is kept. Enough to see a whole fight's worth of moves
/// and small enough that nothing has to be paged.
const KEPT: usize = 500;

impl Ui {
    pub fn say(&mut self, tone: Tone, text: impl Into<String>) {
        let note = Note {
            text: text.into(),
            tone,
        };
        // A move that happened is in the log and on the screen. One that did
        // not is news, and a player looking at the hand misses a log line.
        if tone != Tone::Did {
            self.toast = Some(note.clone());
        }
        self.log.push_back(note);
        while self.log.len() > KEPT {
            self.log.pop_front();
        }
        // A new line is a new bottom: something just happened, and the player
        // is looking at it rather than at where they had scrolled to.
        self.scrolled = 0;
    }

    /// Puts the cursor back at the top and drops any half-made move.
    ///
    /// Called when the game moves on to a different kind of screen. An index
    /// into the last screen's list means nothing in the next one's.
    pub fn reset(&mut self) {
        self.at = 0;
        self.aim = Aim::Idle;
    }

    /// Moves the cursor within a list of `len`, wrapping.
    ///
    /// Wrapping because every list here is short, and a person holding a key
    /// down does not have to notice where the end is.
    pub fn step(&mut self, by: isize, len: usize) {
        if len == 0 {
            self.at = 0;
            return;
        }
        let len = len as isize;
        self.at = ((self.at as isize + by).rem_euclid(len)) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_wraps_both_ways() {
        let mut ui = Ui::default();
        ui.step(-1, 5);
        assert_eq!(ui.at, 4);
        ui.step(1, 5);
        assert_eq!(ui.at, 0);
    }

    /// A list that just emptied, as when the last card in hand was played, must
    /// not leave the cursor pointing past the end of it.
    #[test]
    fn an_empty_list_parks_the_cursor() {
        let mut ui = Ui {
            at: 3,
            ..Ui::default()
        };
        ui.step(1, 0);
        assert_eq!(ui.at, 0);
    }

    #[test]
    fn the_log_forgets_the_oldest_first() {
        let mut ui = Ui::default();
        for i in 0..KEPT + 10 {
            ui.say(Tone::Did, i.to_string());
        }
        assert_eq!(ui.log.len(), KEPT);
        assert_eq!(ui.log.front().map(|n| n.text.as_str()), Some("10"));
    }

    /// A refusal pops up, and a move that worked does not.
    #[test]
    fn only_news_is_toasted() {
        let mut ui = Ui::default();
        ui.say(Tone::Did, "played Bash");
        assert!(ui.toast.is_none());
        ui.say(Tone::Refused, "not enough energy");
        assert_eq!(
            ui.toast.map(|note| note.text).as_deref(),
            Some("not enough energy")
        );
    }

    #[test]
    fn something_happening_scrolls_back_to_it() {
        let mut ui = Ui {
            scrolled: 12,
            ..Ui::default()
        };
        ui.say(Tone::Did, "played Bash");
        assert_eq!(ui.scrolled, 0);
    }
}
