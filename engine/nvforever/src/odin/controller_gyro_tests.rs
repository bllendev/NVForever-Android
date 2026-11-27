//! The fork's **Gyro with a controller** row over upstream's own sensor route
//! (`nvf_play::odin::controller_gyro`, docs/fork.md).
//!
//! Upstream's scene and its synthetic rotations are reused as they are: the fake backend
//! answers `0.02, -0.01` radians while the engine asks for motion and the reference is
//! settled, so these check which asking happens, not the sensor arithmetic. The look route's
//! own device gate is a unit test beside the module (`look_allows`); the ECS route here is
//! `GyroLook`, which that gate reads.

use super::{
    tests::{activate, delta, scene},
    *,
};
use crate::odin::controller_gyro::{Mode, Options as PadOptions};
use nvf_play::bindings::Bindings;

/// A scene with a physical controller selected and the fork's row on `mode`, settled: three
/// frames, which is what upstream's own `activate` takes to pass "reset", then an unstable
/// "on", then the first fresh sample.
fn with_pad(mode: Mode) -> (App, Entity, Vec2) {
    let (mut app, window, point) = scene(false);
    app.world_mut().resource_mut::<Bindings>().device = InputDevice::Controller;
    app.world_mut().resource_mut::<PadOptions>().mode = mode;
    for _ in 0..3 {
        app.update();
    }
    (app, window, point)
}

/// Off is upstream's behaviour: a selected controller cancels the gyro, and holding the
/// touch target does not bring it back, because a controller owns the view.
#[test]
fn off_keeps_upstreams_rule_that_a_selected_controller_cancels_the_gyro() {
    let (mut app, window, point) = with_pad(Mode::Off);
    assert_eq!(delta(&app), Vec2::ZERO);
    activate(&mut app, window, point, false);
    assert_eq!(delta(&app), Vec2::ZERO, "a finger cannot override the controller");
    assert!(!app.world().resource::<State>().requested);
}

/// Always: the gyro adds to the look with no finger anywhere near the screen, which is the
/// whole point on a handheld whose two sticks are already in use.
#[test]
fn always_moves_the_look_with_a_controller_and_no_touch_at_all() {
    let (app, _, _) = with_pad(Mode::Always);
    assert_eq!(delta(&app), Vec2::new(0.02, -0.01));
    assert!(app.world().resource::<State>().requested);
    // Sensor motion is not a key, a mouse count or a game action.
    assert!(app
        .world()
        .resource::<ButtonInput<KeyCode>>()
        .get_pressed()
        .next()
        .is_none());
    assert!(app
        .world()
        .resource::<ButtonInput<MouseButton>>()
        .get_pressed()
        .next()
        .is_none());
    assert_eq!(
        app.world()
            .resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .delta,
        Vec2::ZERO
    );
}

/// While aiming follows the engine's actual sights, which the aim trigger raises, and the
/// motion it admits carries upstream's ADS scale (0.5x by default).
#[test]
fn while_aiming_follows_the_actual_sights_and_keeps_the_ads_scale() {
    let (mut app, _, _) = with_pad(Mode::WhileAiming);
    assert_eq!(delta(&app), Vec2::ZERO, "sights down: nothing");
    app.world_mut()
        .resource_mut::<nvf_play::aim_zoom::AimZoom>()
        .aimed = true;
    app.update();
    app.update();
    assert_eq!(delta(&app), Vec2::new(0.01, -0.005), "half gain while aiming");
    app.world_mut()
        .resource_mut::<nvf_play::aim_zoom::AimZoom>()
        .aimed = false;
    app.update();
    assert_eq!(delta(&app), Vec2::ZERO, "lowering the sights stops it again");
}

/// Upstream's `Gyro only while aiming` is the touch route's own gate and is not asked a
/// second time here: this row is the whole decision while a controller is the device.
#[test]
fn the_touch_only_while_aiming_row_does_not_also_gate_the_controller() {
    let (mut app, _, _) = with_pad(Mode::Always);
    app.world_mut().resource_mut::<Options>().ads_only = true;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(delta(&app), Vec2::new(0.02, -0.01));
}

/// Upstream's sensitivity and inversion rows are reused as they are, and changing this row
/// discards the reference exactly as changing one of theirs does.
#[test]
fn inversion_applies_and_changing_the_row_discards_the_reference() {
    let (mut app, _, _) = with_pad(Mode::Always);
    {
        let mut options = app.world_mut().resource_mut::<Options>();
        options.invert_x = true;
        options.invert_y = true;
    }
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(delta(&app), Vec2::new(-0.02, 0.01));

    app.world_mut().resource_mut::<PadOptions>().mode = Mode::Off;
    app.update();
    assert_eq!(delta(&app), Vec2::ZERO);
    app.world_mut().resource_mut::<PadOptions>().mode = Mode::Always;
    app.update();
    assert_eq!(delta(&app), Vec2::ZERO, "a fresh sensor reference is required");
    app.update();
    app.update();
    assert_eq!(delta(&app), Vec2::new(-0.02, 0.01));
}

/// Turning upstream's own Gyro aiming switch off turns this off too, which is what the
/// greyed-out note on the row says (`crates/nvf-settings-layout`, `NEEDS`).
#[test]
fn upstreams_gyro_switch_still_turns_the_whole_feature_off() {
    let (mut app, _, _) = with_pad(Mode::Always);
    app.world_mut().resource_mut::<Options>().enabled = false;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(delta(&app), Vec2::ZERO);
    assert!(!app.world().resource::<State>().requested);
}

/// The row is taken and kept by its listed names, through the settings page's own route.
#[test]
fn the_settings_page_takes_each_of_the_three_values() {
    let (mut app, _, _) = with_pad(Mode::Off);
    let row = app
        .world()
        .resource::<settings::Settings>()
        .index(crate::odin::controller_gyro::KEY)
        .expect("the row is registered beside upstream's gyro rows");
    for (name, mode) in [
        ("Always", Mode::Always),
        ("While aiming", Mode::WhileAiming),
        ("Off", Mode::Off),
    ] {
        assert!(settings::pick(app.world_mut(), row, name), "takes {name}");
        assert_eq!(app.world().resource::<PadOptions>().mode, mode);
    }
    assert!(!settings::pick(app.world_mut(), row, "LT"), "and nothing else");
}
