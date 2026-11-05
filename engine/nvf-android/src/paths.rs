//! Where things are on the phone, and the command line the game starts with.
//!
//! ```text
//! /storage/emulated/0/NVForever/                                      (shared storage)
//!     Data/                      default game folder (the launcher can pick any other)
//!     nvforever-args.txt         optional: extra game options (see `user_args`)
//!     nvforever-touch.txt        optional: touch control tuning
//! /data/user/0/org.nvforever.android/files/                          (internal files dir)
//!     data-path.txt              the Data folder chosen in the launcher (Java side)
//!     NVForever/                 the game's home: NVForever.ini, saves/, logs/, cache/
//!     schemas/                   the record/NIF/script schemas, unpacked from the library
//! ```
//!
//! The game refuses to run when its home is inside (or around) the game folder
//! (`fnv_install::check_separation`), and `Data/` sits directly in the external files dir,
//! so the home lives in the internal one. Saves and logs are reachable from a PC with
//! `adb exec-out run-as org.nvforever.android ...` while the app is a debug build
//! (docs/android/milestone-2.md).

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

/// The game's home (`NVFOREVER_HOME`): settings, saves, logs.
pub fn home_dir() -> PathBuf {
    internal_files().join("NVForever")
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
fn default_args(data: &Path, schemas: &Path) -> Vec<(String, Option<String>)> {
    let v = |f: &str, val: Option<String>| (f.to_string(), val);
    vec![
        v("--play", None),
        v("--menu", None),
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
pub fn command_line(data: &Path, schemas: &Path, user: &[String]) -> Vec<String> {
    let mut given: Vec<&str> = user.iter().filter(|a| a.starts_with("--")).map(|a| a.split('=').next().unwrap_or(a)).collect();
    // Flags the game's parser refuses together: the user's choice also drops the default's twin.
    for (a, b) in [("--sun-shadows", "--no-sun-shadows"), ("--overlay", "--no-overlay")] {
        if given.contains(&a) {
            given.push(b);
        }
        if given.contains(&b) {
            given.push(a);
        }
    }
    let mut line = vec!["nvforever".to_string()];
    for (flag, value) in default_args(data, schemas) {
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

    #[test]
    fn user_flags_replace_defaults_and_values_follow() {
        let line = command_line(Path::new("/d"), Path::new("/s"), &["--msaa".into(), "4".into(), "--sun-shadows".into()]);
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
