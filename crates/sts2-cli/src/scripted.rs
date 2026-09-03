//! One game for the tests to drive, written down rather than launched.
//!
//! Every front-end test asks the same three things of a game. That it answers
//! what the test chose. That it notes what it was told. That it costs no
//! twenty-second boot. Each test module used to grow its own. Five of them had,
//! and they disagreed about small things while claiming the same one.
//!
//! What the game *says* still comes from [`crate::fixture`]. This only decides
//! when it says it.

use std::collections::VecDeque;

use sts2_core::{Answered, Applied, Construct, ContentVersion, Decision, Play, Setup};

use crate::tui::check::Sent;

/// A game that answers from a script, and notes everything it is told.
///
/// The error type is [`std::convert::Infallible`]: a scripted game has no
/// socket to lose. A refusal is not an error, and is written into the script
/// like any other answer.
pub struct Scripted<V: ContentVersion> {
    /// The answers still to give, in order.
    answers: VecDeque<Answered<V>>,
    /// What it goes on saying once those run out.
    ///
    /// `None` fails the test instead, which is what [`Scripted::untouchable`]
    /// is built on.
    then: Option<Answered<V>>,
    sent: Sent<V>,
    constructed: Vec<Setup<V>>,
    looked: usize,
}

impl<V: ContentVersion> Scripted<V> {
    /// A game that says one thing, forever.
    ///
    /// Enough to draw a screen against, and enough for a claim about what the
    /// front end sends. A screen is a function of the last observation, and
    /// nothing the front end sends depends on what comes back.
    pub fn saying(applied: Applied<V>) -> Scripted<V> {
        Scripted::script(Vec::new(), Some(Ok(applied)))
    }

    /// A game that says these things in turn, then goes on saying the last.
    ///
    /// For a test about a sequence: a refusal and then an answer, or a state
    /// that changes under a command.
    pub fn saying_in_turn(answers: Vec<Answered<V>>) -> Scripted<V> {
        let last = answers.last().cloned();
        Scripted::script(answers, last)
    }

    /// A game that fails the test if it is asked anything at all.
    ///
    /// For a claim that some part of the front end decides without asking. It
    /// holds that claim structurally, rather than by reading the code.
    pub fn untouchable() -> Scripted<V> {
        Scripted::script(Vec::new(), None)
    }

    fn script(answers: Vec<Answered<V>>, then: Option<Answered<V>>) -> Scripted<V> {
        Scripted {
            answers: answers.into(),
            then,
            sent: Sent::default(),
            constructed: Vec::new(),
            looked: 0,
        }
    }

    /// Every decision it has been told, shared with the caller.
    ///
    /// A handle rather than a copy, because the front end takes the game and a
    /// test cannot reach back through it.
    pub fn sent(&self) -> Sent<V> {
        self.sent.clone()
    }

    /// Every position it was put into, which ordinary play does not do.
    pub fn constructed(&self) -> &[Setup<V>] {
        &self.constructed
    }

    /// How many times it was asked where the run is standing.
    pub fn looked(&self) -> usize {
        self.looked
    }

    /// The next answer in the script.
    fn next(&mut self, asked: &str) -> Answered<V> {
        if let Some(answer) = self.answers.pop_front() {
            return answer;
        }
        match &self.then {
            Some(answer) => answer.clone(),
            None => panic!("a game that was to be left alone was asked to {asked}"),
        }
    }
}

impl<V: ContentVersion> Play<V> for Scripted<V> {
    type Error = std::convert::Infallible;

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
        self.sent.note(decision.clone());
        Ok(self.next("play a decision"))
    }

    fn observe(&mut self) -> Result<Answered<V>, Self::Error> {
        self.looked += 1;
        Ok(self.next("say where the run is standing"))
    }
}

impl<V: ContentVersion> Construct<V> for Scripted<V> {
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Self::Error> {
        self.constructed.push(step.clone());
        Ok(self.next("reach a position"))
    }
}

#[cfg(test)]
mod tests {
    use super::Scripted;
    use crate::fixture;
    use serde_json::json;
    use sts2_core::{Decision, Play, Reason, Refusal};

    type V = sts2_content::v0_107_1::V0_107_1;

    fn plain() -> sts2_core::Applied<V> {
        fixture::applied(json!({}))
    }

    /// The claim every `untouchable` test rests on. A game that answered
    /// quietly instead leaves all of those tests proving nothing.
    #[test]
    #[should_panic(expected = "left alone")]
    fn a_game_to_be_left_alone_fails_the_test_when_it_is_asked() {
        let mut game: Scripted<V> = Scripted::untouchable();

        let _ = game.observe();
    }

    /// One answer, however many times it is asked.
    #[test]
    fn a_canned_game_says_the_same_thing_again() {
        let mut game = Scripted::saying(plain());

        assert!(game.play(&Decision::EndTurn).unwrap().is_ok());
        assert!(game.play(&Decision::EndTurn).unwrap().is_ok());
        assert_eq!(game.sent().count(), 2);
    }

    /// A script runs in order, and then holds on its last line.
    #[test]
    fn a_script_holds_on_its_last_answer() {
        let mut game = Scripted::saying_in_turn(vec![
            Ok(plain()),
            Err(Refusal {
                what: "end_turn".into(),
                reason: Reason::NoRun,
            }),
        ]);

        assert!(game.play(&Decision::EndTurn).unwrap().is_ok());
        assert!(game.play(&Decision::EndTurn).unwrap().is_err());
        assert!(
            game.play(&Decision::EndTurn).unwrap().is_err(),
            "the script ran out and the game changed its mind"
        );
    }
}
