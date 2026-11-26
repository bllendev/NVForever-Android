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

use std::path::{Path, PathBuf};

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

/// The fork's one hook in `mobile::request`: make the folder this device reads in place the
/// launcher's game folder, when the launcher has none that works.
///
/// `files` is the app's private files directory, which is the engine home's parent.
pub fn adopt(home: &Path) -> Adopted {
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
