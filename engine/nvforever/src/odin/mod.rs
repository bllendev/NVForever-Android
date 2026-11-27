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
//! | `main.rs` | before `DefaultPlugins` | [`init`] |
//! | `dlss/mod.rs` | the renderer's instance flags | [`vulkan_validation::instance_flags`] |
//! | `reports/mod.rs` | the run log's filter | [`vulkan_validation::log_directives`] |
//! | `main.rs` | just before `App::run` | [`before_run`] |
//! | `android.rs` | the home folder's name | [`home::DIR`] |
//! | `android.rs` | the launcher's crash-test switches | [`crash_tests::apply`] |
//! | `crash.rs` | the allocator `SaysWhenEmpty` wraps | [`alloc::BACKEND`] |
//! | `crash.rs` | one dump when an allocation fails | [`proc_mem::dump_once`] |
//! | `touch/gyro.rs` | the settings row, and three terms of `poll` | [`controller_gyro`] |
//! | `main.rs` | the play branch's plugin list | [`wrist_screen::WristScreenPlugin`] |
//! | `crash.rs` | a lost device's dump | [`gpu_recorder::note_lost`], [`gpu_recorder::log_dump_once`], [`gpu_recorder::report_section`] |
//! | `vats_highlight.rs`, `particles.rs`, `effect_particles.rs`, `decals.rs`, `heat_haze.rs`, `pipboy_arm.rs` | one gate each | [`crash_tests`] |
//! | the render features that are worth recording | one [`gpu_recorder::mark`] each | [`gpu_recorder::ring::feature`] |
//!
//! The Android-only parts of each module are behind `cfg(target_os = "android")`, while their
//! parsing and their wording are plain functions, built and tested everywhere.

use bevy::app::App;

/// Which allocator the game's allocations go to (`crash.rs`'s `SaysWhenEmpty` wraps it).
pub mod alloc;
/// The device's gyro while a physical controller is in use. It lives in `nvf-play`, because
/// `player.rs`'s look route is the other place it is hooked and that crate cannot see this one.
pub use nvf_play::odin::controller_gyro;
/// The crash-test switches the launcher's Advanced page offers: one table, one reader.
pub mod crash_tests;
/// The Android home folder's name, which the fork keeps as earlier builds left it.
pub mod home;
/// What this process's own memory is doing, and the dump on a failed allocation.
pub mod proc_mem;
/// The live glance at the Pip-Boy: the screen drawn on the arm while the game runs. It
/// lives beside the rest of nvforever's modules because it reaches deep into them
/// (`player_body`, `pipboy_controls`), and is declared from here so the fork's own code is
/// all one list.
#[path = "../wrist_screen.rs"]
pub mod wrist_screen;
/// The Khronos Vulkan validation layer, the slowest of the crash tests.
pub mod vulkan_validation;

/// The bounded GPU flight recorder and its parts. It keeps the name the log lines and the
/// docs use, so it sits beside `main.rs` and is declared from here (docs/fork.md).
#[path = "../gpu_recorder/mod.rs"]
pub mod gpu_recorder;

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
/// otherwise on is read ([`gpu_recorder::fault`]). Unset is not off.
pub fn env_off(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v.trim() == "0")
}

/// What must be asked of the Vulkan instance and device before `DefaultPlugins`, which is
/// where the renderer reads these settings (vendored `bevy_render`, `RenderPlugin::build`).
/// The one hook of its own in `main.rs`.
pub fn init(app: &mut App) {
    // VK_EXT_device_fault, so a lost device can say what faulted (gpu_recorder/fault.rs).
    // Nothing is enabled on a driver without it.
    let mut settings = app.world_mut().get_resource_or_init::<bevy::render::renderer::raw_vulkan_init::RawVulkanInitSettings>();
    gpu_recorder::fault::ask_for_extension(&mut settings);
    // The validation layer's deeper checks (synchronization, GPU-assisted), asked for on the
    // instance through VK_EXT_layer_settings. Nothing without both crash-test switches.
    vulkan_validation::ask_for_layer_settings(&mut settings);
    drop(settings);
    // The layer's own library, loaded before the renderer makes its instance
    // (vulkan_validation.rs). Nothing with the switch off.
    vulkan_validation::prepare();
}

/// The fork's own plugins on the configured game just before it runs. Every one of them
/// orders its systems against named sets and systems, so being added last changes nothing
/// about when they run. The other hook of its own in `main.rs`.
pub fn before_run(app: &mut App) {
    // The bounded GPU flight recorder: one summary line every ten seconds, and the whole
    // window only when the device is lost (gpu_recorder/, the Odin 3's DeviceLost). Its
    // `finish` wants the render world, which exists by now.
    app.add_plugins(gpu_recorder::GpuRecorderPlugin);
    // The transparent pass cut into chunks with a breadcrumb marker between them: a crash
    // test, and nothing at all unless both its switches are on
    // (gpu_recorder/transparent_chunks.rs).
    app.add_plugins(gpu_recorder::transparent_chunks::TransparentChunksPlugin);
    // The Vulkan validation layer's crash-test switch: one startup line when it is on,
    // nothing at all when it is off (vulkan_validation.rs).
    app.add_plugins(vulkan_validation::VulkanValidationPlugin);
}
