//! The live World Map on the arm's own screen shape (opt-in; the rules are nvf-play
//! `wrist_live`, the wiring nvf-play `wrist_arm`) [design:
//! spikes/001-live-wrist-world/README.md].
//!
//! The paused Pip-Boy draws its menus into a render target and `pipboy_arm::show_screen`
//! wears that picture on `pipboyscreen:0` through a copy of the shape's own material
//! (`pipboy_arm::menus_material`). This file does the same thing with a different
//! picture and no menus at all: a copy of the shape's own material whose base texture is
//! the one picture this file lays out ([`CANVAS`] texels square, written in place at the
//! live view's own rate), the player's place in the middle of it and the way they face up
//! it. No render target, no second camera, no readback, and no shared NIF or texture
//! asset is touched.
//!
//! What goes under the player's mark is one of two bases, whichever nvf-play's sample
//! names ([`nvf_play::wrist_arm::Shown`]):
//!
//! - the Local Map's own outline of the place round the player, laid out on the processor
//!   by `nvf_play::wrist_local` (one byte a sample) and taken as it is, as the paused
//!   Local Map page draws the very same picture; or
//! - the map picture the load order owns for the worldspace, read once and decoded here,
//!   at the paused World Map page's own strength ([`WORLD_GAIN`]).
//!
//! Either way the window into it is the one nvf-play's [`window`] computes from the
//! sample, and the picture is laid out again only when the window, the player's facing or
//! the base itself changed.
//!
//! Ownership, each frame:
//!
//! - [`wear_map`] runs **before** `show_screen`, so in the frame the menus come up the
//!   live view has already given the shape its own material back and `show_screen` copies
//!   the real one, never ours. It installs nothing unless nvf-play says the live view owns
//!   the wrist, and it restores only while the shape still wears the copy it installed
//!   (under a Stealth Boy the field's copy wins and the live view lets go of it).
//! - [`hide_lit`] runs **after** `show_screen`, which sets `ScreenLit`'s visibility every
//!   frame: the lit copy of the screen is hidden only while the live view owns the screen,
//!   and `show_screen` shows it again by itself on the frame the live view lets go.
//! - [`feed`] writes back what only the body knows (whether the first-person arms are
//!   drawn, and which shape is the screen) for nvf-play's decision on the next frame.
//!
//! The arm pose is not here: `player_body::drive_body` takes it from
//! `WristLive::arm_clip_time`, so the live view plays the same first-person hold the menus
//! do, on the same rig, without the menu's own state.

use crate::pipboy_arm::{PipBoyPart, UV_EXTENT};
use crate::player_body::PlayerBody;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::math::Affine2;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::prelude::*;
use nvf_assets::{GameData, NifMaterial};
use nvf_play::pipboy::device_screen::SCREEN_TINT;
use nvf_play::pipboy::transition::ScreenLook;
use nvf_play::wrist_arm::{WristBody, WristLive};
use nvf_play::wrist_live::{window, LocalPic, Mark, MarkKind, Window};
use nvf_play::wrist_page::{Page, Row};

pub struct WristScreenPlugin;

impl Plugin for WristScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                wear_map.after(nvf_play::player::BodySet).before(crate::pipboy_arm::show_screen),
                feed.after(nvf_play::player::BodySet).before(crate::pipboy_arm::show_screen),
                hide_lit.after(crate::pipboy_arm::show_screen).before(crate::stealth_field::apply),
            ),
        );
    }
}

/// What the live view has put on the arm, so it can take exactly that back.
#[derive(Default)]
struct Worn {
    /// The screen shape it dressed, and the material that shape wore before.
    screen: Option<(Entity, Handle<NifMaterial>)>,
    /// Its own copy of the material, and the material it was made from.
    copy: Option<(Handle<NifMaterial>, Handle<NifMaterial>)>,
    /// The map the worldspace owns: the file it was read from, its size, and its texels
    /// on the processor. Not read at all while the base is the local outline.
    source: Option<(String, [u32; 2], std::sync::Arc<Vec<u8>>)>,
    /// The picture the live view lays out and the screen wears.
    picture: Option<Handle<Image>>,
    /// What was last laid out, so the work is done only when the view moved: the window,
    /// the way the player faces, what was marked on it, and the outline it was laid out
    /// from (held, so the same texels are never mistaken for another picture at the same
    /// address).
    window: Option<(Window, [f32; 2])>,
    marks: Vec<Mark>,
    base: Option<LocalPic>,
    /// The page of letters last laid out and the step of its flicker; `None` while the
    /// MAP page's picture is on the screen (so turning back to it lays the map out again).
    page: Option<(Page, u32)>,
    /// The rows that page was laid out from.
    rows: Vec<Row>,
    /// The game's own Pip-Boy font and its letters' texels, read once: the outer `None`
    /// means it has not been looked for yet, the inner that it could not be read.
    face: Option<Option<PageFont>>,
    /// The picture the log line last named.
    said: Option<String>,
}

impl Worn {
    /// What this cache holds, bytes: the worldspace's map texels on the processor, the
    /// canvas the screen wears, and the Pip-Boy font's letters. Told to
    /// `nvf_play::wrist_local` whenever one of them changes, for the run log's memory line
    /// (nvforever `gpu_recorder`): nothing here may grow with the length of a session.
    fn bytes(&self) -> u64 {
        let source = self.source.as_ref().map_or(0, |(path, _, texels)| path.len() + texels.len());
        let canvas = self.picture.as_ref().map_or(0, |_| (CANVAS * CANVAS * 4) as usize);
        let face = self.face.as_ref().and_then(|f| f.as_ref()).map_or(0, |f| f.ink.len());
        (source + canvas + face) as u64
    }

    /// Say what it holds now ([`nvf_play::wrist_local::note_cache`]).
    fn note(&self) {
        nvf_play::wrist_local::note_cache(nvf_play::wrist_local::Cache::Screen, self.bytes());
    }
}

/// Whether these are the same base: the same outline, or the worldspace's own picture
/// both times (which the window itself then tells apart).
fn same_base(now: Option<&LocalPic>, was: Option<&LocalPic>) -> bool {
    match (now, was) {
        (Some(a), Some(b)) => a.is(b),
        (a, b) => a.is_none() == b.is_none(),
    }
}

/// Up on a north-up map ([`window`]'s facing for a picture that does not turn).
const NORTH: [f32; 2] = [0.0, -1.0];

/// The ink the whole picture is drawn in: the Pip-Boy screens' own colour as the player
/// set it (`interface::pipboy_rgb`, amber until they change it, green among its presets),
/// through the screen's own gain (`SCREEN_TINT`, which is near neutral: the menus' amber
/// is in their drawing, not in the tint, so a picture laid on the screen raw comes out
/// grey). At full strength, so the player's mark and the compass letters come out as
/// bright as the screen can draw them; the map under them is dimmed where it is laid out
/// instead ([`Pic::gain`]).
fn map_ink() -> LinearRgba {
    let rgb = nvf_play::interface::pipboy_rgb();
    let ink = Color::srgb_u8(rgb[0], rgb[1], rgb[2]).to_linear();
    let [r, g, b] = SCREEN_TINT;
    LinearRgba::rgb(ink.red * r, ink.green * g, ink.blue * b)
}

/// The screen's material with a worldspace's map on it, from the shape's own, as
/// `pipboy_arm::menus_material` does it for the menus' picture.
fn map_material(own: &NifMaterial, picture: Handle<Image>) -> NifMaterial {
    let mut m = own.clone();
    m.base.base_color = Color::LinearRgba(map_ink());
    m.base.base_color_texture = Some(picture);
    m.base.emissive_texture = None;
    m.base.normal_map_texture = None;
    m.base.unlit = true;
    m.base.alpha_mode = AlphaMode::Opaque;
    // The shape's canvas (0 to `UV_EXTENT` of its own texture) wears the whole picture.
    m.base.uv_transform = Affine2::from_scale(Vec2::splat(1.0 / UV_EXTENT));
    m.extension.pipboy_screen = true;
    // The plain look (no roll, no wash), but the taps read the whole picture: the map is
    // the texture itself, where `ScreenLook::surface` describes the menus' render target,
    // in which the canvas fills `CANVAS / TARGET` of it.
    let (a, b) = ScreenLook::default().surface();
    (m.extension.pipboy_a, m.extension.pipboy_b) = (a.with_w(1.0), b.with_z(1.0));
    m
}

#[allow(clippy::too_many_arguments)]
fn wear_map(
    wrist: Option<Res<WristLive>>,
    data: Option<Res<GameData>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<NifMaterial>>,
    mut parts: Query<(Entity, &PipBoyPart, &mut MeshMaterial3d<NifMaterial>)>,
    mut worn: Local<Worn>,
) {
    let want = wrist.as_deref().and_then(|w| w.screen_key());
    let screen = parts.iter().find(|(_, p, _)| **p == PipBoyPart::Screen).map(|(e, _, m)| (e, m.0.clone()));
    // Give the screen back first, so the frame a menu takes it never copies our picture.
    if let Some((e, own)) = worn.screen.clone() {
        let ours = worn.copy.as_ref().map(|c| c.0.clone());
        let still = screen.as_ref().is_some_and(|(s, m)| *s == e && ours.as_ref() == Some(m));
        let keep = want.is_some_and(|k| k.screen == e.to_bits()) && still;
        if !keep {
            if still {
                if let Ok((_, _, mut m)) = parts.get_mut(e) {
                    m.0 = own;
                }
            }
            (worn.screen, worn.said) = (None, None);
        }
    }
    let (Some(live), Some(key), Some((e, current))) = (wrist.as_deref(), want, screen) else { return };
    // The body's facts are a frame old: wait until they name the shape that is there now.
    if key.screen != e.to_bits() {
        return;
    }
    let picture = worn.picture.get_or_insert_with(|| images.add(blank())).clone();
    worn.note();
    // The material the shape wears for itself: taken once, before the copy goes on.
    let own = worn.screen.as_ref().map_or(current, |(_, own)| own.clone());
    let name = if live.page == Page::Map {
        let Some(shown) = live.wants() else { return };
        // The base: the local outline nvf-play laid out, or else the worldspace's own
        // picture read from the load order and decoded once.
        let name = match &shown.local {
            Some(_) => shown.texture.clone(),
            None => {
                let path = nvf_play::interface::own_texture(&shown.texture);
                if worn.source.as_ref().is_none_or(|(p, ..)| *p != path) {
                    let Some(data) = data.as_deref() else { return };
                    // The map's own texels, on the processor: the live picture is laid out
                    // from them (`compose`), and the picture itself never goes to the GPU
                    // as it stands.
                    worn.source = read_picture(&path, data).map(|(size, rgba)| (path.clone(), size, rgba));
                    worn.window = None;
                    worn.note();
                    if worn.source.is_none() {
                        warn!("wrist: {path} could not be read for the live map");
                        return;
                    }
                }
                path
            }
        };
        // The player's Map turns row: With me (the picture turns under a mark that points
        // up the glass) or North up (the picture stands still and the mark turns).
        let facing = if live.rotate_map() { shown.forward } else { NORTH };
        let win = window(shown.uv, facing, shown.half, 1.0);
        if worn.window != Some((win, shown.forward)) || worn.marks != shown.marks || worn.page.is_some() || !same_base(shown.local.as_ref(), worn.base.as_ref()) {
            let made = match &shown.local {
                // The outline's own: one byte a sample, drawn at full as the paused Local
                // Map page draws the very same picture.
                Some(pic) => Some(Pic { size: pic.size, rgba: &pic.alpha, stride: 1, gain: 1.0 }),
                None => worn.source.as_ref().map(|(_, size, rgba)| Pic { size: *size, rgba, stride: 4, gain: WORLD_GAIN }),
            };
            let Some(base) = made else { return };
            let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
            compose(&mut out, CANVAS, &base, win, shown.forward, &shown.marks, live.rotate_map());
            if let Some(mut live) = images.get_mut(&picture) {
                live.data = Some(out);
            }
            (worn.window, worn.base, worn.page) = (Some((win, shown.forward)), shown.local.clone(), None);
            worn.marks.clear();
            worn.marks.extend_from_slice(&shown.marks);
        }
        name
    } else {
        // A page of letters: nothing of the world is read for it, and it is laid out
        // again only when its rows or the flicker's step changed.
        let step = flick_step(live.flicker());
        if worn.page != Some((live.page, step)) || worn.rows != live.rows {
            {
                let face = worn.face.get_or_insert_with(|| data.as_deref().and_then(page_font)).as_ref();
                let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
                compose_page(&mut out, CANVAS, &live.rows, face, live.flicker());
                if let Some(mut image) = images.get_mut(&picture) {
                    image.data = Some(out);
                }
            }
            worn.note();
            (worn.page, worn.window, worn.base) = (Some((live.page, step)), None, None);
            worn.rows.clear();
            worn.rows.extend_from_slice(&live.rows);
        }
        live.page.name().to_string()
    };
    if worn.copy.as_ref().is_none_or(|(_, source)| *source != own) {
        let base = materials.get(&own).cloned().unwrap_or_default();
        let made = materials.add(map_material(&base, picture.clone()));
        worn.copy = Some((made, own.clone()));
    }
    let Some((ours, _)) = worn.copy.clone() else { return };
    if let Ok((_, _, mut m)) = parts.get_mut(e) {
        if m.0 != ours {
            m.0 = ours;
        }
    }
    if worn.screen.is_none() {
        worn.screen = Some((e, own));
    }
    if worn.said.as_ref() != Some(&name) {
        info!("wrist: the live map is on the arm's screen ({name})");
        worn.said = Some(name);
    }
}

/// The live picture the arm's screen wears, in texels a side: the glass is a couple of
/// hundred pixels across at arm's length, so a square of this size is sharp enough and
/// cheap to lay out again at every sample [design: this file].
pub const CANVAS: u32 = 256;

/// What the marks are drawn in, against the map under them (0 to 255 per channel, the
/// material's own ink tints the lot): the player's mark is the brightest thing on the
/// screen, the compass letters a step under it.
const MARK: u8 = 255;
const LETTER: u8 = 210;

/// The player's mark, as a share of the picture: a triangle pointing the way they face.
const MARK_SHARE: f32 = 0.07;

/// Where the compass letters sit at the most, as a share of the picture from its middle.
const LETTER_RING: f32 = 0.37;

/// How much of the live picture the device's glass hides at each edge, as a share of the
/// picture: the shape the picture is worn on runs under the Pip-Boy's own frame, so the
/// outermost part of the canvas is never read [observed: the owner's photo of the Odin
/// build map.12, 2026-11-21: the S of the compass stood on the bottom edge of the glass,
/// its lower half under the device's button bar, where N, E and W were clear].
const GLASS_INSET: f32 = 0.10;

/// How far inside the glass the whole of a compass letter's box stays [design].
const LETTER_MARGIN: f32 = 0.02;

/// The texels a dot of a letter is drawn with on a picture `side` texels square.
fn glyph_dot(side: u32) -> u32 {
    (side / 64).max(1)
}

/// How far from the middle the compass letters stand on a picture `side` texels square,
/// across and down: [`LETTER_RING`] at the most, pulled in until the whole of each
/// letter's box is inside the glass ([`GLASS_INSET`]) with [`LETTER_MARGIN`] to spare.
fn letter_ring(side: u32) -> [f32; 2] {
    let s = side.max(1) as f32;
    let dot = glyph_dot(side) as f32 / s;
    let room = 0.5 - GLASS_INSET - LETTER_MARGIN;
    [(room - 5.0 * dot / 2.0).clamp(0.0, LETTER_RING), (room - 7.0 * dot / 2.0).clamp(0.0, LETTER_RING)]
}

/// A picture to read texels from (rows top first): the worldspace's own, four bytes a
/// texel, or the local outline's, one.
pub struct Pic<'a> {
    pub size: [u32; 2],
    pub rgba: &'a [u8],
    /// Bytes a texel: 4 for a picture the load order owns, 1 for the outline's own (how
    /// brightly each sample is drawn).
    pub stride: usize,
    /// How brightly this picture is drawn under the marks, as the paused Pip-Boy draws the
    /// very same thing: the worldspace's own picture at the map's alpha
    /// (`pipboy::data::MAP_ALPHA`), the outline at full, where the marks are at full
    /// ([`map_ink`]).
    pub gain: f32,
}

impl Pic<'_> {
    /// The texel at `uv` at this picture's own strength, clamped to the picture; black
    /// outside a usable picture.
    fn at(&self, uv: [f32; 2]) -> [u8; 3] {
        let (w, h) = (self.size[0].max(1), self.size[1].max(1));
        let stride = self.stride.max(1);
        let x = ((uv[0] * w as f32) as i64).clamp(0, w as i64 - 1) as usize;
        let y = ((uv[1] * h as f32) as i64).clamp(0, h as i64 - 1) as usize;
        let i = (y * w as usize + x) * stride;
        let gain = if self.gain.is_finite() { self.gain.clamp(0.0, 1.0) } else { 1.0 };
        let texel = match self.rgba.get(i..i + stride.min(3)) {
            Some([v]) => [*v; 3],
            Some([r, g, b, ..]) => [*r, *g, *b],
            _ => [0, 0, 0],
        };
        texel.map(|c| (f32::from(c) * gain) as u8)
    }
}

/// How brightly the worldspace's own map picture is drawn under the marks, as the paused
/// World Map page draws it (`pipboy::data::MAP_ALPHA` of the screen's brightest).
const WORLD_GAIN: f32 = nvf_play::pipboy::data::MAP_ALPHA / 255.0;

/// Lay out the screen's picture: the map under the window, then the player's mark and the
/// compass letters over it. North is up, so the letters stand still and the mark turns
/// ([`facing`] as [`nvf_play::wrist_arm::Shown`] gives it, map space with v southward).
/// The marks are drawn white: the screen's own ink colours them (`map_ink`).
pub fn compose(out: &mut [u8], side: u32, map: &Pic, win: Window, facing: [f32; 2], marks: &[Mark], rotate: bool) {
    let side = side.max(1);
    for y in 0..side {
        for x in 0..side {
            let uv = [(x as f32 + 0.5) / side as f32, (y as f32 + 0.5) / side as f32];
            let [r, g, b] = map.at(win.at(uv));
            let i = ((y * side + x) * 4) as usize;
            if let Some(p) = out.get_mut(i..i + 4) {
                p.copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    // Heading up: the picture itself was turned under the player ([`window`] was given
    // their facing), so the letters ride round the glass by the way they face and their
    // own mark points straight up. North up: the letters stand still and the mark turns.
    let turn = if rotate { heading_of(facing) } else { 0.0 };
    let arrow = if rotate { NORTH } else { facing };
    // Every dark pad first, then every mark and letter over them, so one thing's pad
    // never eats another thing's ink where the two meet at the glass's edge.
    for pad in [true, false] {
        draw_marks(out, side, win, marks, pad);
        draw_letters(out, side, turn, pad);
        draw_mark(out, side, arrow, pad);
    }
}

/// The player's heading in radians, clockwise from north, from the facing
/// [`nvf_play::wrist_arm::Shown`] carries (map space, v southward).
fn heading_of(facing: [f32; 2]) -> f32 {
    let n = (facing[0] * facing[0] + facing[1] * facing[1]).sqrt();
    if n.is_finite() && n > 1e-6 {
        facing[0].atan2(-facing[1])
    } else {
        0.0
    }
}

/// What the map's marks are drawn in: a step under the compass letters, so the player's
/// own mark and the letters stay the brightest things on the glass. A quest target and the
/// player's own marker are drawn at the screen's full instead, as the compass draws them
/// over the rest.
const PIN: u8 = 190;

/// A mark's reach from its middle, as a share of the picture [design: big enough to read
/// at the glass's size, small enough not to hide the map under it].
const PIN_SHARE: f32 = 0.035;

/// The marks over the map (`nvf_play::wrist_live::Mark`): a diamond where the thing falls
/// inside the window, else a chevron held at the glass's edge pointing the way it lies
/// [design: the owner, 2026-11-21]. North is up, as the map under them is, so a mark's
/// place on the canvas is the window taken the other way about ([`Window::of`]).
fn draw_marks(out: &mut [u8], side: u32, win: Window, marks: &[Mark], pad: bool) {
    let s = side as f32;
    let r = s * PIN_SHARE;
    // The glass, with room for the mark itself, so nothing is drawn under the device's
    // own frame ([`GLASS_INSET`]).
    let (lo, hi) = (GLASS_INSET * s + r, (1.0 - GLASS_INSET) * s - r);
    if hi <= lo {
        return;
    }
    for m in marks {
        let Some(c) = win.of(m.uv) else { continue };
        let at = [c[0] * s, c[1] * s];
        if !at.iter().all(|v| v.is_finite()) {
            continue;
        }
        let held = [at[0].clamp(lo, hi), at[1].clamp(lo, hi)];
        let (ink, size) = if m.kind >= MarkKind::Quest { (MARK, r) } else { (PIN, r * 0.75) };
        if pad {
            knock_disc(out, side, held, size * KNOCK_REACH);
            continue;
        }
        let away = [at[0] - held[0], at[1] - held[1]];
        if away[0].abs() < 1e-3 && away[1].abs() < 1e-3 {
            diamond(out, side, held, size, ink);
        } else {
            chevron(out, side, held, away, size, ink);
        }
    }
}

/// A small diamond about `at`.
fn diamond(out: &mut [u8], side: u32, at: [f32; 2], r: f32, ink: u8) {
    let (top, right, bottom, left) = ([at[0], at[1] - r], [at[0] + r, at[1]], [at[0], at[1] + r], [at[0] - r, at[1]]);
    fill_triangle(out, side, top, right, bottom, ink);
    fill_triangle(out, side, top, bottom, left, ink);
}

/// A chevron at `at` pointing along `away`: what a mark off the window is held at the
/// glass's edge as.
fn chevron(out: &mut [u8], side: u32, at: [f32; 2], away: [f32; 2], r: f32, ink: u8) {
    let n = (away[0] * away[0] + away[1] * away[1]).sqrt();
    if !n.is_finite() || n <= 1e-6 {
        return;
    }
    let [dx, dy] = [away[0] / n, away[1] / n];
    let point = [at[0] + dx * r, at[1] + dy * r];
    let (bx, by) = (at[0] - dx * r * 0.4, at[1] - dy * r * 0.4);
    let (sx, sy) = (-dy * r * 0.8, dx * r * 0.8);
    fill_triangle(out, side, point, [bx + sx, by + sy], [bx - sx, by - sy], ink);
}

/// The player's own mark: a triangle at the middle, its point the way they face.
fn draw_mark(out: &mut [u8], side: u32, facing: [f32; 2], pad: bool) {
    let n = (facing[0] * facing[0] + facing[1] * facing[1]).sqrt();
    let [fx, fy] = if n.is_finite() && n > 1e-6 { [facing[0] / n, facing[1] / n] } else { NORTH };
    let mid = side as f32 / 2.0;
    let r = side as f32 * MARK_SHARE;
    // The point, and the two back corners a little either side of the tail.
    let point = [mid + fx * r, mid + fy * r];
    if pad {
        knock_disc(out, side, [mid, mid], r * KNOCK_REACH);
        return;
    }
    // A long point and a short tail, so which way it points is read at a glance.
    let (bx, by) = (-fx * r * 0.55, -fy * r * 0.55);
    let (sx, sy) = (-fy * r * 0.5, fx * r * 0.5);
    let a = [mid + bx + sx, mid + by + sy];
    let b = [mid + bx - sx, mid + by - sy];
    fill_triangle(out, side, point, a, b, MARK);
}

/// A filled triangle, by the sign of the three edges.
fn fill_triangle(out: &mut [u8], side: u32, a: [f32; 2], b: [f32; 2], c: [f32; 2], ink: u8) {
    let edge = |p: [f32; 2], q: [f32; 2], x: f32, y: f32| (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0]);
    let lo = |i: usize| [a[i], b[i], c[i]].iter().cloned().fold(f32::MAX, f32::min).floor().max(0.0) as u32;
    let hi = |i: usize| [a[i], b[i], c[i]].iter().cloned().fold(f32::MIN, f32::max).ceil().min(side as f32) as u32;
    for y in lo(1)..hi(1) {
        for x in lo(0)..hi(0) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (e0, e1, e2) = (edge(a, b, px, py), edge(b, c, px, py), edge(c, a, px, py));
            if (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0) {
                put(out, side, x, y, ink);
            }
        }
    }
}

/// The compass letters, at the four sides of a north-up picture, every one of them whole
/// inside the glass ([`letter_ring`]).
fn draw_letters(out: &mut [u8], side: u32, turn: f32, pad: bool) {
    let mid = side as f32 / 2.0;
    let [across, down] = letter_ring(side).map(|r| r * side as f32);
    let turn = if turn.is_finite() { turn } else { 0.0 };
    for (i, glyph) in [N_GLYPH, E_GLYPH, S_GLYPH, W_GLYPH].into_iter().enumerate() {
        // The letter's own bearing, less the way the player faces: north stands at the
        // top of a north-up picture and swings round the glass as they turn.
        let (sin, cos) = (i as f32 * std::f32::consts::FRAC_PI_2 - turn).sin_cos();
        draw_glyph(out, side, glyph, [mid + across * sin, mid - down * cos], pad);
    }
}

/// Our own letters, five wide and seven tall, a row of bits each (the top row first and
/// the leftmost dot in the high bit) [design: this file; nothing is read from the game].
const N_GLYPH: [u8; 7] = [0b10001, 0b11001, 0b11001, 0b10101, 0b10011, 0b10011, 0b10001];
const E_GLYPH: [u8; 7] = [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111];
const S_GLYPH: [u8; 7] = [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110];
const W_GLYPH: [u8; 7] = [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001];

/// A letter, drawn about `at`, as many texels a dot as the picture can spare.
fn draw_glyph(out: &mut [u8], side: u32, glyph: [u8; 7], at: [f32; 2], pad: bool) {
    let dot = glyph_dot(side);
    let (w, h) = (5 * dot, 7 * dot);
    let x0 = at[0] as i64 - (w / 2) as i64;
    let y0 = at[1] as i64 - (h / 2) as i64;
    // The dark pad under it, so a letter reads over the outline's own bright walls
    // [design: the owner, 2026-11-21].
    if pad {
        knock_box(out, side, x0 - i64::from(dot), y0 - i64::from(dot), w + 2 * dot, h + 2 * dot);
        return;
    }
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..5u32 {
            if bits & (1 << (4 - col)) == 0 {
                continue;
            }
            for dy in 0..dot {
                for dx in 0..dot {
                    let (x, y) = (x0 + (col * dot + dx) as i64, y0 + (row as u32 * dot + dy) as i64);
                    if x >= 0 && y >= 0 && (x as u32) < side && (y as u32) < side {
                        put(out, side, x as u32, y as u32, LETTER);
                    }
                }
            }
        }
    }
}

fn put(out: &mut [u8], side: u32, x: u32, y: u32, ink: u8) {
    let i = ((y * side + x) * 4) as usize;
    if let Some(p) = out.get_mut(i..i + 4) {
        p.copy_from_slice(&[ink, ink, ink, 255]);
    }
}

/// The brightest of what is there and `ink`: a letter of a page drawn over its ground.
fn put_over(out: &mut [u8], side: u32, x: u32, y: u32, ink: u8) {
    let i = ((y * side + x) * 4) as usize;
    if let Some(p) = out.get_mut(i..i + 4) {
        let v = p[0].max(ink);
        p.copy_from_slice(&[v, v, v, 255]);
    }
}

/// How much of what is under a letter or a mark is left showing, and how far past the
/// thing itself its dark pad reaches [design: the owner, 2026-11-21: the letters and the
/// marks must read over the outline's own bright walls].
const KNOCKOUT: f32 = 0.22;
const KNOCK_REACH: f32 = 1.35;

/// Take the picture down to [`KNOCKOUT`] of itself in a box, so what is drawn over it
/// reads. Nothing is added: a dark place stays dark, and the map is never brightened.
fn knock_box(out: &mut [u8], side: u32, x0: i64, y0: i64, w: u32, h: u32) {
    for y in y0.max(0)..(y0 + i64::from(h)).min(i64::from(side)) {
        for x in x0.max(0)..(x0 + i64::from(w)).min(i64::from(side)) {
            knock_at(out, side, x as u32, y as u32);
        }
    }
}

/// The same in a disc about `at`.
fn knock_disc(out: &mut [u8], side: u32, at: [f32; 2], r: f32) {
    if !at.iter().all(|v| v.is_finite()) || !r.is_finite() || r <= 0.0 {
        return;
    }
    let (x0, x1) = ((at[0] - r).floor().max(0.0) as u32, (at[0] + r).ceil().clamp(0.0, side as f32) as u32);
    let (y0, y1) = ((at[1] - r).floor().max(0.0) as u32, (at[1] + r).ceil().clamp(0.0, side as f32) as u32);
    for y in y0..y1 {
        for x in x0..x1 {
            let (dx, dy) = (x as f32 + 0.5 - at[0], y as f32 + 0.5 - at[1]);
            if dx * dx + dy * dy <= r * r {
                knock_at(out, side, x, y);
            }
        }
    }
}

fn knock_at(out: &mut [u8], side: u32, x: u32, y: u32) {
    let i = ((y * side + x) * 4) as usize;
    if let Some(p) = out.get_mut(i..i + 3) {
        for c in p {
            *c = (f32::from(*c) * KNOCKOUT) as u8;
        }
    }
}

// ---- the pages of letters (STATUS, QUESTS) ---------------------------------------------

/// The font the pages are set in: the game's own plain monofonto, the one the menus and
/// the HUD already load (`nvf_play::interface::FONT_FILES`, the install's own file for it
/// where its INI names one). No new asset is shipped for this [design].
const PAGE_FONT: &str = "textures\\fonts\\monofonto_large.fnt";

/// The game's Pip-Boy font and its letters' texels on the processor, read once a run: the
/// pages are rasterised here, where the map is, so the screen stays one picture and no
/// second render target or readback is needed.
pub struct PageFont {
    pub font: fnv_font::Font,
    /// The letters' picture: its size, and one byte a texel (how brightly it is drawn).
    pub size: [u32; 2],
    pub ink: Vec<u8>,
}

impl PageFont {
    fn at(&self, x: usize, y: usize) -> u8 {
        let w = self.size[0].max(1) as usize;
        self.ink.get(y * w + x).copied().unwrap_or(0)
    }
}

/// How brightly each texel of a letters' picture is drawn: its own colour through its
/// cover, so a font whose letters are white with an alpha and one whose letters are drawn
/// in the picture both come out the same.
fn ink_of(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).map(|p| ((u32::from(p[0].max(p[1]).max(p[2])) * u32::from(p[3])) / 255) as u8).collect()
}

/// Read [`PAGE_FONT`] and its letters' picture from the load order: the `.tex` a localized
/// install leaves beside the glyph table, else the `.dds` the table names.
fn page_font(data: &GameData) -> Option<PageFont> {
    let path = nvf_play::interface::font_file(PAGE_FONT);
    let bytes = data.read(&path)?;
    let font = match fnv_font::Font::parse(&bytes) {
        Ok(font) => font,
        Err(e) => {
            warn!("wrist: {path}: {e}; the glance's pages are drawn without letters");
            return None;
        }
    };
    let tex = data.read(&font.tex_path()).and_then(|b| fnv_font::Tex::parse(&b).ok()).map(|t| ([t.width, t.height], ink_of(&t.rgba)));
    let (size, ink) = match tex.or_else(|| read_picture(&font.texture_path(), data).map(|(size, rgba)| (size, ink_of(&rgba)))) {
        Some(found) => found,
        None => {
            warn!("wrist: {} could not be read; the glance's pages are drawn without letters", font.texture_path());
            return None;
        }
    };
    info!("wrist: the glance's pages are set in {path} ({} x {} texels)", size[0], size[1]);
    Some(PageFont { font, size, ink })
}

/// The page's own look on the glass: a dark ground, a bright heading over a rule, labels
/// a step under the readings, and the stock bar, a dim ground with a bright fill
/// [design: this file; the paused Pip-Boy pages' own look].
const PAGE_BACK: u8 = 14;
const PAGE_HEAD: u8 = 255;
const PAGE_LABEL: u8 = 195;
const PAGE_VALUE: u8 = 245;
const PAGE_RULE: u8 = 165;
const PAGE_BAR: u8 = 225;
const PAGE_GROUND: u8 = 55;

/// One line of a page, the pad inside the glass and a bar's height, as shares of the
/// picture [design: about ten lines fit the glass at [`CANVAS`]].
const PAGE_LINE: f32 = 0.075;
const PAGE_PAD: f32 = 0.025;
const PAGE_BAR_HEIGHT: f32 = 0.018;
/// How much of the font's own line height a page's line is set at, so the letters have
/// air round them at the glass's size.
const PAGE_TEXT: f32 = 0.78;

/// How many steps the flicker of a page flip is laid out in, so the glass is not composed
/// afresh every frame of it [design].
const FLICK_STEPS: f32 = 6.0;

/// Which step of the flicker a share of it is; `u32::MAX` for no flicker at all, so a
/// page with none never matches one being flipped.
pub fn flick_step(flicker: Option<f32>) -> u32 {
    match flicker {
        Some(f) if f.is_finite() => (f.clamp(0.0, 1.0) * FLICK_STEPS) as u32,
        Some(_) => 0,
        None => u32::MAX,
    }
}

/// Lay out a page of letters on the glass. `face` is the game's own font; without it the
/// rules and the bars still draw, so a run with no data still shows a page.
pub fn compose_page(out: &mut [u8], side: u32, rows: &[Row], face: Option<&PageFont>, flicker: Option<f32>) {
    let side = side.max(1);
    for y in 0..side {
        for x in 0..side {
            put(out, side, x, y, PAGE_BACK);
        }
    }
    let s = side as f32;
    let (lo, hi) = ((GLASS_INSET + PAGE_PAD) * s, (1.0 - GLASS_INSET - PAGE_PAD) * s);
    let line = PAGE_LINE * s;
    let width = hi - lo;
    if width <= 0.0 || line <= 0.0 {
        return;
    }
    let scale = face.map_or(1.0, |f| line * PAGE_TEXT / f.font.size.max(1.0));
    // The first line's baseline, a line below the top of the glass.
    let mut y = lo + line;
    for row in rows {
        if y > hi {
            break;
        }
        match row {
            Row::Head(text) => {
                draw_text(out, side, face, text, lo, y, scale, PAGE_HEAD);
                rule(out, side, lo, hi, y + line * 0.22, PAGE_RULE);
                y += line * 1.35;
            }
            Row::Pair(label, value) => {
                draw_text(out, side, face, label, lo, y, scale, PAGE_LABEL);
                draw_text(out, side, face, value, hi - text_width(face, value, scale), y, scale, PAGE_VALUE);
                y += line;
            }
            Row::Bar(label, share, value) => {
                draw_text(out, side, face, label, lo, y, scale, PAGE_LABEL);
                draw_text(out, side, face, value, hi - text_width(face, value, scale), y, scale, PAGE_VALUE);
                bar(out, side, lo, width, y + line * 0.16, PAGE_BAR_HEIGHT * s, *share);
                y += line * 1.5;
            }
            Row::Text(text) => {
                for one in wrapped(face, text, width, scale) {
                    if y > hi {
                        break;
                    }
                    draw_line(out, side, face, &one, lo, y, scale, PAGE_VALUE);
                    y += line;
                }
            }
            Row::Gap => y += line * 0.5,
        }
    }
    if let Some(at) = flicker {
        scan(out, side, at);
    }
}

/// The lines a text is set in, wrapped to `width` texels of the glass.
fn wrapped(face: Option<&PageFont>, text: &str, width: f32, scale: f32) -> Vec<fnv_font::Line> {
    match face {
        Some(f) if scale > 0.0 => f.font.lines(text, Some(width / scale)),
        _ => Vec::new(),
    }
}

/// How wide a text is set, in texels of the glass.
fn text_width(face: Option<&PageFont>, text: &str, scale: f32) -> f32 {
    face.map_or(0.0, |f| f.font.width(text) * scale)
}

/// One line of text, starting at `x` and standing on `base`.
fn draw_text(out: &mut [u8], side: u32, face: Option<&PageFont>, text: &str, x: f32, base: f32, scale: f32, ink: u8) {
    let Some(face) = face else { return };
    if let Some(line) = face.font.lines(text, None).first() {
        draw_line(out, side, Some(face), line, x, base, scale, ink);
    }
}

fn draw_line(out: &mut [u8], side: u32, face: Option<&PageFont>, line: &fnv_font::Line, x: f32, base: f32, scale: f32, ink: u8) {
    let Some(face) = face else { return };
    for placed in &line.glyphs {
        let glyph = *face.font.glyph(placed.code);
        blit_glyph(out, side, face, &glyph, x + placed.x * scale, base, scale, ink);
    }
}

/// One letter of the game's font, from its place on the font's own picture onto the
/// glass. Two samples each way, so the letters do not break up where they are drawn
/// smaller than the picture holds them.
fn blit_glyph(out: &mut [u8], side: u32, face: &PageFont, glyph: &fnv_font::Glyph, x: f32, base: f32, scale: f32, ink: u8) {
    if !glyph.drawn() || !scale.is_finite() || scale <= 0.0 {
        return;
    }
    let (left, top) = (x + glyph.before * scale, base - glyph.ascent * scale);
    let (w, h) = (glyph.width * scale, glyph.height * scale);
    if !(w > 0.0 && h > 0.0) || !left.is_finite() || !top.is_finite() {
        return;
    }
    let (aw, ah) = (face.size[0].max(1) as f32, face.size[1].max(1) as f32);
    let [u0, v0, u1, v1] = glyph.uv;
    let x0 = left.floor().clamp(0.0, side as f32) as u32;
    let x1 = (left + w).ceil().clamp(0.0, side as f32) as u32;
    let y0 = top.floor().clamp(0.0, side as f32) as u32;
    let y1 = (top + h).ceil().clamp(0.0, side as f32) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            let mut sum = 0u32;
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let fx = (px as f32 + ox - left) / w;
                let fy = (py as f32 + oy - top) / h;
                if !(0.0..1.0).contains(&fx) || !(0.0..1.0).contains(&fy) {
                    continue;
                }
                let sx = ((u0 + (u1 - u0) * fx) * aw).clamp(0.0, aw - 1.0) as usize;
                let sy = ((v0 + (v1 - v0) * fy) * ah).clamp(0.0, ah - 1.0) as usize;
                sum += u32::from(face.at(sx, sy));
            }
            let value = (sum / 4) * u32::from(ink) / 255;
            if value > 0 {
                put_over(out, side, px, py, value.min(255) as u8);
            }
        }
    }
}

/// The rule under a heading: a line from `lo` to `hi` at `y`.
fn rule(out: &mut [u8], side: u32, lo: f32, hi: f32, y: f32, ink: u8) {
    let thick = (side / 128).max(1);
    let y0 = y.max(0.0) as u32;
    for row in y0..(y0 + thick).min(side) {
        for x in lo.max(0.0) as u32..(hi.max(0.0) as u32).min(side) {
            put(out, side, x, row, ink);
        }
    }
}

/// A page's bar: the stock dim ground with a bright fill over it.
fn bar(out: &mut [u8], side: u32, x: f32, width: f32, top: f32, height: f32, share: f32) {
    let share = if share.is_finite() { share.clamp(0.0, 1.0) } else { 0.0 };
    let (x0, y0) = (x.max(0.0) as u32, top.max(0.0) as u32);
    let x1 = (x + width).max(0.0).min(side as f32) as u32;
    let y1 = (top + height.max(1.0)).max(0.0).min(side as f32) as u32;
    let lit = x0 + ((x1.saturating_sub(x0)) as f32 * share) as u32;
    for y in y0..y1 {
        for px in x0..x1 {
            put(out, side, px, y, if px < lit { PAGE_BAR } else { PAGE_GROUND });
        }
    }
}

/// The flip's flicker: the glass dims and comes back while a bright scan runs down it
/// [design: the owner's "a quick screen flicker/scan transition", 2026-11-21].
fn scan(out: &mut [u8], side: u32, at: f32) {
    let at = if at.is_finite() { at.clamp(0.0, 1.0) } else { return };
    let dim = 0.35 + 0.65 * at;
    let band = at * side as f32;
    let reach = (side as f32 * 0.05).max(1.0);
    for y in 0..side {
        let near = (y as f32 + 0.5 - band).abs() <= reach;
        for x in 0..side {
            let i = ((y * side + x) * 4) as usize;
            if let Some(p) = out.get_mut(i..i + 3) {
                for c in p {
                    let v = (f32::from(*c) * dim) as u8;
                    *c = if near { v.max(150) } else { v };
                }
            }
        }
    }
}

/// A picture's texels, read from the load order and decoded here: the map pictures are
/// block-compressed and the graphics card keeps them that way, so the live view decodes
/// its own copy rather than read one back ([`compose`] lays the screen out from it).
fn read_picture(path: &str, data: &GameData) -> Option<([u32; 2], std::sync::Arc<Vec<u8>>)> {
    let bytes = data.read(path)?;
    let dds = fnv_dds::Dds::parse(&bytes).ok()?;
    let (w, h) = (dds.header.width, dds.header.height);
    let top = dds.levels.first()?.first()?;
    let rgba = match dds.format {
        fnv_dds::Format::Dxt1 | fnv_dds::Format::Dxt3 | fnv_dds::Format::Dxt5 => fnv_dds::decode_blocks(dds.format, w, h, top)?,
        // Already texels: taken as they lie, four bytes each.
        _ if top.len() as u32 >= w * h * 4 => top[..(w * h * 4) as usize].to_vec(),
        _ => return None,
    };
    Some(([w, h], std::sync::Arc::new(rgba)))
}

/// An empty picture of the screen's own size, for the live view to lay out into.
fn blank() -> Image {
    let mut image = Image::new(
        Extent3d { width: CANVAS, height: CANVAS, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![0; (CANVAS * CANVAS * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Hide the lit copy of the screen while the live view owns the screen. `show_screen` sets
/// it every frame from the menus' own state, so letting go needs nothing here.
fn hide_lit(wrist: Option<Res<WristLive>>, parts: Query<(Entity, &PipBoyPart)>, mut visibility: Query<&mut Visibility>) {
    if wrist.as_deref().and_then(|w| w.screen_key()).is_none() {
        return;
    }
    for (e, _) in parts.iter().filter(|(_, p)| **p == PipBoyPart::ScreenLit) {
        if let Ok(mut v) = visibility.get_mut(e) {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
    }
}

/// What only the body knows, for nvf-play's decision on the next frame.
fn feed(
    body: Option<Res<PlayerBody>>,
    field: Option<Res<crate::stealth_field::StealthField>>,
    parts: Query<(Entity, &PipBoyPart)>,
    wrist: Option<ResMut<WristBody>>,
) {
    let Some(mut wrist) = wrist else { return };
    let now = WristBody {
        first_person_drawn: body.as_deref().is_some_and(|b| b.first_person_drawn()),
        screen: parts.iter().find(|(_, p)| **p == PipBoyPart::Screen).map(|(e, _)| e.to_bits()),
        // Under a Stealth Boy every mesh wears the field's see-through copy: the live
        // view yields the arm rather than swap materials against it each frame.
        taken: field.as_deref().is_some_and(|f| f.on),
    };
    if *wrist != now {
        *wrist = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    /// A flat grey map to lay out over, so only the marks are bright.
    fn grey() -> Vec<u8> {
        vec![40; 16 * 16 * 4]
    }

    fn laid(facing: [f32; 2]) -> Vec<u8> {
        let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        let map = grey();
        let win = window([0.5; 2], NORTH, 0.1, 1.0);
        compose(&mut out, CANVAS, &Pic { size: [16, 16], rgba: &map, stride: 4, gain: WORLD_GAIN }, win, facing, &[], false);
        out
    }

    /// The brightest texel in a square about a point of the picture.
    fn brightest(out: &[u8], at: [f32; 2], reach: f32) -> u8 {
        let (mid, r) = (CANVAS as f32, (reach * CANVAS as f32) as i64);
        let (cx, cy) = ((at[0] * mid) as i64, (at[1] * mid) as i64);
        let mut best = 0;
        for y in (cy - r).max(0)..(cy + r).min(CANVAS as i64) {
            for x in (cx - r).max(0)..(cx + r).min(CANVAS as i64) {
                best = best.max(out[((y as u32 * CANVAS + x as u32) * 4) as usize]);
            }
        }
        best
    }

    /// The texel of the mark furthest from the middle of the picture: its point.
    fn point_of_mark(out: &[u8]) -> [f32; 2] {
        let mid = CANVAS as f32 / 2.0;
        let mut best = ([0.0, 0.0], -1.0);
        for y in 0..CANVAS {
            for x in 0..CANVAS {
                if out[((y * CANVAS + x) * 4) as usize] != MARK {
                    continue;
                }
                let (dx, dy) = (x as f32 + 0.5 - mid, y as f32 + 0.5 - mid);
                let d = dx * dx + dy * dy;
                if d > best.1 {
                    best = ([dx, dy], d);
                }
            }
        }
        assert!(best.1 >= 0.0, "no mark was drawn at all");
        best.0
    }

    #[test]
    fn the_players_mark_turns_with_them_on_a_picture_that_does_not() {
        // North is up the picture and east to its right, as a north-up map draws them.
        for facing in [NORTH, [1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.6, 0.8]] {
            let out = laid(facing);
            let p = point_of_mark(&out);
            let len = (p[0] * p[0] + p[1] * p[1]).sqrt();
            let dot = (p[0] * facing[0] + p[1] * facing[1]) / len;
            assert!(dot > 0.9, "facing {facing:?}: the mark points {p:?}");
            // And it reaches about as far as the mark is long, not across the map.
            let r = CANVAS as f32 * MARK_SHARE;
            assert!(len > r * 0.75 && len <= r + 1.5, "facing {facing:?}: it reaches {len} of {r}");
        }
    }

    #[test]
    fn the_compass_letters_stand_still_at_the_four_sides() {
        let [across, down] = letter_ring(CANVAS);
        let out = laid([1.0, 0.0]);
        for at in [[0.5, 0.5 - down], [0.5 + across, 0.5], [0.5, 0.5 + down], [0.5 - across, 0.5]] {
            assert!(brightest(&out, at, 0.03) >= LETTER, "no letter at {at:?}");
        }
        // Turning the player does not move them.
        let turned = laid(NORTH);
        for at in [[0.5, 0.5 - down], [0.5 + across, 0.5]] {
            assert!(brightest(&turned, at, 0.03) >= LETTER, "the letters moved with the player");
        }
    }

    /// Every one of the four letters is drawn whole inside the glass: the owner's device
    /// showed the S standing on the bottom edge, half of it under the Pip-Boy's own frame.
    #[test]
    fn no_compass_letter_runs_off_the_glass() {
        for side in [64u32, 128, CANVAS, 512] {
            let mut out = vec![0u8; (side * side * 4) as usize];
            let map = vec![0u8; 16 * 16];
            compose(&mut out, side, &Pic { size: [16, 16], rgba: &map, stride: 1, gain: 1.0 }, window([0.5; 2], NORTH, 0.1, 1.0), NORTH, &[], false);
            let (lo, hi) = ((GLASS_INSET * side as f32) as u32, ((1.0 - GLASS_INSET) * side as f32) as u32);
            for y in 0..side {
                for x in 0..side {
                    let inside = (lo..=hi).contains(&x) && (lo..=hi).contains(&y);
                    if !inside {
                        assert!(out[((y * side + x) * 4) as usize] < LETTER, "a letter's dot at {x},{y} of {side} is under the device's frame");
                    }
                }
            }
            // And they are still out at the sides, not bunched about the player's mark.
            let [across, down] = letter_ring(side);
            assert!(across > 0.2 && down > 0.2, "{side}: the ring collapsed to {across}, {down}");
        }
    }

    #[test]
    fn the_map_under_the_marks_is_the_window_of_the_picture() {
        // Two halves, light and dark: facing north, the player stands on the seam, so the
        // top half of the screen is the light one.
        let mut map = vec![0u8; 16 * 16 * 4];
        for y in 0..16 {
            for x in 0..16 {
                let v = if y < 8 { 200 } else { 20 };
                map[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        let win = window([0.5; 2], NORTH, 0.25, 1.0);
        compose(&mut out, CANVAS, &Pic { size: [16, 16], rgba: &map, stride: 4, gain: WORLD_GAIN }, win, NORTH, &[], false);
        // Clear of the compass letters and the mark in the middle.
        let top = out[((CANVAS / 4 * CANVAS + CANVAS / 4) * 4) as usize];
        let bottom = out[((CANVAS * 3 / 4 * CANVAS + CANVAS / 4) * 4) as usize];
        assert!(top > 60 && bottom < 30 && top > bottom * 3, "the window is upside down or off: {top} over {bottom}");
        // The map is drawn at its own strength, the marks over it at the screen's full.
        assert!(top < MARK / 2, "the map is not dimmed under the marks: {top}");
        assert!(brightest(&out, [0.5, 0.5], 0.05) == MARK, "the player's mark is not at full strength");
    }

    /// The ink is the Pip-Boy screens' own colour as the player set it, at full strength
    /// (the picture carries the dimming), so a green device draws a green map.
    #[test]
    fn the_ink_is_the_screens_own_colour() {
        let was = nvf_play::interface::pipboy_rgb();
        for rgb in [[255u8, 182, 66], [26, 255, 128]] {
            nvf_play::interface::set_pipboy_rgb(rgb);
            let ink = map_ink();
            let want = Color::srgb_u8(rgb[0], rgb[1], rgb[2]).to_linear();
            let [r, g, b] = SCREEN_TINT;
            assert!((ink.red - want.red * r).abs() < 1e-6 && (ink.green - want.green * g).abs() < 1e-6 && (ink.blue - want.blue * b).abs() < 1e-6, "{rgb:?}: {ink:?}");
        }
        nvf_play::interface::set_pipboy_rgb(was);
    }

    /// The local outline's picture is one byte a sample, laid out at full strength as the
    /// paused Local Map page draws the very same thing, and the marks go over it.
    #[test]
    fn the_local_outlines_own_texels_are_laid_out_as_they_are() {
        // Two halves, a faint floor and the bright edge the producer draws walls in.
        let (w, h) = (16usize, 16usize);
        let alpha: Vec<u8> = (0..w * h).map(|i| if i / w < 8 { 200 } else { 20 }).collect();
        let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        let win = window([0.5; 2], NORTH, 0.25, 1.0);
        compose(&mut out, CANVAS, &Pic { size: [w as u32, h as u32], rgba: &alpha, stride: 1, gain: 1.0 }, win, NORTH, &[], false);
        let top = out[((CANVAS / 4 * CANVAS + CANVAS / 4) * 4) as usize];
        let bottom = out[((CANVAS * 3 / 4 * CANVAS + CANVAS / 4) * 4) as usize];
        assert_eq!((top, bottom), (200, 20), "the samples are drawn as they are, north up");
        // Grey: the material's own ink gives it its colour.
        let i = ((CANVAS / 4 * CANVAS + CANVAS / 4) * 4) as usize;
        assert_eq!(&out[i..i + 4], &[200, 200, 200, 255]);
        assert_eq!(brightest(&out, [0.5, 0.5], 0.05), MARK, "the player's mark is still the brightest");
        // The same texels are the same base; another picture of the same bytes is not.
        let pic = |a: &[u8]| LocalPic { alpha: std::sync::Arc::new(a.to_vec()), size: [w as u32, h as u32] };
        let (a, b) = (pic(&alpha), pic(&alpha));
        assert!(same_base(Some(&a), Some(&a.clone())) && !same_base(Some(&a), Some(&b)));
        assert!(same_base(None, None) && !same_base(Some(&a), None));
    }

    /// A quest target inside the window is a mark where it stands; one outside is held at
    /// the glass's edge on its side, and nothing is ever drawn under the device's frame.
    #[test]
    fn the_marks_stand_where_they_are_and_are_held_at_the_edge_when_they_are_not() {
        let map = vec![0u8; 16 * 16];
        let pic = || Pic { size: [16, 16], rgba: &map, stride: 1, gain: 1.0 };
        // The window shows a quarter of the picture about the player: canvas 0…1 is
        // picture 0.25…0.75.
        let win = window([0.5; 2], NORTH, 0.25, 1.0);
        let laid = |marks: &[Mark]| {
            let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
            compose(&mut out, CANVAS, &pic(), win, NORTH, marks, false);
            out
        };
        // Inside: three quarters across and halfway down the canvas.
        let near = laid(&[Mark { kind: MarkKind::Quest, uv: [0.625, 0.5] }]);
        assert_eq!(brightest(&near, [0.75, 0.5], 0.01), MARK, "no mark where the target stands");
        assert!(brightest(&near, [0.25, 0.5], 0.01) < PIN, "something was drawn where nothing is");
        // Outside, far to the east: held at the right-hand edge of the glass, inside it.
        let far = laid(&[Mark { kind: MarkKind::Quest, uv: [0.95, 0.5] }]);
        let held = 1.0 - GLASS_INSET - PIN_SHARE;
        assert_eq!(brightest(&far, [held, 0.5], 0.02), MARK, "the chevron is not at the edge");
        for x in 0..CANVAS {
            for y in 0..CANVAS {
                let inside = ((GLASS_INSET * CANVAS as f32) as u32..=((1.0 - GLASS_INSET) * CANVAS as f32) as u32).contains(&x);
                if !inside {
                    assert!(far[((y * CANVAS + x) * 4) as usize] < PIN, "a mark's texel at {x},{y} is under the device's frame");
                }
            }
        }
        // A place is drawn a step under a quest target, and an unusable place is not drawn.
        let place = laid(&[Mark { kind: MarkKind::Place, uv: [0.625, 0.5] }]);
        assert_eq!(brightest(&place, [0.75, 0.5], 0.01), PIN);
        assert_eq!(laid(&[Mark { kind: MarkKind::Quest, uv: [f32::NAN, 0.5] }]), laid(&[]), "a place that is not a number draws nothing at all");
    }

    /// A font with no game data behind it: one white texel and a glyph for every code
    /// that covers it, so every letter is a solid block and a test can see where the
    /// letters fall. The real one is the game's own ([`page_font`]).
    fn test_face() -> PageFont {
        let glyph = fnv_font::Glyph { uv: [0.0, 0.0, 1.0, 1.0], width: 10.0, height: 14.0, before: 1.0, after: 1.0, ascent: 14.0 };
        let font = fnv_font::Font { size: 20.0, texture: String::new(), glyphs: vec![glyph; 256], extra: Default::default() };
        PageFont { font, size: [2, 2], ink: vec![255; 4] }
    }

    /// The brightest texel in a box of the picture.
    fn max_in(out: &[u8], x0: u32, y0: u32, x1: u32, y1: u32) -> u8 {
        let mut best = 0;
        for y in y0..y1.min(CANVAS) {
            for x in x0..x1.min(CANVAS) {
                best = best.max(out[((y * CANVAS + x) * 4) as usize]);
            }
        }
        best
    }

    fn page(rows: &[Row], face: Option<&PageFont>, flicker: Option<f32>) -> Vec<u8> {
        let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        compose_page(&mut out, CANVAS, rows, face, flicker);
        out
    }

    /// A page of letters is set in the game's own font, inside the glass, with the
    /// heading over its rule, the label at the left and the reading at the right.
    #[test]
    fn a_page_is_set_in_the_games_own_font_inside_the_glass() {
        let face = test_face();
        let rows = [Row::Head("STATUS".into()), Row::Pair("HP".into(), "120/150".into())];
        let out = page(&rows, Some(&face), None);
        let (lo, hi) = ((GLASS_INSET * CANVAS as f32) as u32, ((1.0 - GLASS_INSET) * CANVAS as f32) as u32);
        // Nothing at all under the device's own frame.
        for y in 0..CANVAS {
            for x in 0..CANVAS {
                if !(lo..=hi).contains(&x) || !(lo..=hi).contains(&y) {
                    assert!(out[((y * CANVAS + x) * 4) as usize] <= PAGE_BACK, "a page's ink at {x},{y} is under the device's frame");
                }
            }
        }
        // The heading at the top left, its rule under it, the label below that and the
        // reading over at the right.
        assert_eq!(max_in(&out, lo, lo, lo + 60, lo + 24), PAGE_HEAD, "no heading");
        assert_eq!(max_in(&out, CANVAS / 2, lo + 20, CANVAS / 2 + 4, lo + 32), PAGE_RULE, "no rule under the heading");
        assert_eq!(max_in(&out, lo, lo + 30, lo + 30, lo + 56), PAGE_LABEL, "no label");
        assert_eq!(max_in(&out, hi - 60, lo + 30, hi, lo + 56), PAGE_VALUE, "no reading at the right");
        // The ground between the rows is the screen's own dark, not black.
        assert_eq!(out[((CANVAS - 4) * CANVAS * 4) as usize], PAGE_BACK);
    }

    /// A bar fills by its share, with the stock dim ground beyond it.
    #[test]
    fn a_pages_bar_fills_by_its_share() {
        let face = test_face();
        for share in [0.0, 0.25, 0.5, 1.0] {
            let out = page(&[Row::Bar("HP".into(), share, "x".into())], Some(&face), None);
            let count = |ink: u8| out.chunks_exact(4).filter(|p| p[0] == ink).count();
            let (lit, dim) = (count(PAGE_BAR), count(PAGE_GROUND));
            assert!(lit + dim > 0, "{share}: no bar at all");
            let drawn = lit as f32 / (lit + dim) as f32;
            assert!((drawn - share).abs() < 0.05, "{share}: the bar is {drawn} full");
        }
        // A share that is not a number draws an empty bar rather than a broken one.
        let odd = page(&[Row::Bar("HP".into(), f32::NAN, "x".into())], Some(&face), None);
        assert_eq!(odd.chunks_exact(4).filter(|p| p[0] == PAGE_BAR).count(), 0);
    }

    /// Without the game's font the page still draws its rules and bars, so a run with no
    /// data shows a page rather than a blank screen.
    #[test]
    fn a_page_without_the_games_font_still_draws_its_rules_and_bars() {
        let out = page(&[Row::Head("STATUS".into()), Row::Bar("HP".into(), 1.0, "x".into())], None, None);
        assert!(out.chunks_exact(4).any(|p| p[0] == PAGE_RULE));
        assert!(out.chunks_exact(4).any(|p| p[0] == PAGE_BAR));
    }

    /// The flip's flicker dims the glass and runs a bright scan down it, in a bounded
    /// number of steps so the picture is not laid out afresh every frame.
    #[test]
    fn the_flip_flickers_in_a_bounded_number_of_steps() {
        assert_eq!(flick_step(None), u32::MAX);
        assert_eq!(flick_step(Some(0.0)), 0);
        assert_eq!(flick_step(Some(1.0)), FLICK_STEPS as u32);
        assert_eq!(flick_step(Some(f32::NAN)), 0);
        let face = test_face();
        let rows = [Row::Head("STATUS".into())];
        let still = page(&rows, Some(&face), None);
        let flicking = page(&rows, Some(&face), Some(0.0));
        assert!(max_in(&flicking, 0, 0, CANVAS, CANVAS) >= 150, "the scan is not bright");
        let sum = |out: &[u8]| out.chunks_exact(4).map(|p| u32::from(p[0])).sum::<u32>();
        assert!(sum(&flicking) < sum(&still), "the glass does not dim while it flickers");
        // And it is over by the end, leaving the page as it stands.
        assert_eq!(page(&rows, Some(&face), Some(1.0)).len(), still.len());
    }

    /// Heading up: the map turns under the player, their own mark points up the glass and
    /// the compass letters ride round its edge.
    #[test]
    fn the_map_turns_with_the_player_and_the_letters_ride_round() {
        let map = grey();
        let laid_turning = |facing: [f32; 2]| {
            let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
            // The window the glance itself computes for a turning map: the facing up it.
            let win = window([0.5; 2], facing, 0.1, 1.0);
            compose(&mut out, CANVAS, &Pic { size: [16, 16], rgba: &map, stride: 4, gain: WORLD_GAIN }, win, facing, &[], true);
            out
        };
        let [across, down] = letter_ring(CANVAS);
        // Facing north: as a north-up picture, N at the top.
        let north = laid_turning(NORTH);
        assert!(brightest(&north, [0.5, 0.5 - down], 0.03) >= LETTER, "N is not at the top facing north");
        // Facing east: north now lies to the player's left, so the N rides to the left.
        let east = laid_turning([1.0, 0.0]);
        assert!(brightest(&east, [0.5 - across, 0.5], 0.03) >= LETTER, "N did not ride round facing east");
        // Facing north east no letter stands at a side at all: they ride between them.
        let diagonal = laid_turning([std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2]);
        assert!(brightest(&diagonal, [0.5, 0.5 - down], 0.03) < LETTER, "a letter stayed at the top");
        let d = std::f32::consts::FRAC_1_SQRT_2;
        assert!(brightest(&diagonal, [0.5 - across * d, 0.5 - down * d], 0.03) >= LETTER, "N is not half way round");
        // And the player's own mark points straight up the glass whichever way they face.
        for facing in [NORTH, [1.0, 0.0], [0.0, 1.0], [-0.6, 0.8]] {
            let p = point_of_mark(&laid_turning(facing));
            assert!(p[1] < 0.0 && p[0].abs() < 1.5, "facing {facing:?}: the mark points {p:?}");
        }
    }

    /// A dark pad under every letter and every mark, so they read over the outline's own
    /// bright walls; nothing of the map is brightened by it.
    #[test]
    fn a_letter_and_a_mark_keep_a_dark_pad_under_them() {
        // A map drawn at its brightest everywhere, so only a pad can darken it.
        let map = vec![255u8; 16 * 16 * 4];
        let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        let win = window([0.5; 2], NORTH, 0.25, 1.0);
        let marks = [Mark { kind: MarkKind::Quest, uv: [0.625, 0.5] }];
        compose(&mut out, CANVAS, &Pic { size: [16, 16], rgba: &map, stride: 4, gain: 1.0 }, win, NORTH, &marks, false);
        let at = |x: f32, y: f32| out[(((y * CANVAS as f32) as u32 * CANVAS + (x * CANVAS as f32) as u32) * 4) as usize];
        // Clear of everything: the map at its own strength.
        assert_eq!(at(0.2, 0.8), 255);
        // Just outside the player's mark, and just outside the quest's: the pad.
        let dark = ((255.0 * KNOCKOUT) as u8).max(1);
        assert!(at(0.5, 0.5 + MARK_SHARE * 1.2) <= dark, "no pad under the player's mark");
        assert!(at(0.75 + PIN_SHARE * 1.2, 0.5) <= dark, "no pad under the quest's mark");
        // Under a compass letter too.
        let [_, down] = letter_ring(CANVAS);
        assert!(at(0.5, 0.5 - down) <= dark || brightest(&out, [0.5, 0.5 - down], 0.01) == LETTER);
    }

    /// Every system this file adds, in one schedule: a parameter that holds what another
    /// holds (Bevy's B0002) panics here rather than on the first frame of a shipped build.
    /// **Every system this file adds belongs in this list.**
    #[test]
    fn the_screens_systems_have_no_conflicting_parameters() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems((wear_map, feed, hide_lit).chain());
        schedule.initialize(&mut world).expect("the schedule builds");
    }

    /// The regression the behaviour notes call for: whatever the live view wears, the
    /// screen is asked for only while the arm is held, so a yield gives the shape, the
    /// lit copy and the glow straight back to `pipboy_arm::show_screen`.
    #[test]
    fn nothing_is_worn_or_hidden_once_the_arm_is_let_go() {
        let shown = nvf_play::wrist_arm::Shown {
            key: nvf_play::wrist_live::Key { session: 1, world: 0x3C, screen: 7 },
            texture: "map.dds".to_string(),
            uv: [0.5; 2],
            forward: NORTH,
            half: nvf_play::wrist_live::HALF_WINDOW,
            local: None,
            marks: Vec::new(),
        };
        let held = nvf_play::wrist_arm::WristLive::held_for_test(true, Some(shown.clone()));
        let let_go = nvf_play::wrist_arm::WristLive::held_for_test(false, Some(shown));
        assert!(held.wants().is_some() && held.arm_clip_time().is_some());
        assert!(let_go.wants().is_none() && let_go.arm_clip_time().is_none());
    }
}
