//! A game that answers from a transcript, and the game it took one from.
//!
//! Launching the real game costs a second or two and a couple of gigabytes,
//! and it answers a recording the same way every time. So the answers are
//! worth keeping. [`Cached`] holds a transcript and serves it. On a miss it
//! asks whatever it wraps, and writes down what came back.
//!
//! What it keeps is the wire lines, not the observations they decode to.
//! Decoding stays where it was, and a fix to the decoder does not strand a
//! transcript that took a game launch to make.
//!
//! It knows one line of play. Playing something other than what was recorded
//! asks a question the transcript cannot answer. That gets
//! [`Error::OffScript`], not the answer to a different question.

use std::fs;
use std::path::{Path, PathBuf};

use sts2_core::{Answered, Construct, Decision, Play, Setup, Snapshot, Start, Step};

use super::error::Error;
use crate::wire::command::Command;
use crate::{Response, parse_reply};

/// Something that answers the mod's protocol, one line at a time.
///
/// The one thing [`Cached`] needs of a real game, which is the whole of what
/// the protocol is. [`crate::Oracle`] is the implementation. A test that
/// exercises the cache without a game writes its own.
pub trait Transport {
    /// Sends one encoded line and returns the answer, unparsed.
    fn send_line(&mut self, line: &str) -> Result<String, Error>;
}

impl<V: sts2_content_wire::WireContentVersion> Transport for crate::Oracle<V> {
    fn send_line(&mut self, line: &str) -> Result<String, Error> {
        crate::Oracle::send_line(self, line)
    }
}

/// One command, and what the game said to it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Entry {
    /// The encoded command, as the line that was sent.
    cmd: String,
    /// The game's whole answer, as the line that came back.
    reply: String,
}

/// A game that answers from a transcript.
pub struct Cached<P> {
    /// The game to ask on a miss, for a cache that is allowed to grow.
    inner: Option<P>,
    entries: Vec<Entry>,
    /// How far through the transcript the caller has played.
    cursor: usize,
    /// Where the transcript is kept, when it is kept anywhere.
    path: Option<PathBuf>,
    /// Whether anything has been added since it was read.
    grown: bool,
    /// The last answer, for [`Play::observe`], which moves nothing.
    last: Option<Response>,
}

impl<P> Cached<P> {
    /// The transcript at `path`, or nothing where there is not one.
    ///
    /// Nothing rather than an error. A checkout that has not filled its cache
    /// has no transcript, which is ordinary. The caller says what to do about
    /// it.
    pub fn open(path: &Path) -> Option<Self> {
        let text = fs::read_to_string(path).ok()?;
        let entries: Vec<Entry> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .ok()?;

        Some(Self {
            inner: None,
            entries,
            cursor: 0,
            path: Some(path.to_path_buf()),
            grown: false,
            last: None,
        })
    }

    /// The transcript at `path`, with a game behind it to answer what the
    /// transcript cannot.
    pub fn filling(path: &Path, inner: P) -> Self {
        let mut cached = Self::open(path).unwrap_or(Self {
            inner: None,
            entries: Vec::new(),
            cursor: 0,
            path: Some(path.to_path_buf()),
            grown: false,
            last: None,
        });
        cached.inner = Some(inner);
        cached
    }

    /// The game behind the transcript, for the next one to ask.
    pub fn into_inner(self) -> Option<P> {
        self.inner
    }

    /// How many commands the transcript answers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Writes the transcript back, if it has grown.
    ///
    /// Explicit rather than on drop. Writing can fail, and a caller that has
    /// just spent a game launch on the contents wants to hear about it.
    pub fn flush(&mut self) -> Result<(), Error> {
        let (Some(path), true) = (self.path.as_ref(), self.grown) else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::Io("creating the cache directory", e))?;
        }
        let mut text = String::new();
        for entry in &self.entries {
            let line = serde_json::to_string(entry)
                .map_err(|e| Error::Encode("a transcript entry".to_string(), e))?;
            text.push_str(&line);
            text.push('\n');
        }
        fs::write(path, text).map_err(|e| Error::Io("writing the transcript", e))?;
        self.grown = false;
        Ok(())
    }
}

impl<P: Transport> Cached<P> {
    /// The answer to one command, from the transcript or from the game.
    fn answer<V: sts2_content_wire::WireContentVersion>(
        &mut self,
        cmd: Command<V>,
    ) -> Result<String, Error> {
        let line = serde_json::to_string(&cmd).map_err(|e| Error::Encode(format!("{cmd:?}"), e))?;

        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.cmd != line {
                return Err(Error::OffScript {
                    at: self.cursor,
                    expected: entry.cmd.clone(),
                    played: line,
                });
            }
            self.cursor += 1;
            return Ok(entry.reply.clone());
        }

        let inner = self.inner.as_mut().ok_or_else(|| Error::OffScript {
            at: self.cursor,
            expected: "the transcript to end".to_string(),
            played: line.clone(),
        })?;
        let reply = inner.send_line(&line)?;
        self.entries.push(Entry {
            cmd: line,
            reply: reply.clone(),
        });
        self.cursor += 1;
        self.grown = true;
        Ok(reply)
    }

    fn answered<V: sts2_content_wire::WireContentVersion>(
        &mut self,
        cmd: Command<V>,
    ) -> Result<Answered<V>, Error> {
        let reply = self.answer::<V>(cmd)?;
        super::receive(parse_reply::<V>(&reply)?, &mut self.last)
    }

    /// Plays a whole recording, so filling a transcript is one call.
    ///
    /// A step the game refuses is still an answer, and is recorded as one. A
    /// recording is what a real run did. A refusal in one is a fact about the
    /// game, worth keeping rather than raising here.
    pub fn fill<V: sts2_content_wire::WireContentVersion>(
        &mut self,
        steps: &[Step<V>],
    ) -> Result<(), Error> {
        for step in steps {
            let _refusable = self.step(step)?;
        }
        self.flush()
    }
}

/// What it takes to reach a transcript, and the game behind it.
///
/// The game is optional, and its absence is the ordinary case. A test replays
/// a recording the transcript already answers and has no business launching
/// anything. Filling one is the rare case, and says so by supplying a game.
pub struct FromTranscript<P> {
    pub path: PathBuf,
    pub inner: Option<P>,
}

impl<V: sts2_content_wire::WireContentVersion, P: Transport> Play<V> for Cached<P> {
    type Error = Error;
    type Host = FromTranscript<P>;

    /// Answers the start from the transcript, the way a game answers it.
    ///
    /// Starting is a command like any other, so it is the first entry. A cache
    /// with neither a transcript nor a game cannot answer one.
    ///
    /// A refused start takes the cache with it, there being none to hand back.
    /// Filling one starts from a run the game accepts.
    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Error> {
        let mut cached = match host.inner {
            Some(inner) => Self::filling(&host.path, inner),
            None => Self::open(&host.path).ok_or_else(|| {
                Error::NotStarted(format!("no transcript at {}", host.path.display()))
            })?,
        };
        match cached.answered(Command::from(start))? {
            Ok(_) => Ok(cached),
            Err(refusal) => Err(Error::NotStarted(refusal.to_string())),
        }
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Error> {
        self.answered(Command::from(decision))
    }

    /// The last answer, which is what an observation of a run that has not
    /// moved is. No transcript entry is spent, because nothing moved.
    fn observe(&mut self) -> Result<Snapshot<V>, Error> {
        super::snapshot_of(self.last.as_ref())
    }
}

impl<V: sts2_content_wire::WireContentVersion, P: Transport> Construct<V> for Cached<P> {
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Error> {
        self.answered(Command::from(step))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{Coord, Decision};

    type V = V0_107_1;

    /// A game that answers everything with the same refusal, and counts.
    ///
    /// It answers with a refusal rather than an observation. The transcript
    /// mechanics are what is under test here, and a refusal is a whole valid
    /// answer in one line rather than two hundred.
    #[derive(Default)]
    struct Counting {
        asked: Vec<String>,
    }

    impl Transport for Counting {
        fn send_line(&mut self, line: &str) -> Result<String, Error> {
            self.asked.push(line.to_string());
            Ok(
                r#"{"type":"error","cmd":"travel","reason":{"kind":"not_travelable","at":"1,2"}}"#
                    .to_string(),
            )
        }
    }

    fn travel(col: u8, row: u8) -> Decision<V> {
        Decision::Travel {
            to: Coord::new(col, row),
        }
    }

    #[test]
    fn a_filled_transcript_answers_without_asking_the_game_again() {
        let dir = tempdir();
        let path = dir.join("transcript.jsonl");

        let mut filling = Cached::filling(&path, Counting::default());
        for at in 0..3 {
            let _refused = filling.play(&travel(at, 0)).expect("the game answered");
        }
        filling.flush().expect("the transcript is written");
        assert_eq!(filling.inner.as_ref().unwrap().asked.len(), 3);

        // Reopened with no game behind it at all.
        let mut served = Cached::<Counting>::open(&path).expect("the transcript is there");
        assert_eq!(served.len(), 3);
        for at in 0..3 {
            served
                .play(&travel(at, 0))
                .expect("the transcript answered")
                .expect_err("every recorded answer was a refusal");
        }
    }

    /// A cache with neither a transcript nor a game cannot start a run.
    #[test]
    fn beginning_needs_a_transcript_or_a_game() {
        let dir = tempdir();
        let opening: Start<V> = Start {
            seed: sts2_core::Seed::new("0RACLE1").expect("a fixture's seed is a seed"),
            character: sts2_content::v0_107_1::characters::Character::Regent,
            ascension: sts2_content::v0_107_1::ascension::AscensionLevel::None,
            modifiers: Vec::new(),
        };

        let absent = Cached::<Counting>::begin(
            FromTranscript {
                path: dir.join("nothing.jsonl"),
                inner: None,
            },
            &opening,
        );
        assert!(
            matches!(absent, Err(Error::NotStarted(_))),
            "an absent transcript cannot start a run"
        );
    }

    #[test]
    fn playing_something_else_is_refused_rather_than_answered() {
        let dir = tempdir();
        let path = dir.join("transcript.jsonl");
        let mut filling = Cached::filling(&path, Counting::default());
        let _refused = filling.play(&travel(1, 1)).expect("the game answered");
        filling.flush().expect("the transcript is written");

        let mut served = Cached::<Counting>::open(&path).expect("the transcript is there");
        let error = served
            .play(&travel(9, 9))
            .expect_err("the transcript has a different step here");

        assert!(
            matches!(error, Error::OffScript { at: 0, .. }),
            "{error:?} should name the step it parted at"
        );
    }

    /// Past the end with no game behind it is the same refusal.
    #[test]
    fn a_transcript_that_runs_out_says_so() {
        let dir = tempdir();
        let path = dir.join("transcript.jsonl");
        let mut filling = Cached::filling(&path, Counting::default());
        let _refused = filling.play(&travel(1, 1)).expect("the game answered");
        filling.flush().expect("the transcript is written");

        let mut served = Cached::<Counting>::open(&path).expect("the transcript is there");
        let _refused = served.play(&travel(1, 1)).expect("the recorded step");
        let error = served.play(&travel(2, 2)).expect_err("there is no more");

        assert!(matches!(error, Error::OffScript { at: 1, .. }), "{error:?}");
    }

    #[test]
    fn an_absent_transcript_is_nothing_rather_than_an_error() {
        let dir = tempdir();
        assert!(Cached::<Counting>::open(&dir.join("never-written.jsonl")).is_none());
    }

    /// A game behind it answers what the transcript does not have, and the
    /// transcript keeps it.
    #[test]
    fn a_miss_asks_the_game_and_grows() {
        let dir = tempdir();
        let path = dir.join("transcript.jsonl");

        let mut first = Cached::filling(&path, Counting::default());
        let _refused = first.play(&travel(1, 1)).expect("the game answered");
        first.flush().expect("the transcript is written");

        let mut second = Cached::filling(&path, Counting::default());
        let _refused = second.play(&travel(1, 1)).expect("the transcript answered");
        let _refused = second.play(&travel(2, 2)).expect("the game answered");
        second.flush().expect("the transcript is written");

        // One of the two came from the transcript, so only one was asked.
        assert_eq!(second.inner.as_ref().unwrap().asked.len(), 1);
        assert_eq!(
            Cached::<Counting>::open(&path).expect("it is there").len(),
            2
        );
    }

    /// A directory of this test's own, under the target directory cargo gives
    /// integration tests and benches.
    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "sts2-cached-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("a scratch directory");
        base
    }
}
