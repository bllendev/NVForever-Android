//! Crash test (the launcher's "Crash test: Vulkan validation (slow)" switch): the Khronos
//! Vulkan validation layer on the phone.
//!
//! The APK carries `lib/arm64-v8a/libVkLayer_khronos_validation.so` (the Khronos Android
//! release, downloaded and checked by CI, Apache-2.0; it is never committed). Android's Vulkan
//! loader looks for layers in the app's own native library folder, which it is allowed to do
//! for a debuggable app, and the Odin APK is debuggable; that is how the layer becomes visible
//! to `vkEnumerateInstanceLayerProperties`, which is where wgpu-hal looks for it.
//!
//! [`enable`] does two things before the engine starts a thread:
//!
//! 1. loads the layer library into this process by name. The app's linker namespace searches
//!    the app's library folder (and the APK itself), so the library is in the process whatever
//!    the loader's own search finds, as the pre-Android-9 `System.loadLibrary` recipe did;
//! 2. sets [`nvforever::VULKAN_VALIDATION_ENV`] (the engine adds `VALIDATION` and `DEBUG` to
//!    its instance flags) and wgpu's own `WGPU_VALIDATION`/`WGPU_DEBUG`, so any other instance
//!    this run makes validates too, and reports what became of the library in
//!    [`nvforever::VULKAN_VALIDATION_LIB_ENV`] for the engine's startup line.
//!
//! Nothing here runs with the switch off.

/// The Khronos layer's library, the name the Vulkan loader and the linker both know it by.
const LAYER_SONAME: &str = "libVkLayer_khronos_validation.so";

/// Ask this run to validate. Call before the engine starts (`lib.rs`), not after.
pub fn enable() {
    let status = load_layer();
    // Edition 2021: `set_var` is safe to call; nothing else is running yet.
    std::env::set_var(nvforever::VULKAN_VALIDATION_ENV, "1");
    std::env::set_var(nvforever::VULKAN_VALIDATION_LIB_ENV, &status);
    // wgpu reads these itself (`InstanceFlags::with_env`): any value but "0" turns the flag on.
    std::env::set_var("WGPU_VALIDATION", "1");
    std::env::set_var("WGPU_DEBUG", "1");
    eprintln!("Vulkan validation (crash test): on; {status}");
}

/// Loads the layer library, and says what happened in the words the run log will carry.
#[cfg(target_os = "android")]
fn load_layer() -> String {
    let name = match std::ffi::CString::new(LAYER_SONAME) {
        Ok(name) => name,
        Err(_) => return format!("{LAYER_SONAME} not loaded (bad name)"),
    };
    // SAFETY: a plain `dlopen` of a NUL-terminated name, and `dlerror`/`CStr` on the string it
    // returns. The handle is deliberately never closed: the layer stays for the whole run.
    unsafe {
        if !libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL).is_null() {
            return format!("{LAYER_SONAME} loaded from the app's library folder");
        }
        let error = libc::dlerror();
        let why = if error.is_null() {
            "no reason given".to_string()
        } else {
            std::ffi::CStr::from_ptr(error).to_string_lossy().into_owned()
        };
        // Not fatal: the loader's own search may still find the layer, and a run without it
        // simply validates nothing, which the engine's startup line says.
        format!("{LAYER_SONAME} not loaded ({why}) - is it packaged in this APK?")
    }
}

/// Desktop builds of this crate (`cargo check` on a PC) have no layer to load; the Vulkan
/// loader there finds the layer the Vulkan SDK installed, if any.
#[cfg(not(target_os = "android"))]
fn load_layer() -> String {
    format!("{LAYER_SONAME} not loaded by the shell (not Android)")
}
