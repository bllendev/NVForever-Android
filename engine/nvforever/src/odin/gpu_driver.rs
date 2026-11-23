//! The Vulkan driver this run uses, and the safety marker that lets the next start go back to
//! the system one.
//!
//! On Android the player may install a Vulkan driver of their own (Mesa Turnip for Adreno,
//! through libadrenotools) and select it in the launcher. Everything that opens such a driver
//! is in the Android shell (`engine/nvf-android/src/gpu_driver.rs`); the engine only
//!
//! 1. hands the renderer the driver's entry point ([`use_vulkan_entry`], the vendored
//!    wgpu-hal's Android hook) before the Vulkan instance is created, and
//! 2. says in the run log which driver was asked for and what the adapter turned out to be
//!    ([`GpuDriverPlugin`]), and
//! 3. removes the shell's "the renderer is starting" marker file once the run has drawn
//!    [`DEFAULT_FRAMES`] frames ([`super::RENDERER_MARKER_ENV`]). A start that never gets that
//!    far leaves the file behind, which is how the shell knows, at the next start, that the
//!    selected driver did not survive its own first frames and that this run should be on the
//!    system driver instead (docs/odin-debug.md).
//!
//! With no driver selected neither variable is set, so nothing here runs and the run is the run
//! it was before: the same instance, the same log, no extra file touched.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;

/// How many drawn frames a start must reach before it counts as survived. Three hundred is
/// about five seconds at 60 frames a second: past instance and device creation, past the first
/// swapchain, past the menu's first draws, and well past the frame or two in which the Odin 3's
/// device loss happens (gpu_recorder/).
pub const DEFAULT_FRAMES: u32 = 300;

/// Hand the renderer the `vkGetInstanceProcAddr` of a driver the platform shell has already
/// opened, so the Vulkan instance is created on it instead of on the system `libvulkan.so`.
/// Call before the renderer starts; `false` means one was already set and this one is ignored.
///
/// The pointer is the `vkGetInstanceProcAddr` symbol of the library the shell opened and keeps
/// open for the whole run.
///
/// # Safety
///
/// `get_instance_proc_addr` must be a `vkGetInstanceProcAddr` of a Vulkan implementation that
/// stays loaded for the rest of the process.
#[cfg(target_os = "android")]
pub unsafe fn use_vulkan_entry(get_instance_proc_addr: *const std::ffi::c_void) -> bool {
    if get_instance_proc_addr.is_null() {
        return false;
    }
    // SAFETY: the caller promises this is a Vulkan implementation's `vkGetInstanceProcAddr`,
    // which is what both the transmute and wgpu-hal's hook require of it.
    unsafe {
        let entry: ash::vk::PFN_vkGetInstanceProcAddr = std::mem::transmute(get_instance_proc_addr);
        wgpu::hal::vulkan::set_android_vk_get_instance_proc_addr(entry)
    }
}

/// How many frames the marker should survive: [`super::RENDERER_MARKER_FRAMES_ENV`] when it
/// holds a positive number, [`DEFAULT_FRAMES`] otherwise (including a value that is not a
/// number, so a mistyped test aid cannot make a start count as survived at once).
pub fn frames_wanted(given: Option<&str>) -> u32 {
    given.and_then(|text| text.trim().parse::<u32>().ok()).filter(|frames| *frames > 0).unwrap_or(DEFAULT_FRAMES)
}

/// The shell's marker file and the frames still to go. Counting and removal are here, away from
/// Bevy, so both can be checked without a renderer.
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
/// shell's start marker once the run has drawn enough frames. Both halves are off unless the
/// platform shell set their variables, so a desktop run and an Android run without a selected
/// driver add nothing at all.
pub struct GpuDriverPlugin;

impl Plugin for GpuDriverPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os(super::GPU_DRIVER_ENV).is_some() {
            app.add_systems(Startup, announce);
        }
        let Some(path) = std::env::var_os(super::RENDERER_MARKER_ENV).filter(|p| !p.is_empty()) else { return };
        let wanted = frames_wanted(std::env::var(super::RENDERER_MARKER_FRAMES_ENV).ok().as_deref());
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
    let requested = std::env::var(super::GPU_DRIVER_ENV).unwrap_or_else(|_| "system".into());
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
