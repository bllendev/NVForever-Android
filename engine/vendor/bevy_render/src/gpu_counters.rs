//! NVForever patch: free counters a GPU flight recorder reads (`engine/vendor/README.md`).
//!
//! The game's recorder (`engine/nvforever/src/gpu_recorder/`) needs to know, per frame, how
//! much was uploaded through the render queue and how many readbacks have not come back.
//! Neither has a hook in upstream bevy_render, and the recorder cannot be a dependency of
//! this crate, so the counts live here as plain atomics and the recorder reads and resets
//! them. Each is one relaxed atomic operation at the call site; with no reader they are
//! simply never read.

use core::sync::atomic::{AtomicU32, AtomicU64};

/// Bytes passed to [`RenderQueue::write_buffer`](crate::renderer::RenderQueue::write_buffer)
/// and [`RenderQueue::write_texture`](crate::renderer::RenderQueue::write_texture) since the
/// reader last reset this.
pub static UPLOAD_BYTES: AtomicU64 = AtomicU64::new(0);

/// How many such calls were made since the reader last reset this.
pub static UPLOADS: AtomicU32 = AtomicU32::new(0);

/// Readbacks (`crate::gpu_readback`) whose buffer map has been asked for and has not come
/// back. Goes up when the map is requested and down when its callback runs, so it is a live
/// count, not reset by the reader.
pub static READBACKS_IN_FLIGHT: AtomicU32 = AtomicU32::new(0);

/// Readback maps asked for since the start of the process.
pub static READBACKS_STARTED: AtomicU64 = AtomicU64::new(0);
