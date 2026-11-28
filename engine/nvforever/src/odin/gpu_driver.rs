//! A Vulkan driver the player supplied (Mesa Turnip for Adreno), opened through libadrenotools,
//! and the marker that sends the next start back to the system driver when one does not survive.
//!
//! Qualcomm's Adreno driver is part of the phone's system image and cannot be replaced, but an
//! app may load another one into itself: libadrenotools (BSD-2-Clause, bylaws/libadrenotools)
//! opens the system Vulkan loader in a linker namespace of its own, with a hook that redirects
//! the loader's search for the vendor driver to a library in the app's private storage. That is
//! how the Android emulators and the native ports run Mesa's Turnip on Adreno phones, and it is
//! what this module does. NVForever ships no driver: the player installs a driver zip in the
//! launcher (`GpuDrivers.java`, docs/odin-drivers.md).
//!
//! ```text
//! <internal files>/gpu_drivers/
//!     <id>/                     one installed driver: its .so, and meta.json if the zip had one
//!     selected                  the driver to use ("folder <id>", "library <soname>", "label"),
//!                               absent for the system driver
//!     renderer_starting         written just before the renderer starts, removed by this run
//!                               after [`DEFAULT_FRAMES`] drawn frames
//!     last_fallback             why the last start did not use the selected driver, for the
//!                               launcher to show; absent when the last start was as asked
//!     native_lib_dir            optional: the launcher's ApplicationInfo.nativeLibraryDir
//! ```
//!
//! **With no `selected` file nothing here runs**: no file is read or written, no library is
//! opened, no variable is set, and the run is the run it was before this existed.
//!
//! **The call chain.** [`start`] reads the selection, then `dlopen("libadrenotools.so")` from
//! the app's own native library folder, `dlsym("adrenotools_open_libvulkan")`, and calls it with
//! `ADRENOTOOLS_DRIVER_CUSTOM`, the folder holding the hook libraries (the app's native library
//! folder, which must be the one `ApplicationInfo.nativeLibraryDir` names) and the driver's
//! folder and soname. What comes back is a handle on the hooked Vulkan loader;
//! `dlsym("vkGetInstanceProcAddr")` on it goes to the vendored wgpu-hal's Android hook, whose
//! Vulkan backend then builds its `ash::Entry` from that pointer instead of loading
//! `libvulkan.so` itself. All of this happens in `android::prepare`, before the engine starts,
//! so no Vulkan instance exists yet.
//!
//! **Falling back.** Three things put a run back on the system driver while keeping the
//! player's selection: a `renderer_starting` file left behind by the previous start (the
//! selected driver did not survive its own first frames), the Vulkan validation crash test
//! being on (the layer chain under libadrenotools' private namespace is not something this
//! port has checked, and a crash test that cannot be trusted is worse than none), and
//! libadrenotools failing to open the driver. Each writes its reason to `last_fallback`, which
//! the launcher shows.
//!
//! **What the engine itself does** ([`GpuDriverPlugin`], added from [`super::before_run`]):
//! says in the run log which driver was asked for and what the adapter turned out to be, and
//! removes the start marker once the run has drawn [`DEFAULT_FRAMES`] frames. A start that
//! never gets that far leaves the file behind, which is how the next start knows.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;

/// The folder of everything in the module notes, below the app's private files folder (a
/// driver must not be loaded from shared storage: `dlopen` refuses a library any app could
/// rewrite).
pub const DIR: &str = "gpu_drivers";

/// The Vulkan driver the platform shell was asked for, in the words the run log should carry
/// (`System`, `Turnip 25.0.3`, ...): one startup line says it beside what the adapter turned
/// out to be. Set only by [`start`], and only when the player selected a driver of their own;
/// unset everywhere else, where nothing is logged.
pub const GPU_DRIVER_ENV: &str = "NVF_GPU_DRIVER";

/// The file [`start`] wrote before the renderer started, removed once this run has drawn its
/// frames. A file still there at the next start is how the shell knows the driver the player
/// selected did not survive its own first frames.
pub const RENDERER_MARKER_ENV: &str = "NVF_RENDERER_MARKER";

/// How many drawn frames [`RENDERER_MARKER_ENV`]'s file must survive, instead of
/// [`DEFAULT_FRAMES`]. A test aid, not a setting.
pub const RENDERER_MARKER_FRAMES_ENV: &str = "NVF_RENDERER_MARKER_FRAMES";

/// How many drawn frames a start must reach before it counts as survived. Three hundred is
/// about five seconds at 60 frames a second: past instance and device creation, past the first
/// swapchain, past the menu's first draws, and well past the frame or two in which the Odin 3's
/// device loss happens (gpu_recorder/).
pub const DEFAULT_FRAMES: u32 = 300;

/// The player's choice; absent means the system driver.
const SELECTED: &str = "selected";
/// Written before the renderer starts, removed once the run has drawn its frames.
const MARKER: &str = "renderer_starting";
/// Why the last start did not use the selected driver (the launcher reads it).
const LAST_FALLBACK: &str = "last_fallback";
/// The launcher's copy of `ApplicationInfo.nativeLibraryDir`, used when `dladdr` cannot say.
/// Android only, like the code that opens a driver at all.
#[cfg(target_os = "android")]
const NATIVE_LIB_DIR: &str = "native_lib_dir";

/// libadrenotools itself, and the one call this port makes into it.
#[cfg(target_os = "android")]
const ADRENOTOOLS_SONAME: &str = "libadrenotools.so";
/// `ADRENOTOOLS_DRIVER_CUSTOM` [documented: `include/adrenotools/priv.h`, `1 << 0`].
#[cfg(target_os = "android")]
const ADRENOTOOLS_DRIVER_CUSTOM: i32 = 1;

/// The driver the player selected, as the `selected` file names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// The installed driver's folder below the drivers folder: a plain name, never a path.
    pub folder: String,
    /// The driver library's soname inside that folder, for example `libvulkan_freedreno.so`.
    pub library: String,
    /// What to call it in the log and in the launcher (the zip's `meta.json` name, else the id).
    pub label: String,
}

/// What this run does about the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// No driver selected: nothing is read, written or loaded.
    System,
    /// A driver is selected, and this run tries to open it.
    Try(Selection),
    /// A driver is selected, and this run stays on the system driver for the reason given. The
    /// selection is kept: the player chose it, and only the player unchooses it.
    FallBack(Selection, String),
}

/// A name that may be joined to the drivers folder: one path element, nothing that climbs out.
fn plain_name(word: &str) -> bool {
    !word.is_empty() && word != "." && word != ".." && !word.contains(['/', '\\', '\0'])
}

/// The `selected` file: one `name value` a line, as `odin-advanced.txt` is, written by
/// `GpuDrivers.select`. Anything missing, empty or shaped like a path means no selection, so a
/// damaged file leaves the run on the system driver rather than on a guess.
pub fn parse_selection(text: &str) -> Option<Selection> {
    let mut folder = None;
    let mut library = None;
    let mut label = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        // A blank line, or a name without a value, is skipped rather than fatal.
        let Some((name, value)) = line.split_once(char::is_whitespace) else { continue };
        let value = value.trim();
        match name {
            "folder" => folder = Some(value.to_string()),
            // The label is the only free text here: it is shown, never used as a path.
            "label" => label = Some(value.to_string()),
            "library" => library = Some(value.to_string()),
            _ => {}
        }
    }
    let folder = folder.filter(|f| plain_name(f))?;
    let library = library.filter(|l| plain_name(l))?;
    let label = label.filter(|l| !l.is_empty()).unwrap_or_else(|| folder.clone());
    Some(Selection { folder, library, label })
}

/// What to do, given the selection, whether the previous start left its marker behind, and
/// whether the Vulkan validation crash test is on. No file is touched here, so every branch is
/// checkable without a phone.
pub fn decide(selection: Option<Selection>, marker_left: bool, validation: bool) -> Decision {
    let Some(selection) = selection else { return Decision::System };
    if marker_left {
        let reason = format!(
            "the last start on {} did not reach {DEFAULT_FRAMES} drawn frames, so this run is on the system driver. \
             Start again to try it once more, or choose System in the launcher.",
            selection.label,
        );
        return Decision::FallBack(selection, reason);
    }
    if validation {
        let reason = format!(
            "the Vulkan validation crash test was on, which runs on the system driver; {} was not used. \
             Turn validation off to run on it.",
            selection.label,
        );
        return Decision::FallBack(selection, reason);
    }
    Decision::Try(selection)
}

/// Opens the selected driver, if there is one, and tells the renderer to use it. `files` is the
/// app's private files folder. Returns the words the run log and the engine's startup line
/// carry, or `None` when no driver is selected and nothing at all was done.
///
/// Call before the engine starts and before any Vulkan instance exists (`android::prepare`),
/// and after [`super::crash_tests::apply`], whose validation switch this reads.
pub fn start(files: &Path) -> Option<String> {
    let root = files.join(DIR);
    let selection = std::fs::read_to_string(root.join(SELECTED)).ok().and_then(|text| parse_selection(&text));
    // Nothing selected: not a file read, not a line logged.
    selection.as_ref()?;
    let marker = root.join(MARKER);
    let marker_left = marker.exists();
    let decision = decide(selection, marker_left, super::vulkan_validation::requested());
    let words = match decision {
        Decision::System => return None,
        Decision::FallBack(selection, why) => {
            // The marker has done its work; the next start may try the driver again.
            let _ = std::fs::remove_file(&marker);
            record_fallback(&root, &why);
            eprintln!("gpu driver: {} is selected but not used: {why}", selection.label);
            format!("{} (not used this run), the system driver instead", selection.label)
        }
        Decision::Try(selection) => {
            let dir = root.join(&selection.folder);
            match open_driver(&dir, &selection.library) {
                Ok(()) => {
                    // Before the renderer: a start that dies in instance or device creation
                    // leaves this file, and the next start reads it and stays on the system
                    // driver.
                    if let Err(e) = write_marker(&root, &marker) {
                        eprintln!("gpu driver: the start marker could not be written ({e}); a failed start will not be noticed");
                    } else {
                        // Before any thread starts; read by `GpuDriverPlugin`.
                        std::env::set_var(RENDERER_MARKER_ENV, &marker);
                    }
                    let _ = std::fs::remove_file(root.join(LAST_FALLBACK));
                    eprintln!("gpu driver: {} opened through libadrenotools from {}", selection.label, dir.display());
                    selection.label
                }
                Err(why) => {
                    record_fallback(&root, &why);
                    eprintln!("gpu driver: {} could not be opened: {why}; the system driver instead", selection.label);
                    format!("{} (could not be opened), the system driver instead", selection.label)
                }
            }
        }
    };
    // The one line the run log carries about the driver, and what the startup line says was
    // asked for (`announce`).
    std::env::set_var(GPU_DRIVER_ENV, &words);
    Some(words)
}

/// The marker of a run that is well enough to ask the launcher to start another one (loading a
/// save from the main menu, which can happen inside the first [`DEFAULT_FRAMES`] frames). It
/// says nothing about the driver, so it goes rather than making the next start fall back: one
/// line in `android::restart`.
pub fn restarting() {
    if let Some(marker) = std::env::var_os(RENDERER_MARKER_ENV).filter(|m| !m.is_empty()) {
        let _ = std::fs::remove_file(PathBuf::from(marker));
    }
}

/// Writes the start marker, making its folder first (the player may have cleared it).
fn write_marker(root: &Path, marker: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(marker, b"the renderer was starting when this file was written\n")
}

/// Leaves the reason for the launcher's "the last start fell back to the system driver" note.
fn record_fallback(root: &Path, why: &str) {
    let _ = std::fs::create_dir_all(root);
    if let Err(e) = std::fs::write(root.join(LAST_FALLBACK), why.as_bytes()) {
        eprintln!("gpu driver: the fallback reason could not be written: {e}");
    }
}

/// The folder the app's own libraries were extracted to, which is both where the libadrenotools
/// hooks live and what its `hookLibDir` must be (`ApplicationInfo.nativeLibraryDir`). Asked of
/// the dynamic linker, which knows where this very library was loaded from, so it stays right
/// across an app update; the launcher's file is the fallback for a linker that will not say.
#[cfg(target_os = "android")]
fn native_lib_dir(root: &Path) -> Option<PathBuf> {
    /// An address inside this library, for `dladdr` to name the library by.
    static ANCHOR: u8 = 0;
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    // SAFETY: `dladdr` reads the address and fills the structure; the name it points at is the
    // linker's own string, read before anything can unload this library (nothing does).
    let named = unsafe { libc::dladdr(std::ptr::addr_of!(ANCHOR).cast(), &mut info) != 0 && !info.dli_fname.is_null() };
    if named {
        let own = unsafe { std::ffi::CStr::from_ptr(info.dli_fname) }.to_string_lossy().into_owned();
        if let Some(dir) = Path::new(&own).parent().filter(|d| d.is_dir()) {
            return Some(dir.to_path_buf());
        }
    }
    let text = std::fs::read_to_string(root.join(NATIVE_LIB_DIR)).ok()?;
    let line = text.lines().next()?.trim().to_string();
    (!line.is_empty()).then(|| PathBuf::from(line))
}

/// `adrenotools_open_libvulkan` [documented: `include/adrenotools/driver.h`].
#[cfg(target_os = "android")]
type OpenLibvulkan = unsafe extern "C" fn(
    dlopen_mode: libc::c_int,
    feature_flags: libc::c_int,
    tmp_lib_dir: *const libc::c_char,
    hook_lib_dir: *const libc::c_char,
    custom_driver_dir: *const libc::c_char,
    custom_driver_name: *const libc::c_char,
    file_redirect_dir: *const libc::c_char,
    user_mapping_handle: *mut *mut libc::c_void,
) -> *mut libc::c_void;

/// The last `dlopen`/`dlsym` failure in words.
#[cfg(target_os = "android")]
fn dl_error() -> String {
    // SAFETY: `dlerror` returns either null or a NUL-terminated string owned by the linker.
    unsafe {
        let error = libc::dlerror();
        if error.is_null() {
            "no reason given".to_string()
        } else {
            std::ffi::CStr::from_ptr(error).to_string_lossy().into_owned()
        }
    }
}

/// Opens `dir/library` as this process's Vulkan implementation and hands its
/// `vkGetInstanceProcAddr` to the renderer. `Err` is the sentence the player is shown.
#[cfg(target_os = "android")]
fn open_driver(dir: &Path, library: &str) -> Result<(), String> {
    use std::ffi::CString;

    let driver = dir.join(library);
    if !driver.is_file() {
        return Err(format!("{} is not there (was the driver deleted?)", driver.display()));
    }
    let root = dir.parent().unwrap_or(dir).to_path_buf();
    let hook_dir = native_lib_dir(&root).ok_or("the app's native library folder could not be found")?;
    // libadrenotools joins the folder and the soname as they are given, so the folder must end
    // in a separator [documented: `src/driver.cpp`, `std::string(customDriverDir) + name`].
    let driver_dir = CString::new(format!("{}/", dir.display())).map_err(|_| "the driver's folder is not a usable name".to_string())?;
    let driver_name = CString::new(library).map_err(|_| "the driver's name is not usable".to_string())?;
    let hook_dir = CString::new(hook_dir.display().to_string()).map_err(|_| "the app's native library folder is not a usable name".to_string())?;
    let adrenotools = CString::new(ADRENOTOOLS_SONAME).expect("a constant without a NUL");
    let open_name = c"adrenotools_open_libvulkan";
    let entry_name = c"vkGetInstanceProcAddr";

    // SAFETY: plain `dlopen`/`dlsym` of NUL-terminated names, and one call into libadrenotools
    // with the arguments its header documents (every pointer either null or a live CString for
    // the length of the call). Neither handle is ever closed: the driver stays for the run.
    unsafe {
        let tools = libc::dlopen(adrenotools.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if tools.is_null() {
            return Err(format!("{ADRENOTOOLS_SONAME} is not in this APK ({})", dl_error()));
        }
        let open = libc::dlsym(tools, open_name.as_ptr());
        if open.is_null() {
            return Err(format!("{ADRENOTOOLS_SONAME} has no adrenotools_open_libvulkan ({})", dl_error()));
        }
        let open: OpenLibvulkan = std::mem::transmute(open);
        let vulkan = open(
            libc::RTLD_NOW | libc::RTLD_LOCAL,
            ADRENOTOOLS_DRIVER_CUSTOM,
            // No temporary folder: on Android 10 and later libadrenotools uses a memfd instead,
            // and below that it will say so by failing here rather than writing a patched
            // library into storage.
            std::ptr::null(),
            hook_dir.as_ptr(),
            driver_dir.as_ptr(),
            driver_name.as_ptr(),
            // No file redirection and no imported GPU mapping: this port wants the driver only.
            std::ptr::null(),
            std::ptr::null_mut(),
        );
        if vulkan.is_null() {
            return Err("libadrenotools refused it: the driver may be built for another Adreno generation, \
                        or this phone's linker is too old (Android 9 and later only)"
                .to_string());
        }
        let entry = libc::dlsym(vulkan, entry_name.as_ptr());
        if entry.is_null() {
            return Err(format!("the driver has no vkGetInstanceProcAddr ({})", dl_error()));
        }
        // SAFETY: `entry` is the hooked Vulkan loader's own `vkGetInstanceProcAddr`, from a
        // library that is never closed, which is what the renderer's hook requires of it.
        let entry: ash::vk::PFN_vkGetInstanceProcAddr = std::mem::transmute(entry);
        if !wgpu::hal::vulkan::set_android_vk_get_instance_proc_addr(entry) {
            return Err("the renderer already has a Vulkan entry point for this process".to_string());
        }
    }
    Ok(())
}

/// Desktop builds (`cargo check` on a PC) have no libadrenotools and no Adreno.
#[cfg(not(target_os = "android"))]
fn open_driver(_dir: &Path, _library: &str) -> Result<(), String> {
    Err("a driver of your own is an Android feature".to_string())
}

/// How many frames the marker should survive: [`RENDERER_MARKER_FRAMES_ENV`] when it holds a
/// positive number, [`DEFAULT_FRAMES`] otherwise (including a value that is not a number, so a
/// mistyped test aid cannot make a start count as survived at once).
pub fn frames_wanted(given: Option<&str>) -> u32 {
    given.and_then(|text| text.trim().parse::<u32>().ok()).filter(|frames| *frames > 0).unwrap_or(DEFAULT_FRAMES)
}

/// The start marker and the frames still to go. Counting and removal are here, away from Bevy,
/// so both can be checked without a renderer.
#[derive(Resource, Debug)]
pub struct StartMarker {
    path: PathBuf,
    wanted: u32,
    seen: u32,
    done: bool,
}

impl StartMarker {
    /// The marker at `path`, removed once `wanted` frames have been drawn.
    pub fn new(path: impl Into<PathBuf>, wanted: u32) -> Self {
        Self { path: path.into(), wanted, seen: 0, done: false }
    }

    /// One drawn frame. Returns the file to remove on the frame the count is reached, and
    /// `None` on every other frame, so the removal is attempted exactly once a run.
    pub fn frame(&mut self) -> Option<&Path> {
        if self.done {
            return None;
        }
        self.seen += 1;
        if self.seen < self.wanted {
            return None;
        }
        self.done = true;
        Some(&self.path)
    }
}

/// Says which driver this run asked for and what the adapter turned out to be, and removes the
/// start marker once the run has drawn enough frames. Both halves are off unless [`start`] set
/// their variables, so a desktop run and an Android run without a selected driver add nothing
/// at all.
pub struct GpuDriverPlugin;

impl Plugin for GpuDriverPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os(GPU_DRIVER_ENV).is_some() {
            app.add_systems(Startup, announce);
        }
        let Some(path) = std::env::var_os(RENDERER_MARKER_ENV).filter(|p| !p.is_empty()) else { return };
        let wanted = frames_wanted(std::env::var(RENDERER_MARKER_FRAMES_ENV).ok().as_deref());
        app.insert_resource(StartMarker::new(PathBuf::from(path), wanted));
        // `Last`: the frame's drawing has been recorded and the next schedule run presents it,
        // so "the renderer survived this many frames" is counted a frame early, which is the
        // safe side of the question this marker answers.
        app.add_systems(Last, clear_marker);
    }
}

/// One line saying what was asked for and what arrived. At `warn`, as the validation switch's
/// line is: a run on a driver the player supplied is never the ordinary case, and the line is
/// the first thing a report about it should carry.
fn announce(adapter: Option<Res<RenderAdapterInfo>>) {
    let requested = std::env::var(GPU_DRIVER_ENV).unwrap_or_else(|_| "system".into());
    let reported = adapter.map_or_else(
        || "no adapter (the renderer did not start)".to_string(),
        |info| format!("{} ({:?}, driver {} {})", info.name, info.backend, info.driver, info.driver_info),
    );
    warn!("gpu driver: requested {requested}; the adapter reports {reported}");
}

fn clear_marker(mut marker: ResMut<StartMarker>) {
    let wanted = marker.wanted;
    let Some(path) = marker.frame() else { return };
    match std::fs::remove_file(path) {
        Ok(()) => info!("gpu driver: this run drew {wanted} frames; the start marker {} is removed", path.display()),
        // Not an error worth a louder level: the player may have cleared it from the launcher,
        // and nothing in the run depends on the file existing.
        Err(e) => debug!("gpu driver: the start marker {} could not be removed: {e}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `selected` file, including the shapes that mean "no selection".
    #[test]
    fn gpu_driver_selection_is_read_and_a_damaged_file_means_the_system_driver() {
        let good = parse_selection("folder turnip-25-0-3\nlibrary libvulkan_freedreno.so\nlabel Turnip 25.0.3\n").unwrap();
        assert_eq!(good, Selection { folder: "turnip-25-0-3".into(), library: "libvulkan_freedreno.so".into(), label: "Turnip 25.0.3".into() });
        // Without a label the folder is the label.
        let plain = parse_selection("folder turnip\nlibrary libvulkan_freedreno.so\n").unwrap();
        assert_eq!(plain.label, "turnip");
        // A comment line and trailing blanks change nothing.
        assert!(parse_selection("# the launcher wrote this\nfolder t\nlibrary l.so\n").is_some());
        // Missing halves, and names that are paths rather than one folder.
        assert!(parse_selection("").is_none());
        assert!(parse_selection("folder turnip\n").is_none());
        assert!(parse_selection("library libvulkan_freedreno.so\n").is_none());
        assert!(parse_selection("folder ../../../system/lib64\nlibrary libvulkan.so\n").is_none());
        assert!(parse_selection("folder turnip\nlibrary sub/dir/l.so\n").is_none());
        assert!(parse_selection("folder .\nlibrary l.so\n").is_none());
    }

    /// What each state does about the selection, with nothing touched on disk.
    #[test]
    fn gpu_driver_a_start_that_did_not_survive_and_validation_both_keep_the_selection() {
        let selection = Selection { folder: "turnip".into(), library: "libvulkan_freedreno.so".into(), label: "Turnip".into() };
        assert_eq!(decide(None, false, false), Decision::System);
        assert_eq!(decide(None, true, true), Decision::System, "no selection, nothing to fall back from");
        assert_eq!(decide(Some(selection.clone()), false, false), Decision::Try(selection.clone()));
        // A marker left by the previous start wins over everything else.
        let Decision::FallBack(kept, why) = decide(Some(selection.clone()), true, false) else { panic!("a left marker must fall back") };
        assert_eq!(kept, selection, "the selection is kept: only the player unchooses it");
        assert!(why.contains("300"), "the reason says how many frames were wanted: {why}");
        // The validation crash test runs on the system driver.
        let Decision::FallBack(_, why) = decide(Some(selection), false, true) else { panic!("validation must fall back") };
        assert!(why.contains("validation"), "{why}");
    }

    /// A start with nothing selected reads nothing and writes nothing at all.
    #[test]
    fn gpu_driver_no_selection_touches_no_file() {
        let files = std::env::temp_dir().join(format!("nvf-gpu-driver-{}", std::process::id()));
        let root = files.join(DIR);
        std::fs::create_dir_all(&root).expect("a temporary folder");
        assert_eq!(start(&files), None);
        assert!(!root.join(MARKER).exists());
        assert!(!root.join(LAST_FALLBACK).exists());
        let _ = std::fs::remove_dir_all(&files);
    }

    /// The frame count the marker waits for, and the values that leave it at the default.
    #[test]
    fn gpu_driver_marker_frame_count_falls_back_to_the_default() {
        assert_eq!(frames_wanted(None), DEFAULT_FRAMES);
        assert_eq!(frames_wanted(Some("")), DEFAULT_FRAMES);
        assert_eq!(frames_wanted(Some("0")), DEFAULT_FRAMES, "zero would make every start count as survived");
        assert_eq!(frames_wanted(Some("-5")), DEFAULT_FRAMES);
        assert_eq!(frames_wanted(Some("not a number")), DEFAULT_FRAMES);
        assert_eq!(frames_wanted(Some("3")), 3);
        assert_eq!(frames_wanted(Some(" 12 ")), 12);
    }

    /// The marker is removed on the frame the count is reached, and only once.
    #[test]
    fn gpu_driver_marker_is_removed_once_the_frames_are_drawn() {
        let mut marker = StartMarker::new("/files/gpu_drivers/renderer_starting", 3);
        assert!(marker.frame().is_none());
        assert!(marker.frame().is_none());
        assert_eq!(marker.frame(), Some(Path::new("/files/gpu_drivers/renderer_starting")));
        assert!(marker.frame().is_none(), "the removal is attempted exactly once a run");
        assert!(marker.frame().is_none());
    }
}
