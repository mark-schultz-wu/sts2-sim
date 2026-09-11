// The scenarios, as they are written down.
//
// Plain comments and not a module doc: the build script splices this file in
// with `include!`, and an inner attribute cannot appear there.
//
// One file for the suite rather than a directory each. A scenario is a few
// lines, and a tree of directories holding one recording apiece is a tree
// nobody reads.
//
// `===` opens a scenario and names it. Everything above the first one is the
// build stamp, which every scenario shares because they are all answered by
// the same game. Blank lines and `#` are for the reader.
//
// Included by the build script as well as the crate, so the two cannot drift.

/// One scenario: its name, and the recording that reaches it.
///
/// Text rather than parsed. The build script hashes it to decide whether the
/// answer it holds is still the answer.
pub struct Written {
    pub name: String,
    pub recording: String,
}

/// Every scenario in a suite file.
pub fn read_suite(text: &str) -> Result<Vec<Written>, String> {
    let mut stamp = String::new();
    let mut out: Vec<Written> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix("===") {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err("a scenario with no name".into());
            }
            if out.iter().any(|written| written.name == name) {
                return Err(format!("two scenarios called {name}"));
            }
            if stamp.is_empty() {
                return Err(format!("{name} comes before the build stamp"));
            }
            out.push(Written {
                name,
                recording: stamp.clone(),
            });
            continue;
        }
        match out.last_mut() {
            Some(written) => {
                written.recording.push_str(trimmed);
                written.recording.push('\n');
            }
            None if stamp.is_empty() => {
                stamp.push_str(trimmed);
                stamp.push('\n');
            }
            None => return Err(format!("a line outside any scenario: {trimmed}")),
        }
    }
    Ok(out)
}
