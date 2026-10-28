//! Size-, count-, and age-capped JSON-lines log files (native only).
//!
//! Files in the log directory:
//! - `current.jsonl`: the file being written.
//! - `log-<unix-ms>.jsonl`: rotated files, named by when they were rotated out.
//!
//! The active file is rotated **before** a line would push it past
//! [`FileLogPolicy::max_file_bytes`]. After each rotation (and at startup) the
//! oldest rotated files beyond the file count, and any older than
//! [`FileLogPolicy::max_age_millis`], are deleted. Disk use is therefore capped
//! at roughly `max_files × max_file_bytes`.
//!
//! File age comes from the timestamp in the name, not file-system metadata, so it
//! follows the injected [`Clock`] and is deterministic in tests.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use domain::time::{Clock, UnixMillis};

use super::capture::EntrySink;
use super::entry::LogEntry;

/// Name of the file currently being written.
const CURRENT_FILE_NAME: &str = "current.jsonl";
/// Prefix and extension of rotated files: `log-<unix-ms>.jsonl`.
const ROTATED_PREFIX: &str = "log-";
const ROTATED_EXTENSION: &str = ".jsonl";

const MEGABYTE: u64 = 1024 * 1024;
/// Rotated files older than this are deleted regardless of count (plan §5: 30 days).
const MAX_AGE_MILLIS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Limits for one platform's log files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileLogPolicy {
    /// The active file is rotated before it would grow past this size.
    pub max_file_bytes: u64,
    /// Total files kept, counting the active file.
    pub max_files: usize,
    /// Rotated files older than this are deleted.
    pub max_age_millis: i64,
}

impl FileLogPolicy {
    /// Server (NAS): 10 files × 10 MB (~100 MB).
    pub const SERVER: Self = Self {
        max_file_bytes: 10 * MEGABYTE,
        max_files: 10,
        max_age_millis: MAX_AGE_MILLIS,
    };

    /// Desktop: 5 files × 10 MB (~50 MB).
    pub const DESKTOP: Self = Self {
        max_file_bytes: 10 * MEGABYTE,
        max_files: 5,
        max_age_millis: MAX_AGE_MILLIS,
    };

    /// Mobile: 3 files × 2 MB (~6 MB).
    pub const MOBILE: Self = Self {
        max_file_bytes: 2 * MEGABYTE,
        max_files: 3,
        max_age_millis: MAX_AGE_MILLIS,
    };
}

/// Writes log lines to `current.jsonl` in a directory, rotating and pruning per a [`FileLogPolicy`].
pub struct RotatingFile {
    directory: PathBuf,
    policy: FileLogPolicy,
    clock: Arc<dyn Clock + Send + Sync>,
    /// `None` only briefly during rotation, while the file is closed for renaming.
    file: Option<File>,
    current_bytes: u64,
}

impl RotatingFile {
    /// Opens (or creates) the log directory and its active file, then prunes old files.
    ///
    /// Appends to an existing `current.jsonl`, so logs survive restarts.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory or file can't be created or read.
    pub fn open(
        directory: impl Into<PathBuf>,
        policy: FileLogPolicy,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> io::Result<Self> {
        let directory = directory.into();
        fs::create_dir_all(&directory)?;
        let file = open_append(&directory.join(CURRENT_FILE_NAME))?;
        let current_bytes = file.metadata()?.len();
        let rotating_file = Self {
            directory,
            policy,
            clock,
            file: Some(file),
            current_bytes,
        };
        rotating_file.prune()?;
        Ok(rotating_file)
    }

    /// Appends one line (a newline is added), rotating first if it wouldn't fit.
    ///
    /// A single line larger than the size limit is still written, into a fresh file.
    ///
    /// # Errors
    ///
    /// Returns an error if writing, rotating, or pruning fails.
    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        // Lossless: usize is at most 64 bits on every supported platform.
        let line_bytes = u64::try_from(line.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let would_exceed =
            self.current_bytes.saturating_add(line_bytes) > self.policy.max_file_bytes;
        if self.current_bytes > 0 && would_exceed {
            self.rotate()?;
        }

        let file = self.active_file()?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        self.current_bytes = self.current_bytes.saturating_add(line_bytes);
        Ok(())
    }

    /// Renames the active file to `log-<now>.jsonl`, starts a new one, and prunes.
    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
            // Dropped here: Windows can't rename a file that is still open.
        }
        let current_path = self.directory.join(CURRENT_FILE_NAME);
        fs::rename(&current_path, self.unused_rotated_path())?;

        self.file = Some(open_append(&current_path)?);
        self.current_bytes = 0;
        self.prune()
    }

    /// A rotated-file path named by the current time, bumped by a millisecond if
    /// two rotations happen within the same millisecond.
    fn unused_rotated_path(&self) -> PathBuf {
        let mut millis = self.clock.now().as_millis();
        loop {
            let path = self
                .directory
                .join(rotated_file_name(UnixMillis::from_millis(millis)));
            if !path.exists() {
                return path;
            }
            millis = millis.saturating_add(1);
        }
    }

    /// Deletes rotated files beyond the count limit (oldest first) or past the age limit.
    fn prune(&self) -> io::Result<()> {
        let mut rotated = rotated_files(&self.directory)?;
        // Newest first, so everything after the allowed count is the oldest.
        rotated.sort_by_key(|(rotated_at, _)| std::cmp::Reverse(*rotated_at));

        // The active file counts toward the limit.
        let max_rotated = self.policy.max_files.saturating_sub(1);
        let oldest_allowed = self
            .clock
            .now()
            .as_millis()
            .saturating_sub(self.policy.max_age_millis);

        for (index, (rotated_at, path)) in rotated.iter().enumerate() {
            let over_count = index >= max_rotated;
            let too_old = rotated_at.as_millis() < oldest_allowed;
            if over_count || too_old {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    fn active_file(&mut self) -> io::Result<&mut File> {
        // Only `rotate` leaves this empty, and it always reopens or returns an error;
        // reopen here too so a failed rotation doesn't stop logging for good.
        if self.file.is_none() {
            let file = open_append(&self.directory.join(CURRENT_FILE_NAME))?;
            self.current_bytes = file.metadata()?.len();
            self.file = Some(file);
        }
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file unavailable"))
    }
}

/// An [`EntrySink`] that writes entries to a [`RotatingFile`] as JSON lines.
///
/// Write failures can't be logged (that would recurse into this sink), so they are
/// counted instead; the diagnostics screen can show the count.
pub struct FileSink {
    file: Mutex<RotatingFile>,
    failures: AtomicU64,
}

impl FileSink {
    /// Wraps an open rotating file.
    pub fn new(file: RotatingFile) -> Self {
        Self {
            file: Mutex::new(file),
            failures: AtomicU64::new(0),
        }
    }

    /// How many entries failed to serialize or be written since startup.
    pub fn failure_count(&self) -> u64 {
        self.failures.load(Ordering::Relaxed)
    }
}

impl EntrySink for FileSink {
    fn accept(&self, entry: &LogEntry) {
        // A panic mid-write leaves at worst a partial line, so a poisoned lock is safe to reuse.
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let written = entry
            .to_json_line()
            .map_err(io::Error::from)
            .and_then(|line| file.write_line(&line));
        if written.is_err() {
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn open_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn rotated_file_name(rotated_at: UnixMillis) -> String {
    format!(
        "{ROTATED_PREFIX}{}{ROTATED_EXTENSION}",
        rotated_at.as_millis()
    )
}

/// Parses the rotation time out of a `log-<unix-ms>.jsonl` file name.
fn parse_rotated_file_name(name: &str) -> Option<UnixMillis> {
    let millis = name
        .strip_prefix(ROTATED_PREFIX)?
        .strip_suffix(ROTATED_EXTENSION)?
        .parse()
        .ok()?;
    Some(UnixMillis::from_millis(millis))
}

/// Lists the rotated log files in `directory` with their rotation times.
/// Other files (including `current.jsonl`) are ignored.
fn rotated_files(directory: &Path) -> io::Result<Vec<(UnixMillis, PathBuf)>> {
    let mut rotated = Vec::new();
    for dir_entry in fs::read_dir(directory)? {
        let dir_entry = dir_entry?;
        let name = dir_entry.file_name();
        if let Some(rotated_at) = name.to_str().and_then(parse_rotated_file_name) {
            rotated.push((rotated_at, dir_entry.path()));
        }
    }
    Ok(rotated)
}

#[cfg(test)]
mod tests {
    use domain::test_support::ManualClock;

    use super::*;

    const START: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;

    /// A tiny policy so tests rotate after a few short lines.
    const SMALL: FileLogPolicy = FileLogPolicy {
        max_file_bytes: 20,
        max_files: 3,
        max_age_millis: 30 * DAY_MILLIS,
    };

    fn open(directory: &Path, clock: &Arc<ManualClock>) -> RotatingFile {
        RotatingFile::open(
            directory,
            SMALL,
            Arc::clone(clock) as Arc<dyn Clock + Send + Sync>,
        )
        .unwrap()
    }

    /// File names in the directory, sorted.
    fn file_names(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn current_contents(directory: &Path) -> String {
        fs::read_to_string(directory.join(CURRENT_FILE_NAME)).unwrap()
    }

    #[test]
    fn rotating_file_rotates_before_exceeding_max_file_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        let mut file = open(temp.path(), &clock);

        file.write_line("123456789").unwrap(); // 10 bytes with newline
        file.write_line("123456789").unwrap(); // 20 bytes: exactly at the limit
        clock.advance(1);
        file.write_line("abc").unwrap(); // would pass 20, so it rotates first

        assert_eq!(
            file_names(temp.path()),
            ["current.jsonl", "log-1700000000001.jsonl"]
        );
        assert_eq!(current_contents(temp.path()), "abc\n");
        let rotated = fs::read_to_string(temp.path().join("log-1700000000001.jsonl")).unwrap();
        assert_eq!(rotated.len(), 20, "no file may exceed max_file_bytes");
    }

    #[test]
    fn rotating_file_keeps_at_most_max_files() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        let mut file = open(temp.path(), &clock);

        // Each 19-byte line fills a file, so every write after the first rotates.
        for _ in 0..6 {
            clock.advance(1_000);
            file.write_line("a line of 18 bytes").unwrap();
        }

        // max_files = 3: the active file plus the two newest rotated files.
        assert_eq!(
            file_names(temp.path()),
            [
                "current.jsonl",
                "log-1700000005000.jsonl",
                "log-1700000006000.jsonl"
            ]
        );
    }

    #[test]
    fn rotating_file_deletes_rotated_files_older_than_max_age() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        fs::write(temp.path().join(rotated_file_name(START)), "old\n").unwrap();

        clock.advance(30 * DAY_MILLIS + 1);
        let _file = open(temp.path(), &clock); // pruning also runs at startup

        assert_eq!(file_names(temp.path()), ["current.jsonl"]);
    }

    #[test]
    fn rotating_file_keeps_rotated_files_within_max_age() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        fs::write(temp.path().join(rotated_file_name(START)), "recent\n").unwrap();

        clock.advance(30 * DAY_MILLIS);
        let _file = open(temp.path(), &clock);

        assert_eq!(
            file_names(temp.path()),
            ["current.jsonl", "log-1700000000000.jsonl"]
        );
    }

    #[test]
    fn rotating_file_resumes_current_file_size_after_restart() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        open(temp.path(), &clock).write_line("123456789").unwrap();
        open(temp.path(), &clock).write_line("123456789").unwrap();

        // After two restarts the file is at 20 bytes, so the next line must rotate.
        let mut file = open(temp.path(), &clock);
        file.write_line("x").unwrap();

        assert_eq!(current_contents(temp.path()), "x\n");
        assert_eq!(file_names(temp.path()).len(), 2);
    }

    #[test]
    fn rotating_file_writes_an_oversized_line_into_a_fresh_file() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        let mut file = open(temp.path(), &clock);

        file.write_line("short").unwrap();
        file.write_line("this line is longer than twenty bytes")
            .unwrap();

        assert_eq!(
            current_contents(temp.path()),
            "this line is longer than twenty bytes\n"
        );
    }

    #[test]
    fn rotating_file_names_same_millisecond_rotations_uniquely() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        let mut file = open(temp.path(), &clock);

        // The clock never moves, so all rotations happen in the same millisecond.
        for _ in 0..3 {
            file.write_line("a line of 18 bytes").unwrap();
        }

        assert_eq!(
            file_names(temp.path()),
            [
                "current.jsonl",
                "log-1700000000000.jsonl",
                "log-1700000000001.jsonl"
            ]
        );
    }

    #[test]
    fn file_sink_writes_entries_as_json_lines() {
        let temp = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(START));
        let policy = FileLogPolicy::DESKTOP;
        let file =
            RotatingFile::open(temp.path(), policy, clock as Arc<dyn Clock + Send + Sync>).unwrap();
        let sink = FileSink::new(file);
        let entry = LogEntry {
            time_ms: START.as_millis(),
            level: super::super::entry::LogLevel::Info,
            area: "app".to_owned(),
            message: "started".to_owned(),
            fields: std::collections::BTreeMap::new(),
        };

        sink.accept(&entry);

        let line = current_contents(temp.path());
        assert_eq!(LogEntry::from_json_line(line.trim_end()).unwrap(), entry);
        assert_eq!(sink.failure_count(), 0);
    }

    #[test]
    fn rotated_file_names_parse_back_and_ignore_other_files() {
        assert_eq!(
            parse_rotated_file_name(&rotated_file_name(START)),
            Some(START)
        );
        assert_eq!(parse_rotated_file_name("current.jsonl"), None);
        assert_eq!(parse_rotated_file_name("log-notanumber.jsonl"), None);
        assert_eq!(parse_rotated_file_name("notes.txt"), None);
    }
}
