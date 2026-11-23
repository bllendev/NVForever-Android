//! A Vulkan driver the player supplied (Mesa Turnip for Adreno), opened through libadrenotools.
//!
//! Qualcomm's Adreno driver is part of the phone's system image and cannot be replaced, but an
//! app may load another one into itself: libadrenotools (BSD-2-Clause, bylaws/libadrenotools)
//! opens the system Vulkan loader in a linker namespace of its own, with a hook that redirects
//! the loader's search for the vendor driver to a library in the app's private storage. That is
//! how the Android emulators and the native ports run Mesa's Turnip on Adreno phones, and it is
//! what this module does. NVForever ships no driver: the player installs a driver zip in the
//! launcher (`GpuDrivers.java`).
//!
//! ```text
//! <internal files>/gpu_drivers/
//!     <id>/                     one installed driver: its .so, and meta.json if the zip had one
//!     selected                  the driver to use ("folder <id>", "library <soname>", "label"),
//!                               absent for the system driver
//!     renderer_starting         written just before the renderer starts, removed by the engine
//!                               after 300 drawn frames (`nvforever::odin::gpu_driver`)
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
//! `dlsym("vkGetInstanceProcAddr")` on it is handed to `nvforever::odin::gpu_driver::use_vulkan_entry`,
//! which stores it in the vendored wgpu-hal, whose Android branch then builds its `ash::Entry`
//! from that pointer instead of loading `libvulkan.so` itself. All of this happens before the
//! engine starts, so no Vulkan instance exists yet.
//!
//! **Falling back.** Three things put a run back on the system driver while keeping the
//! player's selection: a `renderer_starting` file left behind by the previous start (the
//! selected driver did not survive its own first frames), the Vulkan validation crash test
//! being on (the layer chain under libadrenotools' private namespace is not something this
//! port has checked, and a crash test that cannot be trusted is worse than none), and
//! libadrenotools failing to open the driver. Each writes its reason to `last_fallback`, which
//! the launcher shows.

use std::path::{Path, PathBuf};

use crate::paths;

/// The folder of everything in the module notes, inside the app's private storage (a driver
/// must not be loaded from shared storage: `dlopen` refuses a library any app could rewrite).
pub fn root() -> PathBuf {
    paths::internal_files().join("gpu_drivers")
}

/// The player's choice; absent means the system driver.
const SELECTED: &str = "selected";
/// Written before the renderer starts, removed by the engine once the run has drawn its frames.
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
    /// The installed driver's folder below [`root`]: a plain name, never a path.
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

/// A name that may be joined to [`root`]: one path element, nothing that climbs out of it.
fn plain_name(word: &str) -> bool {
    !word.is_empty() && word != "." && word != ".." && !word.contains(['/', '\\', '\0'])
}

/// The `selected` file: one `name value` a line, as `nvforever-controls.txt` is, written by
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
        // `marker_left` is already false after a handoff restart: the run before asked for it,
        // so it was alive, and its marker says nothing about the driver (`start`).
        let reason = format!(
            "the last start on {} did not reach {} drawn frames, so this run is on the system driver. \
             Start again to try it once more, or choose System in the launcher.",
            selection.label,
            nvforever::odin::gpu_driver::DEFAULT_FRAMES,
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

/// Opens the selected driver, if there is one, and tells the renderer to use it. Returns the
/// words the run log and the engine's startup line carry, or `None` when no driver is selected
/// and nothing at all was done.
///
/// Call before the engine starts and before any Vulkan instance exists (`lib.rs`), and after
/// [`crate::validation::enable`], whose switch this reads.
pub fn start(validation: bool) -> Option<String> {
    let root = root();
    let selection = std::fs::read_to_string(root.join(SELECTED)).ok().and_then(|text| parse_selection(&text));
    // Nothing selected: not a file read, not a line logged.
    selection.as_ref()?;
    let marker = root.join(MARKER);
    // A marker left by a run that then asked the launcher to start this one (loading a save
    // from the main menu, which can happen inside the first 300 frames) is not a start that
    // died: the run before was well enough to ask (`paths::restart_requested`).
    let marker_left = marker.exists() && !paths::restart_requested(&paths::internal_files());
    match decide(selection, marker_left, validation) {
        Decision::System => None,
        Decision::FallBack(selection, why) => {
            // The marker has done its work; the next start may try the driver again.
            let _ = std::fs::remove_file(&marker);
            record_fallback(&root, &why);
            eprintln!("gpu driver: {} is selected but not used: {why}", selection.label);
            Some(format!("{} (not used this run), the system driver instead", selection.label))
        }
        Decision::Try(selection) => {
            // A marker a handoff restart left behind is stale; this run writes its own below,
            // and a run that stays on the system driver after all must leave none.
            let _ = std::fs::remove_file(&marker);
            let dir = root.join(&selection.folder);
            match open_driver(&dir, &selection.library) {
                Ok(()) => {
                    // Before the renderer: a start that dies in instance or device creation
                    // leaves this file, and the next start reads it and stays on the system
                    // driver (`nvforever::odin::gpu_driver`).
                    if let Err(e) = write_marker(&root, &marker) {
                        eprintln!("gpu driver: the start marker could not be written ({e}); a failed start will not be noticed");
                    } else {
                        // Edition 2021: `set_var` is safe to call; nothing else is running yet.
                        std::env::set_var(nvforever::odin::RENDERER_MARKER_ENV, &marker);
                    }
                    let _ = std::fs::remove_file(root.join(LAST_FALLBACK));
                    eprintln!("gpu driver: {} opened through libadrenotools from {}", selection.label, dir.display());
                    Some(selection.label)
                }
                Err(why) => {
                    record_fallback(&root, &why);
                    eprintln!("gpu driver: {} could not be opened: {why}; the system driver instead", selection.label);
                    Some(format!("{} (could not be opened), the system driver instead", selection.label))
                }
            }
        }
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
    let hook_dir = native_lib_dir(&root()).ok_or("the app's native library folder could not be found")?;
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
        // SAFETY: `entry` is the Vulkan loader's own `vkGetInstanceProcAddr`, from a library
        // that is never closed, which is what the renderer's hook requires of it.
        if !nvforever::odin::gpu_driver::use_vulkan_entry(entry.cast_const()) {
            return Err("the renderer already has a Vulkan entry point for this process".to_string());
        }
    }
    Ok(())
}

/// Desktop builds of this crate (`cargo check` on a PC) have no libadrenotools and no Adreno.
#[cfg(not(target_os = "android"))]
fn open_driver(_dir: &Path, _library: &str) -> Result<(), String> {
    Err("a driver of your own is an Android feature".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `selected` file, including the shapes that mean "no selection".
    #[test]
    fn the_selected_driver_is_read_and_a_damaged_file_means_the_system_driver() {
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
    fn a_start_that_did_not_survive_and_the_validation_switch_both_keep_the_selection() {
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
}
