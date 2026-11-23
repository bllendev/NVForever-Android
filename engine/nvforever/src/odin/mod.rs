//! The Android fork's own code, in one module (docs/fork.md).
//!
//! `src/main.rs` is upstream's file: the crate root of both the desktop binary
//! (`src/desktop.rs`) and the Android shell (`engine/nvf-android`), and the fork touches it
//! in a few places only, so that a sync with upstream NVForever never has to merge 3,000
//! lines of it:
//!
//! 1. `pub mod odin;`, this module;
//! 2. [`args`] instead of `std::env::args()`, so a platform shell can hand the game a command
//!    line of its own (an Android activity has no argv), and can pass the fork's own flags
//!    (`--player-start`) without upstream's parser knowing them;
//! 3. [`init`] before `DefaultPlugins` (what the Vulkan instance and device must be asked
//!    for before the renderer exists) and [`before_run`] just before `App::run` (the fork's
//!    own plugins, then the platform shell's);
//! 4. [`developer_data`] where `main` decides whether `--data` makes this a developer run.
//!
//! The entry points: [`desktop`] is what the desktop binary calls, [`run`] and [`run_with`]
//! what an Android activity calls (`nvf-android`'s `android_main`).

use bevy::app::{App, AppExit};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

// The Vulkan driver this run uses (a player's own driver on Android) and the start marker that
// sends the next start back to the system one. Fork-only code, so it lives here rather than
// beside main.rs.
pub mod gpu_driver;

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

/// The Vulkan driver the platform shell was asked for, in the words the run log should carry
/// (`System`, `Turnip 25.0.3`, ...): one startup line says it beside what the adapter turned
/// out to be ([`gpu_driver`]). Set only by the Android shell, and only when the player selected
/// a driver of their own; unset everywhere else, where nothing is logged.
pub const GPU_DRIVER_ENV: &str = "NVF_GPU_DRIVER";

/// The file the platform shell wrote before the renderer started, removed once this run has
/// drawn its frames ([`gpu_driver`]). A file still there at the next start is how the shell
/// knows the driver the player selected did not survive its own first frames.
pub const RENDERER_MARKER_ENV: &str = "NVF_RENDERER_MARKER";

/// How many drawn frames [`RENDERER_MARKER_ENV`]'s file must survive, instead of
/// [`gpu_driver::DEFAULT_FRAMES`]. A test aid, not a setting.
pub const RENDERER_MARKER_FRAMES_ENV: &str = "NVF_RENDERER_MARKER_FRAMES";

/// Set (anything but `0`) to have every render-graph node write a GPU marker before and
/// after its work, so a lost device names the node the GPU hung in: a crash test the Android
/// launcher offers (`gpu_recorder::breadcrumbs`). Read in the vendored `bevy_render`
/// (`bevy::render::gpu_breadcrumbs`) as well, which is where the markers are recorded.
pub const GPU_BREADCRUMBS_ENV: &str = "NVF_GPU_BREADCRUMBS";

/// Set (anything but `0`), with [`GPU_BREADCRUMBS_ENV`] as well, to draw the transparent
/// phase in chunks with a breadcrumb marker between them, so a lost device names the chunk of
/// draws the GPU hung in rather than the whole pass: a crash test the Android launcher offers
/// (`gpu_recorder::transparent_chunks`). Many more render passes a frame; nothing is drawn
/// differently.
pub const GPU_BREADCRUMB_CHUNKS_ENV: &str = "NVF_GPU_BREADCRUMB_CHUNKS";

/// Set (any value) to draw particles with hard edges where they meet the surface behind them,
/// which is also to read the depth prepass nowhere in the NIF material's fragment shader: a
/// crash test the Android launcher offers (`nvf_assets::nif_material`, `particles`).
pub const NO_SOFT_PARTICLES_ENV: &str = nvf_assets::NO_SOFT_PARTICLES_ENV;

/// Set (any value) to draw no particle systems and no effect particles (they are simulated as
/// before, and nothing of them is drawn): a crash test the Android launcher offers
/// (`particles`, `effect_particles`).
pub const NO_PARTICLES_ENV: &str = "NVF_NO_PARTICLES";

/// Set (any value) to place no decals (blood, bullet holes): a crash test the Android
/// launcher offers (`decals`).
pub const NO_DECALS_ENV: &str = "NVF_NO_DECALS";

/// Set (any value) to draw no heat haze (the screen-space pass at the horizon): a crash test
/// the Android launcher offers (`heat_haze`).
pub const NO_HEAT_HAZE_ENV: &str = "NVF_NO_HEAT_HAZE";

/// Whether this run was asked to draw no particles ([`NO_PARTICLES_ENV`]). Read once.
pub fn no_particles() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os(NO_PARTICLES_ENV).is_some())
}

/// Whether this run was asked to place no decals ([`NO_DECALS_ENV`]). Read once.
pub fn no_decals() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os(NO_DECALS_ENV).is_some())
}

/// Whether this run was asked to draw no heat haze ([`NO_HEAT_HAZE_ENV`]). Read once.
pub fn no_heat_haze() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os(NO_HEAT_HAZE_ENV).is_some())
}

/// Set (anything but `0`) to say this run is a player's start even though `--data` names a
/// game folder, so the home's own plugins.txt is the load order ([`developer_data`]). The
/// Android shell exports it on every start, restarts included
/// (`nvf_android::paths::set_environment`), because every Android start names `--data`: the
/// active data profile's Data folder.
pub const PLAYER_START_ENV: &str = "NVF_PLAYER_START";

/// The command line's form of [`PLAYER_START_ENV`], which the Android launcher writes into
/// the line it hands the game. [`args`] takes it off the line again: it is the fork's own
/// flag, and upstream's parser knows nothing of it.
pub const PLAYER_START_FLAG: &str = "--player-start";

/// Whether [`PLAYER_START_FLAG`] was on the line [`args`] handed the game.
static PLAYER_START: AtomicBool = AtomicBool::new(false);

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
/// own (a desktop run), without the fork's own flags. Called by `main` before anything is
/// parsed.
pub(crate) fn args() -> Vec<String> {
    let line = LINE.lock().expect("the engine starts once, from one thread").take();
    let (line, player_start) = split_player_start(line.unwrap_or_else(|| std::env::args().collect()));
    if player_start {
        PLAYER_START.store(true, Ordering::Relaxed);
    }
    line
}

/// `line` without [`PLAYER_START_FLAG`], and whether it was there.
fn split_player_start(line: Vec<String>) -> (Vec<String>, bool) {
    let mut found = false;
    let kept = line
        .into_iter()
        .filter(|a| {
            found |= a == PLAYER_START_FLAG;
            a != PLAYER_START_FLAG
        })
        .collect();
    (kept, found)
}

/// Whether this run is a player's start: the flag was on the line, or the platform shell
/// exported [`PLAYER_START_ENV`].
pub fn player_start() -> bool {
    PLAYER_START.load(Ordering::Relaxed) || std::env::var_os(PLAYER_START_ENV).is_some_and(|v| v != "0")
}

/// Whether this run keeps the fixed masters as its load order and reports nothing, instead
/// of reading the home's own plugins.txt: a developer or smoke run, which is any run that
/// names its own `--data`. A player's start is a player's start even so ([`player_start`]),
/// which is what every Android start is: the launcher always names the active data profile's
/// folder with `--data`, and without that no mod plugin of that profile could ever load.
pub(crate) fn developer_data(data_given: bool) -> bool {
    is_developer_data(data_given, player_start())
}

/// [`developer_data`] on its two answers alone.
fn is_developer_data(data_given: bool, player_start: bool) -> bool {
    data_given && !player_start
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
    // The transparent pass cut into chunks with a breadcrumb marker between them: a crash
    // test, and nothing at all unless both its switches are on
    // (gpu_recorder/transparent_chunks.rs).
    app.add_plugins(gpu_recorder::transparent_chunks::TransparentChunksPlugin);
    // The Vulkan validation layer's crash-test switch: one startup line when it is on,
    // nothing at all when it is off (vulkan_validation.rs).
    app.add_plugins(vulkan_validation::VulkanValidationPlugin);
    // Which Vulkan driver this run asked for against what the adapter turned out to be, and
    // the Android start marker that sends the next start back to the system driver
    // (gpu_driver.rs). Nothing at all unless the platform shell set its variables.
    app.add_plugins(gpu_driver::GpuDriverPlugin);
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

#[cfg(test)]
mod tests {
    use super::{is_developer_data, split_player_start, PLAYER_START_FLAG};

    fn line(a: &[&str]) -> Vec<String> {
        std::iter::once("nvforever").chain(a.iter().copied()).map(String::from).collect()
    }

    #[test]
    fn the_player_start_flag_leaves_the_line_before_the_game_s_parser_sees_it() {
        let (kept, found) = split_player_start(line(&["--play", "--data", "/d", PLAYER_START_FLAG, "--menu"]));
        assert!(found);
        assert_eq!(kept, line(&["--play", "--data", "/d", "--menu"]), "upstream's Args knows nothing of the flag");
        let (kept, found) = split_player_start(line(&["--play", "--data", "/d"]));
        assert!(!found, "a desktop start does not pass it");
        assert_eq!(kept, line(&["--play", "--data", "/d"]));
    }

    #[test]
    fn a_player_s_start_reads_the_home_s_own_plugin_list_under_data() {
        // A developer or smoke run: --data alone still means the fixed masters.
        assert!(is_developer_data(true, false), "--data alone is a developer run");
        assert!(!is_developer_data(true, true), "--data with a player's start is one (Android's profiles)");
        assert!(!is_developer_data(false, false), "no --data is a player's start");
        assert!(!is_developer_data(false, true));
    }
}
