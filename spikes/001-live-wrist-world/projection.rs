//! The engine's live wrist module driven through the engine's own World Map projection.
//!
//! Neither side is copied: `projection` is the `CELL`/`WorldMap`/`MapTransform` section of
//! `engine/nvf-play/src/pipboy/data/model.rs` as checked in, `wrist_live` is
//! `engine/nvf-play/src/wrist_live.rs` as checked in, and the glue below is the same
//! implementation `model.rs` carries. This is what pins the orientation the module's own
//! tests assume (east to the right, north up the picture, the arrow clockwise from north)
//! to the projection the paused World Map page uses. Run it with
//! `python3 spikes/001-live-wrist-world/run.py`.

#[allow(dead_code)]
mod projection {
    include!(env!("WRIST_WORLD_MAP_RS"));
}

// The checked-in module itself, by path: `include!` cannot carry its `//!` header.
// Its own tests run here too; what only the crate's callers use is dead code in this stage.
#[allow(dead_code)]
#[path = "../../engine/nvf-play/src/wrist_live.rs"]
mod wrist_live;

use projection::{CELL, MAP_SPAN, MapTransform, WorldMap};
use std::f32::consts::FRAC_PI_2;
use std::time::Duration;
use wrist_live::{HALF_WINDOW, Key, Live, Play, Spot};

/// The arm's screen shape's own texture coordinates (nvforever `pipboy_arm::UV_EXTENT`),
/// repeated here because this stage compiles no Bevy crate.
const UV_EXTENT: f32 = 0.8;

// The glue `model.rs` carries, kept here so the stage compiles it from the same
// signatures the crate does.
impl wrist_live::MapPicture for WorldMap {
    fn map_texture(&self) -> &str {
        &self.texture
    }
    fn map_uv(&self, x: f32, y: f32) -> [f32; 2] {
        WorldMap::uv(self, x, y)
    }
}

const KEY: Key = Key { session: 1, world: 42, screen: 7 };

fn t(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

fn play() -> Play {
    Play { session: 1, world: Some(42), screen: Some(7), playing: true, first_person: true, ..Play::default() }
}

/// An authored three-by-three-cell worldspace, its own projection untouched. No game data:
/// the bounds and the picture's name are made up here.
fn map() -> WorldMap {
    WorldMap {
        root: 42,
        transform: MapTransform::default(),
        texture: "synthetic-owned-map.dds".into(),
        usable: [1024.0; 2],
        nw: [-1, 1],
        se: [1, -1],
    }
}

fn spot(map: &WorldMap, x: f32, y: f32, heading: f32) -> Option<Spot<'_>> {
    Some(Spot { map, feet: [x, y], heading })
}

#[test]
fn the_real_projection_puts_the_player_where_the_module_expects() {
    let mut live = Live::default();
    let map = map();
    let middle = live.step(t(0), true, true, &play(), || spot(&map, CELL * 0.5, CELL * 0.5, 0.0)).draw.unwrap();
    assert_eq!((middle.key, middle.texture, middle.uv), (KEY, map.texture.as_str(), [0.5, 0.5]));
    assert_eq!(middle.forward, [0.0, -1.0], "north points up a picture whose y runs down");
    // One cell east and north, and a turn to the east.
    let moved = live.step(t(100), true, false, &play(), || spot(&map, CELL * 1.5, CELL * 1.5, FRAC_PI_2)).draw.unwrap();
    assert!((moved.uv[0] - (0.5 + MAP_SPAN / 3.0)).abs() < 1e-6, "east is {:?}", moved.uv);
    assert!((moved.uv[1] - (0.5 - MAP_SPAN / 3.0)).abs() < 1e-6, "north is {:?}", moved.uv);
    assert!((moved.forward[0] - 1.0).abs() < 1e-6 && moved.forward[1].abs() < 1e-6, "east is {:?}", moved.forward);
    println!("engine projection, 100 ms apart: {:?} -> {:?} facing {:?}", middle.uv, moved.uv, moved.forward);
}

#[test]
fn the_screens_window_shows_the_player_in_the_middle_of_the_real_projection() {
    let mut live = Live::default();
    let map = map();
    // A step east of the middle of the worldspace, facing north.
    let draw = live.step(t(0), true, true, &play(), || spot(&map, CELL * 0.75, CELL * 0.5, 0.0)).draw.unwrap();
    let w = wrist_live::window(draw.uv, draw.forward, HALF_WINDOW, UV_EXTENT);
    let middle = w.at([UV_EXTENT / 2.0; 2]);
    assert!((middle[0] - draw.uv[0]).abs() < 1e-6 && (middle[1] - draw.uv[1]).abs() < 1e-6, "the middle of the screen is {middle:?}, the player at {:?}", draw.uv);
    // North up: the top of the screen is north of the player on the real picture, which
    // `WorldMap::uv` puts at a smaller v.
    let top = w.at([UV_EXTENT / 2.0, 0.0]);
    assert!(top[1] < middle[1], "north is {top:?} above {middle:?}");
    let north = map.uv(CELL * 0.75, CELL * 0.5 + CELL * 0.25);
    assert!(north[1] < middle[1], "a step north on the real projection is {north:?}");
    println!("screen window at {:?} facing north: middle {middle:?}, top {top:?}", draw.uv);
}

#[test]
fn a_position_off_the_picture_draws_nothing() {
    let mut live = Live::default();
    let map = map();
    assert!(live.step(t(0), true, true, &play(), || spot(&map, 0.0, 0.0, 0.0)).draw.is_some());
    // Ten cells east of a three-cell worldspace: outside the picture, so the live view
    // clears instead of clamping the arrow to the edge.
    let off = live.step(t(100), true, false, &play(), || spot(&map, CELL * 10.0, 0.0, 0.0));
    assert_eq!((off.clear, off.draw), (Some(KEY), None));
    assert!(!live.owns(KEY));
}

#[test]
fn an_inherited_picture_keeps_the_root_texture_and_the_child_transform() {
    let mut live = Live::default();
    let mut map = map();
    // A worldspace that uses its parent's map with its own ONAM scale and offset.
    map.transform = MapTransform { scale: 2.0, offset: [CELL * 0.5, 0.0], centre: [0.0; 2] };
    let child = Key { world: 101, ..KEY };
    let shown = live
        .step(t(0), true, true, &Play { world: Some(101), ..play() }, || spot(&map, 0.0, CELL * 0.25, 0.0))
        .draw
        .unwrap();
    assert_eq!((shown.key, shown.uv, shown.texture), (child, [0.5; 2], map.texture.as_str()));
}
