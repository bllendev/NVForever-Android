//! GPU breadcrumbs inside the transparent pass: which *draw* the GPU was in when it stopped.
//!
//! **Why.** The breadcrumbs of a whole render-graph node ([`super::breadcrumbs`]) named the
//! pass on the Odin 3: frames 2 to 13 of a save load completed, frame 14 began
//! `main_transparent_pass_3d` and never ended [observed 2026-11-22, AYN Odin 3, Adreno 830,
//! Android 15]. That pass draws every blended thing in the world in one render pass, in one
//! submission, so "the transparent pass hung" still covers particles, effect particles,
//! decals, alpha-blended world meshes and the sky's blended layers at once. This splits it.
//!
//! **How, and why not a marker between the draws.** A marker is a `vkCmdFillBuffer`, which
//! Vulkan does not allow inside a render pass [documented: the specification's valid usage
//! for `vkCmdFillBuffer`, "outside of a render pass instance"], so no marker can sit between
//! two draws of one pass. Instead the phase is cut into chunks: a run of consecutive sorted
//! items that share a draw function and a pipeline is drawn in a render pass of its own, with
//! a breadcrumb marker before and after it. The order is kept exactly as the phase sorted it
//! and the cuts fall only on batch boundaries, so what is drawn, and in what order, does not
//! change; what changes is that the frame holds one render pass per chunk instead of one, and
//! each of them loads and stores the colour and depth attachments, and with multisampling
//! resolves the target (the vendored `bevy_pbr`'s one resolve ends before this runs, as it
//! does before Bevy's own pass). On a tiler that is the whole cost and it is a large one:
//! this is a crash test, not a setting.
//!
//! **The switch.** `NVF_GPU_BREADCRUMB_CHUNKS` *and* `NVF_GPU_BREADCRUMBS`: the chunks exist
//! to be marked, and the markers come from [`super::breadcrumbs`]. They are the Advanced
//! page's "GPU breadcrumbs inside the transparent pass" and "GPU breadcrumbs" rows, the
//! first greyed until the second is on (`crate::odin::crash_tests`). Off, this module adds no
//! system and allocates nothing, and Bevy's own `main_transparent_pass_3d` draws the phase as
//! it always does.
//!
//! **What the dump says.** The markers carry a node name from a small fixed set
//! (`nvf transparent chunk 00` upwards, [`node_name`]), because the breadcrumb name table is
//! interned for the run and must not grow by a name a frame. What each of those names stood
//! for is kept here, per frame, for the last [`FRAMES_KEPT`] frames: the item range, the draw
//! count, the pipeline and its label, and the kinds of thing in it ([`TransparentKind`],
//! which particles, effect particles and decals tag their drawn entity with while the switch
//! is on). The dump reads the markers, finds every chunk the GPU began and never ended, and
//! prints what those chunks were.
//!
//! **What it cannot say.** Everything [`super::breadcrumbs`] cannot: no barrier orders a
//! marker against the draws around it, so "the GPU began this chunk" is not "every draw of
//! the chunk before it retired". A chunk that merged several runs because the frame had more
//! of them than [`MAX_CHUNKS`] says so (`mixed`), and then it covers more than one pipeline.

use std::collections::VecDeque;
use std::sync::Mutex;

use bevy::camera::{MainPassResolutionOverride, Viewport};
use bevy::core_pipeline::core_3d::{main_opaque_pass_3d, main_transparent_pass_3d, Transparent3d};
use bevy::pbr::end_deferred_resolve;
use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
use bevy::core_pipeline::{Core3d, Core3dSystems};
use bevy::ecs::schedule::{ScheduleCleanupPolicy, Schedules};
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::gpu_breadcrumbs::{self, Mark};
use bevy::render::render_phase::ViewSortedRenderPhases;
use bevy::render::render_resource::{PipelineCache, PipelineDescriptor};
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthStencilTexture, ViewTarget};
use bevy::render::{RenderApp, RenderStartup};

/// Chunks a frame may be cut into. More runs than this and adjacent runs are merged, so the
/// marker names stay a small fixed set (the breadcrumb name table is interned for the run).
pub const MAX_CHUNKS: usize = 32;

/// Frames of chunk tables kept, which must cover the breadcrumb window (8 to 16 frames).
pub const FRAMES_KEPT: usize = 24;

/// The start of every chunk marker's node name.
pub const NODE_PREFIX: &str = "nvf transparent chunk ";

/// The variable the switch is read from, named once in the fork's switch table.
fn env() -> &'static str {
    crate::odin::crash_tests::env("gpu-breadcrumb-chunks")
}

/// Whether this run was asked to cut the transparent pass into chunks. Both switches: the
/// chunks are only worth cutting to put a marker between them.
pub fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| super::breadcrumbs::requested() && crate::odin::crash_tests::gpu_breadcrumb_chunks())
}

/// The marker name of chunk `index`, from the fixed set [`MAX_CHUNKS`] long.
pub fn node_name(index: usize) -> &'static str {
    /// The names, made once, so no marker builds a string and the breadcrumb name table
    /// never grows past them.
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let names = NAMES.get_or_init(|| (0..MAX_CHUNKS).map(|i| format!("{NODE_PREFIX}{i:02}")).collect());
    names.get(index.min(MAX_CHUNKS - 1)).map_or("nvf transparent chunk ?", String::as_str)
}

/// The chunk index a marker's node name stands for, or `None` when the name is not a chunk's.
pub fn node_index(name: &str) -> Option<usize> {
    name.strip_prefix(NODE_PREFIX)?.trim().parse().ok()
}

/// What a game system drawing into the transparent phase tags its drawn entity with, so a
/// chunk can be named by what is in it rather than by a pipeline number. Inserted only while
/// [`on`], and extracted to the render world by [`TransparentChunksPlugin`].
#[derive(Component, Clone, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct TransparentKind(pub String);

/// The tag for a drawn entity. [`tag`] checks [`on`] first, so nothing is built in an
/// ordinary run.
pub fn kind(what: impl Into<String>) -> TransparentKind {
    TransparentKind(what.into())
}

/// Tag `entity` with what to call it in the chunk report, while the switch is on. The one
/// call a drawing system of the game's makes, so an upstream file carries no branch of ours
/// (`particles`, `effect_particles`, `decals`; docs/fork.md). `what` is not called at all in
/// an ordinary run, so the name costs nothing to build.
pub fn tag(commands: &mut Commands, entity: Entity, what: impl FnOnce() -> String) {
    if on() {
        commands.entity(entity).insert(kind(what()));
    }
}

/// One item of the sorted transparent phase, as the chunk plan reads it: nothing but the keys
/// the cuts are made on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    /// The phase item's draw function.
    pub draw_function: u32,
    /// Its pipeline (`CachedRenderPipelineId::id`).
    pub pipeline: usize,
    /// How many items of the phase its draw covers (`batch_range.len()`); 0 draws nothing.
    pub batch: usize,
}

/// One chunk of the transparent phase: a run of consecutive items drawn in a render pass of
/// its own, with a breadcrumb marker on each side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chunk {
    /// Its marker's name index, counted from 0 in phase order ([`node_name`]).
    pub index: usize,
    /// The phase items it covers, as a half-open range of indices.
    pub start: usize,
    pub end: usize,
    /// Draw calls in it (an item whose batch is empty draws nothing).
    pub draws: usize,
    /// The pipeline and draw function of its first item.
    pub pipeline: usize,
    pub draw_function: u32,
    /// Several runs were merged into it because the frame had more than [`MAX_CHUNKS`].
    pub mixed: bool,
    /// What the chunk holds, filled in when it is recorded: the pipeline's label and the
    /// kinds of thing in it. Empty in a plan.
    pub label: String,
}

/// The chunks `items` is cut into: a cut wherever the draw function or the pipeline changes,
/// and only on a batch boundary, so no draw is split. Merged down to `max` chunks, in equal
/// groups of adjacent runs, when a frame has more runs than that.
///
/// Pure, so it is tested without a GPU.
pub fn plan(items: &[Item], max: usize) -> Vec<Chunk> {
    let mut runs: Vec<Chunk> = Vec::new();
    let mut index = 0;
    while index < items.len() {
        let item = items[index];
        let key = (item.draw_function, item.pipeline);
        if !runs.last().is_some_and(|last| (last.draw_function, last.pipeline) == key) {
            runs.push(Chunk { start: index, end: index, pipeline: item.pipeline, draw_function: item.draw_function, ..Chunk::default() });
        }
        // `push` above makes sure of it.
        let Some(run) = runs.last_mut() else { break };
        // As `SortedRenderPhase::render_range` walks it: an empty batch draws nothing and
        // steps one item, a batch draws once and steps over every item it covers.
        if item.batch > 0 {
            run.draws += 1;
        }
        index += item.batch.max(1);
        run.end = index.min(items.len());
    }
    merge(runs, max)
}

/// Runs merged into at most `max` chunks, in equal groups of adjacent runs, and numbered from
/// 0 in phase order.
fn merge(runs: Vec<Chunk>, max: usize) -> Vec<Chunk> {
    let max = max.max(1);
    let group = if runs.len() <= max { 1 } else { runs.len().div_ceil(max) };
    let mut out: Vec<Chunk> = Vec::with_capacity(runs.len().min(max));
    for (nth, run) in runs.into_iter().enumerate() {
        if nth % group == 0 {
            out.push(Chunk { index: out.len(), ..run });
            continue;
        }
        // Every run has a key of its own, so merging two of them always mixes keys.
        let Some(last) = out.last_mut() else { continue };
        last.mixed = true;
        last.draws += run.draws;
        last.end = run.end;
    }
    out
}

/// The chunk tables of the last [`FRAMES_KEPT`] frames, oldest first.
static FRAMES: Mutex<VecDeque<(u64, Vec<Chunk>)>> = Mutex::new(VecDeque::new());

/// Keep `chunks` as what this frame's chunk markers meant. One uncontended lock a frame.
pub fn record(frame: u64, chunks: Vec<Chunk>) {
    let Ok(mut frames) = FRAMES.lock() else { return };
    // One table a frame: a second view's pass replaces the first's rather than hide it.
    if frames.back().is_some_and(|(kept, _)| *kept == frame) {
        frames.pop_back();
    }
    frames.push_back((frame, chunks));
    while frames.len() > FRAMES_KEPT {
        frames.pop_front();
    }
}

/// The dump's transparent-chunk section, or `""` when the switch is off. Reads only host
/// memory and plain locks, so it is safe on the path that handles the loss.
pub fn section() -> String {
    if !on() {
        return String::new();
    }
    let frames: Vec<(u64, Vec<Chunk>)> = match FRAMES.try_lock() {
        Ok(frames) => frames.iter().cloned().collect(),
        // A frame's table was being recorded as the loss came: not worth a wait.
        Err(_) => Vec::new(),
    };
    text(&gpu_breadcrumbs::marks().unwrap_or_default(), &frames)
}

/// One chunk as a line of the dump.
fn describe(chunk: &Chunk) -> String {
    format!(
        "chunk {:02} items {}..{}, {} draw(s), pipeline {}, draw fn {}{}{}",
        chunk.index,
        chunk.start,
        chunk.end,
        chunk.draws,
        chunk.pipeline,
        chunk.draw_function,
        if chunk.mixed { ", several runs merged" } else { "" },
        if chunk.label.is_empty() { String::new() } else { format!(" [{}]", chunk.label) },
    )
}

/// The chunks a frame's table holds, if it was kept.
fn table<'a>(frames: &'a [(u64, Vec<Chunk>)], frame: u64) -> Option<&'a [Chunk]> {
    frames.iter().find(|(kept, _)| *kept == frame).map(|(_, chunks)| chunks.as_slice())
}

/// The whole transparent-chunk section: which chunks the GPU began and never ended, and the
/// chunk tables that say what they were. Pure formatting over the markers and the tables, so
/// it is tested without a GPU.
pub fn text(marks: &[Mark], frames: &[(u64, Vec<Chunk>)]) -> String {
    let mut out = format!(
        "gpu breadcrumbs: the transparent pass was cut into chunks ({}): {} frame(s) of chunk tables kept\n",
        env(),
        frames.len(),
    );
    // A chunk the GPU began and never ended, in marker order. A chunk drawn twice in a frame
    // (two views) is closed once per end, as the node summaries do it.
    let mut open: Vec<(u64, usize)> = Vec::new();
    for mark in marks.iter().filter(|m| m.ran) {
        let Some(index) = node_index(&mark.node) else { continue };
        if mark.end {
            if let Some(at) = open.iter().position(|held| *held == (mark.frame, index)) {
                open.remove(at);
            }
        } else {
            open.push((mark.frame, index));
        }
    }
    if open.is_empty() {
        out.push_str("gpu breadcrumbs: no transparent chunk began and never ended\n");
    }
    for (frame, index) in &open {
        match table(frames, *frame).and_then(|chunks| chunks.iter().find(|c| c.index == *index)) {
            Some(chunk) => out.push_str(&format!("gpu breadcrumbs: began and never ended: frame {frame} {}\n", describe(chunk))),
            None => out.push_str(&format!("gpu breadcrumbs: began and never ended: frame {frame} chunk {index:02}, whose table was not kept\n")),
        }
    }
    // The tables worth printing: the frames with an open chunk, and the newest frame kept.
    let mut wanted: Vec<u64> = open.iter().map(|(frame, _)| *frame).collect();
    if let Some((newest, _)) = frames.last() {
        wanted.push(*newest);
    }
    wanted.sort_unstable();
    wanted.dedup();
    for frame in wanted {
        let Some(chunks) = table(frames, frame) else { continue };
        if chunks.is_empty() {
            out.push_str(&format!("gpu breadcrumbs: frame {frame} drew no transparent chunk\n"));
            continue;
        }
        for chunk in chunks {
            out.push_str(&format!("gpu breadcrumbs: frame {frame} {}\n", describe(chunk)));
        }
    }
    out
}

/// The pipeline's own debug label, as the dump names a chunk's pipeline by. Reads the cached
/// descriptor, which is kept for the run beside the pipeline.
fn pipeline_label(cache: &PipelineCache, pipeline: usize) -> String {
    let Some(cached) = cache.pipelines().nth(pipeline) else { return String::new() };
    let (label, shader) = match &cached.descriptor {
        PipelineDescriptor::RenderPipelineDescriptor(d) => (d.label.clone(), d.fragment.as_ref().and_then(|f| f.shader.path().map(ToString::to_string))),
        PipelineDescriptor::ComputePipelineDescriptor(d) => (d.label.clone(), d.shader.path().map(ToString::to_string)),
        PipelineDescriptor::MeshPipelineDescriptor(d) => (d.label.clone(), d.fragment.as_ref().and_then(|f| f.shader.path().map(ToString::to_string))),
    };
    let mut out = label.map(|l| l.to_string()).unwrap_or_default();
    if let Some(shader) = shader {
        let short = shader.rsplit('/').next().unwrap_or(&shader).to_string();
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&short);
    }
    out
}

/// What the dump calls a chunk: its pipeline's label and the kinds of thing in it
/// ([`TransparentKind`]), at most [`KINDS_SHOWN`] of them.
const KINDS_SHOWN: usize = 3;

/// Items of a chunk looked at for their kind. A chunk of hundreds of items is named by its
/// first few: the dump wants a name, not a census.
const KINDS_SCANNED: usize = 16;

fn label(world: &World, cache: &PipelineCache, chunk: &Chunk, entities: &[Entity]) -> String {
    let mut kinds: Vec<&str> = Vec::new();
    for entity in entities.iter().skip(chunk.start).take(KINDS_SCANNED) {
        let Some(kind) = world.get::<TransparentKind>(*entity) else { continue };
        if !kinds.contains(&kind.0.as_str()) {
            kinds.push(&kind.0);
        }
        if kinds.len() >= KINDS_SHOWN {
            break;
        }
    }
    let pipeline = pipeline_label(cache, chunk.pipeline);
    match (pipeline.is_empty(), kinds.is_empty()) {
        (true, true) => String::new(),
        (false, true) => pipeline,
        (true, false) => kinds.join(", "),
        (false, false) => format!("{pipeline}; {}", kinds.join(", ")),
    }
}

/// The transparent pass, chunk by chunk, with a breadcrumb marker between the chunks: this
/// module's reason to exist (module notes). It replaces Bevy's `main_transparent_pass_3d`,
/// which [`drop_stock_pass`] takes out of the render graph while the switch is on.
///
/// Order-independent transparency is not supported: a view that asks for it draws no
/// transparent item at all while the switch is on, and says so once. NVForever asks for it
/// nowhere, and the crash test is not worth a second path through the OIT buffers.
fn transparent_pass_3d_chunks(
    world: &World,
    view: ViewQuery<(&ExtractedCamera, &ExtractedView, &ViewTarget, &ViewDepthStencilTexture, Option<&MainPassResolutionOverride>, Has<OrderIndependentTransparencySettings>)>,
    phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
    // Said once a run: the OIT refusal, or the chunk table of the first frame drawn.
    mut said: Local<bool>,
) {
    let view_entity = view.entity();
    let (camera, extracted, target, depth, resolution, has_oit) = view.into_inner();
    let Some(phase) = phases.get(&extracted.retained_view_entity) else { return };
    if phase.items.is_empty() {
        return;
    }
    if has_oit {
        if !std::mem::replace(&mut *said, true) {
            error!("transparent chunks: this view asks for order-independent transparency, which the chunked pass does not draw: nothing transparent is drawn in it while {} is set", env());
        }
        return;
    }
    let items: Vec<Item> = phase
        .items
        .values()
        .map(|item| Item { draw_function: item.draw_function.0, pipeline: item.pipeline.id(), batch: item.batch_range.len() as usize })
        .collect();
    let entities: Vec<Entity> = phase.items.keys().map(|(entity, _)| *entity).collect();
    let mut chunks = plan(&items, MAX_CHUNKS);
    for chunk in &mut chunks {
        chunk.label = label(world, &cache, chunk, &entities);
    }
    for chunk in &chunks {
        let name = node_name(chunk.index);
        ctx.gpu_breadcrumb(name, false);
        {
            let mut pass = ctx.begin_tracked_render_pass(wgpu::RenderPassDescriptor {
                label: Some("nvf_transparent_chunk"),
                color_attachments: &[Some(target.get_color_attachment())],
                // As Bevy's own pass: the depth buffer is loaded, and stored because wgpu
                // would otherwise clear it (bevyengine/bevy#3776).
                depth_stencil_attachment: Some(depth.get_attachment(wgpu::StoreOp::Store)),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(viewport) = Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution) {
                pass.set_camera_viewport(&viewport);
            }
            if let Err(error) = phase.render_range(&mut pass, world, view_entity, chunk.start..chunk.end) {
                error!("transparent chunk {}: {error:?}", chunk.index);
            }
        }
        ctx.gpu_breadcrumb(name, true);
    }
    // Once a run: what the cut came to on a frame with a world in it, so a run says what the
    // chunks are without a lost device to read them.
    if !std::mem::replace(&mut *said, true) {
        info!("transparent chunks: {} chunk(s) over {} item(s) this frame: {}", chunks.len(), items.len(), chunks.iter().map(describe).collect::<Vec<_>>().join("; "));
    }
    record(gpu_breadcrumbs::frame(), chunks);
}

/// Take Bevy's own transparent pass out of the render graph, so the phase is drawn once, by
/// [`transparent_pass_3d_chunks`]. Its dependencies (the atmosphere's pass orders itself
/// before it) are kept as edges through it, which is what `RemoveSystemsOnly` does.
/// The schedule is taken out of [`Schedules`] while it is edited, rather than borrowed
/// through `World::resource_scope`: removing a system initializes the schedule, which puts
/// `Schedules` back into the world, and a resource inserted inside a `resource_scope` panics
/// [observed 2026-11-23, a desktop run with the switch on; `bevy_ecs-0.20.1/src/world/
/// mod.rs:2947`].
fn drop_stock_pass(world: &mut World) {
    let taken = world.resource_mut::<Schedules>().remove_temporarily(Core3d);
    let removed = match taken {
        None => Err("the Core3d schedule is not there".to_string()),
        Some(mut schedule) => {
            let removed = schedule
                .remove_systems_in_set(main_transparent_pass_3d, world, ScheduleCleanupPolicy::RemoveSystemsOnly)
                .map_err(|e| format!("{e:?}"));
            world.resource_mut::<Schedules>().reinsert(schedule);
            removed
        }
    };
    match removed {
        Ok(count) => warn!(
            "gpu breadcrumbs: the transparent pass is cut into chunks ({}, crash test): Bevy's own pass is out of the graph ({count} system(s)) and the phase is drawn in up to {MAX_CHUNKS} render passes a frame, a marker between each. Much slower, and only the markers' order differs from an ordinary frame.",
            env()
        ),
        Err(why) => error!("gpu breadcrumbs: the transparent pass could not be cut into chunks ({why}); the phase may now be drawn twice a frame"),
    }
}

/// The chunks, off unless both switches are on ([`on`]).
pub struct TransparentChunksPlugin;

impl Plugin for TransparentChunksPlugin {
    fn build(&self, app: &mut App) {
        if !on() {
            return;
        }
        app.add_plugins(ExtractComponentPlugin::<TransparentKind>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .add_systems(RenderStartup, drop_stock_pass)
            // Where Bevy's own pass sat: after the opaque and transmissive passes, and
            // after the one-resolve window closes (vendored bevy_pbr `end_deferred_resolve`),
            // so each chunk's pass resolves its multisampled target as that pass did.
            .add_systems(
                Core3d,
                transparent_pass_3d_chunks.in_set(Core3dSystems::MainPass).after(main_opaque_pass_3d).after(end_deferred_resolve),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An item drawn on its own (a batch of one).
    fn item(draw_function: u32, pipeline: usize) -> Item {
        Item { draw_function, pipeline, batch: 1 }
    }

    /// A chunk, as the plan leaves it (no label).
    fn chunk(index: usize, start: usize, end: usize, draws: usize, pipeline: usize, draw_function: u32, mixed: bool) -> Chunk {
        Chunk { index, start, end, draws, pipeline, draw_function, mixed, label: String::new() }
    }

    /// Consecutive items of one pipeline are one chunk; a change of pipeline cuts.
    #[test]
    fn a_transparent_chunk_runs_until_the_pipeline_changes() {
        let items = [item(3, 7), item(3, 7), item(3, 9), item(4, 9)];
        assert_eq!(
            plan(&items, MAX_CHUNKS),
            vec![chunk(0, 0, 2, 2, 7, 3, false), chunk(1, 2, 3, 1, 9, 3, false), chunk(2, 3, 4, 1, 9, 4, false)]
        );
    }

    /// A batch covers several items: the chunk steps over all of them and counts one draw, so
    /// no cut ever falls inside a batch.
    #[test]
    fn a_transparent_chunk_steps_over_a_whole_batch() {
        let items = [Item { draw_function: 1, pipeline: 2, batch: 3 }, item(1, 2), item(1, 2), item(1, 5)];
        let chunks = plan(&items, MAX_CHUNKS);
        assert_eq!(chunks, vec![chunk(0, 0, 3, 1, 2, 1, false), chunk(1, 3, 4, 1, 5, 1, false)]);
        // Every item is covered once, in order.
        assert_eq!(chunks.first().map(|c| c.start), Some(0));
        assert_eq!(chunks.last().map(|c| c.end), Some(items.len()));
    }

    /// An item whose batch is empty draws nothing but still belongs to a chunk.
    #[test]
    fn a_transparent_chunk_counts_no_draw_for_an_empty_batch() {
        let items = [Item { draw_function: 1, pipeline: 2, batch: 0 }, item(1, 2)];
        assert_eq!(plan(&items, MAX_CHUNKS), vec![chunk(0, 0, 2, 1, 2, 1, false)]);
    }

    /// More runs than the cap: adjacent runs merge into equal groups, the chunks stay in
    /// phase order, and they still cover every item exactly once.
    #[test]
    fn transparent_chunks_merge_down_to_the_cap() {
        let items: Vec<Item> = (0..10).map(|p| item(1, p)).collect();
        let chunks = plan(&items, 3);
        assert_eq!(chunks.len(), 3, "{chunks:?}");
        assert_eq!(chunks.iter().map(|c| (c.start, c.end, c.draws, c.mixed)).collect::<Vec<_>>(), vec![(0, 4, 4, true), (4, 8, 4, true), (8, 10, 2, true)]);
        assert_eq!(chunks.iter().map(|c| c.index).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    /// The marker names are a small fixed set, and a name reads back as its index.
    #[test]
    fn a_transparent_chunk_marker_name_reads_back_as_its_index() {
        assert_eq!(node_name(0), "nvf transparent chunk 00");
        assert_eq!(node_name(31), "nvf transparent chunk 31");
        // Past the set, the last name rather than a new one.
        assert_eq!(node_name(99), node_name(MAX_CHUNKS - 1));
        assert_eq!(node_index(node_name(7)), Some(7));
        assert_eq!(node_index("bevy_core_pipeline::core_3d::main_transparent_pass_3d"), None);
    }

    /// A marker, as the vendored recorder would have left it.
    fn mark(seq: u32, frame: u64, node: &str, end: bool, ran: bool) -> Mark {
        Mark { seq, frame, node: node.to_string(), end, ran }
    }

    /// A chunk with a label, as a frame's table holds it.
    fn kept(index: usize, start: usize, end: usize, draws: usize, pipeline: usize, label: &str) -> Chunk {
        Chunk { index, start, end, draws, pipeline, draw_function: 1, mixed: false, label: label.to_string() }
    }

    /// The hang: the chunk the GPU began and never ended is named, with what was in it.
    #[test]
    fn the_transparent_chunk_report_names_the_chunk_that_hung() {
        let frames = vec![(
            14,
            vec![
                kept(0, 0, 4, 4, 190, "transparent_mesh_pipeline nif_material.wesl; terrain layer"),
                kept(1, 4, 9, 5, 212, "transparent_mesh_pipeline nif_material.wesl; particles meshes\\effects\\ambient\\fxsprayjet01.nif"),
            ],
        )];
        let marks = vec![
            mark(10, 14, node_name(0), false, true),
            mark(11, 14, node_name(0), true, true),
            mark(12, 14, node_name(1), false, true),
            mark(13, 14, node_name(1), true, false),
        ];
        let text = text(&marks, &frames);
        assert!(text.contains("1 frame(s) of chunk tables kept"), "{text}");
        assert!(text.contains("began and never ended: frame 14 chunk 01 items 4..9, 5 draw(s), pipeline 212, draw fn 1 [transparent_mesh_pipeline nif_material.wesl; particles meshes\\effects\\ambient\\fxsprayjet01.nif]"), "{text}");
        // The frame's whole table follows, chunk 00 included.
        assert!(text.contains("frame 14 chunk 00 items 0..4, 4 draw(s), pipeline 190"), "{text}");
        assert!(!text.contains("no transparent chunk began and never ended"), "{text}");
    }

    /// A frame the GPU finished leaves nothing open, and only the newest table is printed.
    #[test]
    fn the_transparent_chunk_report_says_so_when_nothing_is_open() {
        let frames = vec![(13, vec![kept(0, 0, 2, 2, 190, "a")]), (14, vec![kept(0, 0, 3, 3, 191, "b")])];
        let marks = vec![mark(1, 14, node_name(0), false, true), mark(2, 14, node_name(0), true, true)];
        let text = text(&marks, &frames);
        assert!(text.contains("no transparent chunk began and never ended"), "{text}");
        assert!(text.contains("frame 14 chunk 00 items 0..3, 3 draw(s), pipeline 191, draw fn 1 [b]"), "{text}");
        assert!(!text.contains("frame 13"), "{text}");
    }

    /// Markers the chunk tables no longer cover, and markers of other nodes, are both read
    /// without losing the one answer the section is for.
    #[test]
    fn the_transparent_chunk_report_holds_up_without_a_table() {
        let marks = vec![
            mark(1, 8, "bevy_core_pipeline::core_3d::main_opaque_pass_3d", false, true),
            mark(2, 8, "bevy_core_pipeline::core_3d::main_opaque_pass_3d", true, true),
            mark(3, 8, node_name(2), false, true),
        ];
        let text = text(&marks, &[]);
        assert!(text.contains("0 frame(s) of chunk tables kept"), "{text}");
        assert!(text.contains("began and never ended: frame 8 chunk 02, whose table was not kept"), "{text}");
    }
}
