//! The live wrist map's close picture: the Local Map's own outline of the place round the
//! player, made for the wrist's window [design: the clean-room notes' "green local-map
//! look", option 2; spikes/001-live-wrist-world/README.md].
//!
//! The paused Pip-Boy's Local Map page draws a plan of the collision the physics world
//! holds (`pipboy::data::outline`): a grid of rays cast straight down, a few hundred a
//! frame until the grid is done, then one picture of it — the ground shaded by its slope
//! and the walls and cliffs drawn bright. This module asks that same producer for a grid
//! of the player's own surroundings and hands the finished texels to the glance, so the
//! wrist shows the place as the Local Map draws it instead of the worldspace's painted
//! picture. Nothing new is rendered: no second camera, no readback, and the rays are the
//! ones the Local Map page already casts.
//!
//! What it costs [design: the Odin's budget]:
//!
//! - The rays are cast only while the glance holds the arm and the player asked for the
//!   close picture ([`crate::wrist_arm::WristLive::wants_local`]). A closed wrist costs
//!   nothing at all, and a half-made grid is kept, so raising the arm again carries on
//!   where it left off.
//! - At most [`BUDGET`] of a frame, [`BATCH`] rays at a time: a fraction of a frame at
//!   60 Hz (the paused page may take 6 ms, as nothing else is running there).
//! - One grid is made for a lattice of anchors ([`crate::pipboy::data::outline::Outline`]'s
//!   own key), so walking about keeps it and only leaving its coverage makes another; the
//!   one that is finished stays on the screen while the next is cast.
//! - The picture is laid out once, when the grid is done, and shared with the screen's
//!   owner behind an `Arc` ([`crate::wrist_live::LocalPic`]): no texels are copied per
//!   frame and nothing of the size of a picture is allocated per frame.

use crate::pipboy::data::outline::{self, Outline, OutlineKey};
use crate::wrist_live::{LocalPic, MapPicture};
use bevy::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Which of the glance's two caches a total is for ([`note_cache`]).
#[derive(Clone, Copy, Debug)]
pub enum Cache {
    /// The outline grid being cast and the picture it finished ([`WristMap`]).
    Outline,
    /// The pictures the arm's screen wears (nvforever `wrist_screen`): the worldspace's
    /// own map texels, the canvas it lays out, and the Pip-Boy font's letters.
    Screen,
}

/// What each cache holds now, bytes.
static CACHE_BYTES: [AtomicU64; 2] = [AtomicU64::new(0), AtomicU64::new(0)];

/// Say what a cache holds now. Atomics and not a resource: the run log's memory line is
/// written in `Last` by nvforever's GPU recorder, which sees neither owner's state, and
/// the cost is one relaxed store when a cache changes [design: the owner's out-of-memory
/// abort of 2026-11-22 — a leak has to show as a rising number somewhere].
pub fn note_cache(which: Cache, bytes: u64) {
    CACHE_BYTES[which as usize].store(bytes, Ordering::Relaxed);
}

/// What the glance's caches hold in all, bytes: 0 while it has never been raised.
pub fn cache_bytes() -> u64 {
    CACHE_BYTES.iter().map(|b| b.load(Ordering::Relaxed)).sum()
}

/// How long the wrist's grid may cast rays in a frame, and how many at a time [design:
/// about a tenth of a frame at 60 Hz, so a whole grid is made in about a second of
/// glancing and no frame is lost to it].
pub const BUDGET: Duration = Duration::from_micros(1500);
pub const BATCH: usize = 64;

/// The window the grid is asked for, in the canvas units and the scale the producer
/// measures them in. Outdoors anything smaller clamps to the 5×5 cells round the player
/// (`outline`'s own span), which is all the physics world and the landscape can answer
/// for. Indoors there is no such floor, and the rays are cast four times as close
/// (`outline::INTERIOR_STEP`), so the wrist asks for the room it shows and no more:
/// a bigger ask would cast tens of thousands of rays for a picture of empty space.
fn ask(interior: bool) -> (Vec2, f32) {
    (Vec2::splat(shown_span(interior)), 1.0)
}

/// What the screen shows of the grid, across, in game units [design: about 175 m
/// outdoors, a few hundred feet of road either way, close enough that a building reads as
/// a building at the glass's size and far enough that the way ahead is on it; indoors the
/// rooms about the player, which is as much as an interior holds].
pub const LOCAL_SPAN: f32 = 12288.0;
pub const LOCAL_SPAN_INDOORS: f32 = 2048.0;

/// The one of the two this place shows.
pub fn shown_span(interior: bool) -> f32 {
    if interior {
        LOCAL_SPAN_INDOORS
    } else {
        LOCAL_SPAN
    }
}

/// The feet's height the grid is anchored to, in steps of this many game units: the
/// producer makes a new grid for every step of its own (another floor, indoors), and
/// outdoors its rays start thousands of units over the player, so walking up a hill
/// would otherwise throw a finished grid away for nothing [design].
const FEET_STEP: f32 = 2048.0;

/// NVForever's own name for the picture, for the log line and the screen's owner (it is no
/// file of the load order's).
pub const NAME: &str = "the local map's outline";

/// The grid being cast, the surfaces its rays fall back on, and the last picture made.
#[derive(Resource, Default)]
pub struct WristMap {
    grid: Option<Outline>,
    land: outline::LandCache,
    /// The last grid that finished, as the glance shows it.
    pub ready: Option<Ready>,
    said: Option<OutlineKey>,
}

impl WristMap {
    /// What this cache holds, bytes: the grid's own samples (where each ray landed) and
    /// the texels of the picture that finished. The landscape under the rays is the Local
    /// Map producer's own cache and is left out: it is cleared the moment a grid finishes
    /// ([`build`]), so it cannot be what grows.
    fn bytes(&self) -> u64 {
        let grid = self.grid.as_ref().map_or(0, |g| g.heights.len() * std::mem::size_of::<Option<f32>>());
        let ready = self.ready.as_ref().map_or(0, |r| r.pic.alpha.len());
        (grid + ready) as u64
    }
}

/// A finished outline: where it lies in the world, and its texels.
#[derive(Debug, Clone)]
pub struct Ready {
    /// What the grid was made for (the place and its anchor).
    key: OutlineKey,
    /// Its top left corner (game x, y the largest) and the game units a sample.
    origin: [f32; 2],
    step: f32,
    size: [usize; 2],
    pic: LocalPic,
}

impl Ready {
    /// The game units the picture spans across and down.
    fn span(&self) -> [f32; 2] {
        [self.size[0] as f32 * self.step, self.size[1] as f32 * self.step]
    }

    /// Whether this picture is of the place `(interior, world)` and still covers the whole
    /// window the screen shows round `feet`: a picture that does not is not shown at all,
    /// and the glance falls back to the worldspace's own.
    pub fn covers(&self, interior: Option<u32>, world: u32, feet: [f32; 2]) -> bool {
        if (self.key.0, self.key.1) != (interior, world) || !self.pic.usable() {
            return false;
        }
        let half = self.half_window();
        let uv = self.map_uv(feet[0], feet[1]);
        uv.iter().all(|u| u.is_finite() && *u >= half && *u <= 1.0 - half)
    }
}

impl MapPicture for Ready {
    fn map_texture(&self) -> &str {
        NAME
    }

    /// The grid's own frame: its corner and its span, north up and y southward as a
    /// picture's rows run.
    fn map_uv(&self, x: f32, y: f32) -> [f32; 2] {
        let [w, h] = self.span();
        [(x - self.origin[0]) / w.max(1.0), (self.origin[1] - y) / h.max(1.0)]
    }

    fn half_window(&self) -> f32 {
        (shown_span(self.key.0.is_some()) / 2.0 / self.span()[0].max(1.0)).clamp(0.01, 0.5)
    }

    fn local(&self) -> Option<&LocalPic> {
        Some(&self.pic)
    }
}

pub struct WristLocalPlugin;

impl Plugin for WristLocalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WristMap>()
            // After the glance's own decision, so the rays are cast only for a frame that
            // asked for them; the picture it makes is shown from the frame after, as the
            // body's own facts are (`wrist_arm`).
            .add_systems(Update, build.in_set(crate::PlaySet::Input).after(crate::wrist_arm::decide));
    }
}

/// Cast a few of the wrist's rays, and lay the picture out when the grid is done.
pub(crate) fn build(
    wrist: Option<Res<crate::wrist_arm::WristLive>>,
    mut map: ResMut<WristMap>,
    (location, world, player): (Option<Res<crate::Location>>, Option<Res<crate::PlayWorld>>, Option<Res<crate::player::Player>>),
    phys: Option<ResMut<crate::phys::PhysState>>,
) {
    // Nothing while the glance is down or the player asked for the worldspace's picture:
    // what is made already is kept, so raising again carries on.
    if !wrist.as_deref().is_some_and(|w| w.wants_local()) {
        return;
    }
    let (Some(loc), Some(player), Some(data)) = (location.as_deref(), player.as_deref(), world.as_deref()) else { return };
    // Indoors the producer draws a plan of the rooms, keyed by the cell: the glance opens
    // there too [design: the owner, 2026-11-21].
    let feet = [player.feet[0], player.feet[1]];
    if !feet.iter().all(|v| v.is_finite()) || !player.feet[2].is_finite() {
        return;
    }
    // Outdoors the rays start thousands of units over the player, so the height the grid
    // is anchored to is coarse and walking up a hill does not throw a finished grid away.
    // Indoors they start a little over their head (`outline::INTERIOR_START`): the anchor
    // is the floor they are on, or the rays would begin under it and land on nothing. The
    // producer's own key still snaps it to a floor at a time (`outline::FEET_SNAP`).
    let anchor = match loc.interior {
        Some(_) => player.feet[2],
        None => (player.feet[2] / FEET_STEP).round() * FEET_STEP,
    };
    let ask = ask(loc.interior.is_some());
    let key = Outline::key_for(loc.interior, loc.world, feet, anchor, ask.0, ask.1);
    let m = &mut *map;
    if m.grid.as_ref().is_none_or(|g| g.key != key) {
        m.grid = Some(Outline::new(loc.interior, loc.world, feet, anchor, ask.0, ask.1));
        m.land.clear();
        note_cache(Cache::Outline, m.bytes());
    }
    let Some(grid) = m.grid.as_mut().filter(|g| !g.done()) else { return };
    let Some(mut phys) = phys else { return };
    // The landscape under the rays is the worldspace's; an interior has none.
    let land_of = loc.interior.is_none().then_some((data.0, loc.world));
    outline::cast(grid, &mut phys, &mut m.land, land_of, BUDGET, BATCH);
    if !grid.done() {
        return;
    }
    if grid.hits() == 0 {
        // Nothing at all under the rays: the place's collision was not loaded yet when
        // the grid was begun (an interior the player has only just walked into). Throw it
        // away and begin again rather than keep a picture of nowhere and never look
        // again — the next grid costs no more of a frame than this one did, and only
        // while the glance is up [design].
        m.grid = None;
        return;
    }
    let size = [grid.size[0] as u32, grid.size[1] as u32];
    let pic = LocalPic { alpha: std::sync::Arc::new(grid.alpha()), size };
    let (origin, step, hits) = (grid.origin, grid.step, grid.hits());
    m.ready = Some(Ready { key, origin, step, size: grid.size, pic });
    // The surfaces were for this grid; the next one loads the cells it needs itself.
    m.land.clear();
    note_cache(Cache::Outline, m.bytes());
    if m.said != Some(key) {
        m.said = Some(key);
        info!("wrist: the local map's outline is made for the glance: {}x{} samples {step} apart, {hits} landed", size[0], size[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(origin: [f32; 2], size: [usize; 2], step: f32) -> Ready {
        let pic = LocalPic { alpha: std::sync::Arc::new(vec![40; size[0] * size[1]]), size: [size[0] as u32, size[1] as u32] };
        Ready { key: (None, 7, [0, 0, 0]), origin, step, size, pic }
    }

    /// The picture's frame: the grid's corner is its 0,0 and north is up it, as the Local
    /// Map page lays the very same grid out.
    #[test]
    fn the_grids_corner_is_the_pictures_corner_and_north_is_up() {
        let r = ready([-1000.0, 1000.0], [100, 100], 20.0);
        assert_eq!(r.span(), [2000.0, 2000.0]);
        assert_eq!(r.map_uv(-1000.0, 1000.0), [0.0, 0.0]);
        assert_eq!(r.map_uv(0.0, 0.0), [0.5, 0.5]);
        assert_eq!(r.map_uv(1000.0, -1000.0), [1.0, 1.0]);
        // North (y up in the world) is up the picture (v down), east to its right.
        let north = r.map_uv(0.0, 500.0);
        let east = r.map_uv(500.0, 0.0);
        assert!(north[1] < 0.5 && (north[0] - 0.5).abs() < 1e-6);
        assert!(east[0] > 0.5 && (east[1] - 0.5).abs() < 1e-6);
    }

    /// The window is the close one the wrist shows, and a picture is used only while it
    /// covers the whole of that window: nothing is shown smeared from an edge.
    #[test]
    fn a_picture_is_used_only_while_it_covers_the_window() {
        // The grid the producer makes outdoors: the 5×5 cells round the player.
        let grid = Outline::new(None, 7, [0.0, 0.0], 0.0, ask(false).0, ask(false).1);
        assert!(grid.size[0] as f32 * grid.step >= 5.0 * 4096.0, "{:?} by {}", grid.size, grid.step);
        assert_eq!(grid.size[0], grid.size[1], "a square grid, so one window fits both ways");
        let r = ready(grid.origin, grid.size, grid.step);
        let half = r.half_window();
        assert!((half * 2.0 * r.span()[0] - LOCAL_SPAN).abs() < 1.0, "the window is {} units across", half * 2.0 * r.span()[0]);
        // The middle of the grid is covered; a corner of it is not, nor is another place.
        assert!(r.covers(None, 7, [0.0, 0.0]));
        assert!(r.covers(None, 7, [2944.0, -2944.0]), "the furthest the anchor lattice puts the player from the middle");
        let far = r.span()[0] / 2.0;
        assert!(!r.covers(None, 7, [far, 0.0]), "the window would run off the picture");
        assert!(!r.covers(None, 8, [0.0, 0.0]) && !r.covers(Some(3), 7, [0.0, 0.0]), "another place");
        assert!(!r.covers(None, 7, [f32::NAN, 0.0]));
        // No samples behind it: not a picture.
        let empty = Ready { pic: LocalPic { alpha: std::sync::Arc::new(Vec::new()), size: [8, 8] }, ..r };
        assert!(!empty.covers(None, 7, [0.0, 0.0]));
    }

    /// Indoors the grid is the room about the player, cast at the producer's close step:
    /// small enough that the glance makes it in a second or two, where a grid of the
    /// outdoor span would be tens of thousands of rays for a picture of empty space.
    #[test]
    fn an_interiors_grid_is_the_room_about_the_player_and_not_the_whole_outdoors() {
        let inside = Outline::new(Some(3), 7, [0.0, 0.0], 0.0, ask(true).0, ask(true).1);
        let outside = Outline::new(None, 7, [0.0, 0.0], 0.0, ask(false).0, ask(false).1);
        let rays = |o: &Outline| o.size[0] * o.size[1];
        assert!(rays(&inside) > 1, "a grid of one sample is no picture: {:?}", inside.size);
        assert!(rays(&inside) < rays(&outside), "{:?} indoors against {:?} out", inside.size, outside.size);
        // Close enough that a doorway reads: the producer's own interior step.
        assert_eq!(inside.step, outline::INTERIOR_STEP);
        assert!(inside.size[0] as f32 * inside.step >= LOCAL_SPAN_INDOORS, "{:?} covers less than the window", inside.size);
        // And the window it shows is the whole of what it covers at the most.
        let r = Ready { key: inside.key, origin: inside.origin, step: inside.step, size: inside.size, pic: LocalPic { alpha: std::sync::Arc::new(vec![0; rays(&inside)]), size: [inside.size[0] as u32, inside.size[1] as u32] } };
        assert!(r.half_window() > 0.01 && r.half_window() <= 0.5);
        assert!((r.half_window() * 2.0 * r.span()[0] - LOCAL_SPAN_INDOORS).abs() < inside.step);
    }

    /// The anchor the grid is made for: walking about keeps it, walking off it makes
    /// another, and an ordinary hill does not.
    #[test]
    fn walking_keeps_the_grid_and_a_hill_does_not_throw_it_away() {
        let key = |x: f32, y: f32, z: f32| Outline::key_for(None, 7, [x, y], (z / FEET_STEP).round() * FEET_STEP, ask(false).0, ask(false).1);
        let here = key(0.0, 0.0, 0.0);
        assert_eq!(here, key(2000.0, -2000.0, 0.0), "a few hundred metres of walking keeps it");
        // The producer's own height step would make a new grid for every 128 units; the
        // wrist's anchor is coarse enough that a hill or a staircase does not.
        assert_eq!(here, key(0.0, 0.0, 900.0), "an ordinary climb keeps it");
        assert_ne!(here, key(0.0, 0.0, 4000.0), "another height altogether");
        assert_ne!(here, key(30000.0, 0.0, 0.0), "elsewhere");
    }
}
