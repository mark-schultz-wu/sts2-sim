//! One command, performed against anything that plays the game.
//!
//! What a front end does between parsing a command line and printing an answer.
//! Nothing here knows which implementation answered, or what holds it between
//! two commands. That is [`crate::oracle`]'s, and a simulator brings none of it.
//!
//! The binary is then clap declarations, a `type` line naming the
//! implementation, and printing. Pointing it at a second implementation copies
//! no orchestration, because the orchestration is here and generic.

use anyhow::{Context, Result};
use sts2_core::{Answered, Applied, Construct, ContentVersion, Play, Step};

use crate::cli::{Taking, choose};

/// What a front end can ask of the run itself.
///
/// Every variant is answerable by any implementation. Starting a daemon,
/// reporting one, and stopping one are commands about the machinery instead,
/// and [`crate::oracle`] holds those.
#[derive(Debug)]
pub enum Command {
    /// One step, as a command line spells it.
    Step(crate::cli::Play),
    /// A numbered option, whichever mechanism the game is asking through.
    Choose(Vec<Taking>),
    /// The act, drawn.
    Map,
}

/// What performing one command produced.
///
/// Two shapes because two things get printed. A state goes through the
/// renderer, which the caller owns. The oracle adds a protocol envelope and a
/// game log to it, and a simulator has neither.
#[derive(Debug)]
pub enum Outcome<V: ContentVersion> {
    /// A state to print.
    ///
    /// Boxed because a whole observation dwarfs a drawn map, and this is one
    /// command per process rather than a hot loop.
    Seen(Box<Applied<V>>),
    /// Words that are the whole answer, ending in their own newline.
    Said(String),
}

/// A fault an implementation reports, which is not a refusal.
///
/// Every function here carries it into `anyhow`, so a caller names the
/// implementation once and never again.
pub trait Fault: std::error::Error + Send + Sync + 'static {}

impl<E: std::error::Error + Send + Sync + 'static> Fault for E {}

/// Performs one command, and reports what to print.
///
/// # Errors
///
/// A refusal, a fault, or a command the state does not accept. The run stands
/// where it stood in the first two cases.
pub fn perform<V, G>(game: &mut G, command: &Command) -> Result<Outcome<V>>
where
    V: ContentVersion,
    G: Construct<V>,
    G::Error: Fault,
{
    match command {
        Command::Map => {
            let seen = look(game)?;
            let drawn = crate::render::map::drawn(&seen.observation.map)
                .context("this observation carried no map")?;
            Ok(Outcome::Said(drawn))
        }

        // One extra look before acting, because which mechanism the game is
        // asking through is its state rather than the caller's intent.
        // Observing advances nothing, which is what makes looking safe.
        Command::Choose(indices) => {
            let looked = look(game)?;
            let awaiting = choose::awaiting(&looked)?;
            let decision = choose::command::<V>(&awaiting, indices)?;

            eprintln!("({})", choose::describe(&awaiting));
            Ok(Outcome::Seen(Box::new(send(game, &decision.into())?)))
        }

        Command::Step(play) => match play.step::<V>()? {
            Some(step) => Ok(Outcome::Seen(Box::new(send(game, &step)?))),
            // `observe` reads as a command and is not a step in the run.
            None => Ok(Outcome::Seen(Box::new(look(game)?))),
        },
    }
}

/// Sends one step and returns what any implementation says.
///
/// No context of its own. A caller that has some adds the step it sent. One
/// command at a shell reads better without a line saying it sent what it was
/// asked to send.
///
/// # Errors
///
/// A refusal, or a fault. See [`accepted`] for why a refusal is one here.
pub fn send<V, G>(game: &mut G, step: &Step<V>) -> Result<Applied<V>>
where
    V: ContentVersion,
    G: Construct<V>,
    G::Error: Fault,
{
    accepted(game.step(step)?)
}

/// Asks where the run is standing, without moving it.
///
/// What a front end does when it has nothing in hand, which is every time. This
/// is one process per command, so the last answer went out with the last
/// process.
///
/// # Errors
///
/// A refusal, or a fault. There is no run to report before one is started.
pub fn look<V, G>(game: &mut G) -> Result<Applied<V>>
where
    V: ContentVersion,
    G: Play<V>,
    G::Error: Fault,
{
    accepted(game.observe().context("asking the game where it is")?)
}

/// A refusal, turned into an error.
///
/// It is the game answering rather than this failing, which is why it is a
/// value at the trait. A shell wants a non-zero exit status for a move that did
/// not happen, and that is what an `Err` is here. A front end that keeps
/// playing takes the refusal as a value and words it with
/// [`crate::render::refusal`] instead.
///
/// # Errors
///
/// The refusal, worded by this crate.
pub fn accepted<V: ContentVersion>(answered: Answered<V>) -> Result<Applied<V>> {
    // Worded here rather than through `Display`: `sts2-core` carries the
    // refusal as data and does not name a card, which is this crate's job.
    answered.map_err(|refused| anyhow::anyhow!("{}", crate::render::refusal::refusal(&refused)))
}

/// Folds a recording's steps into a game with nothing in it.
///
/// Reports the state the last step produced. The caller wants to stand where
/// the recording ended, so the states along the way are counted and dropped.
///
/// # Errors
///
/// The step that broke, named by its position, so a divergence points at the
/// move rather than at whatever came after.
pub fn replay<V, G>(game: &mut G, steps: &[Step<V>]) -> Result<Option<Applied<V>>>
where
    V: ContentVersion,
    G: Construct<V>,
    G::Error: Fault,
{
    let total = steps.len();
    let mut last = None;

    for (at, step) in steps.iter().enumerate() {
        eprint!("\rreplaying {}/{total}", at + 1);
        // Line-buffered output would hold the counter until the newline below,
        // so a long replay would show nothing until it had finished.
        let _ = std::io::Write::flush(&mut std::io::stderr());
        last = Some(send(game, step).with_context(|| format!("step {} of {total}", at + 1))?);
    }
    if total > 0 {
        eprintln!();
    }

    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::{Command, Outcome, perform, replay};
    use crate::fixture;
    use serde_json::json;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{Answered, Applied, Construct, Decision, Play, Reason, Refusal, Setup, Step};

    type V = V0_107_1;

    /// A game that answers from a script, and writes down what it was sent.
    ///
    /// Enough to prove what the controller does with an answer. What the real
    /// game says to the same step is the oracle's business, and the live tests'.
    struct Scripted {
        answers: Vec<Answered<V>>,
        sent: Vec<Step<V>>,
        looked: usize,
    }

    impl Scripted {
        fn saying(answers: Vec<Answered<V>>) -> Scripted {
            Scripted {
                answers,
                sent: Vec::new(),
                looked: 0,
            }
        }

        fn next(&mut self) -> Answered<V> {
            match self.answers.is_empty() {
                true => Ok(fixture::applied(json!({}))),
                false => self.answers.remove(0),
            }
        }
    }

    impl Play<V> for Scripted {
        type Error = std::io::Error;

        fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
            self.sent.push(Step::Decision(decision.clone()));
            Ok(self.next())
        }

        fn observe(&mut self) -> Result<Answered<V>, Self::Error> {
            self.looked += 1;
            Ok(self.next())
        }
    }

    impl Construct<V> for Scripted {
        fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Self::Error> {
            self.sent.push(Step::Setup(step.clone()));
            Ok(self.next())
        }
    }

    fn refused() -> Answered<V> {
        Err(Refusal {
            what: "end_turn".into(),
            reason: Reason::NoRun,
        })
    }

    /// `observe` reads the run without adding a step to it.
    #[test]
    fn observing_sends_nothing() {
        let mut game = Scripted::saying(Vec::new());

        let outcome = perform(&mut game, &Command::Step(crate::cli::Play::Observe)).unwrap();

        assert!(matches!(outcome, Outcome::Seen(_)));
        assert_eq!(game.looked, 1);
        assert!(game.sent.is_empty(), "{:?}", game.sent);
    }

    /// The look `choose` takes first is what tells it which answer to send.
    #[test]
    fn choosing_looks_before_it_acts() {
        let asking = fixture::answered(json!({
            "type": "pending", "seq": 7, "cmd": "end_turn", "digest": "AA",
            "state": {},
            "kind": "rewards", "skippable": true,
            "rewards": [{"index": 0, "type": "GoldReward", "populated": true,
                         "taken": false, "amount": 25}]
        }));
        let mut game = Scripted::saying(vec![Ok(asking), Ok(fixture::applied(json!({})))]);

        perform(&mut game, &Command::Choose(vec!["0".parse().unwrap()])).unwrap();

        assert_eq!(game.looked, 1);
        assert_eq!(game.sent.len(), 1, "{:?}", game.sent);
    }

    /// A refusal is the game answering, and a non-zero exit status here.
    #[test]
    fn a_refusal_is_an_error_at_a_command_line() {
        let mut game = Scripted::saying(vec![refused()]);

        let outcome = perform(&mut game, &Command::Step(crate::cli::Play::EndTurn));

        assert!(outcome.is_err());
    }

    /// A replay stops at the step that broke, not at the one after it.
    #[test]
    fn a_replay_stops_where_it_diverged() {
        let steps = vec![
            Step::Decision(Decision::EndTurn),
            Step::Decision(Decision::Proceed),
            Step::Decision(Decision::EndTurn),
        ];
        let mut game = Scripted::saying(vec![
            Ok(fixture::applied(json!({}))),
            refused(),
            Ok(fixture::applied(json!({}))),
        ]);

        let outcome = replay(&mut game, &steps);

        assert!(outcome.is_err());
        assert_eq!(game.sent.len(), 2, "the third step went out anyway");
        assert!(format!("{:#}", outcome.unwrap_err()).contains("step 2 of 3"));
    }

    /// Every step in a whole recording lands, in order.
    #[test]
    fn a_replay_sends_every_step_in_order() {
        let steps = vec![
            Step::Decision(Decision::EndTurn),
            Step::Decision(Decision::Proceed),
        ];
        let mut game = Scripted::saying(Vec::new());

        let last = replay(&mut game, &steps).unwrap();

        assert!(last.is_some());
        assert_eq!(game.sent, steps);
    }

    /// Nothing to fold is nothing to report, rather than a failure.
    #[test]
    fn an_empty_replay_reports_nothing() {
        let mut game = Scripted::saying(Vec::new());

        let last: Option<Applied<V>> = replay(&mut game, &[]).unwrap();

        assert!(last.is_none());
    }
}
