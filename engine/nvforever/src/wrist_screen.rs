//! The live World Map on the arm's own screen shape (opt-in; the rules are nvf-play
//! `wrist_live`, the wiring nvf-play `wrist_arm`) [design:
//! spikes/001-live-wrist-world/README.md].
//!
//! The paused Pip-Boy draws its menus into a render target and `pipboy_arm::show_screen`
//! wears that picture on `pipboyscreen:0` through a copy of the shape's own material
//! (`pipboy_arm::menus_material`). This file does the same thing with a different
//! picture and no menus at all: a copy of the shape's own material whose base texture is
//! the map picture the load order already owns for the worldspace the player is in, and
//! whose UV transform is the window nvf-play's [`window`] computes — the player's place
//! in the middle of the screen, the way they face up it. No render target, no second
//! camera, no readback, and no shared NIF or texture asset is touched.
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
use nvf_play::interface::AMBER_RGB;
use nvf_play::pipboy::device_screen::SCREEN_TINT;
use nvf_play::pipboy::transition::ScreenLook;
use nvf_play::wrist_arm::{WristBody, WristLive};
use nvf_play::wrist_live::{window, Window, HALF_WINDOW};

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
    /// on the processor.
    source: Option<(String, [u32; 2], std::sync::Arc<Vec<u8>>)>,
    /// The picture the live view lays out and the screen wears.
    picture: Option<Handle<Image>>,
    /// What was last laid out, so the work is done only when the view moved.
    window: Option<(Window, [f32; 2])>,
    said: bool,
}

/// Up on a north-up map ([`window`]'s facing for a picture that does not turn).
const NORTH: [f32; 2] = [0.0, -1.0];

/// The ink the map picture is drawn in, as the paused page draws the very same picture:
/// the interface's amber at the map's own alpha over the black screen
/// (`pipboy::data::MAP_ALPHA`), through the screen's own gain (`SCREEN_TINT`, which is
/// near neutral — the menus' amber is in their drawing, not in the tint, so a picture laid
/// on the screen raw comes out grey). The player's own interface colour is not read yet.
fn map_ink() -> LinearRgba {
    let amber = Color::srgb_u8(AMBER_RGB[0], AMBER_RGB[1], AMBER_RGB[2]).to_linear();
    let ([r, g, b], a) = (SCREEN_TINT, nvf_play::pipboy::data::MAP_ALPHA / 255.0);
    LinearRgba::rgb(amber.red * r * a, amber.green * g * a, amber.blue * b * a)
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
    let want = wrist.as_deref().and_then(|w| w.wants());
    let screen = parts.iter().find(|(_, p, _)| **p == PipBoyPart::Screen).map(|(e, _, m)| (e, m.0.clone()));
    // Give the screen back first, so the frame a menu takes it never copies our picture.
    if let Some((e, own)) = worn.screen.clone() {
        let ours = worn.copy.as_ref().map(|c| c.0.clone());
        let still = screen.as_ref().is_some_and(|(s, m)| *s == e && ours.as_ref() == Some(m));
        let keep = want.is_some_and(|s| s.key.screen == e.to_bits()) && still;
        if !keep {
            if still {
                if let Ok((_, _, mut m)) = parts.get_mut(e) {
                    m.0 = own;
                }
            }
            (worn.screen, worn.said) = (None, false);
        }
    }
    let (Some(shown), Some((e, current))) = (want, screen) else { return };
    // The body's facts are a frame old: wait until they name the shape that is there now.
    if shown.key.screen != e.to_bits() {
        return;
    }
    let path = nvf_play::interface::own_texture(&shown.texture);
    if worn.source.as_ref().is_none_or(|(p, ..)| *p != path) {
        let Some(data) = data.as_deref() else { return };
        // The map's own texels, on the processor: the live picture is laid out from them
        // (`compose`), and the picture itself never goes to the GPU as it stands.
        worn.source = read_picture(&path, data).map(|(size, rgba)| (path.clone(), size, rgba));
        worn.window = None;
        if worn.source.is_none() {
            warn!("wrist: {path} could not be read for the live map");
            return;
        }
    }
    let Some((_, size, rgba)) = worn.source.clone() else { return };
    let picture = worn.picture.get_or_insert_with(|| images.add(blank())).clone();
    // The material the shape wears for itself: taken once, before the copy goes on.
    let own = worn.screen.as_ref().map_or(current, |(_, own)| own.clone());
    // North up, as the paused page's map is: the player's own mark turns instead
    // (`compose`), which is what a map on a wrist reads like.
    let win = window(shown.uv, NORTH, HALF_WINDOW, 1.0);
    if worn.window != Some((win, shown.forward)) {
        let mut out = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        compose(&mut out, CANVAS, &Pic { size, rgba: &rgba }, win, shown.forward);
        if let Some(mut live) = images.get_mut(&picture) {
            live.data = Some(out);
        }
        worn.window = Some((win, shown.forward));
    }
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
    if !worn.said {
        info!("wrist: the live World Map is on the arm's screen ({path})");
        worn.said = true;
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

/// Where the compass letters sit, as a share of the picture from its middle.
const LETTER_RING: f32 = 0.37;

/// A picture to read texels from (rows of RGBA, top row first).
pub struct Pic<'a> {
    pub size: [u32; 2],
    pub rgba: &'a [u8],
}

impl Pic<'_> {
    /// The texel at `uv`, clamped to the picture; black outside a usable picture.
    fn at(&self, uv: [f32; 2]) -> [u8; 4] {
        let (w, h) = (self.size[0].max(1), self.size[1].max(1));
        let x = ((uv[0] * w as f32) as i64).clamp(0, w as i64 - 1) as usize;
        let y = ((uv[1] * h as f32) as i64).clamp(0, h as i64 - 1) as usize;
        let i = (y * w as usize + x) * 4;
        self.rgba.get(i..i + 4).map_or([0, 0, 0, 255], |p| [p[0], p[1], p[2], p[3]])
    }
}

/// Lay out the screen's picture: the map under the window, then the player's mark and the
/// compass letters over it. North is up, so the letters stand still and the mark turns
/// ([`facing`] as [`nvf_play::wrist_arm::Shown`] gives it, map space with v southward).
/// The marks are drawn white: the screen's own ink colours them (`map_ink`).
pub fn compose(out: &mut [u8], side: u32, map: &Pic, win: Window, facing: [f32; 2]) {
    let side = side.max(1);
    for y in 0..side {
        for x in 0..side {
            let uv = [(x as f32 + 0.5) / side as f32, (y as f32 + 0.5) / side as f32];
            let texel = map.at(win.at(uv));
            let i = ((y * side + x) * 4) as usize;
            if let Some(p) = out.get_mut(i..i + 4) {
                p.copy_from_slice(&[texel[0], texel[1], texel[2], 255]);
            }
        }
    }
    draw_letters(out, side);
    draw_mark(out, side, facing);
}

/// The player's own mark: a triangle at the middle, its point the way they face.
fn draw_mark(out: &mut [u8], side: u32, facing: [f32; 2]) {
    let n = (facing[0] * facing[0] + facing[1] * facing[1]).sqrt();
    let [fx, fy] = if n.is_finite() && n > 1e-6 { [facing[0] / n, facing[1] / n] } else { NORTH };
    let mid = side as f32 / 2.0;
    let r = side as f32 * MARK_SHARE;
    // The point, and the two back corners a little either side of the tail.
    let point = [mid + fx * r, mid + fy * r];
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

/// The compass letters, at the four sides of a north-up picture.
fn draw_letters(out: &mut [u8], side: u32) {
    let mid = side as f32 / 2.0;
    let ring = side as f32 * LETTER_RING;
    for (glyph, at) in [(N_GLYPH, [mid, mid - ring]), (E_GLYPH, [mid + ring, mid]), (S_GLYPH, [mid, mid + ring]), (W_GLYPH, [mid - ring, mid])] {
        draw_glyph(out, side, glyph, at);
    }
}

/// Our own letters, five wide and seven tall, a row of bits each (the top row first and
/// the leftmost dot in the high bit) [design: this file; nothing is read from the game].
const N_GLYPH: [u8; 7] = [0b10001, 0b11001, 0b11001, 0b10101, 0b10011, 0b10011, 0b10001];
const E_GLYPH: [u8; 7] = [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111];
const S_GLYPH: [u8; 7] = [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110];
const W_GLYPH: [u8; 7] = [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001];

/// A letter, drawn about `at`, as many texels a dot as the picture can spare.
fn draw_glyph(out: &mut [u8], side: u32, glyph: [u8; 7], at: [f32; 2]) {
    let dot = (side / 64).max(1);
    let (w, h) = (5 * dot, 7 * dot);
    let x0 = at[0] as i64 - (w / 2) as i64;
    let y0 = at[1] as i64 - (h / 2) as i64;
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
    if wrist.as_deref().and_then(|w| w.wants()).is_none() {
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
        compose(&mut out, CANVAS, &Pic { size: [16, 16], rgba: &map }, win, facing);
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
        let out = laid([1.0, 0.0]);
        for at in [[0.5, 0.5 - LETTER_RING], [0.5 + LETTER_RING, 0.5], [0.5, 0.5 + LETTER_RING], [0.5 - LETTER_RING, 0.5]] {
            assert!(brightest(&out, at, 0.03) >= LETTER, "no letter at {at:?}");
        }
        // Turning the player does not move them.
        let turned = laid(NORTH);
        for at in [[0.5, 0.5 - LETTER_RING], [0.5 + LETTER_RING, 0.5]] {
            assert!(brightest(&turned, at, 0.03) >= LETTER, "the letters moved with the player");
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
        compose(&mut out, CANVAS, &Pic { size: [16, 16], rgba: &map }, win, NORTH);
        // Clear of the compass letters and the mark in the middle.
        let top = out[((CANVAS / 4 * CANVAS + CANVAS / 4) * 4) as usize];
        let bottom = out[((CANVAS * 3 / 4 * CANVAS + CANVAS / 4) * 4) as usize];
        assert!(top > 150 && bottom < 60, "the window is upside down or off: {top} over {bottom}");
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
        };
        let held = nvf_play::wrist_arm::WristLive::held_for_test(true, Some(shown.clone()));
        let let_go = nvf_play::wrist_arm::WristLive::held_for_test(false, Some(shown));
        assert!(held.wants().is_some() && held.arm_clip_time().is_some());
        assert!(let_go.wants().is_none() && let_go.arm_clip_time().is_none());
    }
}
