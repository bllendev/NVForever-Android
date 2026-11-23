//! Where things are on the phone, and the command line the game starts with.
//!
//! ```text
//! /storage/emulated/0/NVForever/                                      (shared storage)
//!     Data/                      default game folder (the launcher can pick any other)
//!     nvforever-args.txt         optional: extra game options (see `user_args`)
//!     nvforever-touch.txt        optional: touch control tuning
//! /data/user/0/org.nvforever.android/files/                          (internal files dir)
//!     data-path.txt              the active data profile's Data folder (Java side)
//!     profile.txt                the active data profile's home, relative to this folder
//!     profiles.txt               the launcher's own profile list (never read here)
//!     android-continue.txt       one start's save to load (the launcher's CONTINUE)
//!     NVForever/                 the "Vanilla" profile's home, and the one every older
//!                                install already has: NVForever.ini, plugins.txt, saves/,
//!                                logs/, cache/, mods/
//!     profiles/<id>/             any other data profile's home, the same files again
//!     schemas/                   the record/NIF/script schemas, unpacked from the library
//! ```
//!
//! A data profile is a name, a Data folder and a home folder. Everything per profile falls
//! out of the home, because `fnv_install::EngineHome` derives the plugin list, the saves,
//! the logs and the mods folder from its root, and `EngineHome::default_root` takes
//! `NVFOREVER_HOME` first: [`home_dir`] reads `profile.txt` and [`set_environment`] exports
//! it. The one thing that is deliberately *not* per profile is `NVForever.ini`, the game's
//! own settings (owner decision): [`share_settings`] keeps one copy of it in step across
//! the homes.
//!
//! The game refuses to run when its home is inside (or around) the game folder
//! (`fnv_install::check_separation`), and `Data/` sits directly in the external files dir,
//! so the home lives in the internal one; every profile home does too, so no profile can
//! ever break that rule. Saves and logs are reachable from a PC with
//! `adb exec-out run-as org.nvforever.android ...` while the app is a debug build
//! (docs/android.md).

use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/schemas_embedded.rs"));

/// The app's external files folder (`.../Android/data/<package>/files`), needing no permission.
pub fn external_files() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        bevy::android::ANDROID_APP
            .get()
            .and_then(|app| app.external_data_path())
            .unwrap_or_else(|| PathBuf::from("/sdcard/Android/data/org.nvforever.android/files"))
    }
    #[cfg(not(target_os = "android"))]
    {
        std::env::var_os("NVFOREVER_ANDROID_EXTERNAL").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("nvf-android-external"))
    }
}

/// The app's private files folder (`/data/user/0/<package>/files`).
pub fn internal_files() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        bevy::android::ANDROID_APP
            .get()
            .and_then(|app| app.internal_data_path())
            .unwrap_or_else(|| PathBuf::from("/data/data/org.nvforever.android/files"))
    }
    #[cfg(not(target_os = "android"))]
    {
        std::env::var_os("NVFOREVER_ANDROID_INTERNAL").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("nvf-android-internal"))
    }
}

/// The user-visible folder next to the phone's own `Download`, `DCIM`...: the default game
/// folder (`Data`) and the option files (`nvforever-args.txt`, `nvforever-touch.txt`) live
/// here. Needs the "all files access" permission the launcher asks for (the folders under
/// `Android/data` are closed to every file manager since Android 11, so they are not used).
pub fn shared_dir() -> PathBuf {
    #[cfg(not(target_os = "android"))]
    {
        std::env::var_os("NVFOREVER_ANDROID_SHARED").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("nvf-android-shared"))
    }
    #[cfg(target_os = "android")]
    {
        PathBuf::from("/storage/emulated/0/NVForever")
    }
}

/// The folder the player chose in the launcher (`<internal files>/data-path.txt`, one line).
pub fn chosen_data_dir() -> Option<PathBuf> {
    let text = std::fs::read_to_string(internal_files().join("data-path.txt")).ok()?;
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| PathBuf::from(line))
}

/// The user's `Data` folder: the one chosen in the launcher, else `<shared>/Data`; off
/// Android `$NVFOREVER_DATA` first.
pub fn data_dir() -> PathBuf {
    #[cfg(not(target_os = "android"))]
    if let Some(d) = std::env::var_os("NVFOREVER_DATA") {
        return PathBuf::from(d);
    }
    chosen_data_dir().unwrap_or_else(|| shared_dir().join("Data"))
}

/// An option file: `<shared>/<name>`, else (older installs) `<external files>/<name>`.
pub fn option_file(name: &str) -> Option<String> {
    std::fs::read_to_string(shared_dir().join(name)).or_else(|_| std::fs::read_to_string(external_files().join(name))).ok()
}

/// The home of the first data profile, "Vanilla": the one every install has had, so an
/// install made before profiles existed keeps its settings, saves and logs where they are.
pub const DEFAULT_HOME: &str = "NVForever";

/// The name of the active profile's home folder, relative to the internal files folder, as
/// the launcher wrote it to `profile.txt`. A line that is empty, absolute, names a drive or
/// steps up with `..` is refused and [`DEFAULT_HOME`] is used instead, so a damaged file can
/// never point the game's home outside the app's own storage.
pub fn profile_home_rel(line: &str) -> &str {
    let line = line.trim();
    let bad = line.is_empty()
        || line.starts_with('/')
        || line.starts_with('\\')
        || line.contains(':')
        || Path::new(line).components().any(|c| !matches!(c, std::path::Component::Normal(_)));
    if bad {
        DEFAULT_HOME
    } else {
        line
    }
}

/// The active data profile's home folder name (`<internal files>/profile.txt`, one line),
/// else [`DEFAULT_HOME`].
pub fn chosen_home_rel() -> String {
    let text = std::fs::read_to_string(internal_files().join("profile.txt")).unwrap_or_default();
    profile_home_rel(text.lines().next().unwrap_or_default()).to_string()
}

/// The game's home (`NVFOREVER_HOME`): settings, saves, logs, plugins.txt, of the active
/// data profile ([`chosen_home_rel`]).
pub fn home_dir() -> PathBuf {
    internal_files().join(chosen_home_rel())
}

/// Every data profile home the app has, the default one first: `<files>/NVForever` and each
/// folder in `<files>/profiles/`.
fn home_dirs(files: &Path) -> Vec<PathBuf> {
    let mut homes = vec![files.join(DEFAULT_HOME)];
    if let Ok(entries) = std::fs::read_dir(files.join("profiles")) {
        let mut others: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        others.sort();
        homes.extend(others);
    }
    homes
}

/// Keep `NVForever.ini` shared across the data profiles (owner decision): the plugin list
/// and the saves are each profile's own, but the game's own settings (resolution, difficulty,
/// the settings page's rows) follow the player, not the profile.
///
/// The file cannot simply be linked: every writer of it replaces it whole, through a
/// temporary file and a rename (`fnv_install::write_whole`), which would break a hard link
/// and leave a symlink pointing nowhere. So the newest copy among all the profile homes wins
/// and is written into `active` before the game starts. Only one profile ever runs at a time,
/// and the launcher never writes the file, so "newest wins" has one candidate: whichever
/// profile ran last. Errors are returned for the caller to log; they are never fatal, and a
/// profile with no INI yet simply gets the one the player already had.
pub fn share_settings(files: &Path, active: &Path) -> Result<(), String> {
    const INI: &str = "NVForever.ini";
    let newest = home_dirs(files)
        .into_iter()
        .map(|home| home.join(INI))
        .filter_map(|p| p.metadata().ok().filter(|m| m.is_file()).and_then(|m| m.modified().ok()).map(|t| (t, p)))
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let Some((_, newest)) = newest else { return Ok(()) };
    let target = active.join(INI);
    if newest == target {
        return Ok(());
    }
    let bytes = std::fs::read(&newest).map_err(|e| format!("{}: {e}", newest.display()))?;
    if std::fs::read(&target).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    std::fs::create_dir_all(active).map_err(|e| format!("{}: {e}", active.display()))?;
    std::fs::write(&target, &bytes).map_err(|e| format!("{}: {e}", target.display()))
}

/// The save the launcher's CONTINUE asked for, used exactly once
/// (`<internal files>/android-continue.txt`, one line). The file is deleted whether or not
/// it names a save that still exists, so a failed start opens the main menu next time
/// instead of trying the same save for ever.
pub fn continue_save(files: &Path) -> Option<PathBuf> {
    let file = files.join("android-continue.txt");
    let text = std::fs::read_to_string(&file).ok()?;
    let _ = std::fs::remove_file(&file);
    let line = text.lines().next()?.trim();
    let save = PathBuf::from(line);
    (!line.is_empty() && save.is_file()).then_some(save)
}

/// Where the schemas are unpacked.
pub fn schemas_dir() -> PathBuf {
    internal_files().join("schemas")
}

/// Writes the schemas packed in the library (build.rs) below [`schemas_dir`], skipping files
/// that are already identical. Returns the folder.
pub fn unpack_schemas() -> Result<PathBuf, String> {
    let root = schemas_dir();
    for (rel, bytes) in FILES {
        let path = root.join(rel);
        if std::fs::read(&path).is_ok_and(|old| old == *bytes) {
            continue;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(root)
}

/// The options the Android build gives the game, as (flag, value). Chosen for a phone:
/// borderless full screen, no multisampling, no sun shadows, no CPU-baked distant terrain
/// (it is heavy; the game's own LOD chunks still draw), and 3 x 3 streamed cells instead of
/// 5 x 5. Each can be put back with `nvforever-args.txt` (`--msaa 4`, `--sun-shadows`, ...)
/// or on the game's settings page.
/// `load`: the save the launcher's CONTINUE asked for. It takes `--menu`'s place; the game's
/// parser refuses the two together.
fn default_args(data: &Path, schemas: &Path, load: Option<&Path>) -> Vec<(String, Option<String>)> {
    let v = |f: &str, val: Option<String>| (f.to_string(), val);
    vec![
        v("--play", None),
        match load {
            Some(save) => v("--load", Some(save.display().to_string())),
            None => v("--menu", None),
        },
        // A player's start, not a developer's: the active profile's own plugins.txt is the
        // load order, even though --data names its Data folder (nvforever::developer_data).
        v("--player-start", None),
        v("--data", Some(data.display().to_string())),
        v("--esm-schema", Some(schemas.join("fnv").display().to_string())),
        v("--nif-schema", Some(schemas.join("nif").display().to_string())),
        v("--window", Some("borderless".into())),
        v("--msaa", Some("1".into())),
        v("--no-sun-shadows", None),
        v("--no-lodgen", None),
        v("--radius", Some("1".into())),
    ]
}

/// The options in `<shared>/nvforever-args.txt` (`/storage/emulated/0/NVForever/`) (whitespace separated, `#` starts a
/// comment line), empty when there is no such file. Written on a PC with, for example,
/// any text editor or file manager.
pub fn user_args() -> Vec<String> {
    let Some(text) = option_file("nvforever-args.txt") else { return Vec::new() };
    text.lines().filter(|l| !l.trim_start().starts_with('#')).flat_map(str::split_whitespace).map(str::to_string).collect()
}

/// The program's command line: the phone defaults, then `user` with its flags replacing the
/// defaults of the same name (the game's parser rejects a flag given twice).
pub fn command_line(data: &Path, schemas: &Path, user: &[String], load: Option<&Path>) -> Vec<String> {
    let mut given: Vec<&str> = user.iter().filter(|a| a.starts_with("--")).map(|a| a.split('=').next().unwrap_or(a)).collect();
    // Flags the game's parser refuses together: the user's choice also drops the default's twin.
    for (a, b) in [("--sun-shadows", "--no-sun-shadows"), ("--overlay", "--no-overlay"), ("--menu", "--load")] {
        if given.contains(&a) {
            given.push(b);
        }
        if given.contains(&b) {
            given.push(a);
        }
    }
    let mut line = vec!["nvforever".to_string()];
    for (flag, value) in default_args(data, schemas, load) {
        if given.contains(&flag.as_str()) {
            continue;
        }
        line.push(flag);
        line.extend(value);
    }
    line.extend(user.iter().cloned());
    line
}

/// Consume a private activity-restart command line exactly once. JSON preserves
/// save paths containing spaces; the public shared options file is not executable input.
pub fn restart_line(files: &Path) -> Result<Option<Vec<String>>, String> {
    let next = files.join("android-next-run.json");
    let bytes = match std::fs::read(&next) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Cannot read restart request: {e}")),
    };
    std::fs::remove_file(&next).map_err(|e| format!("Cannot consume restart request: {e}"))?;
    let mut line: Vec<String> = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid restart arguments: {e}"))?;
    if line.is_empty() { return Err("Restart arguments are empty".into()); }
    line.insert(0, "nvforever".into());
    Ok(Some(line))
}

/// Checks the game folder the way the game's own start does and says what to do if it fails.
pub fn check_data(data: &Path) -> Result<(), String> {
    if !data.is_dir() {
        return Err(format!("The game folder was not found or cannot be read:\n{}\n\nClose the game, open NVForever again and use the launcher: grant \"all files access\" and choose the Fallout: New Vegas Data folder.", data.display()));
    }
    let game = fnv_install::GameInstall::open(data).map_err(|e| format!("{e}\n\nFolder: {}", data.display()))?;
    game.check_files().map_err(|e| format!("{e}\n\nThe copy is incomplete (copy stopped early?). Copy it again, or choose another folder in the launcher."))?;
    Ok(())
}

/// Environment the game and the graphics stack read, set before any thread starts.
pub fn set_environment(home: &Path, schemas: &Path) {
    // Edition 2021: `set_var` is safe to call; nothing else is running yet.
    std::env::set_var("NVFOREVER_HOME", home);
    // Every Android start is a player's start, even though it names the active data profile's
    // folder with `--data`: the home's own plugins.txt is the load order, so the profile's mod
    // plugins can load (`nvforever::odin::developer_data`). The flag on the command line says
    // the same, but a line the game wrote for an activity restart does not carry it, and this
    // runs on every start.
    std::env::set_var(nvforever::odin::PLAYER_START_ENV, "1");
    // The script engines read the function catalogue from here (nvf_play::script).
    std::env::set_var("NVF_PLAY_SCRIPT_FUNCTIONS", schemas.join("script").join("functions.json"));
    // Vulkan only: the renderer is built for it, and a silent fall-back to GLES would hide a
    // missing driver as a slow, different picture. A value already set (debugging) wins.
    if std::env::var_os("WGPU_BACKEND").is_none() {
        std::env::set_var("WGPU_BACKEND", "vulkan");
    }
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary folder of this test's own, removed by the caller.
    fn temp_dir(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("nvf-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_profile_home_stays_inside_the_app_s_own_storage() {
        assert_eq!(profile_home_rel("profiles/modded"), "profiles/modded");
        assert_eq!(profile_home_rel("  NVForever \n"), "NVForever");
        for bad in ["", "   ", "/sdcard/NVForever", "\\\\server\\share", "C:/NVForever", "../../escape", "profiles/../../escape", "."] {
            assert_eq!(profile_home_rel(bad), DEFAULT_HOME, "refused and kept at the default: {bad:?}");
        }
    }

    #[test]
    fn the_settings_file_is_shared_across_the_profiles() {
        let files = temp_dir("profile-ini-test");
        let vanilla = files.join(DEFAULT_HOME);
        let modded = files.join("profiles").join("modded");
        std::fs::create_dir_all(&vanilla).unwrap();
        std::fs::create_dir_all(&modded).unwrap();
        // Nothing written yet: nothing to share, and no error.
        share_settings(&files, &modded).unwrap();
        assert!(!modded.join("NVForever.ini").exists());
        // The player's settings, written by a Vanilla run, reach a profile that has none.
        std::fs::write(vanilla.join("NVForever.ini"), b"[Settings]\nFov=90\n").unwrap();
        share_settings(&files, &modded).unwrap();
        assert_eq!(std::fs::read(modded.join("NVForever.ini")).unwrap(), b"[Settings]\nFov=90\n");
        // The Modded run changes them: the next Vanilla start takes the newer copy.
        std::fs::write(modded.join("NVForever.ini"), b"[Settings]\nFov=75\n").unwrap();
        age(&vanilla.join("NVForever.ini"));
        share_settings(&files, &vanilla).unwrap();
        assert_eq!(std::fs::read(vanilla.join("NVForever.ini")).unwrap(), b"[Settings]\nFov=75\n");
        // The plugin list is not shared: it is what a profile is for.
        std::fs::write(vanilla.join("plugins.txt"), b"*FalloutNV.esm\n").unwrap();
        share_settings(&files, &modded).unwrap();
        assert!(!modded.join("plugins.txt").exists());
        std::fs::remove_dir_all(files).unwrap();
    }

    /// Date a file an hour back, so "the newest wins" has one answer on any clock.
    fn age(path: &Path) {
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600)).unwrap();
    }

    #[test]
    fn continue_names_one_start_s_save_and_is_consumed() {
        let files = temp_dir("continue-test");
        assert!(continue_save(&files).is_none(), "no request: the main menu");
        let save = files.join("my save.nvfsave");
        std::fs::write(&save, b"save").unwrap();
        std::fs::write(files.join("android-continue.txt"), format!("{}\n", save.display())).unwrap();
        assert_eq!(continue_save(&files).unwrap(), save);
        assert!(continue_save(&files).is_none(), "one start only");
        // A save that is gone (deleted, or its profile's folder unmounted) is not an error.
        std::fs::write(files.join("android-continue.txt"), "/gone/save.nvfsave\n").unwrap();
        assert!(continue_save(&files).is_none());
        assert!(!files.join("android-continue.txt").exists(), "still consumed");
        std::fs::remove_dir_all(files).unwrap();
    }

    #[test]
    fn a_phone_start_is_a_player_s_start_and_continue_replaces_the_menu() {
        let line = command_line(Path::new("/d"), Path::new("/s"), &[], None);
        assert!(line.contains(&"--player-start".to_string()), "the profile's own plugins.txt is the load order");
        assert!(line.contains(&"--menu".to_string()));
        assert!(!line.contains(&"--load".to_string()));
        let save = PathBuf::from("/private/saves/my save.nvfsave");
        let line = command_line(Path::new("/d"), Path::new("/s"), &[], Some(&save));
        assert!(!line.contains(&"--menu".to_string()), "the game's parser refuses --menu with --load");
        let i = line.iter().position(|a| a == "--load").unwrap();
        assert_eq!(line[i + 1], "/private/saves/my save.nvfsave");
        // nvforever-args.txt can still ask for the main menu over a CONTINUE.
        let line = command_line(Path::new("/d"), Path::new("/s"), &["--menu".into()], Some(&save));
        assert_eq!(line.iter().filter(|a| *a == "--menu").count(), 1);
        assert!(!line.contains(&"--load".to_string()));
    }

    #[test]
    fn user_flags_replace_defaults_and_values_follow() {
        let line = command_line(Path::new("/d"), Path::new("/s"), &["--msaa".into(), "4".into(), "--sun-shadows".into()], None);
        assert_eq!(line.iter().filter(|a| *a == "--msaa").count(), 1);
        let i = line.iter().position(|a| a == "--msaa").unwrap();
        assert_eq!(line[i + 1], "4");
        assert!(line.contains(&"--sun-shadows".to_string()));
        assert!(!line.contains(&"--no-sun-shadows".to_string()), "the default's twin is dropped: the game rejects the pair");
    }

    #[test]
    fn restart_arguments_are_private_one_shot_and_preserve_spaces() {
        let root = std::env::temp_dir().join(format!("nvf-restart-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let next = root.join("android-next-run.json");
        let _ = std::fs::remove_file(&next);
        assert!(restart_line(&root).unwrap().is_none());
        let args = vec!["--play", "--load", "/private/saves/my save.nvfsave"];
        std::fs::write(&next, serde_json::to_vec(&args).unwrap()).unwrap();
        assert_eq!(restart_line(&root).unwrap().unwrap(), ["nvforever", "--play", "--load", "/private/saves/my save.nvfsave"]);
        assert!(restart_line(&root).unwrap().is_none());
        std::fs::write(&next, b"not JSON").unwrap();
        assert!(restart_line(&root).is_err());
        assert!(restart_line(&root).unwrap().is_none());
        std::fs::write(&next, b"[]").unwrap();
        assert!(restart_line(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn schemas_are_packed() {
        assert!(FILES.iter().any(|(p, _)| *p == "script/functions.json"));
        assert!(FILES.iter().any(|(p, _)| p.starts_with("fnv/")));
        assert!(FILES.iter().any(|(p, _)| p.starts_with("nif/")));
    }
}
