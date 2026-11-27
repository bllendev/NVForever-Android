//! NVForever patch: GPU breadcrumbs, so a lost device names the node that hung
//! (`engine/vendor/README.md`).
//!
//! **Why.** On the AYN Odin 3 (Adreno 830, Android 15) the Vulkan device is lost about two
//! seconds after a freshly loaded world is first drawn. `VK_EXT_device_fault` reports no
//! description and no addresses, and the Khronos validation layer finds nothing wrong with
//! the API use before the loss, so the suspect is a submission the GPU never finishes: a
//! watchdog reset. The one datum that would name it is *what the GPU was executing*, which
//! no CPU-side record can say. Breadcrumbs say it: a marker the GPU itself writes into
//! host-visible memory before and after each node's work, read back after the loss.
//!
//! **Granularity.** One begin and one end marker per *render-graph node*, which in Bevy 0.20
//! is one system beneath the [`RenderGraph`](crate::renderer::RenderGraph) schedule: the
//! prepass, the shadow passes, opaque, transparent, post-processing, UI and the game's own
//! nodes each record into their own command encoder ([`RenderContext`]), and the markers go
//! into *that* encoder, first and last. Nodes that record nothing never open an encoder and
//! so leave no marker, which is itself the right answer.
//!
//! **How a marker works.** The buffer is `SLOTS` 32-bit words in `HOST_VISIBLE |
//! HOST_COHERENT` memory, allocated through raw Vulkan (not wgpu, which has no
//! persistently-mapped path for this) and mapped for the whole run. Every marker takes the
//! next value of a monotonic counter, claims the slot `seq % SLOTS`, notes in a CPU-side
//! table what that slot means, and records one `vkCmdFillBuffer` of `seq` into it. After the
//! loss the mapping still reads: a slot holding exactly the `seq` the table expects is a
//! marker the GPU *reached*; anything else is a marker it never got to. The last begin that
//! ran with no matching end is the node that hung.
//!
//! **Why each marker is its own command buffer.** The obvious place for the fill is the
//! node's own encoder, but wgpu-core 30 forbids it: the first `as_hal_mut` on an encoder
//! marks it `EncodingApi::Raw`, and the next ordinary wgpu call on it panics with "Mixing
//! the wgpu encoding API with the raw encoding API is not permitted"
//! [observed 2026-11-21, a desktop run with the switch on; `wgpu-core-30.0.1/src/command/
//! mod.rs:568`]. So [`mark`] makes a command encoder of its own, uses it for nothing but the
//! fill, finishes it, and `RenderContext` puts the resulting command buffer into the frame's
//! pending list immediately before the node's own buffer (begin) or immediately after it
//! (end). The order the queue is given is therefore unchanged: marker, node, marker.
//!
//! **Cost**, and why it is off by default: per marker, one relaxed `fetch_add`, two relaxed
//! stores, and a command buffer of one command: a `vkBeginCommandBuffer`,
//! `vkCmdFillBuffer` and `vkEndCommandBuffer` on a buffer wgpu recycles. For a frame of
//! 30-60 nodes that is 60-120 extra one-command buffers and of the order of a few hundred
//! microseconds of CPU time: a diagnostic's price, not a setting's. [observed 2026-11-21,
//! one pair of desktop runs, Goodsprings 12:00, the `dev` profile: the recorder's first
//! 128-frame window read 9.2 ms a frame with the switch on against 8.5 ms with it off. One
//! run each, so that is an order of magnitude, not a measurement.] `NVF_GPU_BREADCRUMBS=1`
//! turns it on (the Android launcher's Advanced page sets it); unset
//! or `0`, every function here returns at its first line and no Vulkan object is ever made.
//!
//! **What it does not promise.** No barrier is issued for the host read: the fills are
//! ordinary transfer writes to coherent memory, so a marker's value becomes visible to the
//! CPU when the driver flushes it, which on a hung queue is best effort. Nor do the fills
//! order themselves against the node's own work: without barriers a driver may overlap
//! commands within a submission, so a marker says the GPU *began that node's commands*, not
//! that every earlier command retired. Both are the usual breadcrumb bargain, and both are
//! the reason the markers are read as a window rather than a single "current node".
//!
//! [`RenderContext`]: crate::renderer::RenderContext

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Slots in the breadcrumb buffer, and entries in the table beside it. Two markers per node
/// and 30-60 nodes a frame: 1024 holds the last 8-16 frames, well past the 2-3 frames the
/// driver can have in flight.
pub const SLOTS: usize = 1024;

/// The next marker's sequence number. Never 0, so a zeroed slot is never mistaken for a
/// marker the GPU reached.
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Per slot, the sequence number of the marker that last claimed it.
static SEQ: [AtomicU32; SLOTS] = [const { AtomicU32::new(0) }; SLOTS];

/// Per slot, what that marker was: `frame << 17 | node << 1 | end`.
static INFO: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];

/// The frame being recorded, bumped by [`begin_frame`] from `render_system`.
static FRAME: AtomicU64 = AtomicU64::new(0);

/// `NVF_GPU_BREADCRUMBS`: unset or `0` is off, anything else is on. Read once.
pub fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("NVF_GPU_BREADCRUMBS").is_ok_and(|v| v.trim() != "0" && !v.trim().is_empty()))
}

/// A new frame is being recorded. Called at the top of `render_system`, before the
/// [`RenderGraph`](crate::renderer::RenderGraph) schedule runs.
#[inline]
pub fn begin_frame() {
    if on() {
        FRAME.fetch_add(1, Ordering::Relaxed);
    }
}

/// The frame [`begin_frame`] last started, which is the frame every marker recorded from
/// here on belongs to. A caller that keeps its own table of what a marker meant (the
/// transparent pass's chunks, nvforever `gpu_recorder::transparent_chunks`) keys it by this,
/// so the dump can say what the node name it reads back stood for on that frame.
#[inline]
pub fn frame() -> u64 {
    FRAME.load(Ordering::Relaxed)
}

/// The names the node indices in [`INFO`] stand for. Each render system interns its name
/// once, on the frame it first records anything, and keeps the index in its own
/// `RenderContextState`, so no marker after that costs a lookup.
static NAMES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// The index for `name`, adding it if it is new. `u16::MAX` means the table is full, which
/// only a run with more than 65535 distinct render systems could reach.
///
/// The table is never emptied and the lookup is linear, so a caller that names markers
/// itself ([`RenderContext::gpu_breadcrumb`](crate::renderer::RenderContext::gpu_breadcrumb))
/// must draw them from a small fixed set of names rather than build a name per frame.
pub fn node(name: &str) -> u16 {
    let Ok(mut names) = NAMES.lock() else { return u16::MAX };
    if let Some(found) = names.iter().position(|known| known == name) {
        return found.min(u16::MAX as usize) as u16;
    }
    if names.len() >= u16::MAX as usize {
        return u16::MAX;
    }
    names.push(name.to_string());
    (names.len() - 1) as u16
}

/// The name a node index stands for.
fn node_name(index: u16) -> String {
    match NAMES.lock() {
        Ok(names) => names.get(index as usize).cloned().unwrap_or_else(|| format!("node {index}")),
        Err(_) => format!("node {index}"),
    }
}

/// One marker as a command buffer of its own (module notes): the GPU is told to write `seq`
/// into its slot, and the table is told what that slot means. `end` marks the end of the
/// node's recording rather than its start. The caller submits it in order around the node's
/// own command buffer.
///
/// `None` when the switch is off, the backend is not Vulkan, or the buffer could not be made.
/// The `vkCmdFillBuffer` is the encoder's only command, so it is outside any render pass.
pub fn mark(device: &wgpu::Device, node: u16, end: bool) -> Option<wgpu::CommandBuffer> {
    if !on() {
        return None;
    }
    let raw = buffer(device)?;
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    let slot = (seq % SLOTS as u64) as usize;
    let frame = FRAME.load(Ordering::Relaxed);
    // The meaning first, the sequence number last: a reader that sees the sequence number
    // sees a meaning already written.
    INFO[slot].store((frame << 17) | (u64::from(node) << 1) | u64::from(end), Ordering::Relaxed);
    SEQ[slot].store(seq as u32, Ordering::Release);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("nvforever gpu breadcrumb") });
    vulkan::fill(&mut encoder, raw, slot, seq as u32);
    Some(encoder.finish())
}

/// What one marker in the window turned out to be, for a report after a lost device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    /// The marker's sequence number: the order the CPU recorded the markers in.
    pub seq: u32,
    /// The frame whose render graph recorded it.
    pub frame: u64,
    /// The render-graph node (the render system's name).
    pub node: String,
    /// The end of that node's recording rather than its start.
    pub end: bool,
    /// The GPU reached this marker: its slot holds the value the fill was told to write.
    pub ran: bool,
}

/// The markers still in the window, oldest first, or `None` when nothing was ever recorded
/// (the switch is off, the backend is not Vulkan, or no node has opened an encoder yet).
///
/// Reads the mapping, which a lost device leaves alone: it is ordinary host memory. Takes no
/// Vulkan call and no device lock, so it is safe to call from the path that handles the loss.
pub fn marks() -> Option<Vec<Mark>> {
    if !on() {
        return None;
    }
    let raw = *RAW.get()?.as_ref()?;
    let next = NEXT.load(Ordering::Relaxed);
    if next <= 1 {
        return None;
    }
    let first = next.saturating_sub(SLOTS as u64).max(1);
    let mut out = Vec::with_capacity((next - first) as usize);
    for seq in first..next {
        let slot = (seq % SLOTS as u64) as usize;
        if SEQ[slot].load(Ordering::Acquire) != seq as u32 {
            // Claimed again since, or never written: not part of the window any more.
            continue;
        }
        let info = INFO[slot].load(Ordering::Relaxed);
        // SAFETY: the mapping is `SLOTS` words long and lives as long as the process; `slot`
        // is within it, and nothing else writes it from the CPU.
        let written = unsafe { std::ptr::read_volatile(raw.mapped.add(slot)) };
        out.push(Mark {
            seq: seq as u32,
            frame: info >> 17,
            node: node_name(((info >> 1) & u64::from(u16::MAX)) as u16),
            end: info & 1 == 1,
            ran: written == seq as u32,
        });
    }
    Some(out)
}

/// The raw Vulkan buffer and its mapping, made once and never destroyed: the whole point is
/// that it outlives a device that is already lost, and a run only makes one.
#[derive(Clone, Copy)]
struct Raw {
    mapped: *mut u32,
    /// The `VkBuffer`, as the plain handle so this struct needs no Vulkan types.
    buffer: u64,
}

// SAFETY: the mapping is `HOST_COHERENT` host memory written only through the atomics'
// discipline above (one CPU writer per slot, by sequence number) and read volatile.
unsafe impl Send for Raw {}
unsafe impl Sync for Raw {}

static RAW: std::sync::OnceLock<Option<Raw>> = std::sync::OnceLock::new();

/// The buffer, made on the first marker of the run. `None` once it is known it cannot be
/// made, so the attempt is never repeated.
fn buffer(device: &wgpu::Device) -> Option<Raw> {
    *RAW.get_or_init(|| {
        let made = vulkan::create(device);
        match made {
            Some(_) => bevy_log::info!("gpu breadcrumbs: on (NVF_GPU_BREADCRUMBS): {SLOTS} slots of host-visible memory; a lost device will name the last node the GPU began and the last it ended"),
            None => bevy_log::warn!("gpu breadcrumbs: asked for (NVF_GPU_BREADCRUMBS) but no breadcrumb buffer could be made (not the Vulkan backend, or no host-visible memory type): no markers are recorded"),
        }
        made
    })
}

#[cfg(feature = "raw_vulkan_init")]
mod vulkan {
    use super::{Raw, SLOTS};
    use ash::vk::{self, Handle as _};

    /// The `ash::Device` every marker records through, cloned from the one the buffer was
    /// made on. wgpu-hal's `CommandEncoder` keeps its own device privately, and it is the
    /// same device: the encoder came from it.
    static DEVICE: std::sync::OnceLock<ash::Device> = std::sync::OnceLock::new();

    /// The breadcrumb buffer: `SLOTS` words of `HOST_VISIBLE | HOST_COHERENT` memory, bound,
    /// zeroed and mapped for the rest of the run.
    ///
    /// Deliberately never freed. The device it belongs to is only destroyed when the process
    /// ends, and freeing it on a lost device would be the one thing that could make the dump
    /// unreadable.
    pub(super) fn create(device: &wgpu::Device) -> Option<Raw> {
        // SAFETY: every Vulkan object below is this module's own making; the hal device,
        // physical device and instance are borrowed through the guard, and only the
        // `ash::Device` (a refcounted handle table) is kept.
        unsafe {
            let hal = device.as_hal::<wgpu::hal::api::Vulkan>()?;
            let raw = hal.raw_device();
            let instance = hal.shared_instance().raw_instance();
            let physical = hal.raw_physical_device();
            let bytes = (SLOTS * size_of::<u32>()) as vk::DeviceSize;
            let info = vk::BufferCreateInfo::default().size(bytes).usage(vk::BufferUsageFlags::TRANSFER_DST).sharing_mode(vk::SharingMode::EXCLUSIVE);
            let buffer = raw.create_buffer(&info, None).ok()?;
            let needs = raw.get_buffer_memory_requirements(buffer);
            let memories = instance.get_physical_device_memory_properties(physical);
            let wanted = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            let kind = (0..memories.memory_type_count).find(|i| needs.memory_type_bits & (1 << i) != 0 && memories.memory_types[*i as usize].property_flags.contains(wanted))?;
            let ask = vk::MemoryAllocateInfo::default().allocation_size(needs.size).memory_type_index(kind);
            let memory = raw.allocate_memory(&ask, None).ok()?;
            raw.bind_buffer_memory(buffer, memory, 0).ok()?;
            let mapped = raw.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty()).ok()?.cast::<u32>();
            // A zeroed slot is "never written", which sequence numbers from 1 up never are.
            std::ptr::write_bytes(mapped, 0, SLOTS);
            let _ = DEVICE.set(raw.clone());
            Some(Raw { mapped, buffer: buffer.as_raw() })
        }
    }

    /// One `vkCmdFillBuffer` of `value` into `slot`, on the encoder's own raw command buffer.
    pub(super) fn fill(encoder: &mut wgpu::CommandEncoder, raw: Raw, slot: usize, value: u32) {
        let Some(device) = DEVICE.get() else { return };
        // SAFETY: the callback neither destroys the command buffer nor ends it, and touches
        // no wgpu state; the buffer filled is this module's own, which wgpu does not track,
        // so no barrier or transition of wgpu's is invalidated. wgpu-core opens the hal
        // encoder before the callback runs, so its handle is in the recording state, and both
        // call sites are outside any render pass. The offset and size are four-byte multiples
        // within the buffer's own size, as `vkCmdFillBuffer` requires.
        unsafe {
            encoder.as_hal_mut::<wgpu::hal::api::Vulkan, _, _>(|hal| {
                let Some(hal) = hal else { return };
                let command_buffer = hal.raw_handle();
                if command_buffer.is_null() {
                    return;
                }
                device.cmd_fill_buffer(command_buffer, vk::Buffer::from_raw(raw.buffer), (slot * size_of::<u32>()) as vk::DeviceSize, size_of::<u32>() as vk::DeviceSize, value);
            });
        }
    }
}

#[cfg(not(feature = "raw_vulkan_init"))]
mod vulkan {
    use super::Raw;

    /// Without `raw_vulkan_init` there is no `ash` to allocate through, so there are no
    /// breadcrumbs and `mark` returns at `buffer`.
    pub(super) fn create(_device: &wgpu::Device) -> Option<Raw> {
        None
    }

    pub(super) fn fill(_encoder: &mut wgpu::CommandEncoder, _raw: Raw, _slot: usize, _value: u32) {}
}
