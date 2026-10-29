//! Runs the game played at random, as bytes.
//!
//! The game plays each run itself, picking from its own legal moves by
//! [`crate::random::Pick`]. A run keeps the index of each move in that list.
//! It keeps a digest of where the run stood before each move, and after the
//! last. The simulator replays one by playing the same index from its own
//! list, for as long as its digests agree. The digest covers the moves, so
//! the lists are the same list wherever it does.
//!
//! One file for every run, because a run is a thousand small steps and fifteen
//! hundred of them are a tree nobody reads. A digest is four bytes, the low
//! half folded into the high. Four find the first step two runs part at as
//! surely as eight do here.

use crate::ids::spelled;
pub use crate::ids::{read_modifier, spell_modifier};
use sts2_core::{Decision, Digest, Digested as _, Room, SeatIndex, Snapshot, Start};

/// What the file starts with, so a wrong one is refused rather than read.
const MAGIC: &[u8; 8] = b"STS2REC\x00";

/// The layout below. A change here is a re-record, not a migration.
const VERSION: u16 = 3;

/// The game a set of answers came from.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub assembly: i64,
    pub content: i64,
}

/// Every recorded run, and the build that played them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub stamp: Stamp,
    pub runs: Vec<Run>,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Won,
    Lost,
    /// Stopped after as many steps as a run is allowed.
    Capped,
    /// Stopped where the simulator, played beside the game, first parted
    /// from it. The last position is the game's there.
    Parted,
}

/// One run the game played.
///
/// A run of several players is the same. Each move also names the seat that
/// played it, and the digest before it is where that seat stood. At the end
/// there is one digest for every seat. A solo run is the run of one player,
/// and its file spells no seats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub seed: String,
    /// Each player, in seat order. Seat 0 hosts.
    pub players: Vec<Player>,
    /// The ascension level, as the game names it.
    pub ascension: String,
    /// The modifiers the run was started with, each as [`spell_modifier`]
    /// writes it.
    pub modifiers: Vec<String>,
    pub ending: Ending,
    /// Where the seat that played each move stood before it. Then where each
    /// seat stood after the last, so there are as many more than
    /// [`Run::moves`] as there are players.
    pub digests: Vec<u32>,
    /// Each move, as its index in the list the game offered its seat.
    pub moves: Vec<u16>,
    /// The seat that played each move. As long as [`Run::moves`].
    pub seats: Vec<SeatIndex>,
}

/// One player of a recorded run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    /// The character, as the game spells its id.
    pub character: String,
    /// See [`sts2_core::Player::wins`].
    pub wins: u32,
}

/// Where a run stands, as a recorded run keeps it. The rules digest of the
/// observation, moves included, with the question the run is parked on.
pub fn position<V: sts2_core::ContentVersion>(snapshot: &Snapshot<V>) -> u32 {
    let rules = snapshot.observation.rules_digest().to_u64();
    let question = Digest::of_question(snapshot.asking.as_ref()).to_u64();
    let both = rules ^ question.rotate_left(29);
    (both ^ (both >> 32)) as u32
}

/// The sections of the observation where the simulator and the game differ,
/// and the question, if they ask different ones.
pub fn parted<V: sts2_core::ContentVersion>(
    simulated: &sts2_core::Snapshot<V>,
    seen: &sts2_core::Snapshot<V>,
) -> String {
    let mut parts: Vec<String> = simulated
        .observation
        .sections()
        .iter()
        .zip(seen.observation.sections().iter())
        .filter(|(a, b)| a != b)
        .map(|(a, _)| format!("{:?}", a.0))
        .collect();
    if sts2_core::Digest::of_question(simulated.asking.as_ref())
        != sts2_core::Digest::of_question(seen.asking.as_ref())
    {
        parts.push("the question".into());
    }
    parts.join(", ")
}

/// How far the simulator followed a recorded run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Followed {
    /// Every position agreed, to the run's end.
    Whole,
    /// The positions agreed up to `step` and not at it. Step `n` is the
    /// position before the run's `n`th move. `after` names the move that led
    /// there, as [`named`] does.
    PartedAt { step: usize, after: String },
}

impl Run {
    /// A run begun with `start` that has no positions yet, ended as `ending`
    /// says until it ends.
    pub fn of<V: sts2_core::ContentVersion>(start: &Start<V>, ending: Ending) -> Run {
        Run {
            seed: start.seed.as_str().to_owned(),
            players: start
                .players
                .iter()
                .map(|player| Player {
                    character: spelled(&player.character),
                    wins: player.wins,
                })
                .collect(),
            ascension: spelled(&start.ascension),
            modifiers: start.modifiers.iter().map(spell_modifier).collect(),
            ending,
            digests: Vec::new(),
            moves: Vec::new(),
            seats: Vec::new(),
        }
    }

    /// What the run was begun with.
    ///
    /// # Panics
    ///
    /// On an id this build does not have, which a file this build recorded
    /// never holds.
    pub fn start<V: sts2_core::ContentVersion>(&self) -> Start<V> {
        Start {
            seed: sts2_core::Seed::new(&self.seed).expect("a recorded seed is a seed"),
            players: self
                .players
                .iter()
                .map(|player| sts2_core::Player {
                    character: from_id(&player.character),
                    wins: player.wins,
                })
                .collect(),
            ascension: from_id(&self.ascension),
            modifiers: self
                .modifiers
                .iter()
                .map(|text| read_modifier(text).unwrap_or_else(|e| panic!("{text:?}: {e}")))
                .collect(),
        }
    }

    /// The players' characters, joined by `+`: a solo run's is its one.
    pub fn characters(&self) -> String {
        let ids: Vec<&str> = self.players.iter().map(|p| p.character.as_str()).collect();
        ids.join("+")
    }
}

/// A value by the id the game spells it with.
fn from_id<T: serde::de::DeserializeOwned>(id: &str) -> T {
    serde_json::from_value(serde_json::Value::String(id.to_owned()))
        .unwrap_or_else(|e| panic!("{id:?}: {e}"))
}

/// A move by what it plays rather than where it sits, so the splits after
/// one move can be counted together across runs. The card, the potion, the
/// fight a turn ends in, the option an event or a rest site offers, or the
/// question answered.
pub fn named<V: sts2_core::ContentVersion>(seen: &Snapshot<V>, decision: &Decision<V>) -> String {
    let observation = &seen.observation;
    match decision {
        Decision::PlayCard { index, .. } => match observation
            .combat
            .as_ref()
            .and_then(|combat| combat.hand.get(usize::from(index.get())))
        {
            Some(card) => format!("play {:?}", card.card),
            None => "play".into(),
        },
        Decision::UsePotion { index, .. } => {
            match observation.potions.get(usize::from(index.get())) {
                Some(held) => format!("drink {:?}", held.potion),
                None => "drink".into(),
            }
        }
        Decision::EndTurn => match observation.combat.as_ref().and_then(|c| c.encounter) {
            Some(encounter) => format!("end turn in {encounter:?}"),
            None => "end turn".into(),
        },
        Decision::ChooseOption { index, .. } => match &observation.room {
            Some(Room::Event { event, options, .. }) => {
                match options.iter().find(|option| option.index == *index) {
                    Some(option) => format!("{event:?} {:?}", option.key.option),
                    None => format!("{event:?}"),
                }
            }
            Some(Room::RestSite { options }) => {
                match options.iter().find(|option| option.index == *index) {
                    Some(option) => format!("rest {:?}", option.id),
                    None => "rest".into(),
                }
            }
            Some(Room::Treasure { .. }) => "treasure".into(),
            _ => "option".into(),
        },
        Decision::Answer(_) => match &seen.asking {
            Some(question) => format!("answer {}", question.kind()),
            None => "answer".into(),
        },
        Decision::Buy { .. } => "buy".into(),
        Decision::Travel { .. } => "travel".into(),
        Decision::EnterRoom { room } => format!("enter {room:?}"),
        Decision::Proceed => "proceed".into(),
        Decision::DiscardPotion { index } => {
            match observation.potions.get(usize::from(index.get())) {
                Some(held) => format!("discard {:?}", held.potion),
                None => "discard".into(),
            }
        }
    }
}

/// Replays `run` at `table`, begun on its start, and says how far the two
/// agree. Each move is its recorded index into its own seat's list.
///
/// Each seat's position is compared with the run's digest before it moves,
/// and every seat's after the last move. `agreed` is told each time one
/// more agrees. Without `checking` nothing is compared, for a table such as
/// a hunt that compares itself.
///
/// # Panics
///
/// Where the table does, and on a move the table refuses.
pub fn follow<V, T>(
    run: &Run,
    table: &mut T,
    checking: bool,
    mut agreed: impl FnMut(usize),
) -> Result<Followed, crate::Fault>
where
    V: sts2_core::ContentVersion,
    T: sts2_core::RunTable<V>,
    crate::Fault: From<T::Error>,
{
    let mut moves = Vec::new();
    let mut after = String::from("the start");
    for (step, (&at, &seat)) in run.moves.iter().zip(&run.seats).enumerate() {
        let seen = table.snapshot(seat)?;
        if checking {
            if position(&seen) != run.digests[step] {
                return Ok(Followed::PartedAt { step, after });
            }
            agreed(step + 1);
        }
        table.moves(seat, &mut moves)?;
        let decision = moves[usize::from(at)];
        after = named(&seen, &decision);
        if let Err(refusal) = table.play(seat, &decision)? {
            panic!("{decision:?} refused at step {step}: {refusal}");
        }
    }
    if checking {
        let ends = &run.digests[run.moves.len()..];
        for (at, (seat, &want)) in sts2_core::SeatIndex::all(ends.len()).zip(ends).enumerate() {
            if position(&table.snapshot(seat)?) != want {
                let step = run.moves.len() + at;
                return Ok(Followed::PartedAt { step, after });
            }
            agreed(run.moves.len() + at + 1);
        }
    }
    Ok(Followed::Whole)
}

impl Recorded {
    /// The whole file.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.stamp.assembly.to_le_bytes());
        out.extend_from_slice(&self.stamp.content.to_le_bytes());
        let count = u32::try_from(self.runs.len()).expect("the runs fit a u32");
        out.extend_from_slice(&count.to_le_bytes());

        for run in &self.runs {
            let text = |out: &mut Vec<u8>, text: &str| {
                out.push(u8::try_from(text.len()).expect("an id is short"));
                out.extend_from_slice(text.as_bytes());
            };
            for id in [&run.seed, &run.ascension] {
                text(&mut out, id);
            }
            out.push(u8::try_from(run.modifiers.len()).expect("a run has few modifiers"));
            for id in &run.modifiers {
                text(&mut out, id);
            }
            assert!(!run.players.is_empty(), "a run has a player");
            out.push(u8::try_from(run.players.len()).expect("at most 255 players"));
            for player in &run.players {
                text(&mut out, &player.character);
                out.extend_from_slice(&player.wins.to_le_bytes());
            }
            out.push(match run.ending {
                Ending::Won => 0,
                Ending::Lost => 1,
                Ending::Capped => 2,
                Ending::Parted => 3,
            });
            assert_eq!(
                run.digests.len(),
                run.moves.len() + run.players.len(),
                "a digest before each move, and one for each seat after the last"
            );
            assert_eq!(run.seats.len(), run.moves.len(), "a seat for each move");
            let solo = run.players.len() == 1;
            assert!(
                run.seats.iter().all(|seat| seat.at() < run.players.len()),
                "a move by a seat the run has"
            );
            let steps = u32::try_from(run.moves.len()).expect("a run's moves fit a u32");
            out.extend_from_slice(&steps.to_le_bytes());
            for digest in &run.digests {
                out.extend_from_slice(&digest.to_le_bytes());
            }
            // Most lists hold fewer than 128 moves, so most take one byte. A
            // solo run's seat is always 0, so its file spells none.
            for (&at, seat) in run.moves.iter().zip(&run.seats) {
                if !solo {
                    out.push(seat.0);
                }
                let mut at = at;
                while at >= 0x80 {
                    out.push((at as u8 & 0x7f) | 0x80);
                    at >>= 7;
                }
                out.push(at as u8);
            }
        }
        out
    }

    /// The whole file, back.
    pub fn decode(bytes: &[u8]) -> Result<Recorded, String> {
        let mut read = Reader { bytes, at: 0 };
        if read.take(8)? != MAGIC {
            return Err("not a file of recorded runs".into());
        }
        let version = read.u16()?;
        if version != VERSION {
            return Err(format!("version {version}, and this reads {VERSION}"));
        }
        let stamp = Stamp {
            assembly: read.i64()?,
            content: read.i64()?,
        };
        let count = read.u32()?;

        let mut runs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let seed = read.text()?;
            let ascension = read.text()?;
            let modifiers = (0..read.u8()?)
                .map(|_| read.text())
                .collect::<Result<Vec<_>, _>>()?;
            let players = (0..read.u8()?)
                .map(|_| {
                    Ok(Player {
                        character: read.text()?,
                        wins: read.u32()?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            if players.is_empty() {
                return Err(format!("run {} has no players", runs.len()));
            }
            let ending = match read.u8()? {
                0 => Ending::Won,
                1 => Ending::Lost,
                2 => Ending::Capped,
                3 => Ending::Parted,
                other => return Err(format!("{other} is no way for a run to end")),
            };
            let steps = read.u32()? as usize;
            let digests = (0..steps + players.len())
                .map(|_| read.u32())
                .collect::<Result<Vec<_>, _>>()?;
            let mut moves = Vec::with_capacity(steps);
            let mut seats = Vec::with_capacity(steps);
            for _ in 0..steps {
                let seat = match players.len() {
                    1 => SeatIndex(0),
                    count => match read.u8()? {
                        seat if usize::from(seat) < count => SeatIndex(seat),
                        seat => return Err(format!("seat {seat} in a run of {count} players")),
                    },
                };
                seats.push(seat);
                moves.push(read.varint()?);
            }
            runs.push(Run {
                seed,
                players,
                ascension,
                modifiers,
                ending,
                digests,
                moves,
                seats,
            });
        }
        if read.at != bytes.len() {
            return Err(format!("{} bytes past the last run", bytes.len() - read.at));
        }
        Ok(Recorded { stamp, runs })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("a length that overflows")?;
        let taken = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| format!("wanted {n} bytes at {} and the file ends", self.at))?;
        self.at = end;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("two")))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("four")))
    }

    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().expect("eight")))
    }

    fn text(&mut self) -> Result<String, String> {
        let len = usize::from(self.u8()?);
        String::from_utf8(self.take(len)?.to_vec()).map_err(|e| format!("an id is not text: {e}"))
    }

    fn varint(&mut self) -> Result<u16, String> {
        let mut value: u32 = 0;
        for shift in [0, 7, 14] {
            let byte = self.u8()?;
            value |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return u16::try_from(value).map_err(|_| format!("move index {value} is too far"));
            }
        }
        Err("a move index longer than three bytes".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(seed: &str, moves: Vec<u16>) -> Run {
        Run {
            seed: seed.into(),
            players: vec![Player {
                character: "SILENT".into(),
                wins: 4,
            }],
            ascension: "None".into(),
            modifiers: Vec::new(),
            ending: Ending::Lost,
            digests: (0..=moves.len() as u32)
                .map(|n| n.wrapping_mul(0x9E37_79B9))
                .collect(),
            seats: vec![SeatIndex(0); moves.len()],
            moves,
        }
    }

    /// A run of two: the seats take the moves in the order given.
    fn coop(seed: &str, moves: Vec<(u8, u16)>) -> Run {
        let player = |character: &str| Player {
            character: character.into(),
            wins: 0,
        };
        Run {
            players: vec![player("IRONCLAD"), player("SILENT")],
            digests: (0..moves.len() as u32 + 2)
                .map(|n| n.wrapping_mul(0x85EB_CA6B))
                .collect(),
            seats: moves.iter().map(|&(seat, _)| SeatIndex(seat)).collect(),
            moves: moves.iter().map(|&(_, at)| at).collect(),
            ..run(seed, Vec::new())
        }
    }

    #[test]
    fn a_file_reads_back_as_it_was_written() {
        let recorded = Recorded {
            stamp: Stamp {
                assembly: 1692500715,
                content: -3,
            },
            runs: vec![
                run("C000000001", vec![0, 3, 127, 128, 300, 16383, 65535]),
                run("C000000002", vec![]),
                Run {
                    modifiers: vec!["FLIGHT".into(), "CHARACTER_CARDS:DEFECT".into()],
                    ..run("C000000003", vec![1])
                },
                coop("C000000004", vec![(0, 3), (1, 200), (1, 0), (0, 65535)]),
                coop("C000000005", vec![]),
            ],
        };

        assert_eq!(Recorded::decode(&recorded.encode()), Ok(recorded));
    }

    #[test]
    fn a_short_index_takes_one_byte() {
        let one = Recorded {
            runs: vec![run("C", vec![5])],
            ..Recorded::default()
        };
        let two = Recorded {
            runs: vec![run("C", vec![5, 6])],
            ..Recorded::default()
        };

        // Four bytes of digest and one of index per step.
        assert_eq!(two.encode().len() - one.encode().len(), 5);
    }

    /// A run of several players spells the seat of each move, and a solo
    /// run does not.
    #[test]
    fn a_seat_takes_one_byte_in_a_run_of_several() {
        let one = Recorded {
            runs: vec![coop("C", vec![(1, 5)])],
            ..Recorded::default()
        };
        let two = Recorded {
            runs: vec![coop("C", vec![(1, 5), (0, 6)])],
            ..Recorded::default()
        };

        assert_eq!(two.encode().len() - one.encode().len(), 6);
    }

    #[test]
    fn a_move_by_a_seat_the_run_lacks_is_refused() {
        let mut bytes = Recorded {
            runs: vec![coop("C", vec![(1, 5)])],
            ..Recorded::default()
        }
        .encode();
        // The seat byte sits just before the last move's one-byte index.
        let seat = bytes.len() - 2;
        assert_eq!(bytes[seat], 1);
        bytes[seat] = 2;

        assert!(Recorded::decode(&bytes).is_err());
    }

    #[test]
    fn a_cut_file_is_refused() {
        let bytes = Recorded {
            runs: vec![run("C", vec![1, 2, 3])],
            ..Recorded::default()
        }
        .encode();

        assert!(Recorded::decode(&bytes[..bytes.len() - 1]).is_err());
    }
}
