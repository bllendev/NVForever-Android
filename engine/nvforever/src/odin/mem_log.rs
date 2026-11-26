//! One line of the run log every [`SUMMARY_SECS`] on Android: what this process's own memory
//! is doing ([`super::proc_mem`]), and which allocator made those figures
//! ([`super::alloc`]).
//!
//! **Why it is not a setting.** The owner's 2026-11-24 session ended in a failed 67,600-byte
//! allocation on a phone with about 10 GB free, and the only reason anything is known about
//! it is that the figures were already in the run log. A diagnostic that has to be turned on
//! is a diagnostic that is off when the failure happens, so this is always on and free: one
//! `/proc/self/status` read, one pass over `/proc/self/maps` to count its lines, one
//! `mallinfo`, every ten seconds, through a buffer that is allocated once (`proc_mem`).
//!
//! Nothing is added to the desktop game: the plugin is only in the Android plugin list, and
//! `proc_mem::sample` answers `None` everywhere else in any case.

use bevy::prelude::*;

/// How often the summary line is written. Ten seconds is often enough to watch resident
/// memory climb through a loading screen, and rare enough that the reads cost nothing a
/// player could feel.
pub const SUMMARY_SECS: f64 = 10.0;

/// Whether this frame writes the line: how long ago the last one was written, or `None` when
/// none has been.
pub fn due(since_last: Option<std::time::Duration>) -> bool {
    since_last.is_none_or(|d| d.as_secs_f64() >= SUMMARY_SECS)
}

/// The fork's memory line in the Android run log (the plugin list in `main.rs`).
pub struct MemoryLogPlugin;

impl Plugin for MemoryLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, allocator_line);
        app.add_systems(Update, summary_line);
    }
}

/// Which allocator the figures below belong to, once (`alloc::NAME`).
fn allocator_line() {
    info!("android memory: this build's allocator is {}", super::alloc::NAME);
}

fn summary_line(mut last: Local<Option<std::time::Instant>>) {
    let now = std::time::Instant::now();
    if !due(last.map(|l| now.duration_since(l))) {
        return;
    }
    *last = Some(now);
    if let Some(mem) = super::proc_mem::sample() {
        info!("android memory: {}", mem.text());
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    /// The first frame writes a line, then one every ten seconds and no more often: a game at
    /// 60 frames a second must not read `/proc` 60 times.
    #[test]
    fn the_memory_line_is_written_once_every_ten_seconds() {
        assert_eq!(super::SUMMARY_SECS, 10.0);
        assert!(super::due(None), "the first frame says what the run started with");
        assert!(!super::due(Some(Duration::ZERO)));
        assert!(!super::due(Some(Duration::from_millis(9_999))));
        assert!(super::due(Some(Duration::from_secs(10))));
        assert!(super::due(Some(Duration::from_secs(600))), "a frame that took a long time still writes one line");
    }
}
