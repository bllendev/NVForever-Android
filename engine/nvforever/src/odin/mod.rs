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
//! | `crash.rs` | the allocator `SaysWhenEmpty` wraps | [`alloc::BACKEND`] |
//! | `crash.rs` | one dump when an allocation fails | [`proc_mem::dump_once`] |
//! | `touch/gyro.rs` | the settings row, and three terms of `poll` | [`controller_gyro`] |
//!
//! The Android-only parts of each module are behind `cfg(target_os = "android")`, while their
//! parsing and their wording are plain functions, built and tested everywhere.

/// Which allocator the game's allocations go to (`crash.rs`'s `SaysWhenEmpty` wraps it).
pub mod alloc;
/// The device's gyro while a physical controller is in use. It lives in `nvf-play`, because
/// `player.rs`'s look route is the other place it is hooked and that crate cannot see this one.
pub use nvf_play::odin::controller_gyro;
/// The Android home folder's name, which the fork keeps as earlier builds left it.
pub mod home;
/// The memory summary line in the Android run log (added to the Android plugin list only).
pub mod mem_log;
/// What this process's own memory is doing, and the dump on a failed allocation.
pub mod proc_mem;
