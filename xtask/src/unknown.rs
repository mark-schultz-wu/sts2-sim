//! `cargo xtask unknown [--character <C>] [--seeds <N>] [--room <ROOM>]`: the
//! census seeds whose first unknown room rolls a fight, a chest or a shop.
//!
//! A scenario that walks into one needs a seed whose first draw of the
//! `UnknownMapPoint` stream lands inside that room's odds. This runs the
//! `unknown_rolls` example over the seeds, which reads the draw off a copy of
//! the stream. It prints each seed that fits, the draw, and the unknown points
//! of the act's first rows.
//!
//! The bands are the odds an act opens with. The shop's needs the point before
//! to hold none, and the seed's own path says whether it does.

use std::process::Command;

use crate::error::{self, ProcessError};

#[derive(clap::Args)]
pub struct Args {
    /// The character whose run is asked.
    #[arg(long, default_value = "IRONCLAD")]
    character: String,
    /// How many seeds to try, from the census's first.
    #[arg(long, default_value_t = 300)]
    seeds: u64,
    /// The room the first unknown room rolls.
    #[arg(long, value_enum, default_value_t = Room::Monster)]
    room: Room,
}

/// A room other than an event, with its band of the first draw.
#[derive(Clone, Copy, clap::ValueEnum)]
enum Room {
    Monster,
    Treasure,
    Shop,
}

impl Room {
    /// `UnknownMapPointOdds`'s base odds, walked in order.
    fn band(self) -> (f32, f32) {
        match self {
            Room::Monster => (0.0, 0.1),
            Room::Treasure => (0.1, 0.12),
            Room::Shop => (0.12, 0.15),
        }
    }
}

pub fn run(args: Args) -> Result<String, ProcessError> {
    let Args {
        character,
        seeds,
        room,
    } = args;
    let output = error::output(
        Command::new("cargo")
            .current_dir(crate::workspace())
            .args([
                "run",
                "-q",
                "-p",
                "sts2-golden",
                "--example",
                "unknown_rolls",
                "--",
            ])
            .args([character.as_str(), "0", &seeds.to_string()]),
    )?;
    let (low, high) = room.band();
    let mut found = String::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut words = line.split_whitespace();
        let (Some(seed), Some(roll)) = (words.next(), words.next()) else {
            continue;
        };
        let Ok(roll) = roll.parse::<f32>() else {
            continue;
        };
        let points: Vec<&str> = words.collect();
        if roll > low && roll <= high && !points.is_empty() {
            found.push_str(&format!("{seed} {roll:.4} {}\n", points.join(" ")));
        }
    }
    Ok(found)
}
