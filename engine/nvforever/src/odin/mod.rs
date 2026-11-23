//! The Android fork's own code, in one module (docs/fork.md).
//!
//! `src/main.rs` is upstream's file: the crate root of both the desktop binary
//! (`src/desktop.rs`) and the Android shell (`engine/nvf-android`), and the fork touches it
//! in a few places only, so that a sync with upstream NVForever never has to merge 3,000
//! lines of it:
//!
//! 1. `pub mod odin;`, this module;
//! 2. [`args`] instead of `std::env::args()`, so a platform shell can hand the game a command
//!    line of its own (an Android activity has no argv);
//! 3. [`before_run`] just before `App::run`, where the platform shell's own plugins go.
//!
//! The entry points: [`desktop`] is what the desktop binary calls, [`run`] and [`run_with`]
//! what an Android activity calls (`nvf-android`'s `android_main`).

use bevy::app::{App, AppExit};
use std::sync::Mutex;

/// The command line [`run`] was given, read once by [`args`]. Unset for a desktop run, which
/// has its own argv.
static LINE: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// What a platform shell adds to the configured game, as [`run_with`] was given it.
type Extend = Box<dyn FnOnce(&mut App) + Send>;

/// What the platform shell wants added to the configured game, taken once by [`before_run`].
static EXTEND: Mutex<Option<Extend>> = Mutex::new(None);

/// The desktop program (`src/desktop.rs`): the game on this process's own command line.
pub fn desktop() -> AppExit {
    crate::main()
}

/// Start the game from a platform shell that has no argv of its own: an Android activity
/// (`engine/nvf-android`). `line` is the whole command line, program name first.
pub fn run(line: Vec<String>) -> AppExit {
    run_with(line, |_| {})
}

/// [`run`], letting the platform shell add its own plugins to the configured game before it
/// starts: Android's touch and controller input (`engine/nvf-android/src/lib.rs`). `extend`
/// runs last, after every plugin and system the game adds, and just before `App::run`.
pub fn run_with(line: Vec<String>, extend: impl FnOnce(&mut App) + Send + 'static) -> AppExit {
    *LINE.lock().expect("the engine starts once, from one thread") = Some(line);
    *EXTEND.lock().expect("the engine starts once, from one thread") = Some(Box::new(extend));
    crate::main()
}

/// The command line the run was started with: the one [`run`] was given, else this process's
/// own (a desktop run). Called by `main` before anything is parsed.
pub(crate) fn args() -> Vec<String> {
    let line = LINE.lock().expect("the engine starts once, from one thread").take();
    line.unwrap_or_else(|| std::env::args().collect())
}

/// The platform shell's own plugins, on the configured game just before it runs.
pub(crate) fn before_run(app: &mut App) {
    let extend = EXTEND.lock().expect("the engine starts once, from one thread").take();
    if let Some(extend) = extend {
        extend(app);
    }
}
