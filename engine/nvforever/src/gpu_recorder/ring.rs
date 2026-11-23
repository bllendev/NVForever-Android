//! The flight recorder's fixed-size ring and the per-frame record in it.
//!
//! Everything here is plain data and arithmetic: no GPU, no clock, no allocation while
//! recording. [`Ring`] holds [`FRAMES`] records in a const-constructible array, so it lives
//! in a `static` and never grows.

/// Frames kept. The device is lost a few seconds after a load on the Odin 3, and 120 frames
/// at 30-60 fps is 2-4 s of them [our choice; the task asked for about 120].
pub const FRAMES: usize = 128;

/// The optional work a frame did, one bit each. A bit is set by
/// [`crate::odin::gpu_recorder::mark`] from the system that does the work, so "set" means that
/// system ran in (or within a frame of) the recorded frame, not that a particular GPU pass
/// was submitted.
pub mod feature {
    /// A loading screen was up (`nvf_play::loading::Loading::active`).
    pub const LOADING: u32 = 1 << 0;
    /// The main world's terrain-shadow job stepped (`terrain_shadows::update_terrain_shadows`).
    pub const TERRAIN_SHADOWS: u32 = 1 << 1;
    /// The terrain-shadow compute ran in the render graph (`terrain_shadows::produce_terrain_shadows`).
    pub const TERRAIN_SHADOWS_GPU: u32 = 1 << 2;
    /// The sun heightfield produced or published cells (`sun_heightfield::update_heightfield`).
    pub const SUN_HEIGHTFIELD: u32 = 1 << 3;
    /// The water reflection camera was on (`reflection::update_reflection`).
    pub const WATER_REFLECTION: u32 = 1 << 4;
    /// A water-pixels readback was asked for (`reflection::read_water_pixels`).
    pub const WATER_PIXELS_READBACK: u32 = 1 << 5;
    /// The VATS mask camera was on (`vats_highlight::view`).
    pub const VATS_MASK: u32 = 1 << 6;
    /// The minimap / Pip-Boy screen camera was on (`minimap_render::update`).
    pub const MINIMAP: u32 = 1 << 7;
    /// Cloud shadows were produced (`cloud_shadows::produce_cloud_shadows`).
    pub const CLOUD_SHADOWS: u32 = 1 << 8;
    /// The ambient occlusion pass ran (`ssao::ao_pass`).
    pub const AO: u32 = 1 << 9;
    /// The AO-in-lighting pass ran (`ssao::ao_lighting_pass`).
    pub const AO_LIGHTING: u32 = 1 << 10;
}

/// Every feature bit and its short name, in bit order (the report's `features` column).
pub const FEATURE_NAMES: [(u32, &str); 11] = [
    (feature::LOADING, "load"),
    (feature::TERRAIN_SHADOWS, "terrain-shadows"),
    (feature::TERRAIN_SHADOWS_GPU, "terrain-shadows-gpu"),
    (feature::SUN_HEIGHTFIELD, "sun-heightfield"),
    (feature::WATER_REFLECTION, "water-reflection"),
    (feature::WATER_PIXELS_READBACK, "water-pixels-readback"),
    (feature::VATS_MASK, "vats-mask"),
    (feature::MINIMAP, "minimap"),
    (feature::CLOUD_SHADOWS, "cloud-shadows"),
    (feature::AO, "ao"),
    (feature::AO_LIGHTING, "ao-lighting"),
];

/// The names of the features `bits` holds, in bit order; `-` for none.
pub fn feature_text(bits: u32) -> String {
    let names: Vec<&str> = FEATURE_NAMES.iter().filter(|(bit, _)| bits & bit != 0).map(|(_, name)| *name).collect();
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(",")
    }
}

/// One recorded frame. Only values an atomic counter or a shared count already holds are
/// in here: nothing is measured for the recorder's sake.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Bevy's frame count at the end of the frame.
    pub index: u64,
    /// Since the last loading screen came up, milliseconds; `None` before the first one.
    pub since_load_ms: Option<u32>,
    /// The whole frame's wall time (`Time<Real>`'s delta), microseconds.
    pub frame_us: u32,
    /// [`feature`] bits.
    pub features: u32,
    /// Bytes written through `RenderQueue::write_buffer` and `write_texture` in the frame.
    pub upload_bytes: u64,
    /// How many such writes.
    pub uploads: u32,
    /// Pipelines queued or compiling at the end of the frame (`pipeline_gate`).
    pub pipelines_busy: u32,
    /// Pipelines asked for since the start of the run (`pipeline_gate`); the difference
    /// between two frames is how many were asked for in between.
    pub pipelines_asked: u32,
    /// Readbacks whose buffer map has not come back (`bevy_render::gpu_counters`).
    pub readbacks: u32,
}

impl Frame {
    /// An unused slot.
    pub const EMPTY: Frame = Frame {
        index: 0,
        since_load_ms: None,
        frame_us: 0,
        features: 0,
        upload_bytes: 0,
        uploads: 0,
        pipelines_busy: 0,
        pipelines_asked: 0,
        readbacks: 0,
    };
}

impl Default for Frame {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// The last [`FRAMES`] records, oldest overwritten first.
pub struct Ring {
    frames: [Frame; FRAMES],
    /// Where the next record goes.
    next: usize,
    /// Records pushed since the start of the run (more than [`FRAMES`] once it has wrapped).
    pushed: u64,
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

impl Ring {
    pub const fn new() -> Self {
        Self { frames: [Frame::EMPTY; FRAMES], next: 0, pushed: 0 }
    }

    /// Keep `frame`, dropping the oldest when the ring is full. Never allocates.
    pub fn push(&mut self, frame: Frame) {
        self.frames[self.next] = frame;
        self.next = (self.next + 1) % FRAMES;
        self.pushed += 1;
    }

    /// How many records are held (at most [`FRAMES`]).
    pub fn len(&self) -> usize {
        (self.pushed as usize).min(FRAMES)
    }

    /// Kept beside [`Ring::len`] (clippy asks for it); the dump reads the records instead.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.pushed == 0
    }

    /// Records pushed since the start of the run, the dropped ones counted.
    pub fn pushed(&self) -> u64 {
        self.pushed
    }

    /// The records held, oldest first.
    pub fn in_order(&self) -> Vec<Frame> {
        let held = self.len();
        // The oldest held record: `next` once the ring has wrapped, slot 0 before that.
        let first = if self.pushed as usize > FRAMES { self.next } else { 0 };
        (0..held).map(|i| self.frames[(first + i) % FRAMES]).collect()
    }
}
