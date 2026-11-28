//! The engine's own words before the run log exists (docs/fork.md).
//!
//! The run log is opened in `run_with_args`, well after the start has already done the work
//! that can refuse it: the game folder, the language files, the Mod Organizer 2 instance and
//! the plugin list. Each of those says `error: ...` on stderr and returns, and on Android
//! stderr goes nowhere a player can reach, so a start that failed left the launcher's Export
//! debug log with nothing at all to export. That is exactly the shape of the owner's report
//! of 2026-11-27: mods added, the game died at the start, and no way to get a log off the
//! device.
//!
//! So on Android the first thing [`capture`] does is open `<home>/logs/start-<UTC>-<pid>.txt`
//! and point this process's stdout and stderr at it. From then on every line the engine
//! prints lands in a file beside the run log, whatever stage it dies at, and `DebugReport`
//! exports it with the rest.
//!
//! The file stays small: a start that gets as far as the run log writes a few lines here and
//! everything else there. The last [`KEEP`] of them are kept, by the same rule and in the
//! same folder as the run logs (`crash::old_files`).
//!
//! `[documented: dup2(2) replaces a file descriptor, so a write to 1 or 2 afterwards goes to
//! the file this opened; Android gives a process no stderr of its own unless something is
//! listening, which is why those lines were lost.]`

use std::path::Path;

/// What a start log's name begins with, beside `nvforever-` and `crash-` in the logs folder.
pub const PREFIX: &str = "start-";

/// What a start log's name ends with.
pub const EXT: &str = ".txt";

/// Start logs kept in the logs folder, the newest ones. Fewer than the run logs: one of
/// these is a few hundred bytes next to its run log, and only a failed start is worth
/// keeping more than the newest of.
pub const KEEP: usize = 8;

/// The first line of every start log, which is what tells a reader of the exported report
/// what these lines are.
pub const BANNER: &str = "NVForever start log: the engine's own output, from before the run log is open.";

/// The start logs to delete so that `keep` remain, given the names in the logs folder. The
/// same rule as the run logs': the name carries the time stamp, so they sort oldest first.
pub fn old_start_logs(names: &[String], keep: usize) -> Vec<String> {
    crate::crash::old_files(names, PREFIX, EXT, keep)
}

/// The name this run's start log takes.
pub fn file_name(stamp: &str, pid: u32) -> String {
    format!("{PREFIX}{stamp}-{pid}{EXT}")
}

/// Open this run's start log and send stdout and stderr to it. Call before anything that can
/// refuse the start; after it, `crash::install` opens the run log as it always did and the
/// two sit side by side.
///
/// Everything it can fail at is ignored on purpose: a start that cannot open a log file is
/// still a start, and this is a diagnostic.
#[cfg(target_os = "android")]
pub fn capture(home: &Path) {
    use std::io::Write;
    use std::os::fd::AsRawFd;

    let dir = fnv_install::EngineHome::new(home).logs();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let names: Vec<String> = std::fs::read_dir(&dir)
        .map(|r| r.flatten().filter_map(|e| e.file_name().into_string().ok()).collect())
        .unwrap_or_default();
    for old in old_start_logs(&names, KEEP.saturating_sub(1)) {
        let _ = std::fs::remove_file(dir.join(old));
    }
    let stamp = crate::crash::stamp(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    );
    let path = dir.join(file_name(&stamp, std::process::id()));
    let Ok(mut file) = std::fs::File::create(&path) else { return };
    let _ = writeln!(file, "{BANNER}\nstarted (UTC): {stamp}\nprocess: {}", std::process::id());
    let _ = file.flush();
    let fd = file.as_raw_fd();
    // SAFETY: `fd` is this process's own open file, and 1 and 2 are the two descriptors
    // being replaced; dup2 closes whatever they were first. Nothing else holds them: this
    // runs before any thread of the engine's starts.
    unsafe {
        libc::dup2(fd, 1);
        libc::dup2(fd, 2);
    }
    // The duplicates keep the file open after this handle goes.
    drop(file);
}

/// Nothing to capture where stderr already goes somewhere a person can read.
#[cfg(not(target_os = "android"))]
pub fn capture(home: &Path) {
    let _ = home;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oldest start logs go and nothing else does, by the same rule the run logs keep.
    #[test]
    fn the_oldest_start_logs_go_and_nothing_else() {
        let names: Vec<String> = vec![
            "start-20261127-100000-1.txt".into(),
            "start-20261127-110000-2.txt".into(),
            "start-20261127-120000-3.txt".into(),
            "nvforever-20261127-090000-1.log".into(),
            "crash-20261127-090000-1.txt".into(),
        ];
        assert_eq!(old_start_logs(&names, 2), vec!["start-20261127-100000-1.txt".to_string()]);
        assert_eq!(old_start_logs(&names, 3), Vec::<String>::new());
        assert_eq!(
            old_start_logs(&names, 0).len(),
            3,
            "a run log and a crash report are never one of these"
        );
    }

    /// The name carries the run's time and process, so the launcher can tell which start
    /// log belongs to which run log.
    #[test]
    fn the_name_says_when_and_which_process() {
        assert_eq!(file_name("20261127-120000", 42), "start-20261127-120000-42.txt");
        assert!(file_name("20261127-120000", 42).starts_with(PREFIX));
        assert!(file_name("20261127-120000", 42).ends_with(EXT));
    }
}
