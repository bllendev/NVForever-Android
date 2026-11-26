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
//! | `main.rs` | `mod odin;` | this module |
//! | `android.rs` | the home folder's name | [`home::DIR`] |
//!
//! The Android-only parts of each module are behind `cfg(target_os = "android")`, while their
//! parsing and their wording are plain functions, built and tested everywhere.

/// The Android home folder's name, which the fork keeps as earlier builds left it.
pub mod home;
