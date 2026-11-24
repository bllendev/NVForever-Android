//! Android native input -> Bevy Gamepad -> the game's existing controller bindings.
use bevy::{
    input::{
        gamepad::{RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent, RawGamepadEvent},
        InputSystems,
    },
    prelude::*,
};
use nvf_android_input::Input;

#[derive(Resource, Clone, Copy)]
pub struct ControlsConfig {
    pub controller: bool,
    pub touch: bool,
    pub diagnostics: bool,
    /// Crash test (launcher switch): the Pip-Boy's hidden warm-up during a load is skipped.
    pub skip_pipboy_warmup: bool,
    /// Crash test (launcher switch): no V.A.T.S. part-outline mask camera.
    pub skip_vats_outline: bool,
    /// Crash test (launcher switch): the run asks for the Khronos Vulkan validation layer
    /// (`nvforever::odin::VULKAN_VALIDATION_ENV`, much slower).
    pub vulkan_validation: bool,
    /// Crash test (launcher switch): with the layer on, its synchronization validation too
    /// (`nvforever::odin::VULKAN_VALIDATION_DEEP_ENV`).
    pub vulkan_validation_sync: bool,
    /// Crash test (launcher switch): with the layer on, GPU-assisted validation
    /// (instrumented shaders) too. Slower again, and a load may not finish.
    pub vulkan_validation_gpuav: bool,
    /// Crash test (launcher switch): every render-graph node writes a GPU marker before and
    /// after its work (`nvforever::odin::GPU_BREADCRUMBS_ENV`), so a lost device names the node.
    pub gpu_breadcrumbs: bool,
    /// Crash test (launcher switch): with the breadcrumbs on, the transparent pass is drawn
    /// in chunks with a marker between them, so a lost device names the chunk of draws
    /// (`nvforever::odin::GPU_BREADCRUMB_CHUNKS_ENV`). Many more render passes a frame.
    pub gpu_breadcrumb_chunks: bool,
    /// Crash test (launcher switch): particles keep hard edges where they meet the surface
    /// behind them, and the NIF material reads the depth prepass nowhere
    /// (`nvforever::odin::NO_SOFT_PARTICLES_ENV`).
    pub no_soft_particles: bool,
    /// Crash test (launcher switch): no particle system and no effect particle is drawn
    /// (`nvforever::odin::NO_PARTICLES_ENV`).
    pub no_particles: bool,
    /// Crash test (launcher switch): no decal is placed (`nvforever::odin::NO_DECALS_ENV`).
    pub no_decals: bool,
    /// Crash test (launcher switch): no heat haze pass (`nvforever::odin::NO_HEAT_HAZE_ENV`).
    pub no_heat_haze: bool,
}
impl Default for ControlsConfig {
    /// What a phone without the file gets: touch controls, nothing else.
    fn default() -> Self {
        Self {
            controller: false,
            touch: true,
            diagnostics: false,
            skip_pipboy_warmup: false,
            skip_vats_outline: false,
            vulkan_validation: false,
            vulkan_validation_sync: false,
            vulkan_validation_gpuav: false,
            gpu_breadcrumbs: false,
            gpu_breadcrumb_chunks: false,
            no_soft_particles: false,
            no_particles: false,
            no_decals: false,
            no_heat_haze: false,
        }
    }
}
impl ControlsConfig {
    pub fn load() -> Self {
        Self::parse(&crate::paths::option_file("nvforever-controls.txt").unwrap_or_default())
    }

    /// `nvforever-controls.txt`: one `name value` a line, the launcher's switches
    /// (LauncherActivity.java `saveControls`). A line that is blank, a comment, short of a
    /// value, or carries anything but true/false/1/0 leaves its setting as it was.
    fn parse(text: &str) -> Self {
        let mut config = Self::default();
        for line in text.lines() {
            let mut words = line.split_whitespace();
            let (Some(name), Some(value)) = (words.next(), words.next()) else {
                continue;
            };
            let enabled = match value {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => continue,
            };
            match name {
                "controller" => config.controller = enabled,
                "touch" => config.touch = enabled,
                "diagnostics" => config.diagnostics = enabled,
                "skip-pipboy-warmup" => config.skip_pipboy_warmup = enabled,
                "skip-vats-outline" => config.skip_vats_outline = enabled,
                "vulkan-validation" => config.vulkan_validation = enabled,
                "vulkan-validation-sync" => config.vulkan_validation_sync = enabled,
                "vulkan-validation-gpuav" => config.vulkan_validation_gpuav = enabled,
                "gpu-breadcrumbs" => config.gpu_breadcrumbs = enabled,
                "gpu-breadcrumb-chunks" => config.gpu_breadcrumb_chunks = enabled,
                "no-soft-particles" => config.no_soft_particles = enabled,
                "no-particles" => config.no_particles = enabled,
                "no-decals" => config.no_decals = enabled,
                "no-heat-haze" => config.no_heat_haze = enabled,
                _ => {}
            }
        }
        config
    }
}
#[derive(Resource)]
struct AndroidPad {
    entity: Entity,
    keys: [bool; 16],
    axes: [f32; 8],
}
const BUTTONS: [GamepadButton; 16] = {
    use GamepadButton::*;
    [
        South,
        East,
        West,
        North,
        LeftTrigger,
        RightTrigger,
        LeftTrigger2,
        RightTrigger2,
        LeftThumb,
        RightThumb,
        Start,
        Select,
        DPadUp,
        DPadDown,
        DPadLeft,
        DPadRight,
    ]
};
pub struct ControllerPlugin;
impl Plugin for ControllerPlugin {
    fn build(&self, app: &mut App) {
        let entity = app
            .world_mut()
            .spawn((
                Gamepad::default(),
                Name::new("Retroid / Android controller"),
            ))
            .id();
        app.insert_resource(AndroidPad {
            entity,
            keys: [false; 16],
            axes: [0.0; 8],
        })
        .add_systems(PreUpdate, pump.before(InputSystems))
        .add_systems(
            PostStartup,
            select_device.after(nvf_play::bindings::apply_stored),
        );
    }
}
/// Android alone puts changing ammunition on D-pad Up and the first hotkey on Right, the
/// way a console pad has them; the desktop keeps the engine's own default
/// (`nvf_play::bindings::DPAD_HOTKEYS`). Only the untouched four-way default is remapped,
/// so a controller profile the player rebound is left alone.
fn migrate_legacy_dpad(bindings: &mut nvf_play::bindings::Bindings) -> bool {
    use nvf_play::{bindings::Control, pad::PadInput};
    let old = [
        (Control::Hotkey1, GamepadButton::DPadUp),
        (Control::AmmoSwap, GamepadButton::DPadRight),
        (Control::Hotkey3, GamepadButton::DPadDown),
        (Control::Hotkey4, GamepadButton::DPadLeft),
    ];
    if !old
        .iter()
        .all(|(control, button)| bindings.pad(*control) == Some(PadInput::Button(*button)))
    {
        return false;
    }
    bindings.set_pad(
        Control::AmmoSwap,
        Some(PadInput::Button(GamepadButton::DPadUp)),
    );
    bindings.set_pad(
        Control::Hotkey1,
        Some(PadInput::Button(GamepadButton::DPadRight)),
    );
    true
}

fn select_device(world: &mut World) {
    let on = world.resource::<ControlsConfig>().controller;
    if on {
        if let Some(mut bindings) = world.get_resource_mut::<nvf_play::bindings::Bindings>() {
            if migrate_legacy_dpad(&mut bindings) {
                info!("controller: migrated M6 defaults to console D-pad (Up changes ammo)");
            }
        }
    }
    nvf_play::bindings::set_device(
        world,
        if on {
            nvf_play::bindings::InputDevice::Controller
        } else {
            nvf_play::bindings::InputDevice::KeyboardMouse
        },
    );
}
fn pump(
    mut pad: ResMut<AndroidPad>,
    config: Res<ControlsConfig>,
    mut raw: MessageWriter<RawGamepadEvent>,
) {
    let events = nvf_android_input::drain();
    if !config.controller {
        return;
    }
    for event in events {
        if config.diagnostics {
            info!(?event, "Android controller input");
        }
        match event {
            Input::Button(index, down) => pad.keys[index] = down,
            Input::Axes(values) => pad.axes = values,
            Input::Reset => {
                pad.keys.fill(false);
                pad.axes.fill(0.0);
            }
        }
        for (index, button) in BUTTONS.iter().enumerate() {
            let digital = if pad.keys[index] { 1.0_f32 } else { 0.0 };
            let value = match index {
                6 => digital.max(pad.axes[4]),
                7 => digital.max(pad.axes[5]),
                12 => digital.max(if pad.axes[7] < -0.5 { 1.0 } else { 0.0 }),
                13 => digital.max(if pad.axes[7] > 0.5 { 1.0 } else { 0.0 }),
                14 => digital.max(if pad.axes[6] < -0.5 { 1.0 } else { 0.0 }),
                15 => digital.max(if pad.axes[6] > 0.5 { 1.0 } else { 0.0 }),
                _ => digital,
            };
            raw.write(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                pad.entity, *button, value,
            )));
        }
        for (index, axis) in [
            GamepadAxis::LeftStickX,
            GamepadAxis::LeftStickY,
            GamepadAxis::RightStickX,
            GamepadAxis::RightStickY,
        ]
        .into_iter()
        .enumerate()
        {
            raw.write(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                pad.entity,
                axis,
                pad.axes[index],
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvf_play::{
        bindings::{Bindings, Control},
        pad::PadInput,
    };

    #[test]
    fn controls_file_defaults_and_every_switch_it_can_set() {
        let empty = ControlsConfig::parse("");
        assert!(!empty.controller && empty.touch && !empty.diagnostics);
        assert!(!empty.skip_pipboy_warmup && !empty.skip_vats_outline && !empty.vulkan_validation);
        assert!(!empty.vulkan_validation_sync && !empty.vulkan_validation_gpuav && !empty.gpu_breadcrumbs);
        assert!(!empty.gpu_breadcrumb_chunks && !empty.no_soft_particles && !empty.no_particles && !empty.no_decals && !empty.no_heat_haze);

        // What the launcher writes with every switch on (the comment line is ignored).
        let on = ControlsConfig::parse(
            "# Applied when starting the game\ncontroller true\ntouch true\nskip-pipboy-warmup true\nskip-vats-outline true\nvulkan-validation true\n\
             vulkan-validation-sync true\nvulkan-validation-gpuav true\ngpu-breadcrumbs true\ngpu-breadcrumb-chunks true\nno-soft-particles true\n\
             no-particles true\nno-decals true\nno-heat-haze true\n",
        );
        assert!(on.controller && on.touch && on.skip_pipboy_warmup && on.skip_vats_outline && on.vulkan_validation);
        assert!(on.vulkan_validation_sync && on.vulkan_validation_gpuav && on.gpu_breadcrumbs);
        assert!(on.gpu_breadcrumb_chunks && on.no_soft_particles && on.no_particles && on.no_decals && on.no_heat_haze);

        // The off state is the file saying false, not only the file being absent.
        let off = ControlsConfig::parse(
            "controller false\ntouch false\nvulkan-validation false\nvulkan-validation-sync false\nvulkan-validation-gpuav false\ngpu-breadcrumbs false\n\
             gpu-breadcrumb-chunks false\nno-soft-particles false\nno-particles false\nno-decals false\nno-heat-haze false\n",
        );
        assert!(!off.controller && !off.touch && !off.vulkan_validation);
        assert!(!off.vulkan_validation_sync && !off.vulkan_validation_gpuav && !off.gpu_breadcrumbs);
        assert!(!off.gpu_breadcrumb_chunks && !off.no_soft_particles && !off.no_particles && !off.no_decals && !off.no_heat_haze);

        // The names are read one at a time: a switch on leaves the others as they were.
        let one = ControlsConfig::parse("no-particles 1\n");
        assert!(one.no_particles && !one.no_soft_particles && !one.no_decals && !one.no_heat_haze && !one.gpu_breadcrumb_chunks);
        assert!(!ControlsConfig::parse("no-heat-haze maybe").no_heat_haze);

        // 1/0 are accepted too; an unknown name, a missing or unreadable value and a blank
        // line change nothing.
        let mixed = ControlsConfig::parse("vulkan-validation 1\ndiagnostics 0\n\nnonsense true\nvulkan-validation\ntouch maybe\n");
        assert!(mixed.vulkan_validation && !mixed.diagnostics && mixed.touch);
        assert!(!ControlsConfig::parse("vulkan-validation 0").vulkan_validation);
    }

    #[test]
    fn legacy_migration_is_idempotent_and_preserves_custom_bindings() {
        // The engine's untouched four-way default (`nvf_play::bindings::DPAD_HOTKEYS`,
        // which `Bindings::game_defaults` fills in): the layout Android remaps, so a fresh
        // profile is migrated and the start says so once.
        let mut bindings = Bindings::game_defaults();
        for (control, button) in [
            (Control::Hotkey1, GamepadButton::DPadUp),
            (Control::AmmoSwap, GamepadButton::DPadRight),
            (Control::Hotkey3, GamepadButton::DPadDown),
            (Control::Hotkey4, GamepadButton::DPadLeft),
        ] {
            assert_eq!(bindings.pad(control), Some(PadInput::Button(button)));
        }
        let mut custom = bindings.clone();
        custom.set_pad(
            Control::Hotkey4,
            Some(PadInput::Button(GamepadButton::North)),
        );
        assert!(!migrate_legacy_dpad(&mut custom));
        assert_eq!(
            custom.pad(Control::AmmoSwap),
            Some(PadInput::Button(GamepadButton::DPadRight))
        );
        assert!(migrate_legacy_dpad(&mut bindings));
        assert_eq!(
            bindings.pad(Control::AmmoSwap),
            Some(PadInput::Button(GamepadButton::DPadUp))
        );
        assert!(!migrate_legacy_dpad(&mut bindings));
    }

    #[test]
    fn console_actions_and_native_input_survive_frames_and_focus_loss() {
        use bevy::input::InputPlugin;
        use GamepadButton::*;
        nvf_android_input::drain();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(InputPlugin)
            // The engine's pad plugin loads the button glyph pictures when the game's data
            // is in, so its asset store has to exist even with no data here.
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Image>()
            .insert_resource(Bindings::game_defaults())
            .insert_resource(ControlsConfig { controller: true, touch: false, ..default() })
            .add_plugins(ControllerPlugin)
            .add_plugins(nvf_play::pad::PadPlugin)
            .insert_resource(State::new(nvf_play::PlayState::Menu));
        app.update();
        let entity = app.world().resource::<AndroidPad>().entity;
        let expected = [
            (Control::Activate, South),
            (Control::MenuMode, East),
            (Control::ReadyItem, West),
            (Control::Jump, North),
            (Control::Block, LeftTrigger2),
            (Control::Use, RightTrigger2),
            (Control::TogglePov, LeftTrigger),
            (Control::Vats, RightTrigger),
            (Control::CrouchSneak, LeftThumb),
            (Control::Grab, RightThumb),
            (Control::Rest, Select),
            (Control::AmmoSwap, DPadUp),
        ];
        for (control, button) in expected {
            assert_eq!(
                app.world().resource::<Bindings>().pad(control),
                Some(PadInput::Button(button))
            );
        }
        for (code, button) in [
            (96, South),
            (97, East),
            (99, West),
            (100, North),
            (102, LeftTrigger),
            (103, RightTrigger),
            (104, LeftTrigger2),
            (105, RightTrigger2),
            (106, LeftThumb),
            (107, RightThumb),
            (108, Start),
            (109, Select),
            (19, DPadUp),
            (20, DPadDown),
            (21, DPadLeft),
            (22, DPadRight),
        ] {
            assert!(nvf_android_input::key(code, true));
            app.update();
            assert!(app
                .world()
                .get::<Gamepad>(entity)
                .unwrap()
                .just_pressed(button));
            app.update();
            let pad = app.world().get::<Gamepad>(entity).unwrap();
            assert!(pad.pressed(button));
            assert!(!pad.just_pressed(button));
            assert!(nvf_android_input::key(code, false));
            app.update();
            assert!(app
                .world()
                .get::<Gamepad>(entity)
                .unwrap()
                .just_released(button));
        }
        nvf_android_input::push(Input::Axes([0.8, 0.6, -0.7, -0.5, 0.9, 1.0, -1.0, -1.0]));
        app.update();
        let pad = app.world().get::<Gamepad>(entity).unwrap();
        assert!(pad.left_stick().x > 0.5 && pad.left_stick().y > 0.3);
        assert!(pad.right_stick().x < -0.4 && pad.right_stick().y < -0.2);
        assert!(pad.pressed(LeftTrigger2) && pad.pressed(RightTrigger2));
        assert!(pad.pressed(DPadLeft) && pad.pressed(DPadUp));
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        // The engine turns the pad into menu keys (`nvf_play::pad::menu_keys`): the
        // D-pad's two into their arrows, and the sticks, in a menu, into the page keys.
        // What the adapter owes is the input itself; which key each stands for is the
        // engine's own table and its own tests.
        assert!(keys.pressed(KeyCode::ArrowUp) && keys.pressed(KeyCode::ArrowLeft));
        assert!(keys.pressed(KeyCode::PageUp) || keys.pressed(KeyCode::PageDown));
        nvf_android_input::push(Input::Reset);
        app.update();
        let pad = app.world().get::<Gamepad>(entity).unwrap();
        assert_eq!(pad.left_stick(), Vec2::ZERO);
        assert_eq!(pad.right_stick(), Vec2::ZERO);
        for button in BUTTONS {
            assert!(!pad.pressed(button));
        }
        assert!(!app
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::ArrowUp));
        // The Pip-Boy's own tab keys sit on the triggers in the engine's table, but it
        // reads them off the *game's* top menu (`nvf_play::pad::MenuPages::at`, from the
        // script runtime), which this harness has none of: so no page key is made here,
        // and what the adapter owes is the trigger reaching the pad, pressed and released,
        // which the loop above checks for both of them.
        let mut pip = nvf_play::pipboy::PipBoy::default();
        pip.phase = nvf_play::pipboy::Phase::Up;
        pip.page = nvf_play::pipboy::Page::Stats;
        app.insert_resource(pip);
        nvf_android_input::key(104, true);
        app.update();
        assert!(app
            .world()
            .get::<Gamepad>(entity)
            .unwrap()
            .just_pressed(LeftTrigger2));
        nvf_android_input::key(104, false);
        app.update();
        nvf_android_input::key(105, true);
        app.update();
        assert!(app
            .world()
            .get::<Gamepad>(entity)
            .unwrap()
            .just_pressed(RightTrigger2));
        nvf_android_input::push(Input::Reset);
        app.update();
    }
}
