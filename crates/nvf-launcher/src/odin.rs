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
//! install with no `data-path.txt` is upstream's flow untouched. Choosing the folder in the
//! launcher, rather than finding a file that is already there, is [`choose_folder`].
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

//! # The launcher rows the fork adds
//!
//! Upstream's Android launcher has four pages and no key handling; the fork adds rows to them
//! rather than a launcher of its own (docs/fork.md, "Dropped on purpose"). The decisions those
//! rows need are here and the Java that draws them is `OdinLauncher.java`:
//!
//! - **Use a folder in place**, beside upstream's importer on the Game folder page
//!   ([`Ask::ChooseFolder`]). All files access is already granted on this fork's builds, so the
//!   folder the player picks is an ordinary filesystem path and reading it needs no copy.
//! - **Load save**, the saves newest first ([`Ask::Saves`]), which starts the game on the
//!   chosen one through upstream's own one-start request ([`chosen_save`]).
//! - a line on Home when the last run ended unexpectedly ([`last_run_note`]), which is what
//!   points the player at Export debug log.
//!
//! Backing saves up and exporting a debug log need no decision from here: they are files, and
//! `SaveBackup.java` and `DebugReport.java` own them.

use serde_json::{json, Value};
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
/// The folders under one storage volume a player is likely to have copied the game to.
const VOLUME_FOLDERS: [&str; 5] = ["NVForever/Data", "Download/Data", "Data", "FalloutNV/Data", "Fallout New Vegas/Data"];

/// Every place an in-place game is looked for, in order: the folder `data-path.txt` names,
/// then the likely folders on the internal storage and on each card under `storage`.
fn candidates(files: &Path, storage: &Path) -> Vec<PathBuf> {
    let mut volumes = vec![storage.join("emulated/0")];
    if let Ok(entries) = std::fs::read_dir(storage) {
        let mut cards: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| !matches!(p.file_name().and_then(|n| n.to_str()), Some("emulated" | "self")))
            .collect();
        cards.sort();
        volumes.extend(cards);
    }
    let mut out: Vec<PathBuf> = configured_data_dir(files).into_iter().collect();
    for volume in &volumes {
        for folder in VOLUME_FOLDERS {
            let path = volume.join(folder);
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// Why the last look for an in-place game found none, for the Game folder page.
static LAST_NOTE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn last_note() -> String {
    LAST_NOTE.lock().map(|n| n.clone()).unwrap_or_default()
}

fn in_place_game(files: &Path, storage: &Path) -> Result<fnv_install::GameInstall, String> {
    let mut seen = Vec::new();
    for path in candidates(files, storage) {
        match fnv_install::GameInstall::open(&path) {
            Ok(game) => {
                let missing = game.missing_files();
                if missing.is_empty() {
                    return Ok(game);
                }
                seen.push(format!("{}: missing {}", path.display(), missing.join(", ")));
            }
            Err(_) if path.exists() => seen.push(format!("{}: no FalloutNV.esm there", path.display())),
            Err(_) => {}
        }
    }
    if seen.is_empty() {
        Err(format!(
            "No game folder found. Looked in the folder data-path.txt names and in {} on each storage volume. If your Data folder is elsewhere, or All files access is off, it cannot be seen.",
            VOLUME_FOLDERS.join(", ")
        ))
    } else {
        Err(format!("Found game folders, none complete: {}", seen.join("; ")))
    }
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

/// Where the fork's builds look for storage volumes; a folder that is not there on a desktop,
/// so a test's own folders are the only ones its scan can see.
fn storage_root() -> &'static Path {
    Path::new(if cfg!(target_os = "android") { "/storage" } else { "/nonexistent-storage" })
}

/// The look for an in-place game folder, which can take seconds on a card that has gone to
/// sleep, so it runs on a thread of its own and the launcher asks again
/// ([`checking`], `OdinLauncher.java`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    /// Nobody has looked yet, or [`look_again`] asked for another one.
    Idle,
    /// A thread is looking now.
    Running,
    /// It finished; [`last_note`] says what it found.
    Done,
}

static LOOK: std::sync::Mutex<Look> = std::sync::Mutex::new(Look::Idle);

/// Whether a look for an in-place game folder is running, which is what the Game folder page
/// says "Checking game folder..." for instead of "No complete game folder is ready".
pub fn checking() -> bool {
    LOOK.lock().map(|l| *l == Look::Running).unwrap_or(false)
}

/// Look once more the next time the launcher asks: after all files access was granted, or a
/// card was put in, the answer from before is no longer the answer.
pub fn look_again() {
    if let Ok(mut look) = LOOK.lock() {
        *look = Look::Idle;
    }
    if let Ok(mut note) = LAST_NOTE.lock() {
        note.clear();
    }
}

/// The fork's one hook in `mobile::request`: note which of the fork's two apps this is (for
/// [`apk_for_this_app`]), and make the folder this device reads in place the launcher's game
/// folder when the launcher has none that works.
///
/// The look itself is [`adopt_now`], started on a thread of its own because this is called
/// from the launcher's UI thread and a storage card can take seconds to answer. Until it
/// finishes, [`checking`] is true and the launcher asks again.
///
/// The home's parent is the app's private files directory.
pub fn adopt(home: &Path) -> Adopted {
    DEBUG_APP.store(debug_app_home(home), Ordering::Relaxed);
    if home.parent().is_none() {
        return Adopted::Nothing;
    }
    let config = fnv_install::EngineHome::new(home).config_file();
    if already_ready(&std::fs::read_to_string(&config).unwrap_or_default()) {
        if let Ok(mut look) = LOOK.lock() {
            *look = Look::Done;
        }
        return Adopted::Nothing;
    }
    let Ok(mut look) = LOOK.lock() else { return Adopted::Nothing };
    if *look != Look::Idle {
        return Adopted::Nothing;
    }
    *look = Look::Running;
    drop(look);
    let home = home.to_path_buf();
    let on_its_own = {
        let home = home.clone();
        std::thread::Builder::new()
            .name("nvf-odin-game-folder".into())
            .spawn(move || look_now(&home))
    };
    // No thread to look on: look here instead, which is slow but never silent.
    if on_its_own.is_err() {
        return look_now(&home);
    }
    Adopted::Nothing
}

/// [`adopt_now`], and then the look is over however it went.
fn look_now(home: &Path) -> Adopted {
    let found = adopt_now(home);
    if let Ok(mut look) = LOOK.lock() {
        *look = Look::Done;
    }
    found
}

/// Wait for the look [`adopt`] started, for a test and for nothing else.
#[cfg(test)]
fn wait_for_check() {
    for _ in 0..2_000 {
        if !checking() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the look for a game folder never finished");
}

/// The look itself: the folder this device reads in place becomes the launcher's `[Game] Path`
/// when the launcher has none that works.
fn adopt_now(home: &Path) -> Adopted {
    let Some(files) = home.parent() else { return Adopted::Nothing };
    let config = fnv_install::EngineHome::new(home).config_file();
    let ini = std::fs::read_to_string(&config).unwrap_or_default();
    if already_ready(&ini) {
        return Adopted::Nothing;
    }
    let game = match in_place_game(files, storage_root()) {
        Ok(game) => game,
        Err(note) => {
            if let Ok(mut last) = LAST_NOTE.lock() {
                *last = note;
            }
            return Adopted::Nothing;
        }
    };
    let root = game.root.display().to_string();
    match crate::launch::write_ini(&config, |text| fnv_install::ini_set(&text, "Game", "Path", &root)) {
        Ok(()) => Adopted::Game(game.root),
        Err(error) => Adopted::Failed(error),
    }
}

/// What the fork's own launcher rows ask for, carried by upstream's `odin` request
/// (`mobile::Request::Odin`, one arm).
#[derive(serde::Deserialize)]
#[serde(tag = "ask", rename_all = "snake_case")]
pub enum Ask {
    /// Read the game in this folder in place: the folder the player picked, or its `Data`
    /// folder. Writes `data-path.txt` and `[Game] Path`.
    ChooseFolder { path: PathBuf },
    /// Look for an in-place game folder again, after all files access was granted.
    LookAgain,
    /// The saves, newest first, for the Load save list.
    Saves,
}

/// Answers an [`Ask`]. Errors are what the launcher shows the player, so they are sentences.
pub fn handle(home: &Path, ask: Ask) -> Result<Value, String> {
    match ask {
        Ask::ChooseFolder { path } => choose_folder(home, &path).map(Value::from),
        Ask::LookAgain => {
            look_again();
            Ok(Value::Null)
        }
        Ask::Saves => Ok(saves(home)),
    }
}

/// Reads the game in `chosen` in place: the folder, or its `Data` folder, either way. It has
/// to open as a complete install before anything is written, so a folder the player picked by
/// mistake changes nothing. Then `data-path.txt` names its `Data` folder (which is what every
/// build of the fork reads, and what the engine's own `data_dir` looks for) and `[Game] Path`
/// becomes the install, which is how upstream's launcher and the engine already find the game.
///
/// Returns the folder, to show the player what was taken.
pub fn choose_folder(home: &Path, chosen: &Path) -> Result<String, String> {
    let files = home.parent().ok_or("App storage is unavailable.")?;
    if !chosen.is_absolute() || chosen.to_string_lossy().contains('\0') {
        return Err("Pick a folder on this device.".into());
    }
    let game = fnv_install::GameInstall::open(chosen)
        .map_err(|_| format!("No FalloutNV.esm in {}. Pick your Fallout: New Vegas folder or its Data folder.", chosen.display()))?;
    let missing = game.missing_files();
    if !missing.is_empty() {
        return Err(format!("That game folder is incomplete: missing {}.", missing.join(", ")));
    }
    let data = game.root.join("Data");
    std::fs::write(files.join(DATA_PATH_FILE), format!("{}\n", data.display()))
        .map_err(|e| format!("Could not remember that folder: {e}"))?;
    let root = game.root.display().to_string();
    let config = fnv_install::EngineHome::new(home).config_file();
    crate::launch::write_ini(&config, |text| fnv_install::ini_set(&text, "Game", "Path", &root))?;
    if let Ok(mut look) = LOOK.lock() {
        *look = Look::Done;
    }
    if let Ok(mut note) = LAST_NOTE.lock() {
        note.clear();
    }
    Ok(root)
}

/// The home's saves, newest first: the name the player sees, when it was saved (seconds since
/// the Unix epoch, which the launcher shows in the device's own time) and how big the file is.
pub fn saves(home: &Path) -> Value {
    let dir = fnv_install::EngineHome::new(home).saves();
    let list: Vec<Value> = fnv_save::list(&dir)
        .into_iter()
        .map(|(path, header)| {
            let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            json!({
                "name": name,
                "savedUnix": header.saved_unix,
                "size": size,
                "player": header.player_name,
                "level": header.level,
            })
        })
        .collect();
    json!({"saves": list})
}

/// The save a start was asked to load, by the name the Load save list showed. `None` when the
/// launcher asked for no particular save, which leaves upstream's Continue and Play as they
/// are. A name that is not one save's, or a save that has gone since the list was drawn, is an
/// error the player sees rather than a start with no save.
pub fn chosen_save(home: &Path, name: Option<&str>) -> Result<Option<PathBuf>, String> {
    let Some(name) = name else { return Ok(None) };
    let dir = fnv_install::EngineHome::new(home).saves();
    let path = fnv_save::path_of(&dir, name).map_err(|_| "That is not one of your saves.".to_string())?;
    if !path.is_file() {
        return Err(format!("{name} is no longer there. The save list has changed."));
    }
    Ok(Some(path))
}

/// What the newest run log says the last run came to, when it did not end normally, for the
/// line Home shows above Export debug log. `None` when there is no log, or the last run ended
/// as it should.
///
/// The engine says so in the log itself and nowhere a launcher can read, so this reads the
/// same three marks `crash.rs` writes: the clean exit, the start that failed, and a crash
/// report having been written.
pub fn last_run_note(home: &Path) -> Option<String> {
    let logs = fnv_install::EngineHome::new(home).logs();
    let mut names: Vec<String> = std::fs::read_dir(&logs)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("nvforever-") && n.ends_with(".log"))
        .collect();
    names.sort();
    let newest = names.pop()?;
    let text = read_tail(&logs.join(&newest), 256 * 1024)?;
    if !ended_abnormally(&text) {
        return None;
    }
    Some(format!(
        "The last run ({newest}) did not end normally. Export debug log keeps that log, so it can be sent before it is rotated away."
    ))
}

/// Whether a run log's text shows a run that did not end normally. The three marks are
/// `crash.rs`'s own (`CLEAN_EXIT`, the start that failed, and a crash report being written);
/// a run still going has none of them, so a log that is being written counts as abnormal only
/// once the game is not running, which is the only time the launcher is drawn.
fn ended_abnormally(text: &str) -> bool {
    !text.contains("run log: the game exited normally")
        && !text.contains("run log: the game stopped at the start:")
        && !text.contains("crash report written to")
}

/// The last `bytes` of a file, as text, with any partial character at the cut dropped. A run
/// log of any size costs one read of this size.
fn read_tail(path: &Path, bytes: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length > bytes {
        file.seek(SeekFrom::Start(length - bytes)).ok()?;
    }
    let mut raw = Vec::new();
    file.take(bytes).read_to_end(&mut raw).ok()?;
    Some(String::from_utf8_lossy(&raw).into_owned())
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

    /// `LOOK` and `LAST_NOTE` belong to the process, so the two tests that drive them take
    /// turns rather than racing each other under cargo's own threads.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    /// A card's folder is found with no data-path.txt at all, and an incomplete one is named
    /// with what it lacks.
    #[test]
    fn a_folder_on_a_card_is_found_and_an_incomplete_one_is_explained() {
        let files = temp("cards");
        let storage = files.join("storage");
        std::fs::create_dir_all(storage.join("emulated/0")).unwrap();
        let root = game(&storage.join("75D7-DC5F/Download"));
        let found = in_place_game(&files, &storage).unwrap();
        assert_eq!(found.root, fnv_install::GameInstall::open(&root).unwrap().root);

        std::fs::remove_file(root.join("Data/Update.bsa")).unwrap();
        let note = in_place_game(&files, &storage).unwrap_err();
        assert!(note.contains("missing Update.bsa"), "{note}");
        std::fs::remove_dir_all(&root).unwrap();
        let note = in_place_game(&files, &storage).unwrap_err();
        assert!(note.starts_with("No game folder found"), "{note}");
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
        assert_eq!(adopt_now(&home), Adopted::Game(opened), "the game folder, not its Data folder");
        let ini = std::fs::read_to_string(home.join("NVForever.ini")).unwrap();
        assert!(ini.contains("MSAA=4"), "the other settings stay: {ini}");
        assert!(already_ready(&ini), "the launcher now has a complete game: {ini}");

        // A second call changes nothing, so a launcher that asks ten times writes once.
        assert_eq!(adopt_now(&home), Adopted::Nothing);
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
        assert_eq!(adopt_now(&home), Adopted::Nothing, "the import keeps the folder it chose");
        assert_eq!(std::fs::read_to_string(home.join("NVForever.ini")).unwrap(), ini);

        // Nothing to adopt: an unreadable card, or a fresh install with no file at all.
        std::fs::write(home.join("NVForever.ini"), "[Graphics]\nMSAA=4\n").unwrap();
        std::fs::write(files.join(DATA_PATH_FILE), "/storage/0000-0000/NVForever/Data\n").unwrap();
        std::fs::remove_dir_all(&other).unwrap();
        assert_eq!(adopt_now(&home), Adopted::Nothing, "a folder that is not there is not adopted");
        assert_eq!(std::fs::read_to_string(home.join("NVForever.ini")).unwrap(), "[Graphics]\nMSAA=4\n");
        let _ = std::fs::remove_dir_all(&files);
    }

    /// The look runs on a thread of its own, so the launcher can say "Checking game
    /// folder..." instead of freezing while a card wakes up, and it runs once.
    #[test]
    fn the_look_happens_off_the_launchers_thread_and_once() {
        let _serial = ONE_AT_A_TIME.lock();
        let files = temp("async-look");
        let home = files.join("NVForever");
        std::fs::create_dir_all(&home).unwrap();
        let root = game(&files.join("card/NVForever"));
        std::fs::write(files.join(DATA_PATH_FILE), format!("{}\n", root.join("Data").display())).unwrap();

        look_again();
        assert_eq!(adopt(&home), Adopted::Nothing, "the answer comes later, not from this call");
        wait_for_check();
        let ini = std::fs::read_to_string(home.join("NVForever.ini")).unwrap();
        assert!(already_ready(&ini), "the folder was adopted on the other thread: {ini}");
        assert!(!checking(), "and the page stops saying it is checking");

        // A second ask starts no second look, and neither does a third once it is ready.
        assert_eq!(adopt(&home), Adopted::Nothing);
        assert!(!checking());
        let _ = std::fs::remove_dir_all(&files);
    }

    /// Choosing a folder in the launcher: the game folder or its Data folder, either way,
    /// and nothing is written for a folder that is not a complete game.
    #[test]
    fn a_chosen_folder_is_checked_before_anything_is_written() {
        let _serial = ONE_AT_A_TIME.lock();
        let files = temp("choose");
        let home = files.join("NVForever");
        std::fs::create_dir_all(&home).unwrap();
        let root = game(&files.join("card/Fallout New Vegas"));

        for refused in [files.join("card"), files.join("nowhere"), PathBuf::from("Data")] {
            let error = choose_folder(&home, &refused).unwrap_err();
            assert!(error.ends_with('.'), "a sentence for the player: {error}");
            assert!(!files.join(DATA_PATH_FILE).exists(), "nothing was remembered: {refused:?}");
        }

        let opened = fnv_install::GameInstall::open(&root).unwrap().root;
        assert_eq!(choose_folder(&home, &root).unwrap(), opened.display().to_string());
        assert_eq!(configured_data_dir(&files), Some(root.join("Data")), "the Data folder is remembered");
        assert!(already_ready(&std::fs::read_to_string(home.join("NVForever.ini")).unwrap()));

        // The Data folder is as good a thing to pick, and names the same install.
        std::fs::remove_file(files.join(DATA_PATH_FILE)).unwrap();
        assert_eq!(choose_folder(&home, &root.join("Data")).unwrap(), opened.display().to_string());
        assert_eq!(configured_data_dir(&files), Some(root.join("Data")));

        // An incomplete game says what it lacks and remembers nothing new.
        std::fs::remove_file(root.join("Data/Update.bsa")).unwrap();
        std::fs::remove_file(files.join(DATA_PATH_FILE)).unwrap();
        let error = choose_folder(&home, &root).unwrap_err();
        assert!(error.contains("missing Update.bsa"), "{error}");
        assert_eq!(configured_data_dir(&files), None);
        let _ = std::fs::remove_dir_all(&files);
    }

    /// The Load save list names a save, and a start can only load one that is there.
    #[test]
    fn a_start_loads_only_a_save_of_this_home() {
        let files = temp("saves");
        let home = files.join("NVForever");
        let dir = home.join("saves");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Goodsprings.nvfsave"), b"not a real save").unwrap();

        assert_eq!(chosen_save(&home, None).unwrap(), None, "Play and Continue are unchanged");
        assert_eq!(chosen_save(&home, Some("Goodsprings")).unwrap(), Some(dir.join("Goodsprings.nvfsave")));
        for bad in ["../NVForever/Goodsprings", "Goodsprings.nvfsave", "Novac", ""] {
            assert!(chosen_save(&home, Some(bad)).is_err(), "refuses {bad:?}");
        }
        // The list itself reads headers, so a file that is not a save is simply not in it.
        assert_eq!(saves(&home)["saves"].as_array().map(Vec::len), Some(0));
        let _ = std::fs::remove_dir_all(&files);
    }

    /// Home says when the last run did not end normally, and says nothing when it did.
    #[test]
    fn home_says_when_the_last_run_ended_unexpectedly() {
        let files = temp("last-run");
        let home = files.join("NVForever");
        let logs = home.join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        assert_eq!(last_run_note(&home), None, "no log at all");

        std::fs::write(logs.join("nvforever-20261127-120000-42.log"), "started\nand then nothing\n").unwrap();
        let note = last_run_note(&home).expect("an unfinished log is an abnormal run");
        assert!(note.contains("nvforever-20261127-120000-42.log"), "{note}");
        assert!(note.contains("Export debug log"), "it points at the row that sends it: {note}");

        // Each of the three marks crash.rs writes means the run is accounted for.
        for ending in ["run log: the game exited normally", "run log: the game stopped at the start: no game folder", "crash report written to logs/crash-1.txt"] {
            std::fs::write(logs.join("nvforever-20261127-120000-42.log"), format!("started\n{ending}\n")).unwrap();
            assert_eq!(last_run_note(&home), None, "accounted for by {ending:?}");
        }
        // Only the newest log is the last run.
        std::fs::write(logs.join("nvforever-20261127-130000-43.log"), "started\n").unwrap();
        assert!(last_run_note(&home).is_some_and(|n| n.contains("130000-43")));
        let _ = std::fs::remove_dir_all(&files);
    }
}
