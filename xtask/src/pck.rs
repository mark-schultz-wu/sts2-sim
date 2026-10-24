//! Reading files out of the game's Godot pack, `SlayTheSpire2.pck`.
//!
//! The pack is Godot 4's format 3: a header, the files' bytes, and a
//! directory at an offset the header names. Each directory entry is a path, an
//! offset from the file base, a size, an MD5, and flags. The game ships it
//! unencrypted, so a file is read by seeking to its offset. A flag that says
//! a file is encrypted stops the read rather than handing back noise.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The pack format this reads.
const FORMAT: u32 = 3;
/// `PACK_FILE_ENCRYPTED`, on one entry.
const ENCRYPTED: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}: not a Godot pack")]
    NotAPack(PathBuf),
    #[error("{path}: pack format {format}, and this reads {FORMAT}")]
    Format { path: PathBuf, format: u32 },
    #[error("{pack}: no {file} in the pack")]
    Missing { pack: PathBuf, file: String },
    #[error("{pack}: {file} is encrypted")]
    Encrypted { pack: PathBuf, file: String },
    #[error("{pack}: {file} is not UTF-8")]
    NotText { pack: PathBuf, file: String },
}

/// One file in the pack.
struct Entry {
    path: String,
    offset: u64,
    size: u64,
    flags: u32,
}

/// An open pack and its directory.
pub struct Pack {
    path: PathBuf,
    file: File,
    entries: Vec<Entry>,
}

impl Pack {
    /// Opens a pack and reads its directory.
    pub fn open(path: &Path) -> Result<Pack, Error> {
        let io = |source| Error::Io {
            path: path.to_owned(),
            source,
        };
        let mut file = File::open(path).map_err(io)?;
        let mut header = [0u8; 40];
        file.read_exact(&mut header).map_err(io)?;
        if &header[0..4] != b"GDPC" {
            return Err(Error::NotAPack(path.to_owned()));
        }
        let word =
            |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().expect("four bytes"));
        let long =
            |at: usize| u64::from_le_bytes(header[at..at + 8].try_into().expect("eight bytes"));
        let format = word(4);
        if format != FORMAT {
            return Err(Error::Format {
                path: path.to_owned(),
                format,
            });
        }
        // Then the engine's major, minor and patch, and the pack's flags.
        let base = long(24);
        let directory = long(32);

        file.seek(SeekFrom::Start(directory)).map_err(io)?;
        let mut reader = std::io::BufReader::new(&mut file);
        let mut take = |count: usize| -> Result<Vec<u8>, Error> {
            let mut bytes = vec![0u8; count];
            reader.read_exact(&mut bytes).map_err(io)?;
            Ok(bytes)
        };
        let word = |bytes: &[u8]| u32::from_le_bytes(bytes[..4].try_into().expect("four bytes"));
        let long = |bytes: &[u8]| u64::from_le_bytes(bytes[..8].try_into().expect("eight bytes"));

        let count = word(&take(4)?);
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let length = word(&take(4)?) as usize;
            let name = take(length)?;
            // The offset, the size, the MD5, then the flags.
            let rest = take(8 + 8 + 16 + 4)?;
            entries.push(Entry {
                path: String::from_utf8_lossy(&name)
                    .trim_end_matches('\0')
                    .to_owned(),
                offset: base + long(&rest[0..]),
                size: long(&rest[8..]),
                flags: word(&rest[32..]),
            });
        }
        Ok(Pack {
            path: path.to_owned(),
            file,
            entries,
        })
    }

    /// One file's text, by its path inside the pack.
    pub fn text(&mut self, name: &str) -> Result<String, Error> {
        let entry = (self.entries.iter())
            .find(|entry| entry.path == name)
            .ok_or_else(|| Error::Missing {
                pack: self.path.clone(),
                file: name.to_owned(),
            })?;
        if entry.flags & ENCRYPTED != 0 {
            return Err(Error::Encrypted {
                pack: self.path.clone(),
                file: name.to_owned(),
            });
        }
        let io = |source| Error::Io {
            path: self.path.clone(),
            source,
        };
        let mut bytes = vec![0u8; entry.size as usize];
        self.file.seek(SeekFrom::Start(entry.offset)).map_err(io)?;
        self.file.read_exact(&mut bytes).map_err(io)?;
        String::from_utf8(bytes).map_err(|_| Error::NotText {
            pack: self.path.clone(),
            file: name.to_owned(),
        })
    }
}
