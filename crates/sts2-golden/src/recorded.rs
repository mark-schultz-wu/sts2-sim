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

use sts2_core::{Digest, Digested as _, Snapshot};

/// What the file starts with, so a wrong one is refused rather than read.
const MAGIC: &[u8; 8] = b"STS2REC\x00";

/// The layout below. A change here is a re-record, not a migration.
const VERSION: u16 = 1;

/// Every recorded run, and the build that played them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub stamp: crate::scenario::Stamp,
    pub runs: Vec<Run>,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Won,
    Lost,
    /// Stopped after as many steps as a run is allowed.
    Capped,
}

/// One run the game played.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The character, as the game spells its id.
    pub character: String,
    pub seed: String,
    /// The ascension level, as the game names it.
    pub ascension: String,
    /// See [`sts2_core::Start::wins`].
    pub wins: u32,
    pub ending: Ending,
    /// Where the run stood before each move, and after the last: one more
    /// than [`Run::moves`].
    pub digests: Vec<u32>,
    /// Each move, as its index in the list the game offered.
    pub moves: Vec<u16>,
}

/// Where a run stands, as a recorded run keeps it. The rules digest of the
/// observation, moves included, with the question the run is parked on.
pub fn position<V: sts2_core::ContentVersion>(snapshot: &Snapshot<V>) -> u32 {
    let rules = snapshot.observation.rules_digest().to_u64();
    let question = Digest::of_question(snapshot.asking.as_ref()).to_u64();
    let both = rules ^ question.rotate_left(29);
    (both ^ (both >> 32)) as u32
}

/// How far the simulator followed a recorded run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Followed {
    /// Every position agreed, to the run's end.
    Whole,
    /// The positions agreed up to this step and not at it. Step `n` is the
    /// position before the run's `n`th move.
    PartedAt(usize),
}

/// Replays `run` on the simulator `driven`, begun on its start and carrying
/// what it carried, and says how far the two agree.
///
/// The move at each step is the recorded index into the simulator's own
/// list. The digest the step is compared on covers that list, so where the
/// positions agree the index names the same move.
///
/// # Panics
///
/// Where the simulator does: an unported path, or a move it refuses. A
/// caller that must survive one runs this in a process of its own.
pub fn follow<V, P>(run: &Run, driven: &mut P) -> Followed
where
    V: sts2_core::ContentVersion,
    P: sts2_core::Play<V>,
    P::Error: std::fmt::Display,
{
    for (step, &want) in run.digests.iter().enumerate() {
        let seen = driven
            .observe()
            .unwrap_or_else(|e| panic!("observing step {step}: {e}"));
        if position(&seen) != want {
            return Followed::PartedAt(step);
        }
        let Some(&at) = run.moves.get(step) else {
            break;
        };
        let decision = seen.observation.moves[usize::from(at)];
        driven
            .play(&decision)
            .unwrap_or_else(|e| panic!("playing {decision:?} at step {step}: {e}"))
            .unwrap_or_else(|refusal| panic!("{decision:?} refused at step {step}: {refusal}"));
    }
    Followed::Whole
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
            for text in [&run.character, &run.seed, &run.ascension] {
                let len = u8::try_from(text.len()).expect("an id is short");
                out.push(len);
                out.extend_from_slice(text.as_bytes());
            }
            out.extend_from_slice(&run.wins.to_le_bytes());
            out.push(match run.ending {
                Ending::Won => 0,
                Ending::Lost => 1,
                Ending::Capped => 2,
            });
            assert_eq!(
                run.digests.len(),
                run.moves.len() + 1,
                "a digest for each position, before each move and after the last"
            );
            let steps = u32::try_from(run.moves.len()).expect("a run's moves fit a u32");
            out.extend_from_slice(&steps.to_le_bytes());
            for digest in &run.digests {
                out.extend_from_slice(&digest.to_le_bytes());
            }
            // Most lists hold fewer than 128 moves, so most take one byte.
            for &at in &run.moves {
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
        let stamp = crate::scenario::Stamp {
            assembly: read.i64()?,
            content: read.i64()?,
        };
        let count = read.u32()?;

        let mut runs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let character = read.text()?;
            let seed = read.text()?;
            let ascension = read.text()?;
            let wins = read.u32()?;
            let ending = match read.u8()? {
                0 => Ending::Won,
                1 => Ending::Lost,
                2 => Ending::Capped,
                other => return Err(format!("{other} is no way for a run to end")),
            };
            let steps = read.u32()? as usize;
            let digests = (0..=steps)
                .map(|_| read.u32())
                .collect::<Result<Vec<_>, _>>()?;
            let moves = (0..steps)
                .map(|_| read.varint())
                .collect::<Result<Vec<_>, _>>()?;
            runs.push(Run {
                character,
                seed,
                ascension,
                wins,
                ending,
                digests,
                moves,
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
            character: "SILENT".into(),
            seed: seed.into(),
            ascension: "None".into(),
            wins: 4,
            ending: Ending::Lost,
            digests: (0..=moves.len() as u32)
                .map(|n| n.wrapping_mul(0x9E37_79B9))
                .collect(),
            moves,
        }
    }

    #[test]
    fn a_file_reads_back_as_it_was_written() {
        let recorded = Recorded {
            stamp: crate::scenario::Stamp {
                assembly: 1692500715,
                content: -3,
            },
            runs: vec![
                run("C000000001", vec![0, 3, 127, 128, 300, 16383, 65535]),
                run("C000000002", vec![]),
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
