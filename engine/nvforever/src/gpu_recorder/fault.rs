//! `VK_EXT_device_fault`: what the driver itself says about a lost device.
//!
//! The single most useful datum after a `DeviceLost`: the driver's own fault description,
//! the addresses it faulted on and its vendor-specific infos. Everything marked
//! [documented] below is the Vulkan specification's `VK_EXT_device_fault` chapter or
//! `vk.xml` (ash 0.38 / Vulkan 1.3.281 headers).
//!
//! - **The extension.** A device-creation callback (Bevy's `raw_vulkan_init`, as
//!   `anti_lag.rs` and `nv_low_latency.rs` do) adds `VK_EXT_device_fault` and chains
//!   `VkPhysicalDeviceFaultFeaturesEXT` with `deviceFault` on, **only** when the adapter
//!   offers the extension and reports that feature [documented: the extension has a feature
//!   structure, and `vkGetDeviceFaultInfoEXT` may be used only with `deviceFault` enabled].
//!   The vendor binary is not asked for (`deviceFaultVendorBinary` stays off): it is a blob
//!   only the vendor's tools read, and it would have to be written to a file.
//! - **Without it** nothing is enabled, nothing is called and nothing changes: the dump then
//!   says the extension is not there and why. The Adreno driver on the Odin 3 may or may not
//!   offer it [hypothesis]; the run log says which at the start.
//! - **The query** runs the moment the loss is known, before the device is destroyed
//!   ([`query`]), through the raw Vulkan device under wgpu (`Device::as_hal`). It is the
//!   two-call pattern the specification gives: counts first with a null info, then the
//!   arrays [documented].
//! - `VK_EXT_DEVICE_FAULT=0` never asks for the extension (a way out if a driver misbehaves
//!   with it enabled).

use std::ffi::CStr;

use ash::vk;
use bevy::prelude::*;
use bevy::render::renderer::raw_vulkan_init::{AdditionalVulkanFeatures, RawVulkanInitSettings};

const EXTENSION: &CStr = vk::EXT_DEVICE_FAULT_NAME;

/// `VK_EXT_DEVICE_FAULT=0`: the extension is never asked for.
fn allowed() -> bool {
    !crate::odin::env_off("NVF_VK_DEVICE_FAULT")
}

/// Marks, in Bevy's `AdditionalVulkanFeatures`, a device made with `VK_EXT_device_fault` on.
pub struct DeviceFaultEnabled;

/// Asks for the extension when Bevy creates the Vulkan device. Called before
/// `DefaultPlugins` (lib.rs), beside the other device-creation callbacks.
pub fn ask_for_extension(settings: &mut RawVulkanInitSettings) {
    // SAFETY: the callback adds the extension and turns its own feature on only when the
    // adapter offers both, and removes nothing.
    unsafe {
        settings.add_create_device_callback(|args, adapter, features| {
            if !supported(adapter) {
                return;
            }
            if !args.extensions.contains(&EXTENSION) {
                args.extensions.push(EXTENSION);
            }
            // Must outlive the device's creation, which only the callback's caller sees the
            // end of: one small allocation is kept for each device made (as `anti_lag.rs`).
            let chained: &'static mut vk::PhysicalDeviceFaultFeaturesEXT<'static> = Box::leak(Box::new(vk::PhysicalDeviceFaultFeaturesEXT::default().device_fault(true)));
            chained.p_next = args.create_info.p_next.cast_mut();
            args.create_info.p_next = std::ptr::from_mut(chained).cast();
            features.insert::<DeviceFaultEnabled>();
        });
    }
}

/// Whether the adapter offers the extension and its `deviceFault` feature.
fn supported(adapter: &wgpu::hal::vulkan::Adapter) -> bool {
    if !allowed() || !adapter.physical_device_capabilities().supports_extension(EXTENSION) {
        return false;
    }
    let mut fault = vk::PhysicalDeviceFaultFeaturesEXT::default();
    let mut features = vk::PhysicalDeviceFeatures2::default().push_next(&mut fault);
    // SAFETY: the chain holds one structure of an extension the device offers; the call is
    // Vulkan 1.1 core, which wgpu requires.
    unsafe { adapter.shared_instance().raw_instance().get_physical_device_features2(adapter.raw_physical_device(), &mut features) };
    fault.device_fault == vk::TRUE
}

/// Say at the start whether a lost device will be able to say why. The marker is in the
/// render world (as `anti_lag.rs` reads its own), so this runs from the plugin's `finish`.
pub fn report_start(app: &App) {
    let on = app
        .get_sub_app(bevy::render::RenderApp)
        .and_then(|render| render.world().get_resource::<AdditionalVulkanFeatures>())
        .is_some_and(|f| f.has::<DeviceFaultEnabled>());
    if on {
        info!("gpu recorder: VK_EXT_device_fault is on; a lost device will be asked what faulted");
    } else {
        info!("gpu recorder: VK_EXT_device_fault is not available (no such extension or feature on this driver, not the Vulkan backend, or NVF_VK_DEVICE_FAULT=0); a lost device is recorded without the driver's fault info");
    }
}

/// A `VkDeviceFaultAddressTypeEXT` in words [documented: `vk.xml`]. Matched against the
/// constants rather than raw numbers, and ash's `Debug` is not used (the crate is taken
/// without its `debug` feature).
fn address_type(ty: vk::DeviceFaultAddressTypeEXT) -> String {
    use vk::DeviceFaultAddressTypeEXT as T;
    let named = [
        (T::NONE, "none"),
        (T::READ_INVALID, "read of an invalid address"),
        (T::WRITE_INVALID, "write to an invalid address"),
        (T::EXECUTE_INVALID, "execution at an invalid address"),
        (T::INSTRUCTION_POINTER_UNKNOWN, "instruction pointer, address unknown"),
        (T::INSTRUCTION_POINTER_INVALID, "instruction pointer, invalid address"),
        (T::INSTRUCTION_POINTER_FAULT, "instruction pointer, faulting address"),
    ];
    named.iter().find(|(known, _)| known.as_raw() == ty.as_raw()).map_or_else(|| format!("address type {}", ty.as_raw()), |(_, name)| (*name).to_string())
}

/// A fixed-size Vulkan description array as text, up to its first NUL.
fn description(chars: &[std::ffi::c_char]) -> String {
    let bytes: Vec<u8> = chars.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// Ask the lost device what faulted, as the dump's section. `None` when the extension is
/// not enabled on this device, the backend is not Vulkan, or the call failed.
///
/// Called the moment the loss is known, from whichever thread saw it, so the device is still
/// alive. Only reads.
pub fn query(device: &wgpu::Device) -> Option<String> {
    // SAFETY: only reads through the hal device, which stays alive in the guard.
    let hal = unsafe { device.as_hal::<wgpu::hal::api::Vulkan>() }?;
    if !hal.enabled_device_extensions().contains(&EXTENSION) {
        return None;
    }
    let raw = hal.raw_device();
    let instance = hal.shared_instance().raw_instance();
    let loader = ash::ext::device_fault::Device::new(instance, raw);
    let get = loader.fp().get_device_fault_info_ext;
    let handle = raw.handle();
    let mut counts = vk::DeviceFaultCountsEXT::default();
    // SAFETY: the extension is enabled on this device (checked above); the first call asks
    // for the counts alone, which a null info means [documented].
    let result = unsafe { get(handle, &mut counts, std::ptr::null_mut()) };
    if result != vk::Result::SUCCESS {
        return Some(format!("gpu recorder: VK_EXT_device_fault: vkGetDeviceFaultInfoEXT gave {} asking for the counts\n", result.as_raw()));
    }
    let mut addresses = vec![vk::DeviceFaultAddressInfoEXT::default(); counts.address_info_count as usize];
    let mut vendors = vec![vk::DeviceFaultVendorInfoEXT::default(); counts.vendor_info_count as usize];
    let mut info = vk::DeviceFaultInfoEXT::default();
    info.p_address_infos = addresses.as_mut_ptr();
    info.p_vendor_infos = vendors.as_mut_ptr();
    // The vendor binary is not asked for (module notes): tell the driver its size is zero.
    counts.vendor_binary_size = 0;
    // SAFETY: both arrays hold the counts the call just reported, and the info's pointers
    // are theirs; no vendor binary is asked for, so `p_vendor_binary_data` stays null.
    let result = unsafe { get(handle, &mut counts, &mut info) };
    if result != vk::Result::SUCCESS && result != vk::Result::INCOMPLETE {
        return Some(format!("gpu recorder: VK_EXT_device_fault: vkGetDeviceFaultInfoEXT gave {} asking for the infos\n", result.as_raw()));
    }
    let addresses: Vec<(String, u64, u64)> =
        addresses.iter().take(counts.address_info_count as usize).map(|a| (address_type(a.address_type), a.reported_address, a.address_precision)).collect();
    let vendors: Vec<(String, u64, u64)> =
        vendors.iter().take(counts.vendor_info_count as usize).map(|v| (description(&v.description), v.vendor_fault_code, v.vendor_fault_data)).collect();
    Some(super::report::fault_text(&description(&info.description), &addresses, &vendors))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A description is read up to its NUL and nothing beyond it.
    #[test]
    fn a_vulkan_description_stops_at_its_nul() {
        let mut chars = [0 as std::ffi::c_char; 16];
        for (slot, byte) in chars.iter_mut().zip(b"GPU hang\0junk".iter()) {
            *slot = *byte as std::ffi::c_char;
        }
        assert_eq!(description(&chars), "GPU hang");
        assert_eq!(description(&[0 as std::ffi::c_char; 4]), "");
    }

    /// Every address type the headers name has words, and an unknown one still prints.
    #[test]
    fn address_types_have_words() {
        assert_eq!(address_type(vk::DeviceFaultAddressTypeEXT::READ_INVALID), "read of an invalid address");
        assert_eq!(address_type(vk::DeviceFaultAddressTypeEXT::EXECUTE_INVALID), "execution at an invalid address");
        assert_eq!(address_type(vk::DeviceFaultAddressTypeEXT::from_raw(99)), "address type 99");
    }
}
