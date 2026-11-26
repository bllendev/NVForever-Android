//! What the Android fork adds to the launcher, in a file of its own (docs/fork.md).
//!
//! # The game folder, read in place
//!
//! Upstream's Android launcher imports the player's game: the directory picker, then a copy
//! of the whole `Data` tree into `Android/data/<application id>/files/<uuid>/`
//! (docs/design/android.md, "Files and import"). That copy needs as much free space as the
//! game folder has, twice that while an older import is being replaced, and it lands where no
//! file manager can reach it from Android 11 on, so a mod or a replacement BSA means
//! importing again.
//!
//! This fork reads the folder **in place** from shared storage instead, which is what every
//! build of it has done and what the owner's device is set up for: one line in
//! `<internal files>/data-path.txt` names the folder, and the app has all files access
//! ([`SHARED_DATA`] is the default the fork's builds use).
//!
//! The hook is one line at the top of `mobile::request`, and the whole of the fork's part is
//! [`adopt`]: when the launcher's own `[Game] Path` does not name a complete game and the
//! folder named in `data-path.txt` does, that folder becomes `[Game] Path`. From there
//! nothing else changes: upstream's launcher pages, its plugin list and the engine all read
//! the game folder from `NVForever.ini` as they already do, and the engine is handed no
//! `--data`, so an Android start stays a player's start and the home's `plugins.txt` is still
//! the load order.
//!
//! What this deliberately does **not** do: it never writes over a `[Game] Path` that already
//! names a complete game, so an import upstream's picker made keeps working and a fresh
//! install with no `data-path.txt` is upstream's flow untouched. Reading in place as a thing
//! the player can choose in the launcher, rather than a file that is already there, is the
//! next phase of this work (.hermes/plans/android-base-phases.md).
//!
//! `[documented: proc(5) is not involved here; the paths and the permission are Android's own:
//! MANAGE_EXTERNAL_STORAGE is what lets this process open a folder under
//! /storage/emulated/0, and folders under Android/data are closed to file managers from
//! Android 11 on.]`

//! # The in-app updater
//!
//! Upstream's Android updater is careful and fails closed: it will not install an APK whose
//! package name or signing certificate differs from the installed app's, and it will not
//! download one at all without a `SHA256SUMS.txt` on the same release
//! (`UpdatePolicy.verifyApkMetadata`, `mobile::check_update`). That is exactly right, and it
//! means an updater pointed at upstream's releases would download an APK onto this fork's
//! install and then delete it, because this fork is a different package signed with a
//! different key.
//!
//! So on Android it asks **this** repository instead ([`RELEASES`], [`DOWNLOADS`]), which
//! publishes `SHA256SUMS.txt` and both APKs beside it (docs/fork.md, "The release scheme").
//! A release carries two APKs, one of them `-debug`, so [`apk_for_this_app`] keeps the two
//! apps apart: the debug app is only ever offered the `-debug` asset, and the app the owner
//! plays only the other. The desktop launcher is unchanged and still asks upstream.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The file naming the game folder to read in place, beside the engine home in the app's
/// private files directory. Keep the name the same as `OdinFork.DATA_PATH`.
pub const DATA_PATH_FILE: &str = "data-path.txt";

/// Where the fork's builds look when nothing was chosen: the folder the owner keeps the game
/// in. Keep this the same as `OdinFork.SHARED_DATA`.
pub const SHARED_DATA: &str = "/storage/emulated/0/NVForever/Data";

/// The game folder named by `data-path.txt`: its first line that is neither blank nor a
/// comment, with the surrounding space removed. Only an absolute path, because a relative one
/// would mean a different folder in every process. `None` when there is no such file, it
/// cannot be read, it is longer than one path could be, or its line is not usable.
pub fn configured_data_dir(files: &Path) -> Option<PathBuf> {
    let path = files.join(DATA_PATH_FILE);
    if path.metadata().ok()?.len() > 4096 {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let line = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'))?;
    // A leading `/`, which is what absolute means on Android (the Java launcher's
    // `OdinFork.dataPath` says so in those words); a path this platform calls absolute
    // counts too, so the tests can use a folder of their own on any machine.
    let absolute = line.starts_with('/') || Path::new(line).is_absolute();
    (absolute && !line.contains('\0')).then(|| PathBuf::from(line))
}

/// The release list the Android app asks about updates: this fork's own.
pub const FORK_RELEASES: &str = "https://api.github.com/repos/bllendev/NVForever-Android/releases?per_page=10";
/// Where this fork's release assets are downloaded from.
pub const FORK_DOWNLOADS: &str = "https://github.com/bllendev/NVForever-Android/releases/download/";
/// Upstream's release list, which the desktop launcher keeps asking.
pub const UPSTREAM_RELEASES: &str = "https://api.github.com/repos/LootJunkieAzza/NVForever/releases?per_page=10";
/// Upstream's release assets, which the desktop launcher keeps asking.
pub const UPSTREAM_DOWNLOADS: &str = "https://github.com/LootJunkieAzza/NVForever/releases/download/";

/// The release list this build asks about updates (`news::RELEASES`).
pub const RELEASES: &str = if cfg!(target_os = "android") { FORK_RELEASES } else { UPSTREAM_RELEASES };
/// The only prefix a release asset's download URL may have (`mobile::REPO_DOWNLOADS`).
pub const DOWNLOADS: &str = if cfg!(target_os = "android") { FORK_DOWNLOADS } else { UPSTREAM_DOWNLOADS };

/// What the `-debug` application id ends with, which is how a process can tell which of the
/// fork's two apps it is without asking Java.
const DEBUG_SUFFIX: &str = ".debug";

/// Set by [`adopt`] from the home's path, because the private files directory is under the
/// application id and the debug variant's ends with [`DEBUG_SUFFIX`].
static DEBUG_APP: AtomicBool = AtomicBool::new(false);

/// Whether a path lies under the debug variant's app storage.
pub fn debug_app_home(home: &Path) -> bool {
    home.components().any(|part| part.as_os_str().to_string_lossy().ends_with(DEBUG_SUFFIX))
}

/// Whether this process is the debug app, as last seen by [`adopt`].
pub fn debug_app() -> bool {
    DEBUG_APP.load(Ordering::Relaxed)
}

/// Whether a release asset's file name is the APK for the app to play, or for the debug one.
pub fn apk_matches(name: &str, debug: bool) -> bool {
    name.to_ascii_lowercase().contains("-debug") == debug
}

/// Whether a release asset's file name is the APK for **this** app: a release carries the
/// app to play and a `-debug` one beside it, and installing the other over this one would be
/// refused by Android anyway (a different package name), so it is never offered.
pub fn apk_for_this_app(name: &str) -> bool {
    apk_matches(name, debug_app())
}

/// The game this launcher should read in place, most wanted first: the folder named in
/// `data-path.txt`, then [`SHARED_DATA`]. A folder answers only when it opens as a complete
/// install, so a card that is not mounted yet, or a permission that was never granted, leaves
/// the launcher as it was rather than writing a path that does not work.
fn in_place_game(files: &Path) -> Option<fnv_install::GameInstall> {
    let chosen = configured_data_dir(files);
    let candidates = chosen.into_iter().chain(std::iter::once(PathBuf::from(SHARED_DATA)));
    candidates
        .filter_map(|path| fnv_install::GameInstall::open(path).ok())
        .find(|game| game.missing_files().is_empty())
}

/// Whether `[Game] Path` already names a complete install, in which case nothing is adopted.
fn already_ready(ini: &str) -> bool {
    fnv_install::Config::from_ini(ini)
        .game_dir
        .and_then(|path| fnv_install::GameInstall::open(path).ok())
        .is_some_and(|game| game.missing_files().is_empty())
}

/// What [`adopt`] did, for the test and for the one log line the Android launcher writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Adopted {
    /// `[Game] Path` already named a complete install, or none was found to adopt.
    Nothing,
    /// That folder is now `[Game] Path`.
    Game(PathBuf),
    /// A folder was found but the settings file could not be written.
    Failed(String),
}

/// The fork's one hook in `mobile::request`: note which of the fork's two apps this is (for
/// [`apk_for_this_app`]), and make the folder this device reads in place the launcher's game
/// folder when the launcher has none that works.
///
/// The home's parent is the app's private files directory.
pub fn adopt(home: &Path) -> Adopted {
    DEBUG_APP.store(debug_app_home(home), Ordering::Relaxed);
    let Some(files) = home.parent() else { return Adopted::Nothing };
    let config = fnv_install::EngineHome::new(home).config_file();
    let ini = std::fs::read_to_string(&config).unwrap_or_default();
    if already_ready(&ini) {
        return Adopted::Nothing;
    }
    let Some(game) = in_place_game(files) else { return Adopted::Nothing };
    let root = game.root.display().to_string();
    match crate::launch::write_ini(&config, |text| fnv_install::ini_set(&text, "Game", "Path", &root)) {
        Ok(()) => Adopted::Game(game.root),
        Err(error) => Adopted::Failed(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete-enough game folder: the required files as empty-but-not-zero-length files,
    /// which is what `missing_files` asks for.
    fn game(root: &Path) -> PathBuf {
        let data = root.join("Data");
        std::fs::create_dir_all(&data).unwrap();
        for name in fnv_install::REQUIRED_DATA_FILES {
            std::fs::write(data.join(name), b"x").unwrap();
        }
        root.to_path_buf()
    }

    fn temp(tail: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nvf-odin-{tail}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The Android app asks this fork's releases, because upstream's updater would refuse
    /// upstream's APK on this package anyway; the desktop launcher keeps asking upstream.
    #[test]
    fn the_android_updater_asks_this_forks_releases() {
        assert!(FORK_RELEASES.starts_with("https://api.github.com/repos/bllendev/NVForever-Android/"));
        assert!(FORK_DOWNLOADS.starts_with("https://github.com/bllendev/NVForever-Android/releases/download/"));
        assert!(FORK_DOWNLOADS.ends_with('/'), "a prefix every asset URL must start with");
        let fork = cfg!(target_os = "android");
        assert_eq!(RELEASES == FORK_RELEASES, fork);
        assert_eq!(DOWNLOADS == FORK_DOWNLOADS, fork);
        assert_eq!(RELEASES == UPSTREAM_RELEASES, !fork);
        assert_eq!(DOWNLOADS == UPSTREAM_DOWNLOADS, !fork);
        // The same release list the news falls back to (`news::RELEASES`).
        assert_eq!(crate::news::RELEASES, RELEASES);
    }

    /// A release carries both APKs, and each of the two apps is offered only its own: the
    /// private files directory is under the application id, so the path says which app this is.
    #[test]
    fn each_app_is_offered_only_its_own_apk() {
        let play = Path::new("/data/user/0/org.nvforever.android.odin/files/NVForever");
        let debug = Path::new("/data/user/0/org.nvforever.android.odin.debug/files/NVForever");
        assert!(!debug_app_home(play));
        assert!(debug_app_home(debug));

        assert!(apk_matches("NVForever-0.8.1-odin.32-arm64.apk", false));
        assert!(!apk_matches("NVForever-0.8.1-odin.32-arm64-debug.apk", false));
        assert!(!apk_matches("NVForever-0.8.1-odin.32-arm64.apk", true));
        assert!(apk_matches("NVForever-0.8.1-odin.32-arm64-DEBUG.apk", true), "the name's case is not the test");
        // An app that has not been noted yet is the app to play, which is what every other
        // test in this crate and every release before the debug variant assumes.
        assert!(apk_for_this_app("NVForever-0.8.1-odin.32-arm64.apk"));
    }

    /// The one line of `data-path.txt`: space, a comment and CRLF are all ordinary in a file
    /// a player may have edited; a relative path is not a path this can use.
    #[test]
    fn the_data_path_file_names_one_absolute_folder() {
        let files = temp("data-path");
        assert_eq!(configured_data_dir(&files), None, "no file at all");
        let path = files.join(DATA_PATH_FILE);
        std::fs::write(&path, "/storage/emulated/0/NVForever/Data\n").unwrap();
        assert_eq!(configured_data_dir(&files), Some(PathBuf::from("/storage/emulated/0/NVForever/Data")));
        std::fs::write(&path, "# chosen in the launcher\r\n   /storage/1234-5678/NVForever/Data   \r\n/ignored\r\n").unwrap();
        assert_eq!(configured_data_dir(&files), Some(PathBuf::from("/storage/1234-5678/NVForever/Data")));
        for bad in ["", "   \n", "Data\n", "../Data\n"] {
            std::fs::write(&path, bad).unwrap();
            assert_eq!(configured_data_dir(&files), None, "refuses {bad:?}");
        }
        std::fs::write(&path, "/storage/emulated/0/NVForever/Data\n".repeat(200)).unwrap();
        assert_eq!(configured_data_dir(&files), None, "refuses a file longer than one path");
        let _ = std::fs::remove_dir_all(&files);
    }

    /// The folder `data-path.txt` names becomes `[Game] Path`, and the launcher's own pages
    /// read it from there. The settings file keeps everything else it had.
    #[test]
    fn a_configured_folder_becomes_the_launchers_game() {
        let files = temp("adopt");
        let home = files.join("NVForever");
        std::fs::create_dir_all(&home).unwrap();
        let root = game(&files.join("card/NVForever"));
        std::fs::write(home.join("NVForever.ini"), "[Graphics]\nMSAA=4\n").unwrap();
        std::fs::write(files.join(DATA_PATH_FILE), format!("{}\n", root.join("Data").display())).unwrap();

        let opened = fnv_install::GameInstall::open(&root).unwrap().root;
        assert_eq!(adopt(&home), Adopted::Game(opened), "the game folder, not its Data folder");
        let ini = std::fs::read_to_string(home.join("NVForever.ini")).unwrap();
        assert!(ini.contains("MSAA=4"), "the other settings stay: {ini}");
        assert!(already_ready(&ini), "the launcher now has a complete game: {ini}");

        // A second call changes nothing, so a launcher that asks ten times writes once.
        assert_eq!(adopt(&home), Adopted::Nothing);
        let _ = std::fs::remove_dir_all(&files);
    }

    /// Nothing is adopted over a game folder that already works (an import upstream's picker
    /// made), and nothing is written when there is no folder to adopt.
    #[test]
    fn an_import_that_already_works_is_left_alone() {
        let files = temp("keep-import");
        let home = files.join("NVForever");
        std::fs::create_dir_all(&home).unwrap();
        let imported = game(&files.join("game-0000"));
        let other = game(&files.join("card/NVForever"));
        let ini = format!("[Game]\nPath={}\n", imported.display());
        std::fs::write(home.join("NVForever.ini"), &ini).unwrap();
        std::fs::write(files.join(DATA_PATH_FILE), format!("{}\n", other.join("Data").display())).unwrap();
        assert_eq!(adopt(&home), Adopted::Nothing, "the import keeps the folder it chose");
        assert_eq!(std::fs::read_to_string(home.join("NVForever.ini")).unwrap(), ini);

        // Nothing to adopt: an unreadable card, or a fresh install with no file at all.
        std::fs::write(home.join("NVForever.ini"), "[Graphics]\nMSAA=4\n").unwrap();
        std::fs::write(files.join(DATA_PATH_FILE), "/storage/0000-0000/NVForever/Data\n").unwrap();
        std::fs::remove_dir_all(&other).unwrap();
        assert_eq!(adopt(&home), Adopted::Nothing, "a folder that is not there is not adopted");
        assert_eq!(std::fs::read_to_string(home.join("NVForever.ini")).unwrap(), "[Graphics]\nMSAA=4\n");
        let _ = std::fs::remove_dir_all(&files);
    }
}
