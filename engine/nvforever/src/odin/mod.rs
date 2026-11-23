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
//! 3. [`init`] before `DefaultPlugins` (what the Vulkan instance and device must be asked
//!    for before the renderer exists) and [`before_run`] just before `App::run` (the fork's
//!    own plugins, then the platform shell's).
//!
//! The entry points: [`desktop`] is what the desktop binary calls, [`run`] and [`run_with`]
//! what an Android activity calls (`nvf-android`'s `android_main`).

use bevy::app::{App, AppExit};
use std::sync::Mutex;

// The fork's engine parts stay in their own files beside main.rs, declared here rather than
// in it. `#[path]` so the files keep the names the docs and the log lines use.
#[path = "../gpu_recorder/mod.rs"]
pub mod gpu_recorder;
#[path = "../vulkan_validation.rs"]
pub mod vulkan_validation;
#[path = "../wrist_screen.rs"]
pub mod wrist_screen;

/// Set (any value) to skip the Pip-Boy's hidden warm-up while a load is covered: a crash
/// test the Android launcher offers, not a test aid (`pipboy_arm`).
pub const NO_PIPBOY_WARM_ENV: &str = "NVF_NO_PIPBOY_WARM";

/// Set (any value) to draw no V.A.T.S. part outline (no mask camera): a crash test the
/// Android launcher offers (`vats_highlight`). V.A.T.S. itself is unchanged.
pub const NO_VATS_OUTLINE_ENV: &str = "NVF_NO_VATS_OUTLINE";

/// Set (any value) to run with the Khronos Vulkan validation layer: a crash test the Android
/// launcher offers, much slower than a normal run (`vulkan_validation`).
pub const VULKAN_VALIDATION_ENV: &str = "NVF_VULKAN_VALIDATION";

/// What the platform shell (`nvf-android`) says it made of the validation layer's library,
/// for the one line the run log gets (`vulkan_validation`). Unset elsewhere.
pub const VULKAN_VALIDATION_LIB_ENV: &str = "NVF_VULKAN_VALIDATION_LIB";

/// Which of the validation layer's deeper checks to ask for on top of the core ones, as a
/// comma list of `sync` and `gpuav` (`1` is both): a crash test the Android launcher offers
/// in two switches of its own (`vulkan_validation`). Needs the validation switch as well.
pub const VULKAN_VALIDATION_DEEP_ENV: &str = "NVF_VULKAN_VALIDATION_DEEP";

/// Set (anything but `0`) to have every render-graph node write a GPU marker before and
/// after its work, so a lost device names the node the GPU hung in: a crash test the Android
/// launcher offers (`gpu_recorder::breadcrumbs`). Read in the vendored `bevy_render`
/// (`bevy::render::gpu_breadcrumbs`) as well, which is where the markers are recorded.
pub const GPU_BREADCRUMBS_ENV: &str = "NVF_GPU_BREADCRUMBS";

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

/// What must be asked of the Vulkan instance and device before `DefaultPlugins`, which is
/// where the renderer reads these settings (vendored `bevy_render`, `RenderPlugin::build`).
pub(crate) fn init(app: &mut App) {
    // VK_EXT_device_fault, so a lost device can say what faulted (gpu_recorder/fault.rs).
    // Nothing is enabled on a driver without it.
    let mut settings = app.world_mut().get_resource_or_init::<bevy::render::renderer::raw_vulkan_init::RawVulkanInitSettings>();
    gpu_recorder::fault::ask_for_extension(&mut settings);
    // The validation layer's deeper checks (synchronization, GPU-assisted), asked for on the
    // instance through VK_EXT_layer_settings. Nothing without both crash-test switches
    // (`vulkan_validation`).
    vulkan_validation::ask_for_layer_settings(&mut settings);
}

/// The fork's own plugins and then the platform shell's, on the configured game just before
/// it runs. Every one of them orders its systems against named sets and systems, so being
/// added last changes nothing about when they run.
pub(crate) fn before_run(app: &mut App) {
    // The bounded GPU flight recorder, written only when the device is lost (gpu_recorder/,
    // the Odin 3's DeviceLost). Its `finish` wants the render world, which exists by now.
    app.add_plugins(gpu_recorder::GpuRecorderPlugin);
    // The Vulkan validation layer's crash-test switch: one startup line when it is on,
    // nothing at all when it is off (vulkan_validation.rs).
    app.add_plugins(vulkan_validation::VulkanValidationPlugin);
    // The live glance at the Pip-Boy: only a run that plays has a body to raise it on, so
    // it goes in beside the rest of the player's body (`player_body`, `main`'s play branch).
    if app.is_plugin_added::<crate::player_body::PlayerBodyPlugin>() {
        app.add_plugins(wrist_screen::WristScreenPlugin);
    }
    let extend = EXTEND.lock().expect("the engine starts once, from one thread").take();
    if let Some(extend) = extend {
        extend(app);
    }
}
