//! The process's own resident memory, for the recorder's summary line.
//!
//! A leak of ours shows as a number that only ever rises; an out-of-memory abort with the
//! GPU's own figures flat is the main memory going, and nothing in the run log said so
//! [design: the owner's Odin session of 2026-11-22, killed about 45 minutes in during a
//! load with GPU memory flat at about 1 GB].
//!
//! On Linux and Android it is `/proc/self/statm`'s second field, the resident set in
//! pages, times the page size [documented: proc(5), "/proc/pid/statm": size, resident,
//! shared, text, lib, data, dt, "in pages"; `sysconf(_SC_PAGESIZE)` for the size, since
//! 16 KB pages are an Android 15 option and this is read on a phone-class device]. The
//! parse is a pure function so it is tested on any machine.
//!
//! On Windows the figure is the working set `sysinfo` already reports for a process; the
//! recorder asks for this process alone, once every [`super::SUMMARY_SECS`], never per
//! frame. Anywhere else there is no reading and the summary simply leaves it out.

/// The resident set in bytes from the text of `/proc/self/statm` and the page size: the
/// second field, in pages. `None` for anything that is not two numbers.
///
/// Built where it is read and where it is tested: a Windows build that is not the test
/// build has no `/proc` to parse.
#[cfg(any(target_os = "linux", target_os = "android", test))]
pub fn rss_of_statm(text: &str, page_bytes: u64) -> Option<u64> {
    if page_bytes == 0 {
        return None;
    }
    let resident: u64 = text.split_ascii_whitespace().nth(1)?.parse().ok()?;
    resident.checked_mul(page_bytes)
}

/// This process's resident memory now, or `None` where the platform has no cheap reading.
pub fn rss_bytes() -> Option<u64> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // SAFETY: `sysconf` reads a value of the C library and touches nothing of ours.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let page = u64::try_from(page).ok()?;
        rss_of_statm(&std::fs::read_to_string("/proc/self/statm").ok()?, page)
    }
    #[cfg(windows)]
    {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
        let pid = Pid::from_u32(std::process::id());
        let mut sys = System::new();
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing().with_memory());
        sys.process(pid).map(sysinfo::Process::memory)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `/proc/self/statm` as the kernel writes it: seven numbers in pages, the resident set
    /// the second.
    #[test]
    fn the_resident_pages_are_the_second_field_of_statm() {
        // A real reading from an aarch64 Android build (4 KB pages): 1 234 567 pages
        // resident is about 4.7 GB — this one is 120 MB.
        assert_eq!(rss_of_statm("262144 30720 8192 42 0 61440 0\n", 4096), Some(30_720 * 4096));
        // 16 KB pages (an Android 15 device) count the same pages four times as large.
        assert_eq!(rss_of_statm("262144 30720 8192 42 0 61440 0\n", 16_384), Some(30_720 * 16_384));
        // No trailing newline, extra spaces, and more fields than proc(5) lists: still the
        // second number.
        assert_eq!(rss_of_statm("  10   20   30", 4096), Some(20 * 4096));
        assert_eq!(rss_of_statm("1 2 3 4 5 6 7 8 9", 1), Some(2));
    }

    /// Anything that is not a reading gives none at all rather than a wrong number.
    #[test]
    fn a_statm_that_cannot_be_read_gives_nothing() {
        for text in ["", "262144", "262144 \n", "a b c", "262144 -1 0", "262144 1.5 0"] {
            assert_eq!(rss_of_statm(text, 4096), None, "{text:?}");
        }
        // A page size of nothing, and one that would overflow the count.
        assert_eq!(rss_of_statm("1 2 3", 0), None);
        assert_eq!(rss_of_statm(&format!("1 {} 3", u64::MAX), 4096), None);
    }
}
