//! Fading the first-person sway out, the fork's own (docs/fork.md): the live wrist map's
//! glance holds the arm rigidly to the view, so while it is up every sway, lag and bob of
//! `crate::view_sway` is exactly nothing.
//!
//! It lives here, not in `view_sway.rs`, so a sync with upstream NVForever never has to
//! merge this into that module: `view_sway` is upstream's file and the glance is ours.

use crate::view_sway::SwayPose;
use bevy::prelude::*;

/// The pose `p` faded out by `share` (0: as it stands, 1: nothing at all), turn and move
/// together.
///
/// What holds the arms rigidly to the view holds them *rigidly*: while the live wrist
/// map's glance has the arm up ([`nvf_play::wrist_arm`]), every sway, lag and bob of
/// `crate::view_sway` is exactly nothing, so a turn can neither leave the device behind nor
/// let it catch up afterwards. The share is the glance's own raise
/// ([`crate::player_body::pipboy_ease`]), so it eases in as the arm comes up and out as it
/// goes down rather than stepping: the sway's own fade (`view_sway::FADE_SECONDS`) leaves
/// thousandths of itself behind for about a second, which is a hair of drift where the
/// owner asked for none [design: the owner's Odin feedback on map.14, 2026-11-22].
pub fn faded(p: SwayPose, share: f32) -> SwayPose {
    let keep = 1.0 - if share.is_finite() { share.clamp(0.0, 1.0) } else { 0.0 };
    if keep >= 1.0 {
        return p;
    }
    if keep <= 0.0 {
        return SwayPose::NONE;
    }
    SwayPose { rotation: Quat::IDENTITY.slerp(p.rotation, keep), offset: p.offset * keep }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_sway::{heft, SwayInput, ViewSway};

    /// A busy frame of walking, turning and firing: enough of a sway to fade.
    fn busy(f: usize) -> SwayInput {
        let t = f as f32 / 60.0;
        SwayInput {
            dt: 1.0 / 60.0,
            heading: 2.0 * (t * 1.3).sin(),
            pitch: 0.6 * (t * 0.9).sin(),
            velocity: Vec2::new(150.0 * (t * 0.7).sin(), 300.0),
            phase: (t / 0.72).fract(),
            drawn: 1.0,
            kick: if f.is_multiple_of(9) { 1.2 } else { 0.0 },
            exertion: 0.8,
            heft: heft(12.0),
            on: true,
            ..SwayInput::default()
        }
    }

    /// The glance holds the arm rigidly: with its raise up, the sway is exactly nothing,
    /// however busy the player is; it eases there and back rather than stepping, and with
    /// the glance down nothing of the pose is touched at all.
    #[test]
    fn the_glances_raise_fades_the_sway_to_exactly_nothing() {
        let mut s = ViewSway::default();
        let mut most = 0.0f32;
        for f in 0..600 {
            let p = s.update(busy(f));
            most = most.max(p.offset.length() + p.rotation.angle_between(Quat::IDENTITY));
            // Up: nothing at all, turn or move.
            assert_eq!(faded(p, 1.0), SwayPose::NONE, "frame {f}");
            // Down: the arms are their own again, to the last bit.
            assert_eq!(faded(p, 0.0), p, "frame {f}");
        }
        assert!(most > 1.0, "the sway it is fading: {most}");
        // Part-way up: between the two, and never more than the sway itself.
        let p = s.update(busy(600));
        let size = |q: SwayPose| q.offset.length() + q.rotation.angle_between(Quat::IDENTITY);
        let shares: Vec<f32> = (0..=10).map(|i| size(faded(p, i as f32 / 10.0))).collect();
        assert!(shares.windows(2).all(|w| w[1] <= w[0] + 1e-6), "it only ever fades: {shares:?}");
        assert!(shares[5] > 0.0 && shares[5] < size(p), "half way is half way: {shares:?}");
        // A share that is not a number leaves the pose alone rather than losing the arms.
        assert_eq!(faded(p, f32::NAN), p);
    }
}
