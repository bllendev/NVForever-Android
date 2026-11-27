//! Everything the Android fork adds to the engine, in files of its own.
//!
//! The fork (`bllendev/NVForever-Android`) is a short stack of commits replayed onto
//! upstream's `main`, so the cost of carrying it is the number of upstream lines it changes.
//! The rule (docs/fork.md): the fork's code lives here, and an upstream file gets a hook of a
//! line or two that calls into it. A hook that grows into a body is moved back here.
//!
//! The hooks this module is reached from:
//!
//! | upstream file | hook | what it reaches |
//! | --- | --- | --- |
//! | `main.rs` | `pub mod odin;` | this module |
//! | `main.rs` | the Android plugin list | [`mem_log::MemoryLogPlugin`] |
//! | `android.rs` | the home folder's name | [`home::DIR`] |
//! | `android.rs` | the launcher's crash-test switches | [`crash_tests::apply`] |
//! | `crash.rs` | the allocator `SaysWhenEmpty` wraps | [`alloc::BACKEND`] |
//! | `crash.rs` | one dump when an allocation fails | [`proc_mem::dump_once`] |
//! | `touch/gyro.rs` | the settings row, and three terms of `poll` | [`controller_gyro`] |
//! | `main.rs` | the play branch's plugin list | [`wrist_screen::WristScreenPlugin`] |
//! | `vats_highlight.rs`, `particles.rs`, `effect_particles.rs`, `decals.rs`, `heat_haze.rs`, `pipboy_arm.rs` | one gate each | [`crash_tests`] |
//!
//! The Android-only parts of each module are behind `cfg(target_os = "android")`, while their
//! parsing and their wording are plain functions, built and tested everywhere.

/// Which allocator the game's allocations go to (`crash.rs`'s `SaysWhenEmpty` wraps it).
pub mod alloc;
/// The device's gyro while a physical controller is in use. It lives in `nvf-play`, because
/// `player.rs`'s look route is the other place it is hooked and that crate cannot see this one.
pub use nvf_play::odin::controller_gyro;
/// The crash-test switches the launcher's Advanced page offers: one table, one reader.
pub mod crash_tests;
/// The Android home folder's name, which the fork keeps as earlier builds left it.
pub mod home;
/// The memory summary line in the Android run log (added to the Android plugin list only).
pub mod mem_log;
/// What this process's own memory is doing, and the dump on a failed allocation.
pub mod proc_mem;
/// The live glance at the Pip-Boy: the screen drawn on the arm while the game runs. It
/// lives beside the rest of nvforever's modules because it reaches deep into them
/// (`player_body`, `pipboy_controls`), and is declared from here so the fork's own code is
/// all one list.
#[path = "../wrist_screen.rs"]
pub mod wrist_screen;

/// Whether `name` is set at all, whatever it holds: how a switch that stands for "do none of
/// this" is read, since the launcher only ever writes it as `1`.
pub fn env_set(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

/// Whether `name` is set to anything but nothing or `0`: how a switch a person may type a
/// value into is read.
///
/// Not the same question as [`env_set`]: `NVF_X=0` is off here and on there. The two are kept
/// apart rather than unified because each variable's own meaning is what a run depends on.
pub fn env_on(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v.trim() != "0" && !v.trim().is_empty())
}

/// Whether `name` is set to exactly `0`: how a switch that turns something *off* that is
/// otherwise on is read. Unset is not off.
pub fn env_off(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v.trim() == "0")
}
