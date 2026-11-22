//! What the recorder writes: the ten-second summary line and the dump a lost device gets.
//!
//! Pure formatting over [`Frame`] records and a [`Sampled`] reading, so it is tested without
//! a GPU.

use super::ring::{feature_text, Frame};

/// Figures read now and then rather than every frame (the GPU allocator's report walks its
/// blocks, and the memory budget is the driver's estimate): what the last reading said.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sampled {
    /// How long ago the reading was taken, seconds.
    pub age_secs: Option<f64>,
    /// `wgpu::Device::generate_allocator_report`: bytes handed out, bytes taken from the
    /// driver, blocks (one `vkAllocateMemory` each) and live allocations.
    pub allocated_bytes: Option<u64>,
    pub reserved_bytes: Option<u64>,
    pub blocks: Option<usize>,
    pub allocations: Option<usize>,
    /// `VK_EXT_memory_budget` through `memory_budget.rs`: (this process's use, budget) MB.
    pub vram_use_mb: Option<(u64, u64)>,
    /// This process's resident memory, bytes (`super::rss`): the main memory, where every
    /// figure above is the GPU's. A leak of ours is a number that only ever rises; the
    /// out-of-memory abort of 2026-11-22 had the GPU's own flat throughout.
    pub rss_bytes: Option<u64>,
    /// What the live wrist map's caches hold, bytes
    /// (`nvf_play::wrist_local::cache_bytes`): the outline grids and the pictures its
    /// screen wears, the one part of the glance that holds anything of a size.
    pub wrist_cache_bytes: Option<u64>,
}

pub(super) const MB: f64 = 1024.0 * 1024.0;

fn mb(bytes: u64) -> f64 {
    bytes as f64 / MB
}

impl Sampled {
    /// The sampled figures as one clause, or `""` when nothing was read.
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let (Some(a), Some(r)) = (self.allocated_bytes, self.reserved_bytes) {
            let blocks = self.blocks.map_or_else(String::new, |b| format!(" in {b} blocks"));
            let live = self.allocations.map_or_else(String::new, |n| format!(", {n} live allocations"));
            parts.push(format!("gpu allocated {:.0} MB, reserved {:.0} MB{blocks}{live}", mb(a), mb(r)));
        }
        if let Some((use_mb, budget)) = self.vram_use_mb {
            parts.push(format!("VRAM {use_mb} of {budget} MB"));
        }
        if let Some(rss) = self.rss_bytes {
            parts.push(format!("resident {:.0} MB", mb(rss)));
        }
        if let Some(wrist) = self.wrist_cache_bytes.filter(|&b| b > 0) {
            parts.push(format!("wrist caches {:.1} MB", mb(wrist)));
        }
        if parts.is_empty() {
            return String::new();
        }
        match self.age_secs {
            Some(age) => format!("{} (read {age:.1} s ago)", parts.join("; ")),
            None => parts.join("; "),
        }
    }
}

/// Totals over the records, for the summary line.
fn totals(frames: &[Frame]) -> (f32, f32, u64, u32, u32, u32, u32) {
    let n = frames.len().max(1) as f32;
    let mean_ms = frames.iter().map(|f| f.frame_us as f32 / 1000.0).sum::<f32>() / n;
    let worst_ms = frames.iter().map(|f| f.frame_us).max().unwrap_or(0) as f32 / 1000.0;
    let bytes = frames.iter().map(|f| f.upload_bytes).sum::<u64>();
    let uploads = frames.iter().map(|f| f.uploads).sum::<u32>();
    let busy = frames.iter().map(|f| f.pipelines_busy).max().unwrap_or(0);
    let readbacks = frames.iter().map(|f| f.readbacks).max().unwrap_or(0);
    let features = frames.iter().fold(0, |acc, f| acc | f.features);
    (mean_ms, worst_ms, bytes, uploads, busy, readbacks, features)
}

/// The one line logged every [`super::SUMMARY_SECS`]: the window the ring holds, nothing
/// per frame.
pub fn summary_line(frames: &[Frame], sampled: &Sampled) -> String {
    if frames.is_empty() {
        return "gpu recorder: no frames recorded yet".to_string();
    }
    let (mean_ms, worst_ms, bytes, uploads, busy, readbacks, features) = totals(frames);
    let sampled = sampled.text();
    let sampled = if sampled.is_empty() { String::new() } else { format!("; {sampled}") };
    format!(
        "gpu recorder: last {} frames: {mean_ms:.1} ms mean, {worst_ms:.1} ms worst; uploads {uploads} writes, {:.1} MB; \
         pipelines busy at most {busy}; readbacks at most {readbacks}; ran {}{sampled}",
        frames.len(),
        mb(bytes),
        feature_text(features),
    )
}

/// One record as a row of the dump.
fn row(f: &Frame) -> String {
    let since = f.since_load_ms.map_or_else(|| "-".to_string(), |ms| format!("{:.1}", ms as f64 / 1000.0));
    format!(
        "  {:>10} {:>8} {:>8.1} {:>6} {:>9.2} {:>6} {:>8} {:>5}  {}",
        f.index,
        since,
        f.frame_us as f64 / 1000.0,
        f.uploads,
        mb(f.upload_bytes),
        f.pipelines_busy,
        f.pipelines_asked,
        f.readbacks,
        feature_text(f.features),
    )
}

/// The dump's heading row.
pub const HEADER: &str = "       frame  load s  frame ms writes  up MB  busy    asked   rb  features";

/// The whole dump: what the ring holds, the sampled figures and the device fault, if any.
/// `pushed` is how many frames the run recorded in all, so a reader can see how many were
/// dropped. `why` is what wgpu said about the loss.
pub fn dump(why: &str, frames: &[Frame], pushed: u64, sampled: &Sampled, fault: Option<&str>) -> String {
    let mut out = String::with_capacity(256 + frames.len() * 96);
    out.push_str(&format!("gpu recorder: the graphics device was lost ({why})\n"));
    if frames.is_empty() {
        out.push_str("gpu recorder: no frames were recorded\n");
    } else {
        out.push_str(&format!("gpu recorder: the last {} of {pushed} frames of this run, oldest first:\n", frames.len()));
        out.push_str(HEADER);
        out.push('\n');
        for f in frames {
            out.push_str(&row(f));
            out.push('\n');
        }
    }
    let sampled = sampled.text();
    if !sampled.is_empty() {
        out.push_str(&format!("gpu recorder: {sampled}\n"));
    }
    match fault {
        Some(text) => out.push_str(text),
        None => out.push_str("gpu recorder: VK_EXT_device_fault: nothing to report\n"),
    }
    out
}

/// What a device fault reads as, given the driver's description, the address infos
/// (`type`, reported address, precision) and the vendor infos (description, code, data).
/// Separate from the Vulkan call so the wording is tested without a GPU.
pub fn fault_text(description: &str, addresses: &[(String, u64, u64)], vendors: &[(String, u64, u64)]) -> String {
    let mut out = String::new();
    out.push_str(&format!("gpu recorder: VK_EXT_device_fault: {}\n", if description.is_empty() { "(no description)" } else { description }));
    if addresses.is_empty() && vendors.is_empty() {
        out.push_str("gpu recorder: VK_EXT_device_fault: no address or vendor infos\n");
        return out;
    }
    for (kind, address, precision) in addresses {
        out.push_str(&format!("gpu recorder: VK_EXT_device_fault address: {kind} at 0x{address:016x} (precision 0x{precision:x})\n"));
    }
    for (description, code, data) in vendors {
        out.push_str(&format!("gpu recorder: VK_EXT_device_fault vendor: {description} (code 0x{code:x}, data 0x{data:x})\n"));
    }
    out
}
