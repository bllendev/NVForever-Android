//! What the process's own memory is doing, on Android: the recorder's summary line, and one
//! dump when an allocation fails.
//!
//! **Why** (the owner's Odin session of 2026-11-24, `0.8.1-odin.31`): three minutes of play at
//! Primm, resident memory climbing 2583 -> 3993 MB, and then
//! `out of memory: an allocation of 67600 bytes failed` with about 10 GB of the phone's 15 GB
//! free. A 64 KB allocation that fails with that much RAM left is not the machine running out
//! of memory, so the figure the recorder already prints (`rss`) cannot say what happened. The
//! suspects each leave their own mark:
//!
//! - **the mapping limit.** Android's allocator (Scudo) serves anything over its largest
//!   primary size class from the secondary allocator, which `mmap`s each allocation with guard
//!   pages of its own; the graphics driver maps its buffers, and every library and archive is
//!   mapped too. `vm.max_map_count` (65530 by default) is a hard ceiling on the lot, and the
//!   `mmap` that crosses it fails whatever the machine has free [documented: proc(5),
//!   `/proc/sys/vm/max_map_count`, "the maximum number of memory map areas a process may
//!   have"]. So: the number of lines of `/proc/self/maps` against that file's number.
//! - **the allocator's own arenas.** `mallinfo`'s `uordblks` (bytes handed out) and `hblkhd`
//!   (bytes it has mapped) say whether the memory is ours and in use, or ours and merely held
//!   [documented: bionic's `malloc.h`, `mallinfo`; the fields are those of the GNU struct].
//! - **where the resident memory sits.** `/proc/self/status`'s `VmSize`, `VmRSS`, `RssAnon` and
//!   `RssFile`: anonymous memory is the heap's and the driver's, file-backed is the mapped
//!   libraries and archives [documented: proc(5), "/proc/pid/status"].
//!
//! **What is printed.** Every [`super::gpu_recorder::SUMMARY_SECS`] those figures join the
//! recorder's one-line summary ([`ProcMem::text`]). The first time an allocation fails
//! ([`dump_once`], called from the fork's one line in `crash.rs`) they are written again with
//! `/proc/self/maps` grouped by name, the [`TOP_GROUPS`] names with the most mappings first,
//! so the line that answers the question is in the run log the player exports.
//!
//! **Nothing here allocates on the failure path.** There is no memory to allocate with: the
//! buffers, the group table and the writer are all in place before the dump starts
//! ([`SCRATCH`], a static; `crash.rs`'s own line writer), the files are read with `open`,
//! `read` and `close` rather than `std::fs` (whose path handling allocates a `CString`), and
//! the output is written one `writeln!` at a time into the log's existing buffer.
//!
//! **Elsewhere.** On Windows and the desktop every reading is `None` and the dump writes
//! nothing; the parsing and the wording are plain functions, built and tested everywhere.

use std::io::Write;

/// Names kept apart in a dump's grouping. Past this a dump counts the rest together: the
//  table is a static of a fixed size, because the dump cannot allocate one.
pub const MAX_GROUPS: usize = 64;
/// How many characters of a group's name are kept.
const NAME_LEN: usize = 56;
/// How many groups a dump lists, the ones with the most mappings first.
pub const TOP_GROUPS: usize = 25;
/// The buffer `/proc` is read through, and the longest line a dump can read. Only the
/// platform that reads `/proc` has one.
#[cfg(target_os = "android")]
const BUF_BYTES: usize = 64 * 1024;

const KB: f64 = 1024.0;

/// A line's lead, so every line of this module's reads the same way in the run log.
const LEAD: &str = "android memory:";

/// What the process says of its own memory. Every field is `None` where it could not be read,
/// and every one of them is `None` off Android.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcMem {
    /// `/proc/self/status`, kB: the whole address space, the resident set, and the resident
    /// set's anonymous and file-backed parts.
    pub vm_size_kb: Option<u64>,
    pub vm_rss_kb: Option<u64>,
    pub rss_anon_kb: Option<u64>,
    pub rss_file_kb: Option<u64>,
    /// The lines of `/proc/self/maps`, and `/proc/sys/vm/max_map_count`.
    pub maps: Option<u64>,
    pub max_maps: Option<u64>,
    /// `mallinfo`: bytes the allocator has handed out, and bytes it has mapped.
    pub uordblks: Option<u64>,
    pub hblkhd: Option<u64>,
}

fn mb_of_kb(kb: u64) -> f64 {
    kb as f64 / KB
}

fn mb_of_bytes(bytes: u64) -> f64 {
    bytes as f64 / (KB * KB)
}

impl ProcMem {
    /// Whether anything at all was read.
    pub fn any(&self) -> bool {
        *self != Self::default()
    }

    /// The figures as one clause for the recorder's summary line, or `""` when nothing was
    /// read.
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(rss) = self.vm_rss_kb {
            let size = self.vm_size_kb.map_or_else(String::new, |s| format!("VmSize {:.0} MB, ", mb_of_kb(s)));
            let split = match (self.rss_anon_kb, self.rss_file_kb) {
                (Some(a), Some(f)) => format!(" (anon {:.0} MB, file {:.0} MB)", mb_of_kb(a), mb_of_kb(f)),
                _ => String::new(),
            };
            parts.push(format!("{size}VmRSS {:.0} MB{split}", mb_of_kb(rss)));
        }
        if let Some(maps) = self.maps {
            let most = self.max_maps.map_or_else(String::new, |m| format!(" of {m}"));
            parts.push(format!("{maps} mappings{most}"));
        }
        if let Some(used) = self.uordblks {
            let mapped = self.hblkhd.map_or_else(String::new, |h| format!(", mapped {:.0} MB", mb_of_bytes(h)));
            parts.push(format!("malloc in use {:.0} MB{mapped}", mb_of_bytes(used)));
        }
        parts.join("; ")
    }
}

/// `/proc/self/status`'s value for `key` in kB: the line `key:` and the number after it
/// [documented: proc(5), "/proc/pid/status", `VmRSS:      5461 kB`]. A key that only starts
/// the same way does not answer (`RssAnon` is not `Rss`).
pub fn status_kb(text: &str, key: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.strip_prefix(key).is_some_and(|rest| rest.starts_with(':')))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// `/proc/sys/vm/max_map_count`'s number.
pub fn max_map_count(text: &str) -> Option<u64> {
    text.trim().parse().ok()
}

/// The bytes a line of `/proc/self/maps` maps: its `start-end` address range
/// [documented: proc(5), "/proc/pid/maps"].
pub fn map_line_bytes(line: &str) -> Option<u64> {
    let range = line.split_whitespace().next()?;
    let (start, end) = range.split_once('-')?;
    let (start, end) = (u64::from_str_radix(start, 16).ok()?, u64::from_str_radix(end, 16).ok()?);
    end.checked_sub(start)
}

/// The name a line of `/proc/self/maps` ends with: everything after its five fixed fields
/// (the address range, the permissions, the offset, the device and the inode), which is a
/// path that may itself hold spaces, a label in brackets, or nothing at all.
pub fn map_line_name(line: &str) -> &str {
    let mut rest = line;
    for _ in 0..5 {
        rest = rest.trim_start();
        match rest.find(char::is_whitespace) {
            Some(i) => rest = &rest[i..],
            None => return "",
        }
    }
    rest.trim()
}

/// The group a mapping's name belongs to: the bracketed labels (`[anon:scudo:primary]`,
/// `[stack]`) as they are, the graphics driver's and memory files' many nodes together, a
/// file by its last path part (so a library's four segments count as that library's), and the
/// unnamed mappings, which are the ones no label explains, together.
pub fn group_name(name: &str) -> &str {
    if name.is_empty() {
        return "(unnamed)";
    }
    if name.starts_with('[') {
        return name;
    }
    if name.starts_with("/dev/kgsl") {
        return "/dev/kgsl* (the graphics driver)";
    }
    if name.starts_with("/memfd:") {
        return "/memfd:*";
    }
    name.rsplit('/').next().unwrap_or(name)
}

/// `/proc/self/maps` grouped by [`group_name`]: how many mappings each name has and how many
/// bytes. A fixed table of [`MAX_GROUPS`] names, because the dump that fills it cannot
/// allocate one; names past that are counted together.
#[derive(Clone)]
pub struct Groups {
    names: [[u8; NAME_LEN]; MAX_GROUPS],
    lens: [u8; MAX_GROUPS],
    counts: [u64; MAX_GROUPS],
    bytes: [u64; MAX_GROUPS],
    used: usize,
    /// The mappings whose name did not fit the table.
    other_count: u64,
    other_bytes: u64,
    /// Every mapping counted, and their bytes.
    total: u64,
    total_bytes: u64,
}

impl Default for Groups {
    fn default() -> Self {
        Self::new()
    }
}

impl Groups {
    pub const fn new() -> Self {
        Self {
            names: [[0; NAME_LEN]; MAX_GROUPS],
            lens: [0; MAX_GROUPS],
            counts: [0; MAX_GROUPS],
            bytes: [0; MAX_GROUPS],
            used: 0,
            other_count: 0,
            other_bytes: 0,
            total: 0,
            total_bytes: 0,
        }
    }

    /// Count one line of `/proc/self/maps`.
    pub fn add(&mut self, line: &str) {
        let bytes = map_line_bytes(line).unwrap_or(0);
        self.total += 1;
        self.total_bytes += bytes;
        let name = group_name(map_line_name(line)).as_bytes();
        let kept = &name[..name.len().min(NAME_LEN)];
        for i in 0..self.used {
            if &self.names[i][..usize::from(self.lens[i])] == kept {
                self.counts[i] += 1;
                self.bytes[i] += bytes;
                return;
            }
        }
        if self.used == MAX_GROUPS {
            self.other_count += 1;
            self.other_bytes += bytes;
            return;
        }
        let i = self.used;
        self.names[i][..kept.len()].copy_from_slice(kept);
        self.lens[i] = kept.len() as u8;
        self.counts[i] = 1;
        self.bytes[i] = bytes;
        self.used = i + 1;
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn group(&self, i: usize) -> (&str, u64, u64) {
        let name = std::str::from_utf8(&self.names[i][..usize::from(self.lens[i])]).unwrap_or("(not text)");
        (name, self.counts[i], self.bytes[i])
    }

    /// The indices of the groups with the most mappings, most first, into `out`; how many were
    /// filled. No allocation: `out` is the caller's, and the pick is a scan a place.
    pub fn top(&self, out: &mut [usize]) -> usize {
        let mut filled = 0;
        while filled < out.len() {
            let mut best: Option<usize> = None;
            for i in 0..self.used {
                if out[..filled].contains(&i) {
                    continue;
                }
                let better = best.is_none_or(|b| (self.counts[i], self.bytes[i]) > (self.counts[b], self.bytes[b]));
                if better {
                    best = Some(i);
                }
            }
            match best {
                Some(i) => {
                    out[filled] = i;
                    filled += 1;
                }
                None => break,
            }
        }
        filled
    }
}

/// The dump's text, written a line at a time into `w`: the figures, then the mappings grouped
/// by name with the [`TOP_GROUPS`] busiest first, then what the allocator says. `size` is the
/// allocation that failed. Nothing here allocates, so it is what the failure path can use.
pub fn write_dump(w: &mut dyn Write, size: usize, mem: &ProcMem, groups: &Groups) -> std::io::Result<()> {
    writeln!(w, "{LEAD} an allocation of {size} bytes failed; this process's own memory follows")?;
    match mem.vm_rss_kb {
        Some(r) => writeln!(w, "{LEAD} VmSize {}, VmRSS {:.0} MB (anon {}, file {})", kb_mb(mem.vm_size_kb), mb_of_kb(r), kb_mb(mem.rss_anon_kb), kb_mb(mem.rss_file_kb))?,
        None => writeln!(w, "{LEAD} /proc/self/status could not be read")?,
    }
    match (mem.maps, mem.max_maps) {
        (Some(n), Some(most)) => {
            let share = n as f64 / most.max(1) as f64 * 100.0;
            writeln!(w, "{LEAD} {n} mappings, {share:.0} % of the {most} this process may have (/proc/sys/vm/max_map_count): an mmap past that fails however much memory is free")?;
        }
        (Some(n), None) => writeln!(w, "{LEAD} {n} mappings; /proc/sys/vm/max_map_count could not be read")?,
        _ => writeln!(w, "{LEAD} /proc/self/maps could not be read")?,
    }
    match (mem.uordblks, mem.hblkhd) {
        (Some(used), Some(mapped)) => writeln!(w, "{LEAD} mallinfo: {:.0} MB handed out, {:.0} MB mapped by the allocator (uordblks, hblkhd)", mb_of_bytes(used), mb_of_bytes(mapped))?,
        _ => writeln!(w, "{LEAD} mallinfo: not read")?,
    }
    if groups.total == 0 {
        writeln!(w, "{LEAD} no mappings were read, so there is nothing to group")?;
        return Ok(());
    }
    writeln!(
        w,
        "{LEAD} /proc/self/maps by name, the {TOP_GROUPS} with the most mappings first ({} mappings, {:.0} MB in all):",
        groups.total,
        mb_of_bytes(groups.total_bytes)
    )?;
    let mut top = [0usize; TOP_GROUPS];
    let filled = groups.top(&mut top);
    for &i in &top[..filled] {
        let (name, count, bytes) = groups.group(i);
        writeln!(w, "{LEAD}   {count:>7} {:>9.0} MB  {name}", mb_of_bytes(bytes))?;
    }
    if groups.other_count > 0 {
        writeln!(
            w,
            "{LEAD}   {:>7} {:>9.0} MB  (every further name, past the {MAX_GROUPS} the dump keeps apart)",
            groups.other_count,
            mb_of_bytes(groups.other_bytes)
        )?;
    }
    Ok(())
}

/// A kB reading as MB, or that it was not read.
fn kb_mb(kb: Option<u64>) -> String {
    kb.map_or_else(|| "not read".to_string(), |kb| format!("{:.0} MB", mb_of_kb(kb)))
}

/// The buffer `/proc` is read through and the group table the dump fills: in place before the
/// failure, because the failure path has no memory to make them with. About 70 kB of this
/// program's own data, touched once every [`super::gpu_recorder::SUMMARY_SECS`] and once more
/// if an allocation ever fails.
#[cfg(target_os = "android")]
static SCRATCH: std::sync::Mutex<Scratch> = std::sync::Mutex::new(Scratch { buf: [0; BUF_BYTES], groups: Groups::new() });

#[cfg(target_os = "android")]
struct Scratch {
    buf: [u8; BUF_BYTES],
    groups: Groups,
}

/// Read `path` (a nul-terminated byte string) into `buf`: what was filled. `open`, `read` and
/// `close` rather than `std::fs`, whose path handling allocates a `CString` and whose reader
/// grows a `Vec`; this is called where there may be no memory to allocate with.
#[cfg(target_os = "android")]
fn read_fixed(path: &[u8], buf: &mut [u8]) -> Option<usize> {
    // SAFETY: `path` is nul-terminated, and the call only opens a file for reading.
    let fd = unsafe { libc::open(path.as_ptr().cast(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    let mut filled = 0;
    while filled < buf.len() {
        // SAFETY: the descriptor is open and `buf[filled..]` is ours to write, that long.
        let n = unsafe { libc::read(fd, buf[filled..].as_mut_ptr().cast(), buf.len() - filled) };
        if n <= 0 {
            break;
        }
        filled += n as usize;
    }
    // SAFETY: the descriptor this call opened, closed once.
    unsafe { libc::close(fd) };
    Some(filled)
}

/// Every line of `/proc/self/maps`, through `buf` (a line longer than it is skipped); whether
/// the file could be read at all. Nothing is allocated: a `/proc/self/maps` of 65,530 lines is
/// megabytes, which is exactly what the failure path cannot ask for.
#[cfg(target_os = "android")]
fn for_each_map_line(buf: &mut [u8], mut line: impl FnMut(&str)) -> bool {
    // SAFETY: a nul-terminated path, opened for reading.
    let fd = unsafe { libc::open(b"/proc/self/maps\0".as_ptr().cast(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return false;
    }
    let mut filled = 0usize;
    loop {
        // SAFETY: the descriptor is open and `buf[filled..]` is ours to write, that long.
        let n = unsafe { libc::read(fd, buf[filled..].as_mut_ptr().cast(), buf.len() - filled) };
        if n < 0 {
            break;
        }
        if n == 0 {
            if filled > 0 {
                if let Ok(text) = std::str::from_utf8(&buf[..filled]) {
                    line(text);
                }
            }
            break;
        }
        filled += n as usize;
        let mut start = 0;
        while let Some(i) = buf[start..filled].iter().position(|&c| c == b'\n') {
            let end = start + i;
            if let Ok(text) = std::str::from_utf8(&buf[start..end]) {
                line(text);
            }
            start = end + 1;
        }
        buf.copy_within(start..filled, 0);
        filled -= start;
        // A line that cannot fit the buffer: start again after it rather than stall.
        if filled == buf.len() {
            filled = 0;
        }
    }
    // SAFETY: the descriptor this call opened, closed once.
    unsafe { libc::close(fd) };
    true
}

/// `mallinfo`'s bytes handed out and bytes mapped [documented: bionic `malloc.h`].
#[cfg(target_os = "android")]
fn mallinfo() -> (Option<u64>, Option<u64>) {
    // SAFETY: the call reads the allocator's own counters and touches nothing of ours.
    let info = unsafe { libc::mallinfo() };
    (Some(info.uordblks as u64), Some(info.hblkhd as u64))
}

/// This process's own memory now: one `/proc/self/status` read, one pass over
/// `/proc/self/maps` to count its lines, `/proc/sys/vm/max_map_count` and `mallinfo`. Called
/// once every [`super::gpu_recorder::SUMMARY_SECS`], never per frame. `None` off Android and
/// while the dump holds the buffer.
pub fn sample() -> Option<ProcMem> {
    #[cfg(target_os = "android")]
    {
        let mut held = SCRATCH.try_lock().ok()?;
        let scratch = &mut *held;
        let mut mem = ProcMem::default();
        if let Some(n) = read_fixed(b"/proc/self/status\0", &mut scratch.buf[..4096]) {
            if let Ok(text) = std::str::from_utf8(&scratch.buf[..n]) {
                mem.vm_size_kb = status_kb(text, "VmSize");
                mem.vm_rss_kb = status_kb(text, "VmRSS");
                mem.rss_anon_kb = status_kb(text, "RssAnon");
                mem.rss_file_kb = status_kb(text, "RssFile");
            }
        }
        let mut count = 0u64;
        if for_each_map_line(&mut scratch.buf, |_| count += 1) {
            mem.maps = Some(count);
        }
        if let Some(n) = read_fixed(b"/proc/sys/vm/max_map_count\0", &mut scratch.buf[..64]) {
            mem.max_maps = std::str::from_utf8(&scratch.buf[..n]).ok().and_then(max_map_count);
        }
        (mem.uordblks, mem.hblkhd) = mallinfo();
        mem.any().then_some(mem)
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Set by the first allocation failure to write the dump, so there is one of it.
static DUMPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One dump of this process's own memory into `w`, the first time an allocation fails: the
/// fork's one line in `crash.rs`, which calls this while it holds the run log's writer. Writes
/// nothing off Android, nothing a second time, and nothing it would have to allocate for.
/// `crash.rs` lets four failures write their line, so a failure that arrives while the
/// recorder holds the buffer has not spent the one dump (below).
pub fn dump_once(w: &mut dyn Write, size: usize) {
    #[cfg(target_os = "android")]
    {
        use std::sync::atomic::Ordering;
        // The buffer first, the "done" flag after it: a failure that cannot have the buffer
        // (the recorder's reading holds it for a moment) leaves the next failure its turn
        // rather than spending the one dump on nothing. The lock orders two threads, and the
        // second of them finds the flag set.
        let Ok(mut held) = SCRATCH.try_lock() else { return };
        if DUMPED.swap(true, Ordering::SeqCst) {
            return;
        }
        let scratch = &mut *held;
        let mut mem = ProcMem::default();
        if let Some(n) = read_fixed(b"/proc/self/status\0", &mut scratch.buf[..4096]) {
            if let Ok(text) = std::str::from_utf8(&scratch.buf[..n]) {
                mem.vm_size_kb = status_kb(text, "VmSize");
                mem.vm_rss_kb = status_kb(text, "VmRSS");
                mem.rss_anon_kb = status_kb(text, "RssAnon");
                mem.rss_file_kb = status_kb(text, "RssFile");
            }
        }
        if let Some(n) = read_fixed(b"/proc/sys/vm/max_map_count\0", &mut scratch.buf[..64]) {
            mem.max_maps = std::str::from_utf8(&scratch.buf[..n]).ok().and_then(max_map_count);
        }
        (mem.uordblks, mem.hblkhd) = mallinfo();
        scratch.groups = Groups::new();
        let (buf, groups) = (&mut scratch.buf, &mut scratch.groups);
        if for_each_map_line(buf, |line| groups.add(line)) {
            mem.maps = Some(groups.total());
        }
        let _ = write_dump(w, size, &mem, groups);
        let _ = w.flush();
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (w, size, &DUMPED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `/proc/self/status` as the kernel writes it, and the four keys the summary wants. A key
    /// that only starts the same way is not the key.
    #[test]
    fn the_status_keys_are_read_in_kb() {
        let text = "Name:\tnvforever\nVmPeak:\t 9437184 kB\nVmSize:\t 9340928 kB\nVmRSS:\t 4089344 kB\n\
                    RssAnon:\t 3389440 kB\nRssFile:\t  699392 kB\nRssShmem:\t     512 kB\nThreads:\t24\n";
        assert_eq!(status_kb(text, "VmSize"), Some(9_340_928));
        assert_eq!(status_kb(text, "VmRSS"), Some(4_089_344));
        assert_eq!(status_kb(text, "RssAnon"), Some(3_389_440));
        assert_eq!(status_kb(text, "RssFile"), Some(699_392));
        // `Vm` and `Rss` are prefixes of real keys, not keys; a missing key answers nothing.
        assert_eq!(status_kb(text, "Vm"), None);
        assert_eq!(status_kb(text, "Rss"), None);
        assert_eq!(status_kb(text, "VmSwap"), None);
        assert_eq!(status_kb("VmRSS:\n", "VmRSS"), None);
        assert_eq!(status_kb("VmRSS:\tlots kB\n", "VmRSS"), None);
        assert_eq!(max_map_count("65530\n"), Some(65_530));
        assert_eq!(max_map_count(""), None);
    }

    /// A line of `/proc/self/maps`: its address range as bytes, and the name after the five
    /// fixed fields, which may hold spaces, be a bracketed label, or be absent.
    #[test]
    fn a_maps_line_gives_its_size_and_its_name() {
        let anon = "7f8a2c0000-7f8a2c1000 rw-p 00000000 00:00 0                              [anon:scudo:primary]";
        assert_eq!(map_line_bytes(anon), Some(4096));
        assert_eq!(map_line_name(anon), "[anon:scudo:primary]");
        let unnamed = "7f8a300000-7f8a700000 rw-p 00000000 00:00 0 ";
        assert_eq!(map_line_bytes(unnamed), Some(4 * 1024 * 1024));
        assert_eq!(map_line_name(unnamed), "");
        // A path with spaces in it, which is why the name is the rest of the line.
        let bsa = "7a00000000-7a10000000 r--s 00000000 fe:09 1234   /storage/75D7-DC5F/NVForever/Data/Fallout - Textures.bsa";
        assert_eq!(map_line_name(bsa), "/storage/75D7-DC5F/NVForever/Data/Fallout - Textures.bsa");
        assert_eq!(map_line_bytes(bsa), Some(256 * 1024 * 1024));
        let deleted = "7000000000-7000001000 r-xp 00000000 fe:09 99   /data/app/lib/libnvf_android.so (deleted)";
        assert_eq!(map_line_name(deleted), "/data/app/lib/libnvf_android.so (deleted)");
        // The kernel's own high mapping, and lines that are not lines.
        assert_eq!(map_line_bytes("ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0  [vsyscall]"), Some(4096));
        assert_eq!(map_line_bytes(""), None);
        assert_eq!(map_line_bytes("not-a-range rw-p"), None);
        assert_eq!(map_line_bytes("7f8a2c1000-7f8a2c0000 rw-p 0 00:00 0"), None, "an end before its start");
        assert_eq!(map_line_name("7f8a2c0000-7f8a2c1000 rw-p 00000000 00:00"), "", "five fields and no name");
    }

    /// The groups: a bracketed label as it is, the driver's and memory files' many nodes
    /// together, a file by its last part, and the unnamed mappings together.
    #[test]
    fn the_groups_keep_the_labels_apart_and_the_drivers_nodes_together() {
        assert_eq!(group_name("[anon:scudo:secondary]"), "[anon:scudo:secondary]");
        assert_eq!(group_name("[stack]"), "[stack]");
        assert_eq!(group_name(""), "(unnamed)");
        assert_eq!(group_name("/dev/kgsl-3d0"), "/dev/kgsl* (the graphics driver)");
        assert_eq!(group_name("/memfd:jit-cache (deleted)"), "/memfd:*");
        assert_eq!(group_name("/data/app/~~x==/lib/arm64/libnvf_android.so"), "libnvf_android.so");
        assert_eq!(group_name("/storage/75D7-DC5F/NVForever/Data/Fallout - Textures.bsa"), "Fallout - Textures.bsa");
    }

    fn maps_sample() -> String {
        let mut text = String::new();
        // 300 secondary allocations of 64 KB each, every one its own mapping: what Android's
        // allocator does with anything over its largest primary size class.
        for i in 0..300u64 {
            let start = 0x7f_0000_0000u64 + i * 0x20000;
            text += &format!("{start:x}-{:x} rw-p 00000000 00:00 0   [anon:scudo:secondary]\n", start + 0x10000);
        }
        for i in 0..40u64 {
            let start = 0x7e_0000_0000u64 + i * 0x200000;
            text += &format!("{start:x}-{:x} rw-p 00000000 00:00 0   [anon:scudo:primary]\n", start + 0x100000);
        }
        for i in 0..12u64 {
            let start = 0x7d_0000_0000u64 + i * 0x400000;
            text += &format!("{start:x}-{:x} rw-s 00000000 00:0a 42   /dev/kgsl-3d0\n", start + 0x200000);
        }
        text += "7c00000000-7c00004000 r-xp 00000000 fe:09 7   /data/app/lib/arm64/libnvf_android.so\n";
        text += "7b00000000-7b10000000 r--s 00000000 fe:09 8   /storage/75D7-DC5F/NVForever/Data/Fallout - Textures.bsa\n";
        text += "7a00000000-7a00400000 rw-p 00000000 00:00 0 \n";
        text
    }

    #[test]
    fn the_maps_group_by_name_with_counts_and_sizes_and_the_busiest_first() {
        let mut groups = Groups::new();
        for line in maps_sample().lines() {
            groups.add(line);
        }
        assert_eq!(groups.total(), 300 + 40 + 12 + 3);
        let mut top = [0usize; TOP_GROUPS];
        let filled = groups.top(&mut top);
        assert_eq!(filled, 6, "six names in the sample");
        let seen: Vec<(String, u64, u64)> = top[..filled].iter().map(|&i| {
            let (name, count, bytes) = groups.group(i);
            (name.to_string(), count, bytes)
        }).collect();
        assert_eq!(seen[0], ("[anon:scudo:secondary]".to_string(), 300, 300 * 0x10000));
        assert_eq!(seen[1], ("[anon:scudo:primary]".to_string(), 40, 40 * 0x100000));
        assert_eq!(seen[2], ("/dev/kgsl* (the graphics driver)".to_string(), 12, 12 * 0x200000));
        // The three single mappings, the largest first.
        assert_eq!(seen[3].0, "Fallout - Textures.bsa");
        assert_eq!(seen[4], ("(unnamed)".to_string(), 1, 0x400000));
        assert_eq!(seen[5].0, "libnvf_android.so");
    }

    /// Past the table's size the rest is counted together rather than lost or grown into.
    #[test]
    fn names_past_the_tables_size_are_counted_together() {
        let mut groups = Groups::new();
        for i in 0..(MAX_GROUPS as u64 + 7) {
            let start = 0x7f_0000_0000u64 + i * 0x2000;
            groups.add(&format!("{start:x}-{:x} r--p 0 fe:09 {i}   /lib/lib{i}.so", start + 0x1000));
        }
        assert_eq!(groups.total(), MAX_GROUPS as u64 + 7);
        let mut top = [0usize; TOP_GROUPS];
        assert_eq!(groups.top(&mut top), TOP_GROUPS);
        let mut out = Vec::new();
        write_dump(&mut out, 65536, &ProcMem::default(), &groups).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("(every further name, past the 64 the dump keeps apart)"), "{text}");
        // A name longer than the table's room is kept short rather than refused.
        let long = "a".repeat(NAME_LEN + 20);
        let start = 0x10_0000u64;
        groups.add(&format!("{start:x}-{:x} r--p 0 fe:09 1   /lib/{long}", start + 0x1000));
        assert_eq!(groups.total(), MAX_GROUPS as u64 + 8);
    }

    /// The summary's clause: the four status figures, the mappings against the limit, and the
    /// allocator's own, with nothing said of what was not read.
    #[test]
    fn the_summary_clause_says_the_mappings_against_their_limit() {
        let mem = ProcMem {
            vm_size_kb: Some(9_340_928),
            vm_rss_kb: Some(4_088_832),
            rss_anon_kb: Some(3_389_440),
            rss_file_kb: Some(699_392),
            maps: Some(48_213),
            max_maps: Some(65_530),
            uordblks: Some(1_489_076_224),
            hblkhd: Some(1_994_391_552),
        };
        assert_eq!(
            mem.text(),
            "VmSize 9122 MB, VmRSS 3993 MB (anon 3310 MB, file 683 MB); 48213 mappings of 65530; malloc in use 1420 MB, mapped 1902 MB"
        );
        assert!(mem.any());
        assert_eq!(ProcMem::default().text(), "", "nothing read: nothing said");
        assert!(!ProcMem::default().any());
        // Only some of it read: the parts that were.
        let some = ProcMem { vm_rss_kb: Some(1024), maps: Some(400), ..ProcMem::default() };
        assert_eq!(some.text(), "VmRSS 1 MB; 400 mappings");
    }

    /// The dump an allocation failure writes: the failing size, the mapping limit with the
    /// share of it used, the allocator's figures, and the grouped mappings.
    #[test]
    fn the_dump_names_the_failing_size_the_limit_and_the_busiest_names() {
        let mem = ProcMem {
            vm_size_kb: Some(9_340_928),
            vm_rss_kb: Some(4_088_832),
            rss_anon_kb: Some(3_389_440),
            rss_file_kb: Some(699_392),
            maps: Some(355),
            max_maps: Some(65_530),
            uordblks: Some(1_489_076_224),
            hblkhd: Some(1_994_391_552),
        };
        let mut groups = Groups::new();
        for line in maps_sample().lines() {
            groups.add(line);
        }
        let mut out = Vec::new();
        write_dump(&mut out, 67_600, &mem, &groups).unwrap();
        let text = String::from_utf8(out).unwrap();
        for want in [
            "an allocation of 67600 bytes failed",
            "VmSize 9122 MB, VmRSS 3993 MB (anon 3310 MB, file 683 MB)",
            "355 mappings, 1 % of the 65530 this process may have",
            "mallinfo: 1420 MB handed out, 1902 MB mapped by the allocator",
            "/proc/self/maps by name, the 25 with the most mappings first (355 mappings,",
            "[anon:scudo:secondary]",
            "/dev/kgsl* (the graphics driver)",
        ] {
            assert!(text.contains(want), "{want} missing from\n{text}");
        }
        // Every line says whose it is, so the run log reads as one thing.
        assert!(text.lines().all(|l| l.starts_with(LEAD)), "{text}");
        // Nothing read at all: the dump still says the size and why there is nothing else.
        let mut out = Vec::new();
        write_dump(&mut out, 4096, &ProcMem::default(), &Groups::new()).unwrap();
        let text = String::from_utf8(out).unwrap();
        for want in ["an allocation of 4096 bytes failed", "/proc/self/status could not be read", "/proc/self/maps could not be read", "mallinfo: not read", "nothing to group"] {
            assert!(text.contains(want), "{want} missing from\n{text}");
        }
    }

    /// Off Android there is nothing to read and nothing to write: the desktop game pays none
    /// of this.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn nothing_is_read_or_written_off_android() {
        assert_eq!(sample(), None);
        let mut out = Vec::new();
        dump_once(&mut out, 65536);
        assert!(out.is_empty());
    }
}
