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

/// The entry point. On Android `#[bevy_main]` generates the `android_main` function the
/// activity calls (and stores the `AndroidApp` handle in `bevy::android::ANDROID_APP`).
/// On other targets it leaves `main` as an ordinary function, so `cargo check` of this
/// crate on a desktop still type-checks everything.
#[bevy::prelude::bevy_main]
pub fn main() {
    logcat::forward_stdio();
    let data = paths::data_dir();
    let home = paths::home_dir();
    if let Err(e) = std::fs::create_dir_all(&home) {
        return diag::run(format!("Cannot create the game's home folder {}: {e}", home.display()));
    }
    let schemas = match paths::unpack_schemas() {
        Ok(dir) => dir,
        Err(e) => return diag::run(format!("Cannot unpack the game's schemas: {e}")),
    };
    if let Err(why) = paths::check_data(&data) {
        return diag::run(why);
    }
    paths::set_environment(&home, &schemas);
    let line = match paths::restart_line(&paths::internal_files()) {
        Ok(Some(line)) => line,
        Ok(None) => paths::command_line(&data, &schemas, &paths::user_args()),
        Err(e) => return diag::run(e),
    };
    eprintln!("NVForever Android: starting: {}", line.join(" "));
    let exit = nvforever::run_with(line, |app| {
        let config = controller::ControlsConfig::load();
        eprintln!("Android controls: controller={}, touch={}", config.controller, config.touch);
        app.insert_resource(config).add_plugins(controller::ControllerPlugin);
        if config.touch { app.add_plugins(touch::TouchControlsPlugin); }
    });
    eprintln!("NVForever Android: the game ended: {exit:?}");
}
