//! The layers a runner wraps a table in, each doing one job.
//!
//! Each is a [`RunTable`] around a `RunTable`. The order a runner nests them
//! in matters, and `docs/RUNNERS.md` says why:
//!
//! ```text
//! Carried< MovesLog< Hunted< Recorded< LogGuard< the game's table >>>>>
//! ```

use std::path::PathBuf;
use std::sync::Mutex;

use sts2_core::{
    ContentVersion, Decision, End, Gift, RunTable, SeatIndex, Settled, Setup, Snapshot, Start,
    Verdict,
};
use sts2_sim::SimTable;

use crate::Fault;
use crate::recorded::{Ending, Recorded as File, Run, Stamp, named, parted, position};

/// Passes every method a layer does not change on to the table it wraps,
/// as `self.0`.
macro_rules! pass_on {
    () => {
        fn seats(&self) -> usize {
            self.0.seats()
        }

        fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error> {
            Ok(self.0.seen(seat)?)
        }

        fn moves(
            &mut self,
            seat: SeatIndex,
            moves: &mut Vec<Decision<V>>,
        ) -> Result<(), Self::Error> {
            Ok(self.0.moves(seat, moves)?)
        }

        fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error> {
            Ok(self.0.snapshot(seat)?)
        }

        fn logged(&mut self) -> Vec<(SeatIndex, String)> {
            self.0.logged()
        }
    };
}

/// A table whose every player carries [`crate::carried::carried`] from
/// before the first move. That is 25 Gorgets and a Mercury Hourglass, so
/// play at random reaches content instead of dying early.
///
/// A solo run grants its own player. A run of several players has each
/// seat's machine make every grant, for every player, in seat order, as each
/// game does.
pub struct Carried<T>(pub T);

impl<V, T> RunTable<V> for Carried<T>
where
    V: sts2_content::Build,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    type Error = Fault;
    type Host = T::Host;
    type Seen<'a>
        = T::Seen<'a>
    where
        Self: 'a;

    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let mut table = T::begin(host, start)?;
        let seats = table.seats();
        let grants: Vec<Setup<V>> = match seats {
            1 => crate::carried::carried::<V>()
                .into_iter()
                .flat_map(|(relic, count)| {
                    std::iter::repeat_n(Setup::Grant(Gift::Relic(relic)), count)
                })
                .collect(),
            seats => crate::carried::carried_by_every_seat::<V>(seats),
        };
        for machine in SeatIndex::all(seats) {
            for grant in &grants {
                table.construct(machine, grant)?.map_err(|refusal| Fault {
                    end: End::Refused,
                    seat: Some(machine),
                    detail: format!("seat {}'s machine refused {grant:?}: {refusal}", machine.0),
                })?;
            }
        }
        Ok(Self(table))
    }

    pass_on!();

    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error> {
        Ok(self.0.play(seat, decision)?)
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error> {
        Ok(self.0.construct(seat, setup)?)
    }

    fn finish(&mut self, verdict: Verdict) -> Verdict {
        self.0.finish(verdict)
    }
}

/// A table that stops the run on an error a game logged, after the move or
/// setup it logged on.
pub struct LogGuard<T>(pub T);

impl<T> LogGuard<T> {
    /// What the games logged, as the fault that stops the run.
    fn checked<V: ContentVersion>(&mut self) -> Result<(), Fault>
    where
        T: RunTable<V>,
    {
        let logged = self.0.logged();
        let Some(&(seat, _)) = logged.first() else {
            return Ok(());
        };
        let errors: Vec<String> = logged
            .into_iter()
            .map(|(seat, error)| format!("seat {}: {error}", seat.0))
            .collect();
        Err(Fault {
            end: End::GameError,
            seat: Some(seat),
            detail: errors.join("; "),
        })
    }
}

impl<V, T> RunTable<V> for LogGuard<T>
where
    V: ContentVersion,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    type Error = Fault;
    type Host = T::Host;
    type Seen<'a>
        = T::Seen<'a>
    where
        Self: 'a;

    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let mut table = Self(T::begin(host, start)?);
        table.checked()?;
        Ok(table)
    }

    pass_on!();

    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error> {
        let settled = self.0.play(seat, decision)?;
        self.checked()?;
        Ok(settled)
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error> {
        let settled = self.0.construct(seat, setup)?;
        self.checked()?;
        Ok(settled)
    }

    fn finish(&mut self, verdict: Verdict) -> Verdict {
        self.0.finish(verdict)
    }
}

/// A table that prints each move as it is played, as
/// `<step> seat <seat>: <move>`, for a log beside the run, when its host
/// says to. The count is the moves played before.
pub struct MovesLog<T> {
    table: T,
    printing: bool,
    played: usize,
}

impl<V, T> RunTable<V> for MovesLog<T>
where
    V: ContentVersion,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    type Error = Fault;
    /// Whether to print, and the table's own host.
    type Host = (bool, T::Host);
    type Seen<'a>
        = T::Seen<'a>
    where
        Self: 'a;

    fn begin((printing, host): Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        Ok(Self {
            table: T::begin(host, start)?,
            printing,
            played: 0,
        })
    }

    fn seats(&self) -> usize {
        self.table.seats()
    }

    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error> {
        Ok(self.table.seen(seat)?)
    }

    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Self::Error> {
        Ok(self.table.moves(seat, moves)?)
    }

    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error> {
        Ok(self.table.snapshot(seat)?)
    }

    fn logged(&mut self) -> Vec<(SeatIndex, String)> {
        self.table.logged()
    }

    /// Printed first, so the log names the move a failure comes after.
    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error> {
        if self.printing {
            println!("{} seat {}: {decision:?}", self.played, seat.0);
        }
        self.played += 1;
        Ok(self.table.play(seat, decision)?)
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error> {
        Ok(self.table.construct(seat, setup)?)
    }

    fn finish(&mut self, verdict: Verdict) -> Verdict {
        self.table.finish(verdict)
    }
}

/// A table played beside the simulator, which stops the run where the two
/// part.
///
/// Before each move it compares the acting seat on the game and on its
/// simulated machine. Where the two differ it writes both, in full, as
/// `parted-sim.txt` and `parted-game.txt` in its directory.
///
/// A run that ends is compared on every seat, and agreed or parted there. A
/// host that names no directory hunts nothing.
pub struct Hunted<V: sts2_content::Build, T> {
    game: T,
    sim: Option<SimTable<V>>,
    /// Where a parting writes the two snapshots it compared.
    dir: PathBuf,
    /// The move the last comparison was before, which a parting comes after.
    after: String,
}

impl<V: sts2_content::Build, T: RunTable<V>> Hunted<V, T>
where
    Fault: From<T::Error>,
{
    /// The fields where seat `seat`'s game and simulator part, or nothing
    /// where they agree. A parting writes both snapshots.
    fn compare(&mut self, seat: SeatIndex) -> Result<Option<(Snapshot<V>, String)>, Fault> {
        let Some(sim) = self.sim.as_mut() else {
            return Ok(None);
        };
        let simulated = sim.snapshot(seat)?;
        let seen = self.game.snapshot(seat)?;
        if position(&simulated) == position(&seen) {
            return Ok(None);
        }
        for (name, snapshot) in [("sim", &simulated), ("game", &seen)] {
            let file = self.dir.join(format!("parted-{name}.txt"));
            if let Err(e) = std::fs::write(&file, format!("{snapshot:#?}\n")) {
                eprintln!("writing {}: {e}", file.display());
            }
        }
        let fields = parted(&simulated, &seen);
        Ok(Some((seen, fields)))
    }
}

impl<V, T> RunTable<V> for Hunted<V, T>
where
    V: sts2_content::Build,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    type Error = Fault;
    /// The directory a parting writes to, where the run is hunted, and the
    /// game's own host.
    type Host = (Option<PathBuf>, T::Host);
    type Seen<'a>
        = T::Seen<'a>
    where
        Self: 'a;

    fn begin((dir, host): Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let game = T::begin(host, start)?;
        let sim = match dir {
            Some(_) => Some(SimTable::begin((), start)?),
            None => None,
        };
        Ok(Self {
            game,
            sim,
            dir: dir.unwrap_or_default(),
            after: "the start".into(),
        })
    }

    fn seats(&self) -> usize {
        self.game.seats()
    }

    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error> {
        Ok(self.game.seen(seat)?)
    }

    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Self::Error> {
        Ok(self.game.moves(seat, moves)?)
    }

    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error> {
        Ok(self.game.snapshot(seat)?)
    }

    fn logged(&mut self) -> Vec<(SeatIndex, String)> {
        self.game.logged()
    }

    /// Compared first, then played on the game, then on every simulated
    /// machine. A simulator that refuses what the game took stops the run.
    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error> {
        if let Some((_, fields)) = self.compare(seat)? {
            return Err(Fault {
                end: End::Parted,
                seat: Some(seat),
                detail: format!("after {} in {fields}", self.after),
            });
        }
        let seen = self.game.snapshot(seat)?;
        self.after = named(&seen, decision);
        let settled = self.game.play(seat, decision)?;
        if settled.is_err() {
            return Ok(settled);
        }
        let Some(sim) = self.sim.as_mut() else {
            return Ok(settled);
        };
        if let Err(refusal) = sim.play(seat, decision)? {
            return Err(Fault {
                end: End::Refused,
                seat: Some(seat),
                detail: format!("the simulator refused {decision:?}: {refusal}"),
            });
        }
        Ok(settled)
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error> {
        let settled = self.game.construct(seat, setup)?;
        let Some(sim) = self.sim.as_mut() else {
            return Ok(settled);
        };
        if let Err(refusal) = sim.construct(seat, setup)? {
            return Err(Fault {
                end: End::Refused,
                seat: Some(seat),
                detail: format!("the simulator refused {setup:?}: {refusal}"),
            });
        }
        Ok(settled)
    }

    /// A run that ended is compared on every seat, and agreed or parted
    /// there. A run stopped for another reason is as it was.
    fn finish(&mut self, verdict: Verdict) -> Verdict {
        let verdict = match verdict.end {
            End::Won | End::Lost | End::Capped | End::Stalled if self.sim.is_some() => {
                let mut parting = None;
                for seat in SeatIndex::all(self.game.seats()) {
                    match self.compare(seat) {
                        Ok(None) => {}
                        Ok(Some((_, fields))) => {
                            parting = Some((seat, format!("at the end in {fields}")));
                            break;
                        }
                        Err(fault) => {
                            parting = Some((seat, fault.detail));
                            break;
                        }
                    }
                }
                match parting {
                    Some((seat, detail)) => Verdict {
                        end: End::Parted,
                        seat: Some(seat),
                        detail,
                        ..verdict
                    },
                    None => Verdict {
                        end: End::Agreed,
                        detail: format!("{:?}", verdict.end).to_lowercase(),
                        ..verdict
                    },
                }
            }
            _ => verdict,
        };
        self.game.finish(verdict)
    }
}

/// The run a [`Recorded`] layer writes, where the panic hook can reach it.
static RECORDING: Mutex<Option<(PathBuf, File)>> = Mutex::new(None);

/// A table that writes the run as a recorded run.
///
/// For each move it keeps where the acting seat stood before it, the move's
/// index in that seat's list, and the seat. As the run ends it keeps where
/// each seat stands, and how the run ended. It writes every run, whatever
/// the verdict: what to keep of them is its caller's.
pub struct Recorded<V: ContentVersion, T> {
    table: T,
    /// Whether a file is written. A host that names none records nothing.
    on: bool,
    /// The last list of moves each seat was given.
    listed: Vec<Vec<Decision<V>>>,
}

impl<V: ContentVersion, T> Recorded<V, T> {
    /// The run being recorded, to write on.
    fn run(&self, write: impl FnOnce(&mut Run)) {
        let mut recording = RECORDING.lock().expect("no panic holds the recording");
        let (_, file) = recording.as_mut().expect("a run is being recorded");
        write(&mut file.runs[0]);
    }
}

impl<V, T> RunTable<V> for Recorded<V, T>
where
    V: sts2_content::Build,
    T: RunTable<V>,
    Fault: From<T::Error>,
{
    type Error = Fault;
    /// The file the run is written to, if any, and the table's own host.
    type Host = (Option<PathBuf>, T::Host);
    type Seen<'a>
        = T::Seen<'a>
    where
        Self: 'a;

    fn begin((out, host): Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let table = T::begin(host, start)?;
        let Some(out) = out else {
            return Ok(Self {
                table,
                on: false,
                listed: Vec::new(),
            });
        };
        let stamp = sts2_headless::Stamp::of(&V::VERSION);
        let file = File {
            stamp: Stamp {
                assembly: stamp.assembly_hash,
                content: i64::from(stamp.content_hash),
            },
            runs: vec![Run::of(start, Ending::Parted)],
        };
        *RECORDING.lock().expect("no panic holds the recording") = Some((out, file));
        Ok(Self {
            listed: vec![Vec::new(); table.seats()],
            on: true,
            table,
        })
    }

    fn seats(&self) -> usize {
        self.table.seats()
    }

    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error> {
        Ok(self.table.seen(seat)?)
    }

    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Self::Error> {
        self.table.moves(seat, moves)?;
        if self.on {
            self.listed[seat.at()].clone_from(moves);
        }
        Ok(())
    }

    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error> {
        Ok(self.table.snapshot(seat)?)
    }

    fn logged(&mut self) -> Vec<(SeatIndex, String)> {
        self.table.logged()
    }

    /// Kept before it is played, so a panic as it plays leaves a run that
    /// reaches the move it panicked on. A move the table refused, or one a
    /// fault stopped before the game took it, is taken off again.
    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error> {
        if !self.on {
            return Ok(self.table.play(seat, decision)?);
        }
        let at = self.listed[seat.at()]
            .iter()
            .position(|listed| listed == decision)
            .unwrap_or_else(|| {
                unimplemented!(
                    "recording {decision:?}, which seat {} was not given",
                    seat.0
                )
            });
        let digest = position(&self.table.snapshot(seat)?);
        self.run(|run| {
            run.digests.push(digest);
            run.moves
                .push(u16::try_from(at).expect("a list of moves fits a u16"));
            run.seats.push(seat);
        });
        let played = self.table.play(seat, decision).map_err(Fault::from);
        let taken = match &played {
            Ok(Ok(_)) => true,
            Ok(Err(_)) => false,
            Err(fault) => fault.end == End::GameError,
        };
        if !taken {
            self.run(|run| {
                run.digests.pop();
                run.moves.pop();
                run.seats.pop();
            });
        }
        played
    }

    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error> {
        Ok(self.table.construct(seat, setup)?)
    }

    /// Where each seat stands as the run ends, then the file. A run that
    /// ended its own way keeps how. Every other is kept as parted, where
    /// the game stood.
    fn finish(&mut self, verdict: Verdict) -> Verdict {
        if !self.on {
            return self.table.finish(verdict);
        }
        let mut ends = Vec::new();
        for seat in SeatIndex::all(self.table.seats()) {
            match self.table.snapshot(seat) {
                Ok(snapshot) => ends.push(position(&snapshot)),
                Err(fault) => eprintln!("seat {} as the run ends: {}", seat.0, Fault::from(fault)),
            }
        }
        let ending = match verdict.end {
            End::Won => Ending::Won,
            End::Lost => Ending::Lost,
            End::Capped => Ending::Capped,
            End::Agreed => match verdict.detail.as_str() {
                "won" => Ending::Won,
                "lost" => Ending::Lost,
                "capped" => Ending::Capped,
                _ => Ending::Parted,
            },
            _ => Ending::Parted,
        };
        let recording = RECORDING
            .lock()
            .expect("no panic holds the recording")
            .take();
        if let Some((out, mut file)) = recording {
            let run = &mut file.runs[0];
            run.ending = ending;
            // A run holds where each seat ended. One a seat gave no end for is
            // not kept, and the verdict says why the run stopped.
            if ends.len() != run.players.len() {
                eprintln!("not keeping the run: a seat gave no position as it ended");
                return self.table.finish(verdict);
            }
            run.digests.extend(ends);
            if let Err(e) = std::fs::write(&out, file.encode()) {
                eprintln!("writing {}: {e}", out.display());
            }
        }
        self.table.finish(verdict)
    }
}
