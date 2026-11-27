//! Crash test (the Android launcher's Advanced page): the Khronos Vulkan validation layer.
//!
//! The Odin 3 (Adreno 830, Android 15) loses its Vulkan device a frame or two after a freshly
//! loaded world is first drawn, and `VK_EXT_device_fault` reports nothing (gpu_recorder/).
//! Invalid API use that desktop drivers forgive is the next thing to rule out, and only the
//! validation layer can say so. This is the switch that asks for it; it is off unless the
//! player turns it on, and it makes the run much slower, so it is a diagnostic, not a setting.
//!
//! What the switch does when `NVF_VULKAN_VALIDATION` is set (`super::crash_tests`'s
//! `vulkan-validation` row):
//!
//! 1. [`instance_flags`] adds `VALIDATION` and `DEBUG` to the renderer's `InstanceFlags`
//!    (one hook in `dlss::render_plugin`, where the renderer's wgpu settings are built). wgpu-hal then enables `VK_LAYER_KHRONOS_validation` if the
//!    Vulkan loader lists it, asks for `VK_EXT_debug_utils` and installs its messenger
//!    [observed: vendor/wgpu-hal/src/vulkan/instance.rs, `Instance::init`].
//! 2. The messenger writes every validation message as a `log` record whose target is
//!    `wgpu_hal::vulkan::instance` (WARNING -> warn, ERROR -> error). Bevy's default filter
//!    holds everything under `wgpu` at error, which would drop the warnings, so
//!    [`log_directives`] turns that module up for the run (`reports::log_filter`).
//! 3. [`VulkanValidationPlugin`] writes one line at startup saying the switch is on and what
//!    became of the layer library itself ([`prepare`]).
//!
//! **Deeper checks** (`NVF_VULKAN_VALIDATION_DEEP`, the two rows that share that variable).
//! The core checks found nothing before the loss, so the two kinds of fault they cannot see
//! are next:
//!
//! - **synchronization validation** (`sync`): a missing barrier or a wrong one between
//!   passes, which a tiler forgives until it does not;
//! - **GPU-assisted validation** (`gpuav`): instrumented shaders that catch an
//!   out-of-bounds descriptor or buffer read on the GPU itself. Much slower again than the
//!   core checks, and on a phone it may be too slow to reach the world at all, so it is its
//!   own switch rather than part of `sync`.
//!
//! Both are asked for through `VK_EXT_layer_settings`: [`ask_for_layer_settings`] chains a
//! `VkLayerSettingsCreateInfoEXT` onto the instance's create info when Bevy makes it
//! (`RawVulkanInitSettings`, as `gpu_recorder::fault` chains its device feature), naming
//! `khronos_validation.validate_core`, `.validate_sync` and `.gpuav_enable` [documented: the
//! Khronos validation layer's layer settings; the layer ignores a setting name it does not
//! know, so an older or newer layer simply keeps its defaults]. The structure may be chained
//! without the extension being enabled, which is how the loader passes settings to a layer
//! [documented: `VK_EXT_layer_settings`, which is implemented by layers, not by drivers].
//! `gpuav` also adds wgpu's own `GPU_BASED_VALIDATION` instance flag, which makes wgpu-hal
//! chain the older `VkValidationFeaturesEXT` as well; a layer that honours both is asked the
//! same thing twice.
//!
//! On Android the layer is `libVkLayer_khronos_validation.so`, packaged in the APK beside
//! `libnvf_android.so` by the fork's workflow (downloaded from a pinned Khronos release and
//! checked against two hashes; never committed) and found by the Vulkan loader in the app's
//! native library folder because the APK extracts its native libraries at install time.
//! [`prepare`] also loads the library by name itself before the renderer exists, the way the
//! pre-Android-9 recipe did, and says what came of it (docs/odin-debug.md).
//!
//! With the switch off nothing here runs: the flags, the filter and the plugin are all the
//! same values they were.

use std::ffi::CStr;

use ash::vk;
use bevy::prelude::*;
use bevy::render::renderer::raw_vulkan_init::RawVulkanInitSettings;
use bevy::render::settings::InstanceFlags;

/// The variables the two rows of the switch table are read from.
fn env() -> &'static str {
    super::crash_tests::env("vulkan-validation")
}

fn deep_env() -> &'static str {
    super::crash_tests::env("vulkan-validation-sync")
}

/// Whether this run was asked to validate (the Advanced page's row).
pub fn requested() -> bool {
    crate::odin::env_on(env())
}

/// Which of the layer's deeper checks this run asks for, on top of the core ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Deep {
    /// Synchronization validation: barriers and hazards between passes.
    pub sync: bool,
    /// GPU-assisted validation: instrumented shaders, out-of-bounds reads caught on the GPU.
    pub gpuav: bool,
}

impl Deep {
    /// Whether anything beyond the core checks is asked for.
    pub fn any(self) -> bool {
        self.sync || self.gpuav
    }

    /// What the startup line says this run asks for.
    pub fn text(self) -> &'static str {
        match (self.sync, self.gpuav) {
            (true, true) => "core, synchronization and GPU-assisted",
            (true, false) => "core and synchronization",
            (false, true) => "core and GPU-assisted",
            (false, false) => "core",
        }
    }
}

/// The deep variable read: a comma or space list of `sync` and `gpuav`,
/// where `1`, `all` or `deep` mean both and `0` or nothing means neither. A word that is
/// neither is ignored, so a newer launcher's switch cannot turn a check off by accident.
pub fn parse_deep(text: &str) -> Deep {
    let mut deep = Deep::default();
    for word in text.split([',', ' ', ';']).map(str::trim).filter(|w| !w.is_empty()) {
        match word {
            "1" | "all" | "deep" => deep = Deep { sync: true, gpuav: true },
            "sync" => deep.sync = true,
            "gpuav" | "gpu" => deep.gpuav = true,
            _ => {}
        }
    }
    deep
}

/// The deeper checks this run asks for. Nothing without the validation switch itself: the
/// settings would be read by a layer that is not loaded.
pub fn deep() -> Deep {
    if !requested() {
        return Deep::default();
    }
    parse_deep(&std::env::var(deep_env()).unwrap_or_default())
}

/// The renderer's instance flags, with validation added when the switch is on. Returned
/// unchanged otherwise, so a normal run creates exactly the instance it did before.
pub fn instance_flags(flags: InstanceFlags) -> InstanceFlags {
    if !requested() {
        return flags;
    }
    // DEBUG as well: without it wgpu-hal does not ask for `VK_EXT_debug_utils`, so the layer
    // would have nowhere to send its messages.
    let mut flags = flags | InstanceFlags::VALIDATION | InstanceFlags::DEBUG;
    if deep().gpuav {
        // wgpu-hal's own path for GPU-assisted validation: it chains `VkValidationFeaturesEXT`
        // with `GPU_ASSISTED` when this flag is set and the layer offers that extension
        // (`engine/vendor/wgpu-hal/src/vulkan/instance.rs`). The layer settings below ask for
        // the same thing the way the layer documents today.
        flags |= InstanceFlags::GPU_BASED_VALIDATION;
    }
    flags
}

/// The layer, and the settings named for it (module notes).
const LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";
const VALIDATE_CORE: &CStr = c"validate_core";
const VALIDATE_SYNC: &CStr = c"validate_sync";
const GPUAV_ENABLE: &CStr = c"gpuav_enable";

/// Chains the layer's settings onto the Vulkan instance Bevy is about to create, so the
/// deeper checks are on. Called from [`super::init`], before `DefaultPlugins`, beside the
/// other raw-Vulkan callbacks; does nothing unless both switches are on.
pub fn ask_for_layer_settings(settings: &mut RawVulkanInitSettings) {
    let deep = deep();
    if !deep.any() {
        return;
    }
    // SAFETY: the callback only adds to the create info's `p_next` chain and removes nothing;
    // it enables no extension and turns no feature off. The structures it chains are leaked so
    // they outlive `vkCreateInstance`, whose end only the callback's caller sees (as
    // `gpu_recorder::fault` leaks its device feature structure).
    unsafe {
        settings.add_create_instance_callback(move |args, _features| {
            let on: &'static mut [vk::Bool32; 3] = Box::leak(Box::new([vk::TRUE; 3]));
            let mut list: Vec<vk::LayerSettingEXT<'static>> = Vec::new();
            let mut setting = |name: &'static CStr, value: &'static vk::Bool32| {
                list.push(vk::LayerSettingEXT {
                    p_layer_name: LAYER.as_ptr(),
                    p_setting_name: name.as_ptr(),
                    ty: vk::LayerSettingTypeEXT::BOOL32,
                    value_count: 1,
                    p_values: std::ptr::from_ref(value).cast(),
                    _marker: std::marker::PhantomData,
                });
            };
            // The core checks stay on: the deeper ones are added to them, not instead of them.
            setting(VALIDATE_CORE, &on[0]);
            if deep.sync {
                setting(VALIDATE_SYNC, &on[1]);
            }
            if deep.gpuav {
                setting(GPUAV_ENABLE, &on[2]);
            }
            let list: &'static [vk::LayerSettingEXT<'static>] = Box::leak(list.into_boxed_slice());
            let chained: &'static mut vk::LayerSettingsCreateInfoEXT<'static> = Box::leak(Box::new(vk::LayerSettingsCreateInfoEXT {
                setting_count: list.len() as u32,
                p_settings: list.as_ptr(),
                ..Default::default()
            }));
            chained.p_next = args.create_info.p_next;
            args.create_info.p_next = std::ptr::from_mut(chained).cast();
        });
    }
}

/// The Khronos layer's library, the name the Vulkan loader and the linker both know it by.
const LAYER_SONAME: &str = "libVkLayer_khronos_validation.so";

/// What became of the layer library, for the startup line. Set once by [`prepare`].
static LIBRARY: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Load the layer library into this process, before the renderer exists.
///
/// Android's Vulkan loader lists layers from the app's own native library folder, which is
/// where the APK's packaged copy ends up; loading it here as well is the belt-and-braces half
/// of the pre-Android-9 recipe, and it is what makes the startup line able to say whether the
/// library is there at all. Called from [`super::init`]; nothing happens with the switch off.
pub fn prepare() {
    if !requested() {
        return;
    }
    let status = load_layer();
    // wgpu reads these itself (`InstanceFlags::with_env`): any value but `0` turns the flag
    // on, so any other instance this run makes validates too.
    std::env::set_var("WGPU_VALIDATION", "1");
    std::env::set_var("WGPU_DEBUG", "1");
    let _ = LIBRARY.set(status);
}

/// Loads the layer library, and says what happened in the words the run log will carry.
#[cfg(target_os = "android")]
fn load_layer() -> String {
    let Ok(name) = std::ffi::CString::new(LAYER_SONAME) else {
        return format!("{LAYER_SONAME} not loaded (bad name)");
    };
    // SAFETY: a plain `dlopen` of a NUL-terminated name, and `dlerror`/`CStr` on the string it
    // returns. The handle is deliberately never closed: the layer stays for the whole run.
    unsafe {
        if !libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL).is_null() {
            return format!("{LAYER_SONAME} loaded from the app's library folder");
        }
        let error = libc::dlerror();
        let why = if error.is_null() { "no reason given".to_string() } else { std::ffi::CStr::from_ptr(error).to_string_lossy().into_owned() };
        // Not fatal: the loader's own search may still find the layer, and a run without it
        // simply validates nothing, which the startup line says.
        format!("{LAYER_SONAME} not loaded ({why}): is it packaged in this APK?")
    }
}

/// Off Android the Vulkan loader finds whatever layer the Vulkan SDK installed, if any, and
/// there is nothing of ours to load.
#[cfg(not(target_os = "android"))]
fn load_layer() -> String {
    format!("{LAYER_SONAME} not loaded by the engine (not Android); the Vulkan SDK's own layer is used if it is installed")
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
    let library = LIBRARY.get().map_or("not reported by this platform", String::as_str);
    // warn, so no log level a player set can hide what this run is doing to its frame rate.
    let deep = deep();
    let deeper = if deep.any() {
        format!(", deeper checks asked for through VK_EXT_layer_settings ({})", deep_env())
    } else {
        String::new()
    };
    warn!(
        "vulkan validation: on ({}, crash test): {} checks, instance flags VALIDATION|DEBUG{}{deeper}, layer library: {library}. \
         The wgpu_hal lines below say whether VK_LAYER_KHRONOS_validation was found; expect a much slower run{}.",
        env(),
        deep.text(),
        if deep.gpuav { "|GPU_BASED_VALIDATION" } else { "" },
        if deep.gpuav { ", and a much slower one again with GPU-assisted validation: a load may not finish at all" } else { "" },
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
        std::env::remove_var(env());
        assert!(!requested());
        assert_eq!(instance_flags(base), base);
        assert_eq!(log_directives(), None);

        std::env::set_var(env(), "1");
        assert!(requested());
        let on = instance_flags(base);
        assert!(on.contains(InstanceFlags::VALIDATION) && on.contains(InstanceFlags::DEBUG));
        assert_eq!(log_directives(), Some("wgpu_hal=debug"));
        // Flags the caller already had are kept.
        let kept = instance_flags(InstanceFlags::DISCARD_HAL_LABELS);
        assert!(kept.contains(InstanceFlags::DISCARD_HAL_LABELS) && kept.contains(InstanceFlags::VALIDATION));
        // The deeper checks are off unless asked for, so a plain validation run is the run it
        // was before this was added.
        assert_eq!(deep(), Deep::default());
        assert!(!kept.contains(InstanceFlags::GPU_BASED_VALIDATION));

        // With both switches on, GPU-assisted validation adds wgpu's own instance flag too.
        std::env::set_var(deep_env(), "sync,gpuav");
        assert_eq!(deep(), Deep { sync: true, gpuav: true });
        assert!(instance_flags(InstanceFlags::empty()).contains(InstanceFlags::GPU_BASED_VALIDATION));
        // Without the validation switch itself the deeper checks are not asked for: the layer
        // that would read the settings is not loaded.
        std::env::remove_var(env());
        assert_eq!(deep(), Deep::default());
        std::env::remove_var(deep_env());
    }

    /// What the deep switch's words mean, including the ones that mean nothing.
    #[test]
    fn the_deep_validation_switch_reads_its_words() {
        assert_eq!(parse_deep(""), Deep { sync: false, gpuav: false });
        assert_eq!(parse_deep("0"), Deep { sync: false, gpuav: false });
        assert_eq!(parse_deep("sync"), Deep { sync: true, gpuav: false });
        assert_eq!(parse_deep("gpuav"), Deep { sync: false, gpuav: true });
        assert_eq!(parse_deep("sync,gpuav"), Deep { sync: true, gpuav: true });
        assert_eq!(parse_deep(" sync , gpuav "), Deep { sync: true, gpuav: true });
        assert_eq!(parse_deep("1"), Deep { sync: true, gpuav: true });
        // A word the layer settings know nothing of changes nothing.
        assert_eq!(parse_deep("sync,best-practices"), Deep { sync: true, gpuav: false });
    }

    /// The words the startup line uses for each combination.
    #[test]
    fn the_deep_validation_switch_says_what_it_asked_for() {
        assert_eq!(Deep::default().text(), "core");
        assert_eq!(Deep { sync: true, gpuav: false }.text(), "core and synchronization");
        assert_eq!(Deep { sync: false, gpuav: true }.text(), "core and GPU-assisted");
        assert_eq!(Deep { sync: true, gpuav: true }.text(), "core, synchronization and GPU-assisted");
        assert!(!Deep::default().any() && Deep { sync: true, gpuav: false }.any());
    }
}
