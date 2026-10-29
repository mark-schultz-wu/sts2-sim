//! What the simulator is held to in a scenario, as bytes.
//!
//! A scenario is a handful of steps and what the game answered. There are
//! many, so the answers are stored as tightly as they read back. One file for
//! the suite, and one hash per part of an observation that moved since the
//! step before it. The question a parked run asks is one more part.
//!
//! Not a directory per scenario. A five-step test does not need five files,
//! and two hundred of those is a tree nobody reads. This is one file, and
//! `just scenarios` is what rewrites it.
//!
//! Only hashes are kept, never the game's own state. A failure names the parts
//! that differ and the simulator prints its side, which is the half a person
//! can act on. Keeping the other half costs more than it says.

use std::collections::BTreeMap;

use sts2_core::{Digest, Section};

/// What the file starts with, so a wrong one is refused rather than read.
const MAGIC: &[u8; 8] = b"STS2SCN\x00";

/// The layout below. A change here is a re-record, not a migration.
const VERSION: u16 = 2;

/// The mask bit a step sets when what the run asks moved. The sections take
/// the low bits, one each.
const QUESTION: u16 = 1 << 15;

/// Every scenario's answers, by name.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Expected {
    /// The build that answered, so a re-pin is caught rather than compared.
    pub stamp: Stamp,
    pub scenarios: BTreeMap<String, Vec<Step>>,
}

pub use sts2_run::recorded::Stamp;

/// What one step left the run showing: each section of the observation,
/// and the question the run is parked on, as [`Digest::of_question`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub sections: Vec<(Section, Digest)>,
    pub question: Digest,
}

/// One step, as the parts that moved and what they moved to.
///
/// Absent means unchanged from the step before, which is most of it. Better
/// than four fifths of an observation matches the one before.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Step {
    pub moved: Vec<(Section, Digest)>,
    /// What the question moved to, when it moved.
    pub question: Option<Digest>,
}

impl Step {
    /// What moved between two steps, sections in section order.
    pub fn between(before: Option<&Seen>, after: &Seen) -> Step {
        let moved = after
            .sections
            .iter()
            .enumerate()
            .filter(|(at, (_, now))| before.is_none_or(|was| was.sections[*at].1 != *now))
            .map(|(_, &pair)| pair)
            .collect();
        let question = match before {
            Some(was) if was.question == after.question => None,
            _ => Some(after.question),
        };
        Step { moved, question }
    }
}

impl Expected {
    /// The whole file.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.stamp.assembly.to_le_bytes());
        out.extend_from_slice(&self.stamp.content.to_le_bytes());
        let count = u16::try_from(self.scenarios.len()).expect("a suite of scenarios fits a u16");
        out.extend_from_slice(&count.to_le_bytes());

        for (name, steps) in &self.scenarios {
            let len = u8::try_from(name.len()).expect("a scenario's name is short");
            out.push(len);
            out.extend_from_slice(name.as_bytes());
            let steps_len = u16::try_from(steps.len()).expect("a scenario is short");
            out.extend_from_slice(&steps_len.to_le_bytes());
            for step in steps {
                let mut mask = 0u16;
                for (section, _) in &step.moved {
                    mask |= 1 << section.number();
                }
                if step.question.is_some() {
                    mask |= QUESTION;
                }
                out.extend_from_slice(&mask.to_le_bytes());
                for (_, digest) in &step.moved {
                    out.extend_from_slice(&digest.to_u64().to_le_bytes());
                }
                if let Some(question) = step.question {
                    out.extend_from_slice(&question.to_u64().to_le_bytes());
                }
            }
        }
        out
    }

    /// The whole file, back.
    pub fn decode(bytes: &[u8]) -> Result<Expected, String> {
        let mut read = Reader { bytes, at: 0 };
        if read.take(8)? != MAGIC {
            return Err("not a scenario file".into());
        }
        let version = read.u16()?;
        if version != VERSION {
            return Err(format!("version {version}, and this reads {VERSION}"));
        }
        let stamp = Stamp {
            assembly: read.i64()?,
            content: read.i64()?,
        };
        let count = read.u16()?;

        let mut scenarios = BTreeMap::new();
        for _ in 0..count {
            let len = usize::from(read.u8()?);
            let name = String::from_utf8(read.take(len)?.to_vec())
                .map_err(|e| format!("a scenario's name is not text: {e}"))?;
            let steps_len = read.u16()?;
            let mut steps = Vec::with_capacity(usize::from(steps_len));
            for _ in 0..steps_len {
                let mask = read.u16()?;
                let mut moved = Vec::new();
                for section in Section::ALL {
                    if mask & (1 << section.number()) != 0 {
                        moved.push((section, Digest::from_u64(read.u64()?)));
                    }
                }
                let question = match mask & QUESTION {
                    0 => None,
                    _ => Some(Digest::from_u64(read.u64()?)),
                };
                steps.push(Step { moved, question });
            }
            scenarios.insert(name, steps);
        }
        Ok(Expected { stamp, scenarios })
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

    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().expect("eight")))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("eight")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(n: u64) -> Digest {
        Digest::from_u64(n)
    }

    #[test]
    fn a_suite_survives_the_round_trip() {
        let mut scenarios = BTreeMap::new();
        scenarios.insert(
            "weak-and-shrunk".to_string(),
            vec![
                Step {
                    moved: Section::ALL.into_iter().map(|s| (s, digest(7))).collect(),
                    question: Some(digest(5)),
                },
                Step {
                    moved: vec![(Section::Creatures, digest(9)), (Section::Legal, digest(3))],
                    question: None,
                },
            ],
        );
        scenarios.insert("nothing-moved".to_string(), vec![Step::default()]);
        let expected = Expected {
            stamp: Stamp {
                assembly: 1_692_500_715,
                content: 3_954_186_980,
            },
            scenarios,
        };

        let bytes = expected.encode();
        assert_eq!(Expected::decode(&bytes), Ok(expected));
    }

    /// The size is the point of the format, so it is asserted rather than
    /// hoped for. Twelve sections at eight bytes is the whole of a first step.
    #[test]
    fn a_step_that_moves_two_sections_costs_eighteen_bytes() {
        let one = Expected {
            stamp: Stamp::default(),
            scenarios: BTreeMap::from([("a".to_string(), vec![Step::default()])]),
        };
        let two = Expected {
            stamp: Stamp::default(),
            scenarios: BTreeMap::from([(
                "a".to_string(),
                vec![
                    Step::default(),
                    Step {
                        question: None,
                        moved: vec![(Section::Creatures, digest(1)), (Section::Legal, digest(2))],
                    },
                ],
            )]),
        };
        assert_eq!(two.encode().len() - one.encode().len(), 18);
    }

    #[test]
    fn a_file_from_somewhere_else_is_refused() {
        assert!(Expected::decode(b"not a scenario file at all").is_err());
        let mut bytes = Expected::default().encode();
        bytes[8] = 99;
        assert!(Expected::decode(&bytes).is_err());
    }
}
