//! Which allocator the game's own allocations go to, chosen when the game is built.
//!
//! Upstream's `crash.rs` wraps one allocator in `SaysWhenEmpty`, which logs an allocation the
//! allocator could not give (#2755). What it wraps is [`BACKEND`]: the system's allocator
//! everywhere, which on Android is bionic's **Scudo**, or **mimalloc** in an
//! `odin-mimalloc` build of the Android library.
//!
//! **Why there is a choice** (the owner's Odin session of 2026-11-24): a 67,600-byte
//! allocation failed with about 10 GB of the phone's 15 GB free. Scudo serves anything over
//! its largest primary size class from its secondary allocator, which `mmap`s each allocation
//! with guard pages of its own, so a long session's large allocations can run a process out of
//! *mappings* (`vm.max_map_count`, 65,530) long before it runs out of memory; and what it
//! frees it tends to keep rather than return. mimalloc takes its memory in large arenas and
//! hands pieces of them out, so the same play would make orders of magnitude fewer mappings.
//! Whether that is what happened is what `proc_mem`'s figures answer, so **this is an
//! experiment, not a fix**, and it is off: the default build is Scudo, exactly as before.
//!
//! **Why it is not a launcher switch.** `#[global_allocator]` is resolved when the program is
//! linked, so a run cannot change its allocator; there is no file key and no environment
//! variable for it, and the launcher offers none. The choice is the cargo feature
//! `odin-mimalloc` on `nvf-android` (which turns on `nvforever/odin-mimalloc`), and the
//! Android CI build leaves it off (docs/fork.md).
//!
//! **Not verified on a device yet.** mimalloc builds a C library, so it needs the NDK's
//! compiler for `aarch64-linux-android`; no NDK was available where this was written, so the
//! feature has never been compiled. Turn it on in CI only once a real build of
//! `nvf-android --target aarch64-linux-android --features odin-mimalloc` has been seen to
//! finish (docs/fork.md).

/// The allocator every allocation of the game's goes to, under `crash::SaysWhenEmpty`.
#[cfg(all(target_os = "android", feature = "odin-mimalloc"))]
pub type Backend = mimalloc::MiMalloc;
/// The allocator every allocation of the game's goes to, under `crash::SaysWhenEmpty`.
#[cfg(not(all(target_os = "android", feature = "odin-mimalloc")))]
pub type Backend = std::alloc::System;

/// [`Backend`] itself, as `SaysWhenEmpty` holds no state of its own.
#[cfg(all(target_os = "android", feature = "odin-mimalloc"))]
pub const BACKEND: Backend = mimalloc::MiMalloc;
/// [`Backend`] itself, as `SaysWhenEmpty` holds no state of its own.
#[cfg(not(all(target_os = "android", feature = "odin-mimalloc")))]
pub const BACKEND: Backend = std::alloc::System;

/// What the run log says this build's allocator is, in the one startup line
/// (the recorder's summary carries the figures; this says what made them).
pub const NAME: &str = if cfg!(all(target_os = "android", feature = "odin-mimalloc")) {
    "mimalloc (the odin-mimalloc build)"
} else if cfg!(target_os = "android") {
    "the system's (bionic's Scudo)"
} else {
    "the system's"
};

#[cfg(test)]
mod tests {
    /// The default build is the system's allocator, and says so: the experiment is off unless
    /// a build asks for it.
    #[test]
    fn the_default_build_keeps_the_systems_allocator() {
        assert!(!super::NAME.contains("mimalloc") || cfg!(feature = "odin-mimalloc"));
        #[cfg(not(feature = "odin-mimalloc"))]
        assert_eq!(super::NAME, if cfg!(target_os = "android") { "the system's (bionic's Scudo)" } else { "the system's" });
    }
}
