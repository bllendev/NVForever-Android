//! What the Android fork adds to the play layer, in files of its own (docs/fork.md).
//!
//! The fork is a short stack of commits replayed onto upstream's `main`, so its cost is the
//! number of upstream lines it changes. The rule: the fork's code lives here, and an upstream
//! file gets a hook of a line or two that calls into it.
//!
//! The hooks this module is reached from:
//!
//! | upstream file | hook | what it reaches |
//! | --- | --- | --- |
//! | `lib.rs` | `pub mod odin;` | this module |
//! | `player.rs` | which input device may mix sensor radians | [`controller_gyro::look_allows`] |
//! | `nvforever/src/touch/gyro.rs` | the settings row, and three terms of `poll` | [`controller_gyro`] |

/// Letting the device's gyro add to the look while a physical controller is in use.
pub mod controller_gyro;
