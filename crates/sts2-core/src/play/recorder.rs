//! Writing a run down as it is played, around any implementation.
//!
//! [`Recorder`] wraps something that implements [`Play`] and implements `Play`
//! itself, so anything driving a game can be handed one instead and never know.
//! Every step the inner game accepts is written to a [`Tape`] on the way past.
//!
//! A step is written after the inner game answers, and only if it accepted.
//! [`Play::observe`] is never written. A step that cannot be written ends the
//! recording and is reported through [`Recorder::fault`], rather than failing
//! the move.
//!
//! The crate README says why recording is a wrapper rather than a feature of an
//! implementation, and where each of those three rules comes from.

use std::marker::PhantomData;

use crate::{
    Answered, Construct, ContentVersion, Decision, Play, Recording, Setup, Snapshot, Start, Step,
};

/// Somewhere a run is written down, one step at a time.
///
/// One step at a time rather than all at the end. A run being played is a run
/// open to interruption. A recording that only exists once the session closes
/// cleanly is a recording of the sessions that did not need one.
pub trait Tape<V: ContentVersion> {
    /// Why a step was not written. Reported, never returned to the caller that
    /// made the move. See the module docs.
    type Error: std::fmt::Display;

    fn record(&mut self, step: &Step<V>) -> Result<(), Self::Error>;
}

/// The tape a test wants: the steps, in order, in memory.
impl<V: ContentVersion> Tape<V> for Recording<V> {
    type Error = std::convert::Infallible;

    fn record(&mut self, step: &Step<V>) -> Result<(), Self::Error> {
        self.push(step.clone());
        Ok(())
    }
}

/// An implementation, and a note of the run being played through it.
pub struct Recorder<V: ContentVersion, G, T> {
    inner: G,
    /// Taken when the tape fails, which is what stops the recording.
    tape: Option<T>,
    fault: Option<String>,
    build: PhantomData<V>,
}

impl<V: ContentVersion, G, T> Recorder<V, G, T> {
    pub fn new(inner: G, tape: T) -> Recorder<V, G, T> {
        Recorder {
            inner,
            tape: Some(tape),
            fault: None,
            build: PhantomData,
        }
    }

    /// The game underneath, for a caller that needs the implementation's own
    /// surface rather than [`Play`]: the oracle's log, a simulator's internals.
    pub fn inner(&self) -> &G {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut G {
        &mut self.inner
    }

    /// What has been recorded, while it is still recording.
    pub fn tape(&self) -> Option<&T> {
        self.tape.as_ref()
    }

    /// Why the recording stopped, for a front end that has to say so.
    ///
    /// `None` covers both "still recording" and "was never asked to stop".
    /// [`Recorder::recording`] is the question about whether it is still going.
    pub fn fault(&self) -> Option<&str> {
        self.fault.as_deref()
    }

    /// Whether steps are still being written.
    pub fn recording(&self) -> bool {
        self.tape.is_some()
    }

    /// Stops recording and hands the tape back.
    pub fn stop(&mut self) -> Option<T> {
        self.tape.take()
    }

    /// Gives the game back, and the tape with it.
    pub fn into_parts(self) -> (G, Option<T>) {
        (self.inner, self.tape)
    }
}

impl<V, G, T> Recorder<V, G, T>
where
    V: ContentVersion,
    G: Play<V>,
    T: Tape<V>,
{
    /// Writes one step, if the game took it and the tape is still going.
    ///
    /// Takes the outcome rather than being called only on success. The rule
    /// about what is recorded is then in one place, not at each call site.
    fn keep(&mut self, step: Step<V>, outcome: &Result<Answered<V>, G::Error>) {
        if !matches!(outcome, Ok(Ok(_))) {
            return;
        }
        let Some(tape) = self.tape.as_mut() else {
            return;
        };

        if let Err(why) = tape.record(&step) {
            self.fault = Some(why.to_string());
            self.tape = None;
        }
    }
}

impl<V, G, T> Play<V> for Recorder<V, G, T>
where
    V: ContentVersion,
    G: Play<V>,
    T: Tape<V>,
{
    type Error = G::Error;

    /// What the game underneath needs, and the tape to write it to.
    ///
    /// The tape is built by the caller, who is holding the [`Start`] anyway.
    /// So a tape that names the run it records needs nothing from here.
    type Host = (G::Host, T);

    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let (inner, tape) = host;
        Ok(Recorder::new(G::begin(inner, start)?, tape))
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
        let outcome = self.inner.play(decision);
        self.keep(Step::Decision(decision.clone()), &outcome);
        outcome
    }

    /// Not recorded: it moves nothing.
    fn observe(&mut self) -> Result<Snapshot<V>, Self::Error> {
        self.inner.observe()
    }
}

/// Constructing is recordable, and has to be: a run reached by `fight` cannot be
/// reproduced from a tape that dropped it.
impl<V, G, T> Construct<V> for Recorder<V, G, T>
where
    V: ContentVersion,
    G: Construct<V>,
    T: Tape<V>,
{
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Self::Error> {
        let outcome = self.inner.setup(step);
        self.keep(Step::Setup(step.clone()), &outcome);
        outcome
    }
}
