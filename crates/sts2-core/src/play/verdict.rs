//! How a run that a runner played came out, as its last line.
//!
//! Every runner ends a run on one [`Verdict`], printed as one line, and
//! every tool that starts a runner reads it back with one parser. A panic
//! ends the process, so the runner's panic hook prints the verdict then.

use crate::SeatIndex;

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum End {
    /// The run is over, won.
    Won,
    /// The run is over, lost.
    Lost,
    /// The runner played as many moves as it was given.
    Capped,
    /// No seat had a move, and the run is not over.
    Stalled,
    /// A hunt's simulator followed the game to where the run stopped.
    Agreed,
    /// A hunt's simulator and game part at this move.
    Parted,
    /// The process panicked.
    Panicked,
    /// The table refused a move it reported as legal.
    Refused,
    /// A table fault, which is not a refusal.
    Failed,
    /// A game logged an error.
    GameError,
    /// A game stopped answering, or never started.
    GameGone,
}

impl End {
    const NAMES: [(End, &'static str); 11] = [
        (End::Won, "won"),
        (End::Lost, "lost"),
        (End::Capped, "capped"),
        (End::Stalled, "stalled"),
        (End::Agreed, "agreed"),
        (End::Parted, "parted"),
        (End::Panicked, "panicked"),
        (End::Refused, "refused"),
        (End::Failed, "failed"),
        (End::GameError, "game-error"),
        (End::GameGone, "game-gone"),
    ];

    fn name(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(end, _)| *end == self)
            .map(|(_, name)| *name)
            .expect("every end has a name")
    }
}

impl std::fmt::Display for End {
    /// The end as a verdict line spells it, such as `game-error`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// How a run came out, and where it stood then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub end: End,
    /// The moves played before it ended.
    pub step: usize,
    /// The furthest act and floor any seat reached.
    pub act: u32,
    pub floor: u32,
    /// The seat it is about, where it is about one.
    pub seat: Option<SeatIndex>,
    /// Where in the code a panic was raised, as `file:line:column`.
    pub place: Option<String>,
    /// What else there is to say, on the rest of the line.
    pub detail: String,
}

/// What every verdict line opens with.
const OPENS: &str = "verdict";

impl std::fmt::Display for Verdict {
    /// `verdict <end> <step> <act> <floor> <seat or -> <place or -> <detail>`,
    /// all on one line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let seat = self.seat.map_or("-".to_owned(), |seat| seat.0.to_string());
        let place = self.place.as_deref().unwrap_or("-");
        let detail = self.detail.replace('\n', " ");
        write!(
            f,
            "{OPENS} {} {} {} {} {seat} {place} {detail}",
            self.end.name(),
            self.step,
            self.act,
            self.floor
        )
    }
}

/// A line that is no verdict.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a verdict line: {0:?}")]
pub struct NotAVerdict(pub String);

impl std::str::FromStr for Verdict {
    type Err = NotAVerdict;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let not = || NotAVerdict(line.to_owned());
        let mut words = line.splitn(8, ' ');
        if words.next() != Some(OPENS) {
            return Err(not());
        }
        let mut word = || words.next().ok_or_else(not);
        let end = word()?;
        let end = End::NAMES
            .iter()
            .find(|(_, name)| *name == end)
            .map(|(end, _)| *end)
            .ok_or_else(not)?;
        let step = word()?.parse().map_err(|_| not())?;
        let act = word()?.parse().map_err(|_| not())?;
        let floor = word()?.parse().map_err(|_| not())?;
        let seat = match word()? {
            "-" => None,
            seat => Some(SeatIndex(seat.parse().map_err(|_| not())?)),
        };
        let place = match word()? {
            "-" => None,
            place => Some(place.to_owned()),
        };
        let detail = words.next().unwrap_or_default().to_owned();
        Ok(Self {
            end,
            step,
            act,
            floor,
            seat,
            place,
            detail,
        })
    }
}

impl Verdict {
    /// The verdict a runner's output ends on: its last line that is one.
    pub fn last_in(output: &str) -> Option<Self> {
        output.lines().rev().find_map(|line| line.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::{End, Verdict};
    use crate::SeatIndex;

    /// A verdict reads back as it was written, the free text included.
    #[test]
    fn a_verdict_reads_back_as_written() {
        let verdict = Verdict {
            end: End::Panicked,
            step: 817,
            act: 2,
            floor: 31,
            seat: Some(SeatIndex(1)),
            place: Some("crates/sts2-sim/src/run/state.rs:5925:17".into()),
            detail: "not implemented: TheLanternKey, which opens on a fight".into(),
        };
        let line = verdict.to_string();
        assert_eq!(line.parse::<Verdict>(), Ok(verdict.clone()));
        let output = format!("at 2 31\n{line}\n");
        assert_eq!(Verdict::last_in(&output), Some(verdict));
    }

    /// Every end reads back as itself, and nothing else is a verdict.
    #[test]
    fn every_end_has_a_name_of_its_own() {
        for (end, _) in End::NAMES {
            let verdict = Verdict {
                end,
                step: 0,
                act: 0,
                floor: 0,
                seat: None,
                place: None,
                detail: String::new(),
            };
            assert_eq!(
                verdict.to_string().parse::<Verdict>().map(|v| v.end),
                Ok(end)
            );
        }
        assert!("at 1 2".parse::<Verdict>().is_err());
    }
}
