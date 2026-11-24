//! The fork's own test of the Quick Loot list taking the vertical D-pad from the wheel's
//! hotkeys (docs/fork.md). `weapon_wheel.rs` is upstream's file: it keeps the thin hook
//! alone (the `ql` parameter and the two lines of `read` that let a press through or not),
//! so a sync with upstream NVForever has no test of ours to merge there.
//!
//! A child module of `crate::weapon_wheel`, so the wheel's own state is in reach.

use super::*;


/// A Quick Loot list with rows moves its selection with the D-pad's Up and Down, so a
/// hotkey bound to either (the owner's: Ammo Swap on Up, Hotkey 4 on Down) is not
/// pressed by them while the list has them; the D-pad's left and right, and the
/// controls' own keys, are untouched.
#[test]
fn a_quick_loot_list_takes_the_vertical_dpad_from_the_hotkeys_bound_to_it() {
    use crate::bindings::{Bindings, Control};
    use crate::pad::PadInput;
    use crate::quick_loot::QuickLoot;
    use bevy::input::gamepad::{RawGamepadButtonChangedEvent, RawGamepadEvent};
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::ButtonState;
    use GamepadButton::{DPadDown, DPadLeft, DPadRight, DPadUp};

    // The control the wheel took this frame's press for: `button` on the pad, or the
    // key of `key`'s control, with the list owning the vertical D-pad or not.
    let pressed = |owns: bool, device, button: Option<GamepadButton>, key: Option<Control>| {
        let mut bindings = Bindings::default();
        bindings.device = device;
        for (c, b) in [(Control::AmmoSwap, DPadUp), (Control::Hotkey4, DPadDown), (Control::Hotkey1, DPadRight), (Control::LowerWeapon, DPadLeft)] {
            bindings.rebind_pad(c, PadInput::Button(b));
        }
        let code = key.map(|c| bindings.get(c).key.expect("a key of its own"));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, bevy::state::app::StatesPlugin))
            .insert_state(PlayState::Playing)
            .insert_resource(bindings)
            .insert_resource(SimState::over_placed([]))
            .insert_resource({ let mut ql = QuickLoot::default(); ql.owns_vertical_dpad = owns; ql })
            .init_resource::<PlayerControl>()
            .init_resource::<WeaponWheel>().init_resource::<Hotkeys>().init_resource::<WheelAid>().init_resource::<WheelStats>()
            .add_message::<ItemUsed>().add_message::<UiSound>()
            .add_systems(Update, read);
        let pad = app.world_mut().spawn(Gamepad::default()).id();
        if let Some(b) = button {
            app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(pad, b, 1.0)));
        }
        if let Some(code) = code {
            app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: Key::Character("2".into()), state: ButtonState::Pressed, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        app.update();
        let wheel = app.world().resource::<WeaponWheel>();
        // Nothing the list took turned into a tap or opened the wheel either.
        if wheel.press.is_none() {
            assert_eq!(app.world().resource::<WheelStats>().taps, 0, "{button:?}: a slot was used");
            assert!(!wheel.is_open());
        }
        wheel.press.map(|(slot, _)| CONTROLS[slot])
    };
    let on_pad = |owns, button| pressed(owns, InputDevice::Controller, Some(button), None);
    // Both of the owner's remapped vertical hotkeys: the list's while it has them.
    for (button, control) in [(DPadUp, Control::AmmoSwap), (DPadDown, Control::Hotkey4)] {
        assert_eq!(on_pad(true, button), None, "{button:?} reached the hotkeys while the list had it");
        assert_eq!(on_pad(false, button), Some(control), "{button:?} is {control:?} with no list");
    }
    // The D-pad's right is Hotkey 1 whatever the list does; its left is bound to no
    // slot here, so no slot takes it.
    assert_eq!(on_pad(true, DPadRight), Some(Control::Hotkey1));
    assert_eq!(on_pad(true, DPadLeft), None);
    // The keys are never the list's: Ammo Swap's key is its own with the list owning
    // the D-pad the same control is bound to.
    assert_eq!(pressed(true, InputDevice::KeyboardMouse, None, Some(Control::AmmoSwap)), Some(Control::AmmoSwap));
}
