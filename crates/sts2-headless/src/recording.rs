//! Reading and writing a recording, in the oracle's spelling.
//!
//! JSON Lines: a build stamp, then one line per step, each step spelled as a
//! [`crate::wire::command::Command`]. In a run of several players each step
//! also names its seat, as `"seat"` on the same line.
//!
//! The crate README has the format and the example, and says why the format is
//! here rather than in `sts2-core`.

use crate::stamp::Stamp;
use crate::wire::command::Command;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use sts2_core::{Recording, SeatIndex, Start, Step, Tape};

/// Writes the stamp and every step.
///
/// For a recording taken all at once. A caller writing as it plays uses
/// [`open`] and [`append`] instead, which is what makes a killed session still
/// leave a replayable file.
pub fn write<V: sts2_content_wire::WireContentVersion>(
    recording: &Recording<V>,
    mut out: impl Write,
) -> io::Result<()> {
    open::<V>(&mut out)?;
    began(&mut out, recording.start())?;
    let solo = recording.start().players.len() == 1;
    for (step, &seat) in recording.steps().iter().zip(recording.seats()) {
        match solo {
            true => append(&mut out, step)?,
            false => append_by(&mut out, seat, step)?,
        }
    }
    Ok(())
}

/// Writes the command that starts the run, which is a recording's first line.
///
/// Its own function rather than a case inside [`append`], because a [`Start`]
/// is not a [`Step`]. A file has exactly one of these, and it comes first.
pub fn began<V: sts2_content_wire::WireContentVersion>(
    mut out: impl Write,
    start: &Start<V>,
) -> io::Result<()> {
    let wire: Command<V> = start.into();
    writeln!(out, "{}", serde_json::to_string(&wire)?)
}

/// Writes the stamp line that opens a recording of this build.
pub fn open<V: sts2_content_wire::WireContentVersion>(mut out: impl Write) -> io::Result<()> {
    writeln!(out, "{}", serde_json::to_string(&Stamp::of(&V::VERSION))?)
}

/// Writes one step of a solo run.
///
/// Nothing here buffers across calls, so a process that dies between two steps
/// has written the first.
pub fn append<V: sts2_content_wire::WireContentVersion>(
    mut out: impl Write,
    step: &Step<V>,
) -> io::Result<()> {
    let wire: Command<V> = step.into();
    writeln!(out, "{}", serde_json::to_string(&wire)?)
}

/// Writes one step of a run of several players, which `seat` took.
///
/// The seat comes first on the line, so a reader sees whose step it is
/// before what it is.
pub fn append_by<V: sts2_content_wire::WireContentVersion>(
    mut out: impl Write,
    seat: SeatIndex,
    step: &Step<V>,
) -> io::Result<()> {
    let wire: Command<V> = step.into();
    let line = serde_json::to_string(&wire)?;
    let fields = line
        .strip_prefix('{')
        .expect("a command is spelled as an object");
    writeln!(out, "{{\"seat\":{},{fields}", seat.0)
}

/// A file being written to as the run is played.
///
/// The [`Tape`] a [`Recorder`](sts2_core::Recorder) writes to when the run is
/// meant to survive the process. Flushed per step, which is the whole reason
/// the format is one line per step.
///
/// Parameterised by the build, because the stamp is. A writer opened for one
/// build and fed another's steps produces a file whose header lies.
#[derive(Debug)]
pub struct Writer<V: sts2_content_wire::WireContentVersion> {
    path: PathBuf,
    file: File,
    steps: usize,
    /// How many players the run has.
    players: usize,
    build: PhantomData<V>,
}

impl<V: sts2_content_wire::WireContentVersion> Writer<V> {
    /// Begins a recording of `start`, replacing whatever was at the path.
    ///
    /// The run is named at the top rather than recorded as it goes. A
    /// recording has one, and it is what every later line is relative to. The
    /// caller has it, because it is what they are about to begin.
    pub fn create(path: impl Into<PathBuf>, start: &Start<V>) -> io::Result<Writer<V>> {
        let path = path.into();
        let mut file = File::create(&path)?;
        open::<V>(&mut file)?;
        began(&mut file, start)?;
        file.flush()?;

        Ok(Writer {
            path,
            file,
            steps: 0,
            players: start.players.len(),
            build: PhantomData,
        })
    }

    /// Carries on a recording that already exists.
    ///
    /// What a replay wants. The file already holds the steps being replayed.
    /// Rewriting it means destroying the run to reproduce it. A replay that
    /// fails halfway then leaves the file somebody opened to investigate
    /// truncated at the failure.
    ///
    /// The steps already there are counted rather than assumed to be none, so
    /// [`Writer::steps`] is the run's length rather than this session's share
    /// of it.
    pub fn resume(path: impl Into<PathBuf>) -> io::Result<Writer<V>> {
        let path = path.into();
        let existing = std::fs::read_to_string(&path)?;

        // A file with no start after its stamp is not a recording to carry
        // on. The start says how many players the run has.
        let mut lines = existing.lines().filter(|l| !l.trim().is_empty());
        let start = lines
            .nth(1)
            .and_then(|line| serde_json::from_str::<Command<V>>(line.trim()).ok())
            .and_then(Command::into_start)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{} starts no run; there is no recording to carry on",
                        path.display()
                    ),
                )
            })?;
        let steps = lines.count();

        let file = File::options().append(true).open(&path)?;
        Ok(Writer {
            path,
            file,
            steps,
            players: start.players.len(),
            build: PhantomData,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How long the run on disk is, this session's steps included.
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Writes a step `seat` took, in a run of several players.
    ///
    /// # Panics
    ///
    /// On a solo run, whose steps name no seat, or a seat the run lacks.
    pub fn record_by(&mut self, seat: SeatIndex, step: &Step<V>) -> io::Result<()> {
        assert!(
            self.players > 1 && seat.at() < self.players,
            "seat {} in a run of {} players",
            seat.0,
            self.players
        );
        append_by(&mut self.file, seat, step)?;
        self.file.flush()?;
        self.steps += 1;
        Ok(())
    }
}

impl<V: sts2_content_wire::WireContentVersion> Tape<V> for Writer<V> {
    type Error = io::Error;

    /// # Panics
    ///
    /// On a run of several players, whose steps go through
    /// [`Writer::record_by`].
    fn record(&mut self, step: &Step<V>) -> Result<(), io::Error> {
        assert_eq!(self.players, 1, "a step of a co-op run names its seat");
        append(&mut self.file, step)?;
        // Insurance, not the mechanism: `File` does not buffer, so the step is
        // already down by the line above. It is here so that wrapping the file
        // in something that *does* buffer stays correct rather than silently
        // losing the last few steps of every killed session.
        self.file.flush()?;
        self.steps += 1;
        Ok(())
    }
}

/// Reads a recording, refusing one taken against another build.
///
/// The refusal is the point. Ids are the same strings across builds and mean
/// different things. A recording from another version replays without complaint
/// and produces a different run, which is the failure this project exists to
/// make impossible.
pub fn read<V: sts2_content_wire::WireContentVersion>(
    input: impl BufRead,
) -> Result<Recording<V>, Error> {
    let mut lines = input.lines();

    let head = lines.next().transpose()?.ok_or(Error::Empty)?;
    let stamp: Stamp = serde_json::from_str(head.trim()).map_err(|e| Error::Header {
        line: head.trim().to_string(),
        source: e,
    })?;

    if !stamp.describes(&V::VERSION) {
        return Err(Error::WrongBuild {
            expected: Stamp::of(&V::VERSION).to_string(),
            found: stamp.to_string(),
        });
    }

    // The first command starts the run, and everything after it is a step. A
    // file that opens with anything else is a fragment rather than a run, and
    // saying so here is what lets `Recording` hold a `Start` rather than hope
    // for one.
    let mut recording: Option<Recording<V>> = None;
    for (at, line) in lines.enumerate() {
        let line = line?;
        // A recording gets concatenated, edited and pasted, and a trailing
        // newline should not be a parse error.
        if line.trim().is_empty() {
            continue;
        }
        // Numbered as a reader would: the stamp is line 1.
        let at = at + 2;
        let named = |line: &str| line.trim().to_string();

        let command: Command<V> = serde_json::from_str(line.trim()).map_err(|e| Error::Step {
            at,
            line: named(&line),
            source: e,
        })?;

        // A command spells no seat, so it reads past one. Whether the line
        // should have one depends on the run.
        let seat = serde_json::from_str::<Seated>(line.trim())
            .map_err(|e| Error::Step {
                at,
                line: named(&line),
                source: e,
            })?
            .seat;

        match recording.as_mut() {
            None => {
                let start = command
                    .into_start()
                    .filter(|_| seat.is_none())
                    .ok_or_else(|| Error::NotAStart {
                        at,
                        line: named(&line),
                    })?;
                recording = Some(Recording::new(start));
            }
            Some(recording) => {
                let step = command.into_step().ok_or_else(|| Error::NotAStep {
                    at,
                    line: named(&line),
                })?;
                let players = recording.start().players.len();
                let seat = match (players, seat) {
                    (1, None) => SeatIndex(0),
                    (2.., Some(seat)) if usize::from(seat) < players => SeatIndex(seat),
                    _ => {
                        return Err(Error::Seat {
                            at,
                            players,
                            line: named(&line),
                        });
                    }
                };
                recording.push_by(seat, step);
            }
        }
    }

    recording.ok_or(Error::NoStart)
}

/// Whose a step is, read off its line beside the command.
#[derive(serde::Deserialize)]
struct Seated {
    seat: Option<u8>,
}

/// Why a recording did not read.
///
/// Its own type rather than a [`crate::Error`] variant. That one is what goes
/// wrong between this process and the game, and a file is neither.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the recording is empty; it should open with a build stamp")]
    Empty,

    #[error("recorded against {found}, but this is {expected}")]
    WrongBuild { expected: String, found: String },

    #[error("the first line is not a build stamp: {line}")]
    Header {
        line: String,
        source: serde_json::Error,
    },

    #[error("line {at} is not a step this build knows: {line}")]
    Step {
        at: usize,
        line: String,
        source: serde_json::Error,
    },

    /// A command the protocol accepts but a run is not made of.
    ///
    /// `observe` and `quit` parse perfectly well and are not steps. Observing
    /// changes nothing, and quitting ends the session rather than the run. A
    /// recording holding one cannot be replayed past it. Refused on the way in
    /// rather than skipped, because a file containing one was written by
    /// something that misunderstood the format.
    #[error("line {at} is a command but not a step in a run: {line}")]
    NotAStep { at: usize, line: String },

    /// A recording opens with the command that starts the run. One that opens
    /// with anything else is a fragment, and replaying it plays onto a run
    /// nothing started.
    #[error("line {at} does not start a run, so this is a fragment rather than a run: {line}")]
    NotAStart { at: usize, line: String },

    /// A step of a run of several players names one of its seats, and a
    /// solo run's step names none.
    #[error("line {at} does not name a seat of a run of {players} players as it should: {line}")]
    Seat {
        at: usize,
        players: usize,
        line: String,
    },

    /// The stamp read, and then the file ended.
    #[error("the recording has no commands in it, so it starts no run")]
    NoStart,

    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::Seed;
    use sts2_core::{Coord, Decision, Start};

    type V = V0_107_1;
    type Character = <V as sts2_core::ContentVersion>::Character;
    type AscensionLevel = <V as sts2_core::ContentVersion>::AscensionLevel;

    fn recorded() -> Recording<V> {
        let mut r = Recording::new(Start {
            seed: Seed::new("SP1RE1").expect("a seed"),
            players: vec![sts2_core::Player {
                character: Character::Ironclad,
                wins: 0,
            }],
            ascension: AscensionLevel::None,
            modifiers: vec![],
        });
        r.push(Decision::Travel {
            to: Coord::new(1, 1),
        });
        r
    }

    fn written(r: &Recording<V>) -> String {
        let mut out = Vec::new();
        write(r, &mut out).expect("writing to a Vec cannot fail");
        String::from_utf8(out).expect("json is utf-8")
    }

    #[test]
    fn a_recording_round_trips() {
        let text = written(&recorded());
        assert_eq!(
            read::<V>(text.as_bytes()).expect("its own build"),
            recorded()
        );
    }

    /// The stamp is a line of its own, so a step is a line of its own too.
    #[test]
    fn it_is_one_line_per_step() {
        let text = written(&recorded());
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(lines.len(), 3, "a stamp and two steps: {text}");
        assert!(lines[0].contains("contentHash"), "{text}");
        assert_eq!(lines[2], r#"{"cmd":"travel","col":1,"row":1}"#);
    }

    /// A file cut off mid-run is a shorter run, not a broken file. The format
    /// is lines so that this holds.
    #[test]
    fn a_truncated_recording_replays_as_far_as_it_got() {
        let text = written(&recorded());
        let cut = text.lines().take(2).collect::<Vec<_>>().join("\n");

        let read = read::<V>(cut.as_bytes()).expect("still a recording");
        assert!(read.is_empty(), "the start is not a step");
        assert_eq!(read.start().seed.as_str(), "SP1RE1");
    }

    /// A file that opens with anything else is a fragment of a run.
    #[test]
    fn a_recording_that_starts_no_run_is_refused() {
        let text = written(&recorded());
        let mut lines: Vec<&str> = text.lines().collect();
        lines.remove(1);

        let err = read::<V>(lines.join("\n").as_bytes()).expect_err("no start");

        assert!(matches!(err, Error::NotAStart { at: 2, .. }), "{err}");
    }

    /// The stamp, and nothing after it.
    #[test]
    fn a_recording_of_nothing_is_refused() {
        let text = written(&recorded());
        let stamp = text.lines().next().expect("a stamp");

        let err = read::<V>(stamp.as_bytes()).expect_err("no run in it");

        assert!(matches!(err, Error::NoStart), "{err}");
    }

    /// The refusal this format exists for.
    #[test]
    fn a_recording_from_another_build_is_refused() {
        let text = written(&recorded()).replace(
            &<V as sts2_core::ContentVersion>::VERSION
                .content_hash
                .to_string(),
            "1234567",
        );

        let err = read::<V>(text.as_bytes()).expect_err("another build's content hash");

        assert!(matches!(err, Error::WrongBuild { .. }), "{err}");
        assert!(err.to_string().contains("1234567"), "{err}");
    }

    /// A rebalance leaves every id in place, so the content hash alone accepts
    /// it. This is why the stamp carries both hashes.
    #[test]
    fn a_rebalanced_build_is_refused_too() {
        let text = written(&recorded()).replace(
            &<V as sts2_core::ContentVersion>::VERSION
                .assembly_hash
                .to_string(),
            "999",
        );

        assert!(matches!(
            read::<V>(text.as_bytes()),
            Err(Error::WrongBuild { .. })
        ));
    }

    #[test]
    fn an_empty_file_is_not_a_recording() {
        assert!(matches!(read::<V>(&b""[..]), Err(Error::Empty)));
    }

    /// The line number is the reader's, counting the stamp as line 1.
    #[test]
    fn a_step_this_build_does_not_know_names_its_line() {
        let mut text = written(&recorded());
        text.push_str("{\"cmd\":\"teleport\"}\n");

        let err = read::<V>(text.as_bytes()).expect_err("no such command");

        assert!(matches!(err, Error::Step { at: 4, .. }), "{err}");
        assert!(err.to_string().contains("teleport"), "{err}");
    }

    /// Parses fine and is still not a run. The daemon has always filtered these
    /// out when writing. Reading now says so, rather than trusting that it did.
    #[test]
    fn a_command_that_is_not_a_step_is_refused_by_name() {
        for cmd in [r#"{"cmd":"observe"}"#, r#"{"cmd":"quit"}"#] {
            let mut text = written(&recorded());
            text.push_str(cmd);
            text.push('\n');

            let err = read::<V>(text.as_bytes()).expect_err("not a step");
            assert!(matches!(err, Error::NotAStep { at: 4, .. }), "{cmd}: {err}");
        }
    }

    /// A constructed run is the kind most worth keeping, since it is the kind
    /// nobody reaches by playing.
    #[test]
    fn a_constructed_run_records_and_reads_back() {
        let mut r: Recording<V> = Recording::new(Start {
            seed: Seed::new("SP1RE1").expect("a seed"),
            players: vec![sts2_core::Player {
                character: Character::Ironclad,
                wins: 0,
            }],
            ascension: AscensionLevel::None,
            modifiers: vec![],
        });
        r.push(sts2_core::Setup::Fight {
            encounter: <V as sts2_core::ContentVersion>::Encounter::ShrinkerBeetleWeak,
        });

        let text = written(&r);
        assert!(
            text.contains(r#"{"cmd":"fight","encounter":"SHRINKER_BEETLE_WEAK"}"#),
            "{text}"
        );
        assert_eq!(read::<V>(text.as_bytes()).expect("its own build"), r);
    }

    // -----------------------------------------------------------------------
    // Writing as the run is played.
    // -----------------------------------------------------------------------

    /// A path nothing else in this suite is using.
    fn scratch(name: &str) -> PathBuf {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        std::env::temp_dir().join(format!("sts2-{name}-{}-{at}.jsonl", std::process::id()))
    }

    #[test]
    fn a_writer_leaves_a_file_that_reads_back_as_the_run() {
        let path = scratch("writer");
        let mut writer = Writer::<V>::create(&path, recorded().start()).expect("a file");

        for step in recorded().steps() {
            writer.record(step).expect("a step is written");
        }
        assert_eq!(writer.steps(), recorded().len());

        let file = std::io::BufReader::new(File::open(&path).expect("the file"));
        assert_eq!(read::<V>(file).expect("it reads back"), recorded());

        std::fs::remove_file(&path).ok();
    }

    /// The reason the format is one line per step. A session that is killed
    /// leaves a file complete up to its last decision. Read here while the
    /// writer is still open. A flush that only happens on drop is a flush that
    /// does not happen when the process is killed.
    #[test]
    fn every_step_is_on_disk_before_the_next_one_is_asked_for() {
        let path = scratch("flush");
        let mut writer = Writer::<V>::create(&path, recorded().start()).expect("a file");

        let steps = recorded();
        let first = &steps.steps()[0];
        writer.record(first).expect("a step is written");

        let file = std::io::BufReader::new(File::open(&path).expect("the file"));
        let so_far = read::<V>(file).expect("a part-written recording still reads");
        assert_eq!(
            so_far.steps(),
            std::slice::from_ref(first),
            "the step is not on disk yet"
        );

        std::fs::remove_file(&path).ok();
    }

    /// A replay carries on the file it is replaying rather than truncating it.
    /// Rewriting means destroying the run to reproduce it.
    #[test]
    fn resuming_keeps_what_was_there_and_counts_it() {
        let path = scratch("resume");
        let mut writer = Writer::<V>::create(&path, recorded().start()).expect("a file");
        let steps = recorded();
        writer.record(&steps.steps()[0]).expect("a step is written");
        drop(writer);

        let mut carried = Writer::<V>::resume(&path).expect("it resumes");
        assert_eq!(
            carried.steps(),
            1,
            "neither the stamp nor the start is a step, and the step is"
        );
        carried
            .record(&Decision::Proceed.into())
            .expect("a step is written");
        assert_eq!(carried.steps(), 2);

        let file = std::io::BufReader::new(File::open(&path).expect("the file"));
        let whole = read::<V>(file).expect("it reads back");
        assert_eq!(whole.start(), steps.start(), "the run it carried on");
        assert_eq!(
            whole.steps(),
            &[steps.steps()[0].clone(), Decision::Proceed.into()],
            "what was there, and what this session added"
        );

        std::fs::remove_file(&path).ok();
    }

    /// An empty file has no stamp, so it is not a recording to carry on. Said
    /// rather than underflowed.
    #[test]
    fn resuming_nothing_is_refused() {
        let path = scratch("empty");
        File::create(&path).expect("a file");

        let err = Writer::<V>::resume(&path).expect_err("there is nothing to carry on");
        assert!(err.to_string().contains("starts no run"), "{err}");

        std::fs::remove_file(&path).ok();
    }

    /// Two players, and a step each.
    fn together() -> Recording<V> {
        let player = |character| sts2_core::Player { character, wins: 0 };
        let mut r = Recording::new(Start {
            seed: Seed::new("C000000001").expect("a seed"),
            players: vec![player(Character::Ironclad), player(Character::Silent)],
            ascension: AscensionLevel::None,
            modifiers: vec![],
        });
        r.push_by(SeatIndex(1), Decision::EndTurn);
        r.push_by(
            SeatIndex(0),
            Decision::Travel {
                to: Coord::new(1, 1),
            },
        );
        r
    }

    #[test]
    fn a_run_of_several_players_round_trips() {
        let text = written(&together());
        assert_eq!(
            read::<V>(text.as_bytes()).expect("its own build"),
            together()
        );
    }

    /// The start names each player, and each step its seat, first.
    #[test]
    fn a_run_of_several_players_names_each_and_their_seats() {
        let text = written(&together());
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(
            lines[1],
            r#"{"cmd":"start","seed":"C000000001","ascension":"None","players":[{"character":"IRONCLAD"},{"character":"SILENT"}]}"#
        );
        assert_eq!(lines[2], r#"{"seat":1,"cmd":"end_turn"}"#);
        assert_eq!(lines[3], r#"{"seat":0,"cmd":"travel","col":1,"row":1}"#);
    }

    /// A solo run's file is what it was before runs had players.
    #[test]
    fn a_solo_run_names_no_seat() {
        let text = written(&recorded());
        assert!(text.lines().all(|line| !line.contains("seat")), "{text}");
        assert!(
            text.lines()
                .nth(1)
                .is_some_and(|l| l.contains(r#""character":"IRONCLAD""#))
        );
    }

    #[test]
    fn a_seat_where_the_run_has_none_is_refused() {
        let text =
            written(&recorded()).replace(r#"{"cmd":"travel""#, r#"{"seat":0,"cmd":"travel""#);
        let err = read::<V>(text.as_bytes()).expect_err("a solo step names no seat");
        assert!(
            matches!(
                err,
                Error::Seat {
                    at: 3,
                    players: 1,
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn a_step_of_several_players_with_no_seat_is_refused() {
        let text = written(&together()).replace(r#"{"seat":1,"#, "{");
        let err = read::<V>(text.as_bytes()).expect_err("whose step it is is missing");
        assert!(
            matches!(
                err,
                Error::Seat {
                    at: 3,
                    players: 2,
                    ..
                }
            ),
            "{err}"
        );

        let text = written(&together()).replace(r#"{"seat":1,"#, r#"{"seat":2,"#);
        let err = read::<V>(text.as_bytes()).expect_err("a seat the run lacks");
        assert!(
            matches!(
                err,
                Error::Seat {
                    at: 3,
                    players: 2,
                    ..
                }
            ),
            "{err}"
        );
    }

    /// A solo run spells its character as the game's `start` does. A start
    /// that names one player in a list, or players both ways, is not a start.
    #[test]
    fn a_start_is_spelled_one_way() {
        let stamp = written(&recorded())
            .lines()
            .next()
            .expect("a stamp")
            .to_owned();
        for start in [
            r#"{"cmd":"start","seed":"ABC","ascension":"None","players":[{"character":"SILENT"}]}"#,
            r#"{"cmd":"start","seed":"ABC","ascension":"None","character":"SILENT","players":[{"character":"SILENT"},{"character":"DEFECT"}]}"#,
            r#"{"cmd":"start","seed":"ABC","ascension":"None"}"#,
        ] {
            let err = read::<V>(format!("{stamp}\n{start}\n").as_bytes()).expect_err(start);
            assert!(matches!(err, Error::NotAStart { at: 2, .. }), "{err}");
        }
    }

    /// A co-op file carried on goes on naming seats.
    #[test]
    fn a_run_of_several_players_carries_on() {
        let path = scratch("together");
        let mut out = File::create(&path).expect("a file");
        write(&together(), &mut out).expect("written");
        drop(out);

        let mut carried = Writer::<V>::resume(&path).expect("it carries on");
        assert_eq!(carried.steps(), 2);
        carried
            .record_by(SeatIndex(1), &Decision::Proceed.into())
            .expect("a step is written");

        let file = std::io::BufReader::new(File::open(&path).expect("the file"));
        let whole = read::<V>(file).expect("it reads back");
        let mut want = together();
        want.push_by(SeatIndex(1), Decision::Proceed);
        assert_eq!(whole, want);

        std::fs::remove_file(&path).ok();
    }
}
