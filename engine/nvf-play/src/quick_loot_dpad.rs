//! The fork's own D-pad scrolling of the Quick Loot list (docs/fork.md): the binding it
//! takes from the hotkeys, and the tests of both it and the Hide HUD key's Keep Quick Loot
//! row. `quick_loot.rs` is upstream's file, so only the thin hooks stay there (the
//! `owns_vertical_dpad` field, the three lines of `follow` that move the selection, and
//! the one answer `draw` asks the Hide HUD key); everything a sync would otherwise have to
//! merge is here.
//!
//! A child module of `crate::quick_loot`, so the list's own state is in reach.

use super::*;
use crate::bindings::ControlInput;
use crate::pad::PadInput;

/// Whether the D-pad's Up or Down is this control's controller binding: the list has
/// those while it moves its selection with them ([`QuickLoot::owns_vertical_dpad`]).
pub(crate) fn vertical_dpad_binding(input: &ControlInput, c: Control) -> bool {
    matches!(input.bindings.pad(c), Some(PadInput::Button(GamepadButton::DPadUp | GamepadButton::DPadDown)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests that run `follow` share this module's switches ([`ON`]) and what it
    /// publishes ([`HOLDS`]), and the harness runs tests side by side: they take this in
    /// turn. A panic in one is not allowed to fail the next for a poisoned lock: the
    /// failure that matters is the panic itself.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn alone() -> std::sync::MutexGuard<'static, ()> {
        ONE_AT_A_TIME.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A container of three items in a world built in memory, as menu/model/tests.rs builds
    /// one (no game data), and the list shown on it.
    fn shown_on_a_box() -> (SimState, Shown) {
        use crate::script_draw::tests::{record, sub, top};
        use fnv_esm::{LoadOrder, Plugin, Schema};
        const CAN: u32 = 0x100;
        const CAPS: u32 = 0x101;
        const ANVIL: u32 = 0x102;
        const BOX: u32 = 0x120;
        const CELL: u32 = 0x200;
        const BOX_REF: u32 = 0x300;
        let z = |s: &str| [s.as_bytes(), &[0]].concat();
        let group = |label: [u8; 4], kind: i32, body: &[u8]| [&b"GRUP"[..], &((24 + body.len()) as u32).to_le_bytes(), &label, &kind.to_le_bytes(), &[0; 8], body].concat();
        let misc = |fid: u32, edid: &str, full: &str, value: i32, weight: f32| {
            record(b"MISC", 0, fid, &[sub(b"EDID", z(edid)), sub(b"FULL", z(full)), sub(b"DATA", [value.to_le_bytes(), weight.to_le_bytes()].concat())])
        };
        let cnto = |item: u32, count: i32| sub(b"CNTO", [item.to_le_bytes(), count.to_le_bytes()].concat());
        let cont = record(
            b"CONT",
            0,
            BOX,
            &[sub(b"EDID", z("TestBox")), sub(b"FULL", z("Cash Box")), cnto(CAPS, 200), cnto(CAN, 2), cnto(ANVIL, 1), sub(b"DATA", [&[0u8][..], &10f32.to_le_bytes()].concat())],
        );
        // The player, so the list's other column can be read (`MenuModel::refresh`).
        let npc = record(b"NPC_", 0, 0x7, &[sub(b"EDID", z("Player")), sub(b"DATA", [&100i32.to_le_bytes()[..], &[5u8, 6, 7, 4, 3, 2, 1]].concat())]);
        let cell = record(b"CELL", 0, CELL, &[sub(b"EDID", z("TestCell"))]);
        let refr = record(b"REFR", 0, BOX_REF, &[sub(b"NAME", BOX.to_le_bytes().to_vec()), sub(b"DATA", vec![0; 24])]);
        let children = group(CELL.to_le_bytes(), 6, &group(CELL.to_le_bytes(), 9, &refr));
        let cells = top(b"CELL", &group(0i32.to_le_bytes(), 2, &group(0i32.to_le_bytes(), 3, &[cell, children].concat())));
        let hedr = [&1.34f32.to_le_bytes()[..], &0i32.to_le_bytes(), &0x800u32.to_le_bytes()].concat();
        let miscs = [misc(CAN, "TestCan", "tin can", 2, 1.0), misc(CAPS, "TestCaps", "Bottle Cap", 1, 0.0), misc(ANVIL, "TestAnvil", "anvil", 1, 250.0)].concat();
        let bytes = [record(b"TES4", 1, 0, &[sub(b"HEDR", hedr)]), top(b"MISC", &miscs), top(b"CONT", &cont), top(b"NPC_", &npc), cells].concat();
        let plugins = vec![Plugin::parse(&bytes, "Test.esm".into()).unwrap()];
        let lo = LoadOrder::new(&plugins).unwrap();
        let schema = Schema::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/fnv")).unwrap();
        let forms: &'static fnv_sim::FormStore = Box::leak(Box::new(fnv_sim::FormStore::build(&lo, &schema)));
        let refs: &'static fnv_sim::RefTable = Box::leak(Box::new(fnv_sim::RefTable::build(&lo, &schema)));
        let mut sim = SimState { forms, refs, world: fnv_sim::World::new(forms, refs, fnv_sim::SimConfig::default()) };
        let model = MenuModel::open_container(&mut sim.world, FormId(BOX_REF)).expect("the box opens");
        assert_eq!(model.list(Side::Container).rows.len(), 3, "three rows to move through");
        let shown = Shown { target: BOX_REF, holder: FormId(BOX_REF), kind: Kind::Container, crime: false, title: "Cash Box".into(), model, since_read: 0.0 };
        (sim, shown)
    }

    /// The mouse wheel moves the selection and, on a controller, so do the D-pad's Up and
    /// Down while the list has them, one row per press ([`QuickLoot::owns_vertical_dpad`]:
    /// the hotkeys bound to them were told the same, so they did not act).
    #[test]
    fn the_wheel_and_the_dpad_move_the_selection_one_row_at_a_time() {
        use crate::bindings::Bindings;
        use bevy::input::gamepad::{RawGamepadButtonChangedEvent, RawGamepadEvent};
        use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
        use crate::bindings::InputDevice;
        use GamepadButton::{DPadDown, DPadUp};
        let (sim, shown) = shown_on_a_box();
        let target = crate::activate::Targeted {
            form_id: shown.target,
            name: shown.title.clone(),
            action: Action::Open,
            distance: 50.0,
            source: crate::activate::pick::Source::Collision,
            crime: false,
            empty: false,
            through: None,
        };
        let mut bindings = Bindings::default();
        bindings.device = InputDevice::Controller;
        // The owner's remapping: the hotkeys that sit on the vertical D-pad.
        bindings.set_pad(Control::AmmoSwap, Some(PadInput::Button(GamepadButton::DPadUp)));
        bindings.set_pad(Control::Hotkey4, Some(PadInput::Button(GamepadButton::DPadDown)));
        // The NVF Features row is on: without it the list follows no target.
        let _alone = alone();
        ON.store(true, Ordering::Relaxed);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin))
            .insert_resource(bindings)
            .insert_resource(sim)
            .insert_resource(Target(Some(target)))
            .insert_resource(QuickLoot { shown: Some(shown), ..default() })
            .add_message::<crate::crime::ItemsTaken>()
            .add_message::<crate::pickpocket::Pickpocketed>()
            .add_message::<RefChanged>()
            .add_message::<crate::ui_sounds::UiSound>()
            .add_systems(Update, follow);
        let pad = app.world_mut().spawn(Gamepad::default()).id();
        let row = |app: &App| app.world().resource::<QuickLoot>().shown.as_ref().expect("the list stays").selected();
        // The list is the one the hotkeys were told about a frame ago.
        let owns = |app: &mut App, on| app.world_mut().resource_mut::<QuickLoot>().owns_vertical_dpad = on;
        let button = |app: &mut App, b, down| {
            app.world_mut().write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(pad, b, if down { 1.0 } else { 0.0 })));
            app.update();
        };
        // The first frame follows the target it was shown on, and says it has the D-pad.
        app.update();
        assert_eq!(row(&app), 0);
        assert!(app.world().resource::<QuickLoot>().owns_vertical_dpad, "a list of rows on a controller has the vertical D-pad");
        // One row per press, down and up; a button held moves nothing more.
        button(&mut app, DPadDown, true);
        assert_eq!(row(&app), 1, "D-pad Down moves the selection down a row");
        app.update();
        assert_eq!(row(&app), 1, "held, it stays where it is");
        button(&mut app, DPadDown, false);
        button(&mut app, DPadDown, true);
        assert_eq!(row(&app), 2);
        button(&mut app, DPadDown, false);
        button(&mut app, DPadUp, true);
        assert_eq!(row(&app), 1, "D-pad Up moves it up");
        button(&mut app, DPadUp, false);
        // Not the list's (the hotkeys were told the same, so Ammo Swap had the press).
        owns(&mut app, false);
        button(&mut app, DPadUp, true);
        assert_eq!(row(&app), 1, "with the hotkeys owning the D-pad the list does not move");
        button(&mut app, DPadUp, false);
        // The mouse wheel moves it whatever the D-pad does: up the list rolling up.
        let wheel = |app: &mut App, y| {
            app.world_mut().write_message(MouseWheel { unit: MouseScrollUnit::Line, x: 0.0, y, window: Entity::PLACEHOLDER, phase: bevy::input::touch::TouchPhase::Moved });
            app.update();
        };
        wheel(&mut app, -1.0);
        assert_eq!(row(&app), 2, "the wheel rolled down moves the selection down");
        wheel(&mut app, 1.0);
        assert_eq!(row(&app), 1, "and rolled up, up");
        owns(&mut app, false);
        wheel(&mut app, 1.0);
        assert_eq!(row(&app), 0, "the wheel is the list's whatever the D-pad is");
        ON.store(false, Ordering::Relaxed);
    }

    /// The Hide HUD key with Keep Quick Loot on (the default): the HUD is gone and the
    /// list is not: it follows its target and still owns the vertical D-pad, as with the
    /// HUD shown. With the row off it goes with the HUD, as it did before the row
    /// [design: the owner, 2026-11-22].
    #[test]
    fn a_hidden_hud_keeps_quick_loot_unless_its_row_is_off() {
        use crate::bindings::{Bindings, InputDevice};
        use crate::hud::HudHidden;
        let (sim, shown) = shown_on_a_box();
        let target = crate::activate::Targeted {
            form_id: shown.target,
            name: shown.title.clone(),
            action: Action::Open,
            distance: 50.0,
            source: crate::activate::pick::Source::Collision,
            crime: false,
            empty: false,
            through: None,
        };
        let mut bindings = Bindings::default();
        bindings.device = InputDevice::Controller;
        let _alone = alone();
        ON.store(true, Ordering::Relaxed);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin))
            .insert_resource(bindings)
            .insert_resource(sim)
            .insert_resource(Target(Some(target)))
            .insert_resource(QuickLoot { shown: Some(shown), ..default() })
            // The parts the key hides, as `tweaks::hide_hud_key` sets them: the HUD's own
            // pieces, and Quick Loot kept (Keep Quick Loot on).
            .insert_resource(HudHidden { on: true, compass: true, ammo: true, needs: true, sneak: true, quick_loot: false })
            .add_message::<crate::crime::ItemsTaken>()
            .add_message::<crate::pickpocket::Pickpocketed>()
            .add_message::<RefChanged>()
            .add_message::<crate::ui_sounds::UiSound>()
            .add_systems(Update, follow);
        app.update();
        let ql = app.world().resource::<QuickLoot>();
        assert!(ql.shown.is_some(), "the HUD is hidden and the list stays");
        assert!(ql.owns_vertical_dpad, "and it owns the vertical D-pad as it does with the HUD shown");
        assert!(holds_input(), "and holds its keys");
        // The row off but the key up: nothing is hidden, so nothing changes either.
        {
            let mut hidden = app.world_mut().resource_mut::<HudHidden>();
            (hidden.on, hidden.quick_loot) = (false, true);
        }
        app.update();
        assert!(app.world().resource::<QuickLoot>().shown.is_some(), "with the key off the row hides nothing");
        // The row off and the key down: the list goes with the rest of the HUD, as before
        // the row. (It cannot come back in this harness: opening one needs the script
        // runtime, which the fixture has none of; `follow` only keeps the list it was
        // given.)
        app.world_mut().resource_mut::<HudHidden>().on = true;
        app.update();
        let ql = app.world().resource::<QuickLoot>();
        assert!(ql.shown.is_none(), "with Keep Quick Loot off a hidden HUD takes the list");
        assert!(!ql.owns_vertical_dpad && !holds_input(), "not drawn, so not holding the keys either");
        ON.store(false, Ordering::Relaxed);
    }
}
