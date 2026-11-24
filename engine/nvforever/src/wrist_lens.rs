//! Where the live wrist map's glance holds the first-person arm, and the frame it holds it
//! in: the fork's own (docs/fork.md), so a sync with upstream NVForever never has to merge
//! it into `player_body.rs`. That file keeps the thin hooks alone: it passes
//! [`view_frame`]'s frame and [`lens_at`]'s offsets to its own `place_lens`.

use bevy::prelude::*;

/// The field of view the arm is drawn with for the live wrist map's glance
/// (`crate::odin::wrist_screen`), degrees as `crate::player_body::PIPBOY_FOV` counts them.
/// The menus' 47 fills the picture with the device, which is what a menu wants; a glance
/// keeps the road ahead in view, so the whole Pip-Boy sits in the lower left instead. The
/// lens draws the rig across the view about the eye, so a narrow arms view throws what is
/// off the eye's axis off the picture as well: at the menus' 47 the hand fills the screen
/// and the device is past the left edge, at 85 the device is still cut by it, at 110 it is
/// whole and the road ahead stays clear [design: fitted on the desktop against the vanilla
/// Pip-Boy 3000 at Goodsprings, 2026-11-21, spikes/001-live-wrist-world].
pub(crate) const WRIST_FOV: f32 = 110.0;

/// How far below the eye the glance holds the arm, game units: a menu puts the device in
/// the middle of the view, a glance keeps it under the road ahead [design: fitted on the
/// desktop at Goodsprings, 2026-11-21, spikes/001-live-wrist-world]. `NVF_WRIST_DROP`
/// overrides it while it is being fitted.
const WRIST_DROP: f32 = 3.0;

pub(crate) fn wrist_drop() -> f32 {
    std::env::var("NVF_WRIST_DROP").ok().and_then(|v| v.parse().ok()).filter(|v: &f32| v.is_finite()).unwrap_or(WRIST_DROP)
}

/// How far left of the eye the glance holds the arm, game units, at the Glance position
/// row's Lower left (`nvf_play::wrist_arm::Position`, the default): the owner's first
/// Odin photo had the device in the middle right of a 16:9 picture, over the way ahead,
/// where the reference framing keeps it low on the left with the world clear beside it
/// [design: the owner, 2026-11-21]. Centre is 0. `NVF_WRIST_SIDE` overrides it while it
/// is being fitted. Fitted on the desktop at Goodsprings, 1920 x 1080: at 26 the device
/// ran off the left edge, at 14 it sits whole in the lower left with the road ahead clear.
const WRIST_SIDE: f32 = 14.0;

pub(crate) fn wrist_side() -> f32 {
    std::env::var("NVF_WRIST_SIDE").ok().and_then(|v| v.parse().ok()).filter(|v: &f32| v.is_finite()).unwrap_or(WRIST_SIDE)
}

/// The view's own frame in the rig's space (game axes: x east, y north, z up), looking
/// along its +Y with +Z up, the heading turned about the world's up and then the pitch:
/// exactly the frame the camera is given (`nvf_play::space::look_rotation`, the same two
/// angles in Bevy's axes), so the arms' frame and the view's are one thing.
pub(crate) fn view_frame(heading: f32, pitch: f32) -> Quat {
    Quat::from_rotation_z(-heading) * Quat::from_rotation_x(pitch)
}

/// Where the lens node stands: at the eye, moved down the world by `drop` and along the
/// view's own right by `side` (`crate::player_body::place_lens`).
///
/// The frame must be the view's own. The arms were placed in the shortest turn from +Y to
/// the look direction (`Quat::from_rotation_arc`) until 2026-11-22, which points the same
/// way but rolls about the look by an angle that grows with the heading and the pitch, and
/// which has no answer at all when the view faces -Y (an arbitrary perpendicular axis is
/// chosen there). The scale across the view cannot tell (`place_lens`'s `diag(k, 1, k)` is
/// the same at any roll about +Y) and `drop` is along the world's up, so ordinary play and
/// the menus' raise never showed it; the glance's `side`, the one offset in the frame's own
/// axes, rode the roll: turning a full circle swung the device round the picture and
/// snapped it as the view passed south [observed: the owner's Odin clip of build map.14,
/// where the Pip-Boy drifts across the glass through a 360 and leaves the frame].
pub(crate) fn lens_at(eye: Vec3, view: Quat, drop: f32, side: f32) -> Vec3 {
    eye - Vec3::Z * drop + view * Vec3::X * side
}

/// How far down and to which side the glance holds the arm at a raise of `glance` (0 to 1),
/// `centred` as the Glance position row's Centre asks: what `player_body`'s `place_lens`
/// is given.
pub(crate) fn offsets(glance: f32, centred: bool) -> (f32, f32) {
    (wrist_drop() * glance, if centred { 0.0 } else { -wrist_side() * glance })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The arms' frame is the view's own: it looks where the view looks and its right is
    /// the view's right, at every heading and pitch, and it turns smoothly all the way
    /// round. (`Quat::from_rotation_arc`, which placed them until 2026-11-22, points the
    /// same way but rolls about the look; south, where the view faces -Y, it has no answer
    /// at all.)
    #[test]
    fn the_arms_frame_is_the_views_own_the_whole_way_round() {
        let turns = (0..=72).map(|i| i as f32 * std::f32::consts::TAU / 72.0);
        for pitch in [-1.2, -0.35, 0.0, 0.35, 1.2] {
            for heading in turns.clone() {
                let view = view_frame(heading, pitch);
                // It looks where the view looks.
                let look = Vec3::from_array(nvf_play::space::look_dir(heading, pitch));
                assert!((view * Vec3::Y).distance(look) < 1e-5, "{heading} {pitch}: {} against {look}", view * Vec3::Y);
                // Its right is the heading's right, level with the world whatever the pitch:
                // what the camera itself calls right (`space::look_rotation`).
                let right = Vec3::new(heading.cos(), -heading.sin(), 0.0);
                assert!((view * Vec3::X).distance(right) < 1e-5, "{heading} {pitch}: {} against {right}", view * Vec3::X);
            }
        }
    }

    /// The glance's device sits in the same place in the picture at every heading: its
    /// drop and its side offset are in the view's own frame, so a full turn moves it not at
    /// all and nothing jumps as the view passes south [the owner's Odin clip, map.14].
    #[test]
    fn the_glances_device_keeps_its_place_in_the_picture_through_a_full_turn() {
        let eye = Vec3::new(120.0, -40.0, 96.0);
        let (drop, side) = (WRIST_DROP, -WRIST_SIDE);
        for pitch in [-0.6, 0.0, 0.6] {
            let mut was: Option<Vec3> = None;
            for i in 0..=360 {
                let heading = i as f32 * std::f32::consts::TAU / 360.0;
                let view = view_frame(heading, pitch);
                // Where the lens node stands, read in the view's own frame: the same offset
                // from the eye at every heading.
                let seen = view.inverse() * (lens_at(eye, view, drop, side) - eye);
                let want = view.inverse() * (-Vec3::Z * drop) + Vec3::X * side;
                assert!(seen.distance(want) < 1e-4, "{heading}: {seen} against {want}");
                if let Some(was) = was {
                    // And it never jumps: a degree of turn moves it a hair at the most.
                    assert!(seen.distance(was) < 1e-3, "{heading}: it jumped from {was} to {seen}");
                }
                was = Some(seen);
            }
        }
        // With no glance up (the menus' raise, ordinary play) the node is the eye itself,
        // whatever the frame: nothing of this touches them.
        for heading in [0.0, 1.0, std::f32::consts::PI] {
            assert_eq!(lens_at(eye, view_frame(heading, 0.2), 0.0, 0.0), eye);
        }
    }

    /// The raise eases the offsets in and out, Centre keeps the device in the middle of
    /// the picture, and a glance that is down moves nothing at all.
    #[test]
    fn the_raise_eases_the_offsets_and_centre_keeps_the_side() {
        assert_eq!(offsets(0.0, false), (0.0, 0.0));
        let (drop, side) = offsets(1.0, false);
        assert!(drop > 0.0 && side < 0.0, "lower left: {drop} {side}");
        let (half_drop, half_side) = offsets(0.5, false);
        assert!((half_drop - drop / 2.0).abs() < 1e-6 && (half_side - side / 2.0).abs() < 1e-6);
        assert_eq!(offsets(1.0, true), (drop, 0.0), "Centre drops the arm but keeps it in the middle");
    }
}
