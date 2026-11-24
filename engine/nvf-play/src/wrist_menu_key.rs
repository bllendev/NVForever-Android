//! The live wrist map's claim on the Pip-Boy key, the fork's own (docs/fork.md).
//!
//! With no binding of its own the glance follows a *hold* of Menu Mode, which
//! `crate::flashlight::MenuKeyHold` already measures for the light, so the moment it
//! becomes the glance's is decided here and `flashlight.rs` keeps three thin hooks alone:
//! the `glance` field, `MenuKeyHold::step` reading it, and `menu_key` setting it from
//! [`takes_key`]. That keeps a sync with upstream NVForever off this module's logic and
//! its tests.
//!
//! A child module of `crate::flashlight`, so the hold's own seconds are in reach.

use super::MenuKeyHold;
use crate::bindings::{Control, ControlInput};

/// Whether a hold of the Pip-Boy key belongs to the glance this frame: the live wrist map
/// is on and has no key of its own. With its own key bound, that key asks for it and this
/// one is the Pip-Boy's alone, exactly as it always was.
pub(crate) fn takes_key(input: &ControlInput, wrist: Option<&crate::wrist_arm::WristLive>) -> bool {
    wrist.is_some_and(|w| w.live()) && !input.bound(Control::GlancePipBoy)
}

impl MenuKeyHold {
    /// The key has been held past the glance's moment: the live wrist map takes the hold
    /// while it is on, and the release then opens nothing (`crate::wrist_arm`). A shorter
    /// tap is untouched, so the Pip-Boy opens exactly as it always did.
    pub fn glancing(&self) -> bool {
        self.held.is_some_and(|t| t >= crate::wrist_arm::GLANCE_SECONDS)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{KeyOutcome, MenuKeyHold};

    /// Run the key down for `secs` in frames of `dt`, then up, with the glance on or off;
    /// every outcome but Nothing.
    fn hold_glancing(h: &mut MenuKeyHold, secs: f32, dt: f32, glance: bool) -> Vec<KeyOutcome> {
        h.glance = glance;
        let mut out = vec![h.step(true, true, dt, true)];
        let mut t = dt;
        while t < secs {
            out.push(h.step(true, false, dt, true));
            t += dt;
        }
        out.push(h.step(false, false, dt, true));
        out.retain(|o| *o != KeyOutcome::Nothing);
        out
    }

    /// The owner's rule: a tap opens the Pip-Boy exactly as before, whether the glance
    /// is on or off; only a hold past its moment belongs to the glance.
    #[test]
    fn a_tap_still_opens_the_pipboy_with_the_glance_on() {
        let dt = 1.0 / 60.0;
        let mut h = MenuKeyHold::default();
        assert_eq!(hold_glancing(&mut h, crate::wrist_arm::GLANCE_SECONDS - 0.05, dt, true), vec![KeyOutcome::OpenInventory]);
        // And the same key with the glance off is untouched: a tap opens, a long hold
        // still switches the light.
        let mut off = MenuKeyHold::default();
        assert_eq!(hold_glancing(&mut off, 0.1, dt, false), vec![KeyOutcome::OpenInventory]);
        assert_eq!(hold_glancing(&mut off, 3.0, dt, false), vec![KeyOutcome::ToggleLight]);
    }

    #[test]
    fn a_hold_belongs_to_the_glance_and_opens_nothing_when_let_go() {
        let dt = 1.0 / 60.0;
        let mut h = MenuKeyHold::default();
        // Held well past both moments: the light is not switched (it keeps its own key)
        // and the release opens no menu.
        assert_eq!(hold_glancing(&mut h, 3.0, dt, true), vec![]);
        // And the key says so while it is down, which is what the glance reads.
        assert!(!h.glancing());
        h.glance = true;
        h.step(true, true, dt, true);
        assert!(!h.glancing(), "a press is not a hold yet");
        let mut t = dt;
        while t < crate::wrist_arm::GLANCE_SECONDS + dt {
            h.step(true, false, dt, true);
            t += dt;
        }
        assert!(h.glancing(), "held past its moment, the glance has the key");
        h.step(false, false, dt, true);
        assert!(!h.glancing(), "let go, it has nothing");
    }
}
