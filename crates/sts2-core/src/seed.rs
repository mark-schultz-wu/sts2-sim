//! A run's seed, as the game spells it.

/// Why a string is not a seed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A seed with nothing in it.
    #[error("a seed must have at least one character")]
    Empty,

    /// A character the alphabet does not contain.
    #[error("{0:?} is not a character a seed can contain")]
    Character(char),
}

/// A run's seed, as the game spells it.
///
/// A `Seed` is the text the game echoes back, so two that differ are two runs.
/// [`Seed::new`] refuses any other spelling, and [`Seed::fold`] converts what a
/// person typed.
///
/// The number a run's streams are seeded from is the RNG's `stream_seed`. It is
/// a free function there, so a client driving a game need not compile a PRNG.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Seed(String);

impl Seed {
    /// The characters a seed is made of.
    ///
    /// Thirty-four: the digits and the letters, less `I` and `O`, which are left
    /// out so that neither can be confused with `1` or `0`.
    pub const ALPHABET: &'static str = "0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";

    /// What the game generates when it picks one -- `SeedHelper.seedDefaultLength`.
    ///
    /// Not a limit. The game accepts other lengths, and its own headless driver
    /// defaults to a seven-character one.
    pub const DEFAULT_LENGTH: usize = 10;

    /// The seed this text spells, or why it spells none.
    ///
    /// Whitespace, lowercase, `I` and `O` are all refused. [`Seed::fold`]
    /// accepts them.
    pub fn new(text: &str) -> Result<Seed, Error> {
        if text.is_empty() {
            return Err(Error::Empty);
        }

        match text.chars().find(|c| !Self::ALPHABET.contains(*c)) {
            Some(bad) => Err(Error::Character(bad)),
            None => Ok(Seed(text.to_string())),
        }
    }

    /// The seed a person typing this text meant.
    ///
    /// `SeedHelper.CanonicalizeSeed` trims, uppercases, and maps `O` to `0` and
    /// `I` to `1`, so a player typing `abcdef` reaches the run `ABCDEF`. What
    /// comes back is not always the text that went in.
    pub fn fold(typed: &str) -> Result<Seed, Error> {
        let folded: String = typed
            .trim()
            .chars()
            .map(|c| match c.to_ascii_uppercase() {
                'O' => '0',
                'I' => '1',
                upper => upper,
            })
            .collect();

        Seed::new(&folded)
    }

    /// The spelling, which is what goes to the game.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod seeds {
    use super::*;

    /// Every one of these is a seed the game plays once folded, and `new`
    /// refuses all of them.
    #[test]
    fn new_refuses_a_spelling_it_would_have_to_change() {
        assert_eq!(Seed::new("abcdef"), Err(Error::Character('a')));
        assert_eq!(Seed::new(" ABCDEF"), Err(Error::Character(' ')));
        assert_eq!(Seed::new("ABCDEF\n"), Err(Error::Character('\n')));
        assert_eq!(Seed::new("IO"), Err(Error::Character('I')));

        assert_eq!(Seed::new("ABCDEF").unwrap().as_str(), "ABCDEF");
    }

    /// `fold` folds the way the game folds.
    #[test]
    fn fold_matches_canonicalize_seed() {
        let canonical = Seed::new("ABCDEF").unwrap();

        for typed in ["abcdef", "  ABCDEF  ", "AbCdEf", "\tabcdef\n"] {
            assert_eq!(Seed::fold(typed).unwrap(), canonical, "{typed:?}");
        }

        // `I` and `O` are outside the alphabet so they cannot be confused with
        // `1` and `0`, which is what a person typing them meant.
        assert_eq!(Seed::fold("IO").unwrap().as_str(), "10");
        assert_eq!(Seed::fold("io").unwrap().as_str(), "10");
    }

    /// A seed that already is one folds to itself.
    #[test]
    fn fold_leaves_a_canonical_seed_alone() {
        for text in ["ABCDEF", "SP1RE1", "0", Seed::ALPHABET] {
            assert_eq!(
                Seed::fold(text).unwrap(),
                Seed::new(text).unwrap(),
                "{text}"
            );
        }
    }

    /// The alphabet is the game's, and it is missing exactly two letters.
    #[test]
    fn the_alphabet_omits_the_letters_that_look_like_digits() {
        assert_eq!(Seed::ALPHABET.len(), 34);
        assert!(!Seed::ALPHABET.contains('I'));
        assert!(!Seed::ALPHABET.contains('O'));

        for c in "0123456789".chars() {
            assert!(Seed::ALPHABET.contains(c));
        }
    }

    /// What no fold can rescue.
    #[test]
    fn a_seed_that_is_not_one_says_which_character() {
        assert_eq!(Seed::new(""), Err(Error::Empty));
        assert_eq!(Seed::fold(""), Err(Error::Empty));
        assert_eq!(Seed::fold("   "), Err(Error::Empty));

        assert_eq!(Seed::fold("AB-CD"), Err(Error::Character('-')));
        assert_eq!(Seed::fold("ABÉ"), Err(Error::Character('É')));
        assert_eq!(Seed::new("AB CD"), Err(Error::Character(' ')));
    }
}
