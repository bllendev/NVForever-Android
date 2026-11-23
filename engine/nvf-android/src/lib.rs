//! NVForever on Android: Milestone 2, the whole game.
//!
//! The game is the `nvforever` library, the same code the desktop program runs. This crate
//! is the Android shell around it:
//!
//! 1. [`paths`]: the phone's folders, the schemas unpacked from the library, the command line
//!    the game starts with (phone defaults, overridable from `nvforever-args.txt`);
//! 2. [`touch`]: fingers turned into the keys and mouse motion the game reads;
//! 3. [`logcat`]: stdout/stderr to `adb logcat -s NVForever`;
//! 4. [`diag`]: when the game cannot start (no game folder, an incomplete copy), a screen that
//!    says why, instead of an app that closes by itself (this was all of Milestone 1).
//!
//! Plan, build steps and the on-device checklist: `docs/android.md`.

mod controller;
mod diag;
mod logcat;
mod paths;
mod probe;
mod touch;
mod validation;

/// The entry point. On Android `#[bevy_main]` generates the `android_main` function the
/// activity calls (and stores the `AndroidApp` handle in `bevy::android::ANDROID_APP`).
/// On other targets it leaves `main` as an ordinary function, so `cargo check` of this
/// crate on a desktop still type-checks everything.
#[bevy::prelude::bevy_main]
pub fn main() {
    logcat::forward_stdio();
    let data = paths::data_dir();
    // The active data profile: its Data folder, and its own home (plugins.txt, saves, logs).
    let home = paths::home_dir();
    if let Err(e) = std::fs::create_dir_all(&home) {
        return diag::run(format!("Cannot create the game's home folder {}: {e}", home.display()));
    }
    // The game's own settings are the player's, not the profile's (paths::share_settings).
    if let Err(e) = paths::share_settings(&paths::internal_files(), &home) {
        eprintln!("warning: the settings could not be shared between the data profiles: {e}");
    }
    let schemas = match paths::unpack_schemas() {
        Ok(dir) => dir,
        Err(e) => return diag::run(format!("Cannot unpack the game's schemas: {e}")),
    };
    if let Err(why) = paths::check_data(&data) {
        return diag::run(why);
    }
    paths::set_environment(&home, &schemas);
    // Crash test from the launcher: no hidden Pip-Boy warm-up while a load is covered
    // (the engine's `pipboy_arm`). Set before the engine starts any thread.
    let tests = controller::ControlsConfig::load();
    if tests.skip_pipboy_warmup {
        std::env::set_var(nvforever::odin::NO_PIPBOY_WARM_ENV, "1");
    }
    if tests.skip_vats_outline {
        std::env::set_var(nvforever::odin::NO_VATS_OUTLINE_ENV, "1");
    }
    // Crash test from the launcher: every render-graph node writes a GPU marker before and
    // after its work, so a lost device names the node the GPU hung in
    // (`nvforever::odin::gpu_recorder::breadcrumbs`).
    if tests.gpu_breadcrumbs {
        std::env::set_var(nvforever::odin::GPU_BREADCRUMBS_ENV, "1");
    }
    // Crash test from the launcher: with the breadcrumbs on, the transparent pass is drawn in
    // chunks with a marker between them, so a lost device names the chunk of draws
    // (`nvforever::odin::gpu_recorder::transparent_chunks`). Much slower.
    if tests.gpu_breadcrumb_chunks {
        std::env::set_var(nvforever::odin::GPU_BREADCRUMB_CHUNKS_ENV, "1");
    }
    // Crash tests from the launcher, one drawn feature each: soft particles, particles,
    // decals, heat haze.
    if tests.no_soft_particles {
        std::env::set_var(nvforever::odin::NO_SOFT_PARTICLES_ENV, "1");
    }
    if tests.no_particles {
        std::env::set_var(nvforever::odin::NO_PARTICLES_ENV, "1");
    }
    if tests.no_decals {
        std::env::set_var(nvforever::odin::NO_DECALS_ENV, "1");
    }
    if tests.no_heat_haze {
        std::env::set_var(nvforever::odin::NO_HEAT_HAZE_ENV, "1");
    }
    // Crash test from the launcher: the Khronos Vulkan validation layer, loaded before the
    // renderer's instance is made (validation.rs). Much slower; off unless asked for. The two
    // deeper checks are its own switches, and mean nothing without it.
    if tests.vulkan_validation {
        validation::enable(validation::deep_words(tests.vulkan_validation_sync, tests.vulkan_validation_gpuav));
    }
    let line = match paths::restart_line(&paths::internal_files()) {
        Ok(Some(line)) => line,
        // The launcher's CONTINUE names one start's save (paths::continue_save).
        Ok(None) => paths::command_line(&data, &schemas, &paths::user_args(), paths::continue_save(&paths::internal_files()).as_deref()),
        Err(e) => return diag::run(e),
    };
    eprintln!("NVForever Android: starting: {}", line.join(" "));
    let exit = nvforever::odin::run_with(line, |app| {
        let config = controller::ControlsConfig::load();
        eprintln!("Android controls: controller={}, touch={}", config.controller, config.touch);
        app.insert_resource(config).add_plugins(controller::ControllerPlugin);
        if config.touch { app.add_plugins(touch::TouchControlsPlugin); }
    });
    eprintln!("NVForever Android: the game ended: {exit:?}");
}
