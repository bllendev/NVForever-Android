//! A bounded GPU flight recorder, written only when the graphics device is lost.
//!
//! **Why** (AYN Odin 3, Android 15, Adreno 830 Vulkan): the game hits wgpu's `DeviceLost`
//! ("Unknown Device is lost") a few seconds after a fresh-process save load, and once
//! 18 minutes into outdoor combat. All the evidence a run leaves today is that one line and,
//! later, a `BufferAsyncError` on the water-pixels readback, which is a victim of the loss,
//! not its cause. This records what the frames before the loss were doing, so the next
//! occurrence names a suspect. It is diagnostics only: nothing here changes what is drawn,
//! and there is no setting for it.
//!
//! **Cost.** Nothing is measured for the recorder's sake. Per frame it is one atomic swap
//! for each counter, two atomic loads, one uncontended mutex lock and one write of a
//! [`ring::Frame`] (about 60 bytes) into a fixed array: well under a microsecond, no
//! allocation, no log line. Marking a feature ([`mark`]) is a single relaxed `fetch_or`.
//! Figures too costly to read every frame (the GPU allocator's report, which walks its
//! blocks) are read once every [`SUMMARY_SECS`] and reported with their age.
//!
//! **What a frame records** ([`ring::Frame`]): Bevy's frame index, the time since the last
//! loading screen came up, the frame's wall time, which optional work ran
//! ([`ring::feature`]), the bytes and writes uploaded through `RenderQueue::write_buffer`
//! and `write_texture` (counted in the vendored `bevy_render`, `engine/vendor/README.md`),
//! the pipelines queued or compiling and the pipelines asked for so far
//! (`pipeline_gate.rs`), and the readbacks whose buffer map has not come back (counted in
//! the vendored `bevy_render`'s `gpu_readback`). Read every [`SUMMARY_SECS`] instead of
//! every frame: the GPU allocator's handed-out and reserved bytes, its blocks (one
//! `vkAllocateMemory` each) and live allocations, and this process's use of the
//! `VK_EXT_memory_budget` budget (`memory_budget.rs` reads that one every 2 s anyway).
//!
//! **Where it is written.**
//! - A one-line summary in the run log every [`SUMMARY_SECS`], at most.
//! - On a lost device: the whole ring, one line a frame, in the run log and the console
//!   ([`log_dump_once`], as one write), and the same text in the crash report under the
//!   loss (crash.rs, both the panic path and [`crash::render_error`]'s no-panic path).
//! - Nothing else, ever. A run that does not lose its device writes the summary line only.
//!
//! **The driver's own fault info** (`fault.rs`): with `VK_EXT_device_fault` the loss is
//! followed at once by `vkGetDeviceFaultInfoEXT`, whose description, address infos and
//! vendor infos go into the dump. The extension is enabled at device creation only when the
//! adapter offers it and its `deviceFault` feature; without it the dump says so and
//! everything else is recorded as before.
//!
//! **Marks and frames.** A feature bit is set by the system that does the work, from the
//! main world or the render world, and the bits are swapped out at the end of each main-world
//! frame (`Last`). A render-world mark can therefore land in the record of the next frame.
//! That is a frame of slack in a 128-frame window, which the cost rules out closing.
//!
//! [`crash::render_error`]: crate::crash

pub mod breadcrumbs;
pub mod fault;
pub mod report;
pub mod ring;
pub mod rss;
#[cfg(test)]
mod tests;
pub mod transparent_chunks;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;

use report::{Sampled, MB};
use ring::{Frame, Ring};

/// How often the summary line is logged and the sampled figures are read.
pub const SUMMARY_SECS: f64 = 10.0;

/// The ring. One uncontended lock a frame; read again only when the device is lost.
static RING: Mutex<Ring> = Mutex::new(Ring::new());

/// The optional work of the frame being recorded ([`ring::feature`] bits).
static FEATURES: AtomicU32 = AtomicU32::new(0);

/// The device, for [`fault::query`] from whichever thread sees the loss.
static DEVICE: OnceLock<wgpu::Device> = OnceLock::new();

/// Set the first time the device is known lost.
static LOST: AtomicBool = AtomicBool::new(false);
/// Set by the first caller to log the dump, so it is logged once.
static DUMPED: AtomicBool = AtomicBool::new(false);
/// What the driver said about the fault, asked for the moment the loss was known.
static FAULT: OnceLock<Option<String>> = OnceLock::new();
/// The last reading of the figures too costly to read every frame, and when it was taken.
static SAMPLED: Mutex<(Sampled, Option<std::time::Instant>)> = Mutex::new((
    Sampled { age_secs: None, allocated_bytes: None, reserved_bytes: None, blocks: None, allocations: None, vram_use_mb: None, rss_bytes: None, proc_mem: None },
    None,
));

/// The last reading with its age filled in.
fn sampled_now() -> Sampled {
    let Ok(held) = SAMPLED.try_lock() else { return Sampled::default() };
    let (mut sampled, at) = *held;
    sampled.age_secs = at.map(|at| at.elapsed().as_secs_f64());
    sampled
}

/// Note that `bits` of [`ring::feature`] work ran. One relaxed `fetch_or`; callable from any
/// thread and any world.
#[inline]
pub fn mark(bits: u32) {
    FEATURES.fetch_or(bits, Ordering::Relaxed);
}

/// The graphics device is lost: ask the driver what faulted while the device is still there,
/// and have the dump written. Called from the log writer that sees wgpu's line
/// (`crash::note_device_lost`), so it must not log (that would recurse into the subscriber)
/// and must not wait on the ring. The one thing it does wait on is the driver's
/// `vkGetDeviceFaultInfoEXT`, which a lost device answers at once [documented], and only
/// where the extension is enabled; the GPU allocator is deliberately *not* walked here (its
/// lock may be held by the thread that is dying).
pub fn note_lost() {
    if LOST.swap(true, Ordering::SeqCst) {
        return;
    }
    let fault = DEVICE.get().and_then(fault::query);
    let _ = FAULT.set(fault);
}

/// Whether the device is known lost.
pub fn lost() -> bool {
    LOST.load(Ordering::SeqCst)
}

/// The dump's text: what the ring holds, the sampled figures and the driver's fault info.
/// `why` is what wgpu said about the loss.
pub fn dump_text(why: &str) -> String {
    let (frames, pushed) = match RING.try_lock() {
        Ok(ring) => (ring.in_order(), ring.pushed()),
        // A frame was being recorded as the loss came: the records are not worth a wait.
        Err(_) => (Vec::new(), 0),
    };
    let fault = FAULT.get().and_then(|f| f.as_deref());
    // The breadcrumbs last: the node the GPU was in is the answer the rest of the dump is
    // context for, and it is `""` unless this run was asked for them (`breadcrumbs`). The
    // transparent pass's chunks come after the nodes, being the finer grain of the same
    // answer, and are `""` unless that switch was asked for too.
    format!(
        "{}{}{}",
        report::dump(why, &frames, pushed, &sampled_now(), fault),
        breadcrumbs::section(),
        transparent_chunks::section()
    )
}

/// The dump for a crash report, or `""` when the device is not lost. Appended under the
/// loss in the report (crash.rs).
pub fn report_section() -> String {
    if !lost() {
        return String::new();
    }
    format!("\n\n{}", dump_text("reported by wgpu"))
}

/// Log the dump, once a run, as one write (the run log and the console get it). Called from
/// the paths that end a run for a lost device (crash.rs); never from a log writer.
pub fn log_dump_once(why: &str) {
    if !lost() || DUMPED.swap(true, Ordering::SeqCst) {
        return;
    }
    error!("{}", dump_text(why));
}

pub struct GpuRecorderPlugin;

impl Plugin for GpuRecorderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Recorder>().add_systems(Startup, stash_device).add_systems(Last, record);
    }

    /// The render world exists by now: say whether `VK_EXT_device_fault` is on, and whether
    /// this run records GPU breadcrumbs.
    fn finish(&self, app: &mut App) {
        fault::report_start(app);
        breadcrumbs::report_start();
        // Android: which allocator this build's allocations go to, and the first reading of
        // the figures an out-of-memory abort is read back against (`odin::alloc`,
        // `odin::proc_mem`). Nothing is read or said on any other platform.
        #[cfg(target_os = "android")]
        {
            let mem = crate::odin::proc_mem::sample().map_or_else(String::new, |m| format!("; {}", m.text()));
            info!("android memory: this build's allocator is {}{mem}", crate::odin::alloc::NAME);
        }
    }
}

/// What the recorder keeps between frames in the main world.
#[derive(Resource, Default)]
struct Recorder {
    /// When the last loading screen came up (real seconds), and whether one is up now.
    load_started: Option<f64>,
    loading: bool,
    /// When the summary was last logged and the costly figures last read.
    last_summary: Option<f64>,
}

/// Keep the device for [`fault::query`], which runs where no `World` is in reach.
fn stash_device(device: Option<Res<RenderDevice>>) {
    if let Some(device) = device {
        let _ = DEVICE.set(device.wgpu_device().clone());
    }
}

/// Read the GPU allocator's totals. Walks its blocks, so this is the part read every
/// [`SUMMARY_SECS`] rather than every frame.
fn sample_allocator(device: Option<&RenderDevice>) -> (Option<u64>, Option<u64>, Option<usize>, Option<usize>) {
    let Some(report) = device.and_then(|d| d.wgpu_device().generate_allocator_report()) else {
        return (None, None, None, None);
    };
    (Some(report.total_allocated_bytes), Some(report.total_reserved_bytes), Some(report.blocks.len()), Some(report.allocations.len()))
}

/// One record a frame, and the summary line every [`SUMMARY_SECS`]. The whole per-frame cost
/// of the recorder is here.
#[expect(clippy::too_many_arguments, reason = "the recorder reads what others already keep rather than measure its own")]
fn record(
    mut state: ResMut<Recorder>,
    real: Res<Time<Real>>,
    frame: Res<FrameCount>,
    gate: Option<Res<crate::pipeline_gate::PipelineGate>>,
    loading: Option<Res<nvf_play::loading::Loading>>,
    budget: Option<Res<crate::memory_budget::MemoryBudget>>,
    device: Option<Res<RenderDevice>>,
    mut summary: Local<bool>,
) {
    use bevy::render::gpu_counters as counters;
    let now = real.elapsed_secs_f64();
    // The loading screen: when the last one came up, and whether one is up now. Each end
    // of one is worth the process's resident memory on its own line: the abort of
    // 2026-11-22 came during a load, and a load is where the main memory moves most
    // (`rss`). Two readings a load, never per frame.
    let up = loading.is_some_and(|l| l.active());
    if up != state.loading {
        if up {
            state.load_started = Some(now);
        }
        if let Some(bytes) = rss::rss_bytes() {
            let since = state.load_started.filter(|_| !up).map_or_else(String::new, |at| format!(", {:.1} s of loading", now - at));
            info!("gpu recorder: resident memory {:.0} MB at the {} of a loading screen{since}", bytes as f64 / MB, if up { "start" } else { "end" });
        }
    }
    state.loading = up;
    let since_load_ms = state.load_started.map(|started| ((now - started) * 1000.0).clamp(0.0, f64::from(u32::MAX)) as u32);
    let features = FEATURES.swap(0, Ordering::Relaxed) | if up { ring::feature::LOADING } else { 0 };
    let record = Frame {
        index: u64::from(frame.0),
        since_load_ms,
        frame_us: real.delta().as_micros().min(u128::from(u32::MAX)) as u32,
        features,
        upload_bytes: counters::UPLOAD_BYTES.swap(0, Ordering::Relaxed),
        uploads: counters::UPLOADS.swap(0, Ordering::Relaxed),
        pipelines_busy: gate.as_deref().map_or(0, |g| g.busy()),
        pipelines_asked: gate.as_deref().map_or(0, |g| g.asked()),
        readbacks: counters::READBACKS_IN_FLIGHT.load(Ordering::Relaxed),
    };
    if let Ok(mut ring) = RING.try_lock() {
        ring.push(record);
    }
    // Everything below runs once every SUMMARY_SECS, not every frame.
    let due = state.last_summary.is_none_or(|last| now - last >= SUMMARY_SECS);
    if !due {
        return;
    }
    state.last_summary = Some(now);
    let (allocated_bytes, reserved_bytes, blocks, allocations) = sample_allocator(device.as_deref());
    let sampled = Sampled {
        age_secs: Some(0.0),
        allocated_bytes,
        reserved_bytes,
        blocks,
        allocations,
        vram_use_mb: budget.and_then(|b| b.vram_use_mb),
        // The main memory: one file read, once every SUMMARY_SECS.
        rss_bytes: rss::rss_bytes(),
        // Android only: the mappings against their limit, the allocator's own figures and
        // the resident set's two parts (`odin::proc_mem`). Two small /proc reads and one
        // pass over /proc/self/maps, once every SUMMARY_SECS; nothing at all elsewhere.
        proc_mem: crate::odin::proc_mem::sample(),
    };
    if let Ok(mut held) = SAMPLED.try_lock() {
        *held = (sampled, Some(std::time::Instant::now()));
    }
    // The first tick is the run's first frames: nothing worth a line yet.
    if !std::mem::replace(&mut *summary, true) {
        return;
    }
    let frames = RING.try_lock().map(|r| r.in_order()).unwrap_or_default();
    info!("{}", report::summary_line(&frames, &sampled));
}
