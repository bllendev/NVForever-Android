//! The data probe: opens the game's `Data` folder through `fnv-vfs` and says what it found.
//!
//! This is the Milestone 1 proof that the *data layer* (BSA parsing, zlib inflate, the
//! archive-precedence rules) runs unchanged on an ARM phone. It is deliberately free of
//! Bevy and of any Android API so it can be unit-tested on the host (`cargo test`), and
//! so later milestones can reuse it from the real loading screen.
//!
//! Nothing here writes to disk: the game's `Data` folder is read-only input
//! (docs/design/install-layout.md).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The plugin every Fallout: New Vegas install has; its presence is the cheapest check
/// that the user pointed us at the right folder.
const BASE_PLUGIN: &str = "FalloutNV.esm";

/// What [`probe`] found. `Display` renders the multi-line text the on-screen overlay shows.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeReport {
    /// The folder that was probed.
    pub data_dir: PathBuf,
    /// How the probe ended.
    pub outcome: Outcome,
}

/// The ways a probe can end. Each one is a message the player can act on.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The folder does not exist (or cannot be read): wrong path or no storage access.
    MissingFolder,
    /// The folder exists but `FalloutNV.esm` is not in it: not a `Data` folder.
    NoBasePlugin,
    /// `fnv-vfs` could not open the archives (corrupt or truncated BSA, I/O error).
    VfsError(String),
    /// Everything opened.
    Ok(Found),
}

/// The numbers a successful probe reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// Archives (BSA) opened, in precedence order, lowest first.
    pub archives: Vec<String>,
    /// Files visible through the VFS (archives and loose files, one entry per path).
    pub files: usize,
    /// Of those, how many are meshes (`.nif`).
    pub meshes: usize,
    /// A mesh that was extracted from an archive as a decompression check, if one exists:
    /// its path and its size in bytes.
    pub sample: Option<(String, usize)>,
    /// Time to open and index everything.
    pub open_time: Duration,
}

/// Probe `data_dir`. Blocking, and slow on a real install (it indexes every archive), so
/// the app runs it on a background task.
pub fn probe(data_dir: &Path) -> ProbeReport {
    let report = |outcome| ProbeReport { data_dir: data_dir.to_path_buf(), outcome };

    if !data_dir.is_dir() {
        return report(Outcome::MissingFolder);
    }
    if !data_dir.join(BASE_PLUGIN).is_file() {
        return report(Outcome::NoBasePlugin);
    }

    let started = Instant::now();
    let vfs = match fnv_vfs::Vfs::open_default(data_dir) {
        Ok(v) => v,
        Err(e) => return report(Outcome::VfsError(e.to_string())),
    };
    let open_time = started.elapsed();

    let meshes = vfs.paths(|p| p.ends_with(".nif"));
    // Decompression check: read the first mesh that really lives in an archive. A loose
    // file would only test `std::fs`, which is not what Android could break.
    let sample = meshes
        .iter()
        .find(|p| vfs.source_of(p).is_some_and(|s| s != "loose"))
        .and_then(|p| match vfs.read(p) {
            Some(Ok(bytes)) => Some((p.clone(), bytes.len())),
            _ => None,
        });

    report(Outcome::Ok(Found {
        archives: vfs.archives().map(str::to_owned).collect(),
        files: vfs.paths(|_| true).len(),
        meshes: meshes.len(),
        sample,
        open_time,
    }))
}

impl std::fmt::Display for ProbeReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Data: {}", self.data_dir.display())?;
        match &self.outcome {
            Outcome::MissingFolder => write!(
                f,
                "Folder not found. Choose the Fallout New Vegas Data folder in the launcher."
            ),
            Outcome::NoBasePlugin => write!(f, "Folder found, but {BASE_PLUGIN} is not in it.\nIs this the game's Data folder?"),
            Outcome::VfsError(e) => write!(f, "Could not open the archives:\n{e}"),
            Outcome::Ok(found) => {
                writeln!(f, "{} archives, {} files ({} meshes), opened in {:.1}s", found.archives.len(), found.files, found.meshes, found.open_time.as_secs_f32())?;
                match &found.sample {
                    Some((path, size)) => write!(f, "Inflate check OK: {path} = {size} bytes"),
                    None => write!(f, "No mesh in an archive to test inflate with."),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch folder under the system temp dir, removed on drop.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new(name: &str) -> Scratch {
            let p = std::env::temp_dir().join(format!("nvf-android-probe-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Scratch(p)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_folder_is_reported() {
        let r = probe(Path::new("/definitely/not/here"));
        assert_eq!(r.outcome, Outcome::MissingFolder);
        assert!(r.to_string().contains("Folder not found"));
    }

    #[test]
    fn folder_without_base_plugin_is_reported() {
        let s = Scratch::new("noplugin");
        let r = probe(&s.0);
        assert_eq!(r.outcome, Outcome::NoBasePlugin);
        assert!(r.to_string().contains("FalloutNV.esm"));
    }

    #[test]
    fn loose_files_only_install_opens_without_archives() {
        // No BSA archives, one loose mesh: the probe must open it and count the mesh, and
        // must NOT claim an inflate check (a loose file proves nothing about archives).
        let s = Scratch::new("loose");
        std::fs::write(s.0.join(BASE_PLUGIN), b"TES4").unwrap();
        std::fs::create_dir_all(s.0.join("meshes").join("x")).unwrap();
        std::fs::write(s.0.join("meshes").join("x").join("a.nif"), b"nif").unwrap();

        let report = probe(&s.0);
        let Outcome::Ok(f) = &report.outcome else { panic!("unexpected outcome: {:?}", report.outcome) };
        assert!(f.archives.is_empty());
        assert_eq!(f.files, 1);
        assert_eq!(f.meshes, 1);
        assert_eq!(f.sample, None);
        // The overlay text says so instead of pretending the inflate path was exercised.
        assert!(report.to_string().contains("No mesh in an archive"));
    }
}
