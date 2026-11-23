//! Touch controls: the game is written for a keyboard and a mouse, so the phone's fingers are
//! turned into the keys and mouse motion the game already reads. Nothing in `nvf-play` knows
//! about touch.
//!
//! **Playing** (the game holds the cursor: `CursorOptions::grab_mode` is not `None`):
//! - a finger down on the left 42 % of the screen is a floating stick: W/A/S/D, and a gentle
//!   push holds Shift, which the game reads as *walk* (it runs by default);
//! - a finger down anywhere else drags the view (mouse motion);
//! - round buttons: FIRE (left mouse), AIM (right mouse), JUMP, USE (E), R (reload, holster
//!   when held), SNK (sneak), VATS, and at the top Esc, Tab (Pip-Boy), `...` (more keys), KB.
//!
//! **In a menu** (cursor free): one finger is the mouse pointer (it moves the cursor, and
//! presses the left button once the finger has rested two frames, so hover highlights come
//! first); a second finger dragged up and down is the mouse wheel. Esc, Tab, Enter and the
//! keyboard button stay on screen.
//!
//! The keys go through `ButtonInput<KeyCode>` after Bevy's own input system, the way the
//! game's controller support does (`nvf_play::pad`). The bindings are the game's defaults
//! (`nvf_play::bindings`); rebinding a key on the settings page leaves these buttons on the
//! old key.

use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use std::collections::{HashMap, HashSet};

/// Mouse counts per pixel of finger travel (`look_gain` in nvforever-touch.txt).
const DEFAULT_LOOK_GAIN: f32 = 1.3;
/// The stick's radius at 720 px of screen height (`stick_radius`).
const DEFAULT_STICK_RADIUS: f32 = 90.0;
/// Fraction of the screen width, from the left, where a finger down starts the stick.
const STICK_ZONE: f32 = 0.42;
/// Frames a menu finger rests before the left button goes down.
const CLICK_AFTER_FRAMES: u32 = 2;

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Key(KeyCode),
    Mouse(MouseButton),
    /// The soft keyboard.
    Soft,
    /// The row of extra keys.
    More,
}

#[derive(Clone, Copy, PartialEq)]
enum Show {
    Play,
    Menu,
    Both,
}

/// A round on-screen button. Offsets are in pixels at 720 px of screen height, from the
/// right or left edge and from the bottom or top edge.
struct Btn {
    label: &'static str,
    x: f32,
    y: f32,
    r: f32,
    from_right: bool,
    from_bottom: bool,
    act: Act,
    /// A tap (press and release a frame later) rather than held while the finger is down.
    tap: bool,
    show: Show,
    /// Only while the extra row is open.
    extra: bool,
}

#[allow(clippy::too_many_arguments)] // Fixed touch-button descriptor, not a public call API.
const fn btn(label: &'static str, x: f32, y: f32, r: f32, from_bottom: bool, act: Act, tap: bool, show: Show, extra: bool) -> Btn {
    Btn { label, x, y, r, from_right: true, from_bottom, act, tap, show, extra }
}

const BUTTONS: &[Btn] = &[
    // The right-hand cluster (playing).
    btn("FIRE", 150.0, 170.0, 72.0, true, Act::Mouse(MouseButton::Left), false, Show::Play, false),
    btn("AIM", 290.0, 90.0, 48.0, true, Act::Mouse(MouseButton::Right), false, Show::Play, false),
    btn("JUMP", 70.0, 330.0, 44.0, true, Act::Key(KeyCode::Space), true, Show::Play, false),
    btn("USE", 280.0, 250.0, 46.0, true, Act::Key(KeyCode::KeyE), true, Show::Play, false),
    btn("R", 410.0, 110.0, 40.0, true, Act::Key(KeyCode::KeyR), false, Show::Play, false),
    btn("SNK", 410.0, 230.0, 38.0, true, Act::Key(KeyCode::ControlLeft), true, Show::Play, false),
    btn("VATS", 70.0, 470.0, 40.0, true, Act::Key(KeyCode::KeyV), true, Show::Play, false),
    // The top row (always, except the extras).
    btn("ESC", 50.0, 48.0, 30.0, false, Act::Key(KeyCode::Escape), true, Show::Both, false),
    btn("PIP", 125.0, 48.0, 30.0, false, Act::Key(KeyCode::Tab), true, Show::Both, false),
    btn("...", 200.0, 48.0, 30.0, false, Act::More, true, Show::Play, false),
    btn("KB", 275.0, 48.0, 30.0, false, Act::Soft, true, Show::Both, false),
    btn("ENT", 350.0, 48.0, 30.0, false, Act::Key(KeyCode::Enter), true, Show::Menu, false),
    // The extra row (playing, after `...`): quick save and load, hotkeys, and the rest.
    btn("F5", 50.0, 120.0, 30.0, false, Act::Key(KeyCode::F5), true, Show::Play, true),
    btn("F9", 125.0, 120.0, 30.0, false, Act::Key(KeyCode::F9), true, Show::Play, true),
    btn("1", 200.0, 120.0, 30.0, false, Act::Key(KeyCode::Digit1), true, Show::Play, true),
    btn("2", 275.0, 120.0, 30.0, false, Act::Key(KeyCode::Digit2), true, Show::Play, true),
    btn("3", 350.0, 120.0, 30.0, false, Act::Key(KeyCode::Digit3), true, Show::Play, true),
    btn("4", 425.0, 120.0, 30.0, false, Act::Key(KeyCode::Digit4), true, Show::Play, true),
    btn("Q", 500.0, 120.0, 30.0, false, Act::Key(KeyCode::KeyQ), true, Show::Play, true),
    btn("F", 575.0, 120.0, 30.0, false, Act::Key(KeyCode::KeyF), true, Show::Play, true),
    btn("H", 650.0, 120.0, 30.0, false, Act::Key(KeyCode::KeyH), true, Show::Play, true),
    btn("T", 725.0, 120.0, 30.0, false, Act::Key(KeyCode::KeyT), true, Show::Play, true),
    btn("~", 800.0, 120.0, 30.0, false, Act::Key(KeyCode::Backquote), true, Show::Play, true),
];

/// What a finger that is down is doing.
enum Role {
    Stick { origin: Vec2 },
    Look { last: Vec2 },
    Button(usize),
    Pointer { age: u32, pressed: bool },
    Scroll { last: Vec2 },
}

/// Tunable from `nvforever-touch.txt` beside `Data` (lines `name value`, `#` comments):
/// `look_gain` (default 1.3, mouse counts per pixel: the view's speed), `stick_radius`
/// (90, pixels at 720p) and `scale` (1.0, size of every button and the stick).
#[derive(Resource, Clone, Copy)]
struct TouchConfig {
    look_gain: f32,
    stick_radius: f32,
    scale: f32,
}

impl TouchConfig {
    fn load() -> Self {
        let mut c = TouchConfig { look_gain: DEFAULT_LOOK_GAIN, stick_radius: DEFAULT_STICK_RADIUS, scale: 1.0 };
        let text = crate::paths::option_file("nvforever-touch.txt").unwrap_or_default();
        for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
            let mut it = line.split(|ch: char| ch.is_whitespace() || ch == '=').filter(|t| !t.is_empty());
            let (Some(name), Some(value)) = (it.next(), it.next().and_then(|v| v.parse::<f32>().ok())) else { continue };
            match name {
                "look_gain" => c.look_gain = value.clamp(0.1, 10.0),
                "stick_radius" => c.stick_radius = value.clamp(30.0, 300.0),
                "scale" => c.scale = value.clamp(0.4, 3.0),
                _ => {}
            }
        }
        c
    }
}

#[derive(Resource, Default)]
struct TouchState {
    play: bool,
    roles: HashMap<u64, Role>,
    held_keys: HashSet<KeyCode>,
    held_mouse: HashSet<MouseButton>,
    /// Pressed this frame, released at the start of the next.
    tap_keys: Vec<KeyCode>,
    tap_mouse: Vec<MouseButton>,
    /// Buttons with a finger on them (drawn lit).
    down: HashSet<usize>,
    /// The stick's origin and the finger, while it is in use.
    stick: Option<(Vec2, Vec2)>,
    more_open: bool,
    keyboard_open: bool,
}

pub struct TouchControlsPlugin;

impl Plugin for TouchControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchState>()
            .insert_resource(TouchConfig::load())
            .add_systems(Startup, spawn_overlay)
            // After Bevy's input systems (the keys are set on top of the real ones) and before
            // the UI reads the pointer, as the game's own `loading::hold_input` does.
            .add_systems(PreUpdate, drive.after(InputSystems).before(bevy::ui::UiSystems::Focus))
            .add_systems(Update, layout_overlay);
    }
}

fn centre(b: &Btn, w: f32, h: f32, u: f32) -> Vec2 {
    let x = if b.from_right { w - b.x * u } else { b.x * u };
    let y = if b.from_bottom { h - b.y * u } else { b.y * u };
    Vec2::new(x, y)
}

fn visible(b: &Btn, play: bool, more_open: bool) -> bool {
    let mode = match b.show {
        Show::Both => true,
        Show::Play => play,
        Show::Menu => !play,
    };
    mode && (!b.extra || more_open)
}

fn button_at(p: Vec2, w: f32, h: f32, u: f32, play: bool, more_open: bool) -> Option<usize> {
    BUTTONS
        .iter()
        .enumerate()
        .filter(|(_, b)| visible(b, play, more_open))
        .map(|(i, b)| (i, centre(b, w, h, u).distance(p) / (b.r * u)))
        .filter(|(_, d)| *d <= 1.2)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn soft_keyboard(show: bool) {
    #[cfg(target_os = "android")]
    if let Some(app) = bevy::android::ANDROID_APP.get() {
        if show {
            app.show_soft_input(true);
        } else {
            app.hide_soft_input(true);
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = show;
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut windows: Query<(Entity, &mut Window, &CursorOptions), With<PrimaryWindow>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<AccumulatedMouseMotion>,
    mut wheel: MessageWriter<MouseWheel>,
    touches: Res<Touches>,
    cfg: Res<TouchConfig>,
    mut st: ResMut<TouchState>,
) {
    let Ok((window_entity, mut window, cursor)) = windows.single_mut() else { return };
    let (w, h) = (window.width(), window.height());
    let u = (h / 720.0).max(0.5) * cfg.scale;
    let play = cursor.grab_mode != CursorGrabMode::None;
    let st = &mut *st;

    // Last frame's taps end now.
    for k in st.tap_keys.drain(..) {
        keys.release(k);
    }
    for b in st.tap_mouse.drain(..) {
        mouse.release(b);
    }

    // The phone's Back button (winit reports it as BrowserBack) is the game's Escape.
    if keys.just_pressed(KeyCode::BrowserBack) {
        keys.press(KeyCode::Escape);
        st.tap_keys.push(KeyCode::Escape);
    }

    // Playing and menus use the fingers differently: start over when the mode changes.
    if st.play != play {
        for k in st.held_keys.drain() {
            keys.release(k);
        }
        for b in st.held_mouse.drain() {
            mouse.release(b);
        }
        st.roles.clear();
        st.down.clear();
        st.stick = None;
        st.play = play;
    }

    // Fingers that have lifted. A menu tap that ended before the click came gets it now.
    let live: HashSet<u64> = touches.iter().map(|t| t.id()).collect();
    let ended: Vec<u64> = st.roles.keys().copied().filter(|id| !live.contains(id)).collect();
    for id in ended {
        if let Some(Role::Pointer { pressed: false, .. }) = st.roles.remove(&id) {
            mouse.press(MouseButton::Left);
            st.tap_mouse.push(MouseButton::Left);
        }
    }

    // Fingers that just came down.
    for t in touches.iter_just_pressed() {
        let (id, p) = (t.id(), t.position());
        if let Some(i) = button_at(p, w, h, u, play, st.more_open) {
            st.roles.insert(id, Role::Button(i));
            match BUTTONS[i].act {
                Act::Key(k) if BUTTONS[i].tap => {
                    keys.press(k);
                    st.tap_keys.push(k);
                }
                Act::More => st.more_open = !st.more_open,
                Act::Soft => {
                    st.keyboard_open = !st.keyboard_open;
                    soft_keyboard(st.keyboard_open);
                }
                _ => {}
            }
            continue;
        }
        if play {
            let has_stick = st.roles.values().any(|r| matches!(r, Role::Stick { .. }));
            if p.x < w * STICK_ZONE && !has_stick {
                st.roles.insert(id, Role::Stick { origin: p });
            } else {
                st.roles.insert(id, Role::Look { last: p });
            }
        } else if st.roles.values().any(|r| matches!(r, Role::Pointer { .. })) {
            st.roles.insert(id, Role::Scroll { last: p });
        } else {
            window.set_cursor_position(Some(p));
            st.roles.insert(id, Role::Pointer { age: 0, pressed: false });
        }
    }

    // What every finger asks for this frame.
    let mut want_keys: HashSet<KeyCode> = HashSet::new();
    let mut want_mouse: HashSet<MouseButton> = HashSet::new();
    let mut look = Vec2::ZERO;
    st.down.clear();
    st.stick = None;
    let mut down = Vec::new();
    let mut stick = None;
    for t in touches.iter() {
        let p = t.position();
        let Some(role) = st.roles.get_mut(&t.id()) else { continue };
        match role {
            Role::Button(i) => {
                down.push(*i);
                match BUTTONS[*i].act {
                    Act::Key(k) if !BUTTONS[*i].tap => {
                        want_keys.insert(k);
                    }
                    Act::Mouse(m) => {
                        want_mouse.insert(m);
                    }
                    _ => {}
                }
            }
            Role::Stick { origin } => {
                let radius = cfg.stick_radius * u;
                let mut off = p - *origin;
                if off.length() > radius * 1.6 {
                    // Drag the stick's centre along with a finger that wanders off.
                    *origin = p - off.normalize() * radius * 1.6;
                    off = p - *origin;
                }
                let v = off / radius;
                if v.y < -0.3 {
                    want_keys.insert(KeyCode::KeyW);
                }
                if v.y > 0.3 {
                    want_keys.insert(KeyCode::KeyS);
                }
                if v.x < -0.3 {
                    want_keys.insert(KeyCode::KeyA);
                }
                if v.x > 0.3 {
                    want_keys.insert(KeyCode::KeyD);
                }
                // A gentle push walks (the game's Run key reads as walk while it runs by default).
                if v.length() > 0.3 && v.length() < 0.65 {
                    want_keys.insert(KeyCode::ShiftLeft);
                }
                stick = Some((*origin, p));
            }
            Role::Look { last } => {
                look += (p - *last) * cfg.look_gain;
                *last = p;
            }
            Role::Pointer { age, pressed } => {
                window.set_cursor_position(Some(p));
                *age += 1;
                if *age >= CLICK_AFTER_FRAMES {
                    *pressed = true;
                }
                if *pressed {
                    want_mouse.insert(MouseButton::Left);
                }
            }
            Role::Scroll { last } => {
                let dy = p.y - last.y;
                *last = p;
                if dy != 0.0 {
                    // A finger dragged up reads the list further down: wheel down (negative).
                    wheel.write(MouseWheel { unit: MouseScrollUnit::Pixel, x: 0.0, y: dy, window: window_entity, phase: TouchPhase::Moved });
                }
            }
        }
    }
    st.down.extend(down);
    st.stick = stick;
    motion.delta += look;

    // Make the real input match: press what is newly wanted, release what is no longer.
    let release_keys: Vec<KeyCode> = st.held_keys.difference(&want_keys).copied().collect();
    for k in release_keys {
        keys.release(k);
    }
    for k in want_keys.difference(&st.held_keys) {
        keys.press(*k);
    }
    st.held_keys = want_keys;
    let release_mouse: Vec<MouseButton> = st.held_mouse.difference(&want_mouse).copied().collect();
    for b in release_mouse {
        mouse.release(b);
    }
    for b in want_mouse.difference(&st.held_mouse) {
        mouse.press(*b);
    }
    st.held_mouse = want_mouse;
}

#[derive(Component)]
struct TouchBtn(usize);
#[derive(Component)]
struct StickRing;
#[derive(Component)]
struct StickKnob;

const IDLE: Color = Color::srgba(1.0, 0.78, 0.25, 0.28);
const LIT: Color = Color::srgba(1.0, 0.78, 0.25, 0.75);

fn spawn_overlay(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Touch controls"),
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            GlobalZIndex(1000),
        ))
        .with_children(|root| {
            for (i, b) in BUTTONS.iter().enumerate() {
                root.spawn((
                    TouchBtn(i),
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(2.0 * b.r),
                        height: Val::Px(2.0 * b.r),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(IDLE),
                    Visibility::Hidden,
                ))
                .with_children(|c| {
                    c.spawn((Text::new(b.label), TextFont { font_size: FontSize::Px(if b.r > 35.0 { 20.0 } else { 15.0 }), ..default() }, TextColor(Color::WHITE)));
                });
            }
            root.spawn((
                StickRing,
                Node { position_type: PositionType::Absolute, width: Val::Px(2.0 * DEFAULT_STICK_RADIUS), height: Val::Px(2.0 * DEFAULT_STICK_RADIUS), ..default() },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
                Visibility::Hidden,
            ));
            root.spawn((
                StickKnob,
                Node { position_type: PositionType::Absolute, width: Val::Px(0.9 * DEFAULT_STICK_RADIUS), height: Val::Px(0.9 * DEFAULT_STICK_RADIUS), ..default() },
                BackgroundColor(Color::srgba(1.0, 0.78, 0.25, 0.55)),
                Visibility::Hidden,
            ));
        });
}

#[allow(clippy::type_complexity)]
fn layout_overlay(
    windows: Query<&Window, With<PrimaryWindow>>,
    st: Res<TouchState>,
    cfg: Res<TouchConfig>,
    mut buttons: Query<(&TouchBtn, &mut Node, &mut BackgroundColor, &mut Visibility), (Without<StickRing>, Without<StickKnob>)>,
    mut ring: Query<(&mut Node, &mut Visibility), (With<StickRing>, Without<TouchBtn>, Without<StickKnob>)>,
    mut knob: Query<(&mut Node, &mut Visibility), (With<StickKnob>, Without<TouchBtn>, Without<StickRing>)>,
) {
    let Ok(window) = windows.single() else { return };
    let (w, h) = (window.width(), window.height());
    let u = (h / 720.0).max(0.5) * cfg.scale;
    for (TouchBtn(i), mut node, mut bg, mut vis) in &mut buttons {
        let b = &BUTTONS[*i];
        let show = visible(b, st.play, st.more_open);
        let c = centre(b, w, h, u);
        node.left = Val::Px(c.x - b.r * u);
        node.top = Val::Px(c.y - b.r * u);
        node.width = Val::Px(2.0 * b.r * u);
        node.height = Val::Px(2.0 * b.r * u);
        bg.0 = if st.down.contains(i) || (matches!(b.act, Act::More) && st.more_open) { LIT } else { IDLE };
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
    let radius = cfg.stick_radius * u;
    if let Ok((mut node, mut vis)) = ring.single_mut() {
        match st.stick {
            Some((origin, _)) => {
                node.left = Val::Px(origin.x - radius);
                node.top = Val::Px(origin.y - radius);
                node.width = Val::Px(2.0 * radius);
                node.height = Val::Px(2.0 * radius);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    if let Ok((mut node, mut vis)) = knob.single_mut() {
        match st.stick {
            Some((origin, finger)) => {
                let at = origin + (finger - origin).clamp_length_max(radius);
                let size = 0.9 * radius;
                node.left = Val::Px(at.x - size / 2.0);
                node.top = Val::Px(at.y - size / 2.0);
                node.width = Val::Px(size);
                node.height = Val::Px(size);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
