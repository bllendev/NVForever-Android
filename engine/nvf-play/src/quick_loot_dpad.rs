//! The Quick Loot list scrolled with the D-pad, on a controller (the fork's own file;
//! docs/fork.md's fork-owned module rule).
//!
//! On a controller the list could only be scrolled with the right stick, which is the
//! look: picking the second item meant moving the view off the container. The D-pad's Up
//! and Down move the selection instead, one row per press, while a list with rows shows.
//!
//! The D-pad is already bound to hotkeys, so the list takes those two buttons only while
//! it has rows to move through ([`owns`]), and only the pad's own presses: a control's key
//! or mouse button still works, for the Keyboard and Mouse With a Controller tweak
//! (`bindings::ControlInput::just_pressed_with_pad`). The weapon wheel asks [`binds`] for
//! any slot sitting on the vertical D-pad, so a hotkey on Up or Down is not fired by a
//! scroll; the answer it reads is the previous frame's, as Ready Item's already is.
//!
//! The hooks this file is reached from: `quick_loot.rs` (the `owns_vertical_dpad` field and
//! three lines of [`follow`](crate::quick_loot::follow)) and `weapon_wheel.rs` (one line).

use crate::bindings::{Control, ControlInput};
use crate::pad::PadInput;
use bevy::input::gamepad::GamepadButton;

/// Whether the D-pad's Up or Down is this control's controller binding: the list has those
/// while it moves its selection with them ([`owns`]).
pub fn binds(input: &ControlInput, c: Control) -> bool {
    matches!(input.bindings.pad(c), Some(PadInput::Button(GamepadButton::DPadUp | GamepadButton::DPadDown)))
}

/// Whether the list has the vertical D-pad this frame: a controller works the controls,
/// nothing is suppressing them, and the list shown has rows to move through.
pub fn owns(input: &ControlInput, rows: bool) -> bool {
    input.controller() && !input.suppressed() && rows
}

/// The D-pad's Up and Down as this frame's presses, `(false, false)` unless the list has
/// them ([`owns`]).
pub fn pressed(input: &ControlInput, owns: bool) -> (bool, bool) {
    let down = |b| owns && input.pads.iter().any(|p| p.just_pressed(b));
    (down(GamepadButton::DPadUp), down(GamepadButton::DPadDown))
}

/// How many rows the selection moves this frame: the mouse wheel first (it is the list's
/// whatever the D-pad does, and up the list is up), else one row per D-pad press. Down is
/// positive, as `menu::model::Command::Scroll` counts it.
pub fn steps(notches: f32, up: bool, down: bool) -> i32 {
    if notches > 0.0 {
        return -1;
    }
    if notches < 0.0 {
        return 1;
    }
    i32::from(down) - i32::from(up)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wheel wins over the D-pad, up the list is up, and the two buttons at once move
    /// nothing.
    #[test]
    fn the_wheel_comes_first_and_one_press_is_one_row() {
        assert_eq!(steps(1.0, false, false), -1, "the wheel rolled up moves the selection up");
        assert_eq!(steps(-1.0, false, false), 1);
        // A wheel notch is the list's whatever the D-pad says.
        assert_eq!(steps(1.0, false, true), -1);
        assert_eq!(steps(-1.0, true, false), 1);
        // No notch: one row per press, down positive.
        assert_eq!(steps(0.0, false, true), 1);
        assert_eq!(steps(0.0, true, false), -1);
        assert_eq!(steps(0.0, false, false), 0);
        assert_eq!(steps(0.0, true, true), 0, "both at once moves nothing");
    }
}
