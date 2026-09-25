//! Where a replayed step allocates, under DHAT.
//!
//! The replay is the one `instructions` counts. DHAT keeps every heap block
//! with the stack that asked for it, inlined frames included. So a site has
//! a name even where the optimiser folded it into its caller. Map generation
//! is left out, as `instructions` leaves it out.
//!
//! A site is the innermost frame of the simulator's own on the stack. Beside
//! it is the caller it was reached from, which is the frame of ours outside it.
//!
//! ```text
//! cargo xtask allocations
//! cargo xtask allocations --top 30
//! ```

use std::collections::HashMap;
use std::process::Command;

use crate::instructions::{divide_by, workspace};

/// A frame in map generation, which a search inside a fight never runs.
fn in_map_generation(frame: &str) -> bool {
    frame.contains("ActMapBuilder") && frame.contains(">::generate")
}

/// A frame of the simulator's own, by where its source lives. An inlined
/// frame carries no module path, so its name alone cannot say.
fn ours(frame: &str) -> bool {
    frame.contains("(crates/")
}

pub fn run(args: &[String]) -> Result<String, String> {
    let mut top = 20;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--top" => {
                top = args
                    .next()
                    .ok_or("--top names a count")?
                    .parse()
                    .map_err(|e| format!("--top: {e}"))?;
            }
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }

    let root = workspace()?;
    let dir = root.join("target/profile");
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let built = Command::new("cargo")
        .args([
            "build",
            "-q",
            "--release",
            "-p",
            "sts2-golden",
            "--example",
            "replay_sites",
        ])
        .current_dir(&root)
        .status()
        .map_err(|e| format!("running cargo: {e}"))?;
    if !built.success() {
        return Err("the replay did not build".into());
    }

    let out = dir.join("allocations.dhat.json");
    let replayed = Command::new("valgrind")
        .arg("--tool=dhat")
        // Each source path from the repository down, so a frame's own file says
        // whose it is.
        .arg(format!("--fullpath-after={}/", root.display()))
        .arg(format!("--dhat-out-file={}", out.display()))
        .arg(root.join("target/release/examples/replay_sites"))
        .arg("rules")
        .current_dir(&root)
        .output()
        .map_err(|e| format!("running valgrind, which this needs installed: {e}"))?;
    if !replayed.status.success() {
        return Err(format!(
            "the replay failed under DHAT:\n{}",
            String::from_utf8_lossy(&replayed.stderr)
        ));
    }
    let steps = divide_by(&String::from_utf8_lossy(&replayed.stdout))?;

    let text =
        std::fs::read_to_string(&out).map_err(|e| format!("reading {}: {e}", out.display()))?;
    let profile: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("reading DHAT's output: {e}"))?;
    let frames: Vec<&str> = profile["ftbl"]
        .as_array()
        .ok_or("DHAT's output has no frame table")?
        .iter()
        .map(|frame| frame.as_str().unwrap_or(""))
        .collect();
    let points = profile["pps"]
        .as_array()
        .ok_or("DHAT's output has no program points")?;

    // Blocks and bytes by site and the caller of ours outside it.
    let mut sites: HashMap<(String, String), (u64, u64)> = HashMap::new();
    let (mut blocks, mut bytes) = (0, 0);
    for point in points {
        let stack: Vec<&str> = point["fs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|at| frames.get(at.as_u64()? as usize).copied())
            .collect();
        if stack.iter().any(|frame| in_map_generation(frame)) {
            continue;
        }
        let (count, size) = (
            point["tbk"].as_u64().unwrap_or(0),
            point["tb"].as_u64().unwrap_or(0),
        );
        blocks += count;
        bytes += size;
        // Innermost first, which is the order DHAT lists a stack in.
        let mut of_ours = stack
            .iter()
            .filter(|frame| ours(frame))
            .map(|frame| name(frame));
        let site = of_ours
            .next()
            .unwrap_or_else(|| "(none of ours)".to_owned());
        let caller = of_ours.find(|caller| *caller != site).unwrap_or_default();
        let entry = sites.entry((site, caller)).or_default();
        entry.0 += count;
        entry.1 += size;
    }

    let per_step = |count: u64| count as f64 / steps as f64;
    let mut report = format!(
        "{steps} steps outside map generation: {:.2} blocks and {:.0} bytes a step\n\
         the {top} sites that allocate most often, blocks and bytes a step:\n",
        per_step(blocks),
        per_step(bytes),
    );
    let mut sites: Vec<_> = sites.into_iter().collect();
    sites.sort_by_key(|site| std::cmp::Reverse(site.1.0));
    for ((site, caller), (count, size)) in sites.into_iter().take(top) {
        report.push_str(&format!(
            "  {:>7.3} {:>7.0}  {site}\n{:>18}< {caller}\n",
            per_step(count),
            per_step(size),
            "",
        ));
    }
    Ok(report)
}

/// A frame's function and where in the source it stands, without the address
/// DHAT opens it with.
fn name(frame: &str) -> String {
    let frame = frame.split_once(": ").map_or(frame, |(_, rest)| rest);
    frame
        .replace("sts2_content::v0_107_1::V0_107_1", "V")
        .replace("sts2_content::v0_107_1::", "")
}
