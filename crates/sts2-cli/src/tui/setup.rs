//! Picking a seed and a character, before there is a run.
//!
//! Nothing here talks to a game. A run has not started, so there is nothing to
//! ask it. There is nothing to draw beside this either: no strip, because
//! there are no numbers, and no log, because nothing has happened.
//!
//! What this produces is a [`Start`]. Taking one to an implementation is
//! [`super::run`]'s job, which is why this screen is not part of [`App`]. The
//! app is a window onto a run, and holds one.
//!
//! [`App`]: super::app::App

use anyhow::{Context, Result};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use sts2_core::{ContentVersion, Start};

use super::screen::start;
use super::state::{NewRun, SEED_MAX};

/// The setup screen, and what has been typed into it.
#[derive(Debug)]
pub struct Screen<V: ContentVersion> {
    setting_up: NewRun,
    roster: Vec<V::Character>,
}

impl<V: ContentVersion> Default for Screen<V> {
    fn default() -> Screen<V> {
        Screen {
            setting_up: NewRun::default(),
            roster: playable::<V>(),
        }
    }
}

/// What one key press did.
enum Typed<V: ContentVersion> {
    /// The screen changed, or the key meant nothing here.
    Nothing,
    /// The player asked to leave without starting anything.
    Leaving,
    /// The run the screen describes.
    Start(Start<V>),
}

/// Every character this build lets a run be played as.
pub fn playable<V: ContentVersion>() -> Vec<V::Character> {
    <V::Character as sts2_core::Table>::ALL
        .iter()
        .copied()
        .filter(|c| sts2_core::CharacterTable::playable(*c))
        .collect()
}

impl<V: ContentVersion> Screen<V> {
    /// Says why the last attempt started nothing, for the next frame to show.
    pub fn refused(&mut self, why: String) {
        self.setting_up.why = Some(why);
    }

    /// The setup screen's keys.
    ///
    /// Only the arrows move the cursor. `j` and `k` are seed characters here,
    /// which is the whole reason this screen reads keys rather than actions.
    fn key(&mut self, key: KeyEvent) -> Typed<V> {
        let setting_up = &mut self.setting_up;
        let roster = &self.roster;

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('c') {
                return Typed::Leaving;
            }
            return Typed::Nothing;
        }

        match key.code {
            KeyCode::Esc => return Typed::Leaving,
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
            KeyCode::Enter => return self.described(),
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
        Typed::Nothing
    }

    /// The run this screen describes.
    ///
    /// Everything it can reject itself, it rejects onto the screen. There is no
    /// log yet to write a reason into.
    fn described(&mut self) -> Typed<V> {
        let setting_up = &mut self.setting_up;
        let Some(character) = self.roster.get(setting_up.at).copied() else {
            setting_up.why = Some("this build has no playable character".into());
            return Typed::Nothing;
        };

        let typed = match setting_up.seed.trim().is_empty() {
            true => made_up(),
            false => setting_up.seed.clone(),
        };
        let seed = match sts2_core::Seed::fold(&typed) {
            Ok(seed) => seed,
            Err(why) => {
                setting_up.why = Some(format!("{typed:?} is not a seed: {why}"));
                return Typed::Nothing;
            }
        };

        // Ascension `None` and no modifiers. Both are real choices this screen
        // does not offer yet, and `sts2 run start` is where they are made.
        let ascension = match crate::cli::ascension::<V>("None") {
            Ok(level) => level,
            Err(why) => {
                setting_up.why = Some(why.to_string());
                return Typed::Nothing;
            }
        };

        // The folded seed is what travels, so a typed `ORACLE1` starts
        // `0RACLE1`, the alphabet having no `O`.
        Typed::Start(Start {
            seed,
            character,
            ascension,
            modifiers: Vec::new(),
        })
    }
}

impl<V: ContentVersion> Screen<V> {
    /// Draws and reads keys until this describes a run, or the player leaves.
    ///
    /// `None` when they left. Blocked on rather than polled, the way the run
    /// screen is. A player picking a character sends nothing, and nothing is
    /// what happens.
    pub fn collect(&mut self, terminal: &mut DefaultTerminal) -> Result<Option<Start<V>>> {
        loop {
            terminal
                .draw(|frame| {
                    start::draw::<V>(frame, frame.area(), &self.roster, &self.setting_up);
                })
                .context("drawing the setup screen")?;

            let Event::Key(key) = event::read().context("reading a key")? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match self.key(key) {
                Typed::Nothing => {}
                Typed::Leaving => return Ok(None),
                Typed::Start(start) => return Ok(Some(start)),
            }
        }
    }
}

/// A seed nobody typed, for a blank field.
///
/// The clock rather than a generator, because this needs no quality. It is a
/// name for a run, and the game seeds its own randomness from it.
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use sts2_content::v0_107_1::V0_107_1;

    type V = V0_107_1;

    /// Draws the screen and drives it with real keys.
    ///
    /// Keys rather than actions, because the whole point of this screen is that
    /// a letter is a letter here. `k` types a `k` instead of moving the cursor,
    /// and only a `KeyEvent` can say so.
    fn typing(
        w: u16,
        h: u16,
        keys: &str,
        before: impl FnOnce(&mut Screen<V>),
    ) -> (String, Option<Start<V>>) {
        let mut screen = Screen::<V>::default();
        before(&mut screen);

        let mut started = None;
        for key in keys.chars() {
            // A newline in the script is the Enter key, so a test can spell
            // "type this and start" as one string.
            let code = match key {
                '\n' => KeyCode::Enter,
                c => KeyCode::Char(c),
            };
            if let Typed::Start(start) = screen.key(KeyEvent::from(code)) {
                started = Some(start);
            }
        }

        let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("a test terminal");
        terminal
            .draw(|frame| {
                start::draw::<V>(frame, frame.area(), &screen.roster, &screen.setting_up);
            })
            .expect("a frame");

        let buffer = terminal.backend().buffer();
        let text = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, started)
    }

    fn press(screen: &mut Screen<V>, code: KeyCode) -> Option<Start<V>> {
        match screen.key(KeyEvent::from(code)) {
            Typed::Start(start) => Some(start),
            _ => None,
        }
    }

    /// A bare `sts2` with nothing running opens on a character to pick and a
    /// seed to type. Not on advice about a command to go and run.
    #[test]
    fn a_fresh_front_end_opens_on_a_new_run() {
        let (out, started) = typing(100, 30, "", |_| {});
        println!("\n{out}\n");

        assert!(out.contains("a new run"), "{out}");
        assert!(out.contains("Ironclad"), "the roster: {out}");
        assert!(out.contains("seed"), "{out}");
        // Nothing is asked of the game until a character is chosen, which is
        // what keeps the twenty-second boot off the first frame.
        assert!(started.is_none(), "nothing started a run");
    }

    /// A letter is a letter here. `k` is "move up" on every other screen, and
    /// typing a seed with a `k` in it must not walk the roster.
    #[test]
    fn the_seed_field_takes_letters_that_are_keys_elsewhere() {
        let (out, _) = typing(100, 30, "kjdw", |_| {});
        assert!(out.contains("KJDW"), "the seed field: {out}");
        // The first character is still the one under the cursor.
        let (before, _) = out.split_once("seed").expect("a seed field");
        assert!(before.contains("> Defect"), "the cursor moved: {before}");
    }

    /// Folded the way the game folds it, so what is typed and what is played
    /// can differ. The alphabet has no `O`.
    #[test]
    fn starting_describes_the_character_and_the_folded_seed() {
        let (_, started) = typing(100, 30, "oracle1\n", |screen| {
            // Past Defect, onto Ironclad.
            press(screen, KeyCode::Down);
        });
        let start = started.expect("the screen described a run");

        assert_eq!(start.seed.as_str(), "0RACLE1", "the seed was not folded");
        assert_eq!(crate::render::spelling(&start.character), "IRONCLAD");
        assert!(start.modifiers.is_empty());
    }

    /// A blank field is not a blank seed. It means "pick one", and what is
    /// picked has to be a seed the game accepts.
    #[test]
    fn a_blank_seed_is_made_up_rather_than_refused() {
        let (_, started) = typing(100, 30, "\n", |_| {});
        let start = started.expect("the screen described a run");

        assert_eq!(
            start.seed.as_str().chars().count(),
            sts2_core::Seed::DEFAULT_LENGTH
        );
        assert!(
            start
                .seed
                .as_str()
                .chars()
                .all(|c| sts2_core::Seed::ALPHABET.contains(c)),
            "{:?} is not spelled in the game's alphabet",
            start.seed
        );
    }
}
