//! The device's gyro while a physical controller is in use (docs/fork.md).
//!
//! Upstream's gyro aiming is a **touch** feature: `nvforever/src/touch/gyro.rs` admits sensor
//! motion only while the saved input device is Keyboard and Mouse, and activation is a finger
//! held on the Gyro touch target. `engine/nvf-play/src/player.rs` gates the look mix the same
//! way. Both are deliberate (docs/design/android-gyro-aiming.md: "controller selection ...
//! cancel[s] activation and drain[s] motion"), so that a pad and a finger can never fight over
//! the view.
//!
//! On a handheld with a built-in controller that means the gyro does nothing at all: the
//! device is a Controller the moment the player touches a stick, which cancels the gyro, and
//! there is no finger free to hold a touch target anyway. The owner reported exactly that of
//! `android-v0.8.1-test.5` on an AYN Odin 3: "gyro doesn't work".
//!
//! So this adds one row of its own, **Gyro with a controller**, with three values:
//!
//! | value | what the gyro does |
//! | --- | --- |
//! | `Off` | nothing, which is upstream's behaviour and the default |
//! | `While aiming` | adds to the look only while the engine has the sights actually up, which on a controller is the aim trigger |
//! | `Always` | adds to the look whenever the player is playing |
//!
//! Everything else is upstream's and is reused as it is: the Java sensor listener and its
//! bounds ([`AndroidGyro`], `GyroMotion`), the sample decoding, the per-frame radians clamp,
//! and the **Gyro sensitivity**, **Gyro ADS scale**, **Invert gyro X** and **Invert gyro Y**
//! rows. The listener is registered by upstream's own `poll`, which asks Java for `"on"`
//! whenever the engine wants motion; this row is what makes it want motion with a pad in hand,
//! so no change to the Java side is needed.
//!
//! `Gyro only while aiming` and the Gyro touch target keep deciding the **touch** route alone:
//! with a controller selected, this row is the whole decision.
//!
//! [`AndroidGyro`]: https://developer.android.com/develop/sensors-and-location/sensors/sensors_position

use crate::bindings::InputDevice;
use crate::settings::{self, ChoiceList, Setting};
use bevy::prelude::*;

/// The stored row (`crates/nvf-settings-layout/src/lib.rs`, docs/settings-reference-android.md).
pub const KEY: &str = "odin-controller-gyro-v1";

const NOTE: &str = "Adds the device gyro to the look while a physical controller is in use. While aiming applies it only with the sights up. Gyro sensitivity, ADS scale and inversion apply as for touch.";

/// When the gyro may add to the look with a controller in hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    /// Upstream's behaviour: a selected controller cancels the gyro.
    #[default]
    Off,
    /// Only while `AimZoom` reports the sights actually up (the aim trigger).
    WhileAiming,
    /// Whenever the player is playing.
    Always,
}

impl Mode {
    /// The values as the settings page lists them, in this order.
    pub const NAMES: [&'static str; 3] = ["Off", "While aiming", "Always"];

    /// This value's name, which is also what the settings file keeps.
    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// The value of this name, or `None` when it is not one of [`Mode::NAMES`].
    pub fn of_name(name: &str) -> Option<Self> {
        [Self::Off, Self::WhileAiming, Self::Always]
            .into_iter()
            .find(|mode| mode.name() == name)
    }
}

/// The row's value, read by the two hooks below.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Options {
    pub mode: Mode,
}

/// Whether this row is in charge of the gyro right now, and with which value: a physical
/// controller is the selected device and the row is not `Off`. `None` for the device is a
/// world with no [`crate::bindings::Bindings`] yet, which is nobody's gyro.
pub fn on_pad(options: Option<&Options>, device: Option<InputDevice>) -> OnPad {
    let mode = options.map_or(Mode::Off, |o| o.mode);
    OnPad((device == Some(InputDevice::Controller) && mode != Mode::Off).then_some(mode))
}

/// What [`on_pad`] found, so a caller asks once a frame and reads it twice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OnPad(Option<Mode>);

impl OnPad {
    /// Whether this row, rather than the Gyro touch target, decides the gyro now.
    pub fn engaged(self) -> bool {
        self.0.is_some()
    }

    /// Whether the gyro should be asked for motion, or `None` when upstream's touch
    /// activation is still the one to ask.
    pub fn wanted(self, aimed: bool) -> Option<bool> {
        self.0.map(|mode| mode == Mode::Always || aimed)
    }
}

/// The hook in `player.rs`: which saved input device may mix sensor radians into the look.
/// Keyboard and Mouse is upstream's answer; a controller is this row's.
pub fn look_allows(options: Option<&Options>, device: InputDevice) -> bool {
    device == InputDevice::KeyboardMouse || on_pad(options, Some(device)).engaged()
}

/// Registers the row, beside upstream's gyro rows (`touch::gyro::install`).
pub fn install(app: &mut App) {
    app.init_resource::<Options>();
    settings::add_choice_list(
        app,
        Setting {
            key: KEY,
            group: "Controller",
            label: "Gyro with a controller",
            note: NOTE,
            switches: "",
            get: |_| false,
            set: |_, _| {},
            slider: None,
        },
        ChoiceList {
            names: |_| Mode::NAMES.iter().map(|name| (*name).to_string()).collect(),
            chosen: |w| w.resource::<Options>().mode.name().to_string(),
            save: |w| w.resource::<Options>().mode.name().to_string(),
            load: take,
            pick: take,
            ask_keep: false,
        },
    );
}

fn take(world: &mut World, name: &str) -> bool {
    match Mode::of_name(name) {
        Some(mode) => {
            world.resource_mut::<Options>().mode = mode;
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Off is upstream's behaviour, and the default, so a build that gains this row plays
    /// exactly as the one before it until the player chooses otherwise.
    #[test]
    fn off_is_the_default_and_leaves_a_controller_exactly_as_upstream_has_it() {
        let off = Options::default();
        assert_eq!(off.mode, Mode::Off);
        assert!(!on_pad(Some(&off), Some(InputDevice::Controller)).engaged());
        assert!(!on_pad(None, Some(InputDevice::Controller)).engaged());
        assert_eq!(on_pad(Some(&off), Some(InputDevice::Controller)).wanted(true), None);
        // Keyboard and Mouse is upstream's own route, whatever this row says.
        for mode in [Mode::Off, Mode::WhileAiming, Mode::Always] {
            let options = Options { mode };
            assert!(look_allows(Some(&options), InputDevice::KeyboardMouse));
            assert!(!on_pad(Some(&options), Some(InputDevice::KeyboardMouse)).engaged());
        }
    }

    /// With a controller selected: While aiming follows the sights, Always does not, and
    /// neither answers for a keyboard and mouse.
    #[test]
    fn the_chosen_value_decides_only_while_a_controller_is_the_device() {
        let aiming = Options { mode: Mode::WhileAiming };
        let always = Options { mode: Mode::Always };
        for (options, aimed, want) in [
            (&aiming, false, Some(false)),
            (&aiming, true, Some(true)),
            (&always, false, Some(true)),
            (&always, true, Some(true)),
        ] {
            let pad = on_pad(Some(options), Some(InputDevice::Controller));
            assert!(pad.engaged());
            assert_eq!(pad.wanted(aimed), want, "{options:?} aimed {aimed}");
            assert!(look_allows(Some(options), InputDevice::Controller));
            // A world with no bindings is nobody's gyro.
            assert_eq!(on_pad(Some(options), None).wanted(aimed), None);
        }
    }

    /// The names the page lists are the names the settings file keeps, and nothing else is
    /// taken: a file edited by hand cannot leave the row on a value that does not exist.
    #[test]
    fn every_listed_name_round_trips_and_nothing_else_is_taken() {
        for mode in [Mode::Off, Mode::WhileAiming, Mode::Always] {
            assert_eq!(Mode::of_name(mode.name()), Some(mode));
        }
        for bad in ["", "off", "ON", "While Aiming", "always", "LT"] {
            assert_eq!(Mode::of_name(bad), None, "refuses {bad:?}");
        }
        assert!(NOTE.len() <= 200, "a row's note stays within 200 characters");
        assert!(!NOTE.contains('\u{2013}') && !NOTE.contains('\u{2014}'));
    }
}
