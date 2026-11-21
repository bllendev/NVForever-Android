//! Crash test (the Android launcher's switch): the Khronos Vulkan validation layer.
//!
//! The Odin 3 (Adreno 830, Android 15) loses its Vulkan device a frame or two after a freshly
//! loaded world is first drawn, and `VK_EXT_device_fault` reports nothing (gpu_recorder/).
//! Invalid API use that desktop drivers forgive is the next thing to rule out, and only the
//! validation layer can say so. This is the switch that asks for it; it is off unless the
//! player turns it on, and it makes the run much slower, so it is a diagnostic, not a setting.
//!
//! What the switch does when [`crate::VULKAN_VALIDATION_ENV`] is set (any value):
//!
//! 1. [`instance_flags`] adds `VALIDATION` and `DEBUG` to the renderer's `InstanceFlags`
//!    (`dlss::render_plugin`). wgpu-hal then enables `VK_LAYER_KHRONOS_validation` if the
//!    Vulkan loader lists it, asks for `VK_EXT_debug_utils` and installs its messenger
//!    [observed: vendor/wgpu-hal/src/vulkan/instance.rs, `Instance::init`].
//! 2. The messenger writes every validation message as a `log` record whose target is
//!    `wgpu_hal::vulkan::instance` (WARNING -> warn, ERROR -> error). Bevy's default filter
//!    holds everything under `wgpu` at error, which would drop the warnings, so
//!    [`log_directives`] turns that module up for the run (`reports::log_filter`).
//! 3. [`VulkanValidationPlugin`] writes one line at startup saying the switch is on and what
//!    the platform shell found of the layer library itself
//!    ([`crate::VULKAN_VALIDATION_LIB_ENV`]).
//!
//! On Android the layer is `libVkLayer_khronos_validation.so`, packaged in the APK beside
//! `libnvf_android.so` and found by the Vulkan loader in the app's native library folder
//! because the Odin APK is debuggable; `nvf-android` also loads the library itself before the
//! engine starts, and reports the result in that variable (docs/odin-debug.md).
//!
//! With the switch off nothing here runs: the flags, the filter and the plugin are all the
//! same values they were.

use bevy::prelude::*;
use bevy::render::settings::InstanceFlags;

/// Whether this run was asked to validate (the launcher's crash-test switch).
pub fn requested() -> bool {
    std::env::var_os(crate::VULKAN_VALIDATION_ENV).is_some()
}

/// The renderer's instance flags, with validation added when the switch is on. Returned
/// unchanged otherwise, so a normal run creates exactly the instance it did before.
pub fn instance_flags(flags: InstanceFlags) -> InstanceFlags {
    if !requested() {
        return flags;
    }
    // DEBUG as well: without it wgpu-hal does not ask for `VK_EXT_debug_utils`, so the layer
    // would have nowhere to send its messages.
    flags | InstanceFlags::VALIDATION | InstanceFlags::DEBUG
}

/// The log directives the validation messages need, or `None` when the switch is off.
/// `wgpu=error` (Bevy's default) would otherwise swallow every warning the layer writes;
/// `debug` also shows wgpu-hal's own line when the layer is asked for but not found.
pub fn log_directives() -> Option<&'static str> {
    requested().then_some("wgpu_hal=debug")
}

/// Says in the run log whether this run validates, and what became of the layer library.
pub struct VulkanValidationPlugin;

impl Plugin for VulkanValidationPlugin {
    fn build(&self, app: &mut App) {
        if !requested() {
            return;
        }
        app.add_systems(Startup, announce);
    }
}

fn announce() {
    let library = std::env::var(crate::VULKAN_VALIDATION_LIB_ENV).unwrap_or_else(|_| "not reported by this platform".into());
    // warn, so no log level a player set can hide what this run is doing to its frame rate.
    warn!(
        "vulkan validation: on ({}, crash test): instance flags VALIDATION|DEBUG, layer library: {library}. \
         The wgpu_hal lines below say whether VK_LAYER_KHRONOS_validation was found; expect a much slower run.",
        crate::VULKAN_VALIDATION_ENV
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The switch is read from the environment, which the test runner shares between tests in
    /// the process, so both states are checked in one test.
    #[test]
    fn off_changes_nothing_and_on_adds_validation_and_the_filter() {
        let base = InstanceFlags::empty();
        // Edition 2021: `set_var`/`remove_var` are safe to call.
        std::env::remove_var(crate::VULKAN_VALIDATION_ENV);
        assert!(!requested());
        assert_eq!(instance_flags(base), base);
        assert_eq!(log_directives(), None);

        std::env::set_var(crate::VULKAN_VALIDATION_ENV, "1");
        assert!(requested());
        let on = instance_flags(base);
        assert!(on.contains(InstanceFlags::VALIDATION) && on.contains(InstanceFlags::DEBUG));
        assert_eq!(log_directives(), Some("wgpu_hal=debug"));
        // Flags the caller already had are kept.
        let kept = instance_flags(InstanceFlags::DISCARD_HAL_LABELS);
        assert!(kept.contains(InstanceFlags::DISCARD_HAL_LABELS) && kept.contains(InstanceFlags::VALIDATION));
        std::env::remove_var(crate::VULKAN_VALIDATION_ENV);
    }
}
