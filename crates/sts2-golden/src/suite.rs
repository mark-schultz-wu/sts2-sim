// The scenarios, as they are written down.
//
// One file each, under `tests/scenarios/`, named for what the scenario pins.
// The name is the file's, so a failure names a file somebody can open.
//
// A file is the commands, one JSON object per line, with `#` for the reader.
// No build stamp: a recording file carries one so it cannot be replayed into
// the wrong game, and a scenario is answered by whichever build the caller
// names. The recorded answers carry the stamp instead, and the test refuses a
// set from another build.
//
// Plain comments and not a module doc: the build script splices this file in
// with `include!`, and an inner attribute cannot appear there.

/// One scenario: its name, and the commands that reach it.
///
/// Text rather than parsed. The build script hashes it to decide whether the
/// answer it holds is still the answer.
pub struct Written {
    pub name: String,
    pub commands: String,
}

/// Every scenario in a directory, in name order.
///
/// Ordered, because a suite that reads differently on two machines writes a
/// different file on each.
pub fn read_suite(dir: &std::path::Path) -> Result<Vec<Written>, String> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("reading {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|end| end == "txt"))
        .collect();
    files.sort();

    let mut out = Vec::with_capacity(files.len());
    for path in files {
        let name = path
            .file_stem()
            .ok_or_else(|| format!("{} has no name", path.display()))?
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;

        let mut commands = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            commands.push_str(line);
            commands.push('\n');
        }
        if commands.is_empty() {
            return Err(format!("{} has no commands in it", path.display()));
        }
        out.push(Written { name, commands });
    }
    Ok(out)
}
