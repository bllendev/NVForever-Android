//! The live wrist World Map in the running game: the opt-in switch, the frame's decision,
//! and the request nvforever's arm and its screen follow. The rules themselves are
//! [`crate::wrist_live`] (pure, and tested on their own); this file is only the wiring
//! [design: spikes/001-live-wrist-world/README.md].
//!
//! The player turns it on at Gameplay › Pip-Boy on the settings page (`wrist-map`, off
//! until they do), chooses how its key works (`wrist-map-raise`: Hold or Toggle),
//! which picture its screen shows (`wrist-map-style`: Local, the place round them as the
//! Local Map page draws it, or World, the worldspace's own picture), where in the frame
//! the arm comes up (`wrist-map-position`: Lower left or Centre) and how much of the map
//! the glass shows (`wrist-map-zoom`: Near or Far).
//! `NVF_WRIST_LIVE=1` forces it on for a developer whatever the row says, and
//! `NVF_WRIST_FORCE_RAISE=1` holds the glance up with no key at all so a screenshot can
//! be taken of it. Both are developers' switches, named nowhere a player looks.
//!
//! Its key is [`crate::bindings::Control::GlancePipBoy`], rebindable on the Controls page
//! and unbound by default: with no binding of its own the glance follows a *hold* of the
//! Pip-Boy key, which `crate::flashlight::MenuKeyHold` already measures for the light, so
//! a tap of that key opens the Pip-Boy exactly as it always did and only a hold past
//! [`GLANCE_SECONDS`] comes here. With the row off that key is untouched.
//!
//! Nothing here opens a menu, pauses, or writes any [`crate::pipboy::PipBoy`] state: the
//! paused Pip-Boy, the bindings and the HUD are left exactly as they are, and with the
//! mode off [`decide`] leaves after one comparison.
//!
//! Who owns what, each frame:
//!
//! - [`decide`] (in [`crate::PlaySet::Input`]) fills [`Play`] from state the
//!   engine already keeps, steps the rules, and publishes [`WristLive::open`] (the arm
//!   pose it asks for), [`WristLive::shown`] (what its screen should show) and
//!   [`glance_holds`] (the hands' controls it holds for the rest of the frame).
//! - nvforever's `player_body::drive_body` takes the pose from
//!   [`WristLive::arm_clip_time`], the same first-person hold the menus use.
//! - nvforever's `wrist_screen` wears the map on the screen shape, and writes back what
//!   only the body knows into [`WristBody`]: whether the first-person arms are drawn, and
//!   which shape is the Pip-Boy's screen. That is read on the frame after it is written;
//!   a frame late can only delay opening or closing, never force either.

use crate::bindings::{Control, ControlInput};
use crate::settings::{add_choice_list, add_setting, ChoiceList, Setting};
use crate::control::PlayerControl;
use crate::pipboy::data::model::WorldMap;
use crate::pipboy::data::read;
use crate::player::Player;
use crate::wrist_live::{Key, Live, LocalPic, MapPicture, Mark, MarkKind, Play, Spot, MARKS_MOST};
use crate::wrist_page::{Facts, Page, Row};
use bevy::prelude::*;
use std::time::Duration;

/// How the glance's key works [design: the owner, 2026-11-21].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Raise {
    /// The arm is up while the key is held, and goes down when it is let go.
    #[default]
    Hold,
    /// A hold raises the arm and leaves it up; the next hold puts it down.
    Toggle,
}

impl Raise {
    pub const NAMES: [&'static str; 2] = ["Hold", "Toggle"];

    pub fn name(self) -> &'static str {
        Raise::NAMES[self as usize]
    }

    pub fn of(name: &str) -> Option<Raise> {
        match name {
            "Hold" => Some(Raise::Hold),
            "Toggle" => Some(Raise::Toggle),
            _ => None,
        }
    }
}

/// What the glance's screen shows [design: the clean-room notes' "green local-map look";
/// the owner's goal of 2026-11-21].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// The Local Map's own outline of the place round the player
    /// ([`crate::wrist_local`]): the roads, the buildings and the walls about them, as the
    /// paused Local Map page draws them. The worldspace's own picture stands in where
    /// there is no outline yet.
    #[default]
    Local,
    /// The picture the load order owns for the worldspace, as the paused World Map page
    /// shows it.
    World,
}

impl Style {
    pub const NAMES: [&'static str; 2] = ["Local", "World"];

    pub fn name(self) -> &'static str {
        Style::NAMES[self as usize]
    }

    pub fn of(name: &str) -> Option<Style> {
        match name {
            "Local" => Some(Style::Local),
            "World" => Some(Style::World),
            _ => None,
        }
    }
}

/// Where in the frame the glance holds the device [design: the owner, 2026-11-21, after
/// the first Odin photo: the arm came up in the middle right, over the way ahead].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// Low on the left, the middle of the picture clear to walk through.
    #[default]
    LowerLeft,
    /// Where the menus hold it: under the middle of the view.
    Centre,
}

impl Position {
    pub const NAMES: [&'static str; 2] = ["Lower left", "Centre"];

    pub fn name(self) -> &'static str {
        Position::NAMES[self as usize]
    }

    pub fn of(name: &str) -> Option<Position> {
        match name {
            "Lower left" => Some(Position::LowerLeft),
            "Centre" => Some(Position::Centre),
            _ => None,
        }
    }
}

/// Which way up the glance holds its map [design: the owner, 2026-11-21].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Turning {
    /// Heading up: the picture turns under the player, their own mark points up the
    /// glass, and the compass letters orbit the inside of the glass. What a map held in
    /// the hand reads like.
    #[default]
    WithMe,
    /// North stays at the top of the glass and the player's mark turns on it, as the
    /// paused World Map page draws it.
    NorthUp,
}

impl Turning {
    pub const NAMES: [&'static str; 2] = ["With me", "North up"];

    pub fn name(self) -> &'static str {
        Turning::NAMES[self as usize]
    }

    pub fn of(name: &str) -> Option<Turning> {
        match name {
            "With me" => Some(Turning::WithMe),
            "North up" => Some(Turning::NorthUp),
            _ => None,
        }
    }
}

/// How much of the map the glance's screen shows [design: the owner, 2026-11-21].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Zoom {
    /// The window the picture itself asks for: about 175 m of the Local Map's outline
    /// across the glass ([`crate::wrist_local::LOCAL_SPAN`]), or
    /// [`crate::wrist_live::HALF_WINDOW`] of a worldspace's own picture.
    #[default]
    Near,
    /// Twice as much of it either way, for finding the way rather than the next corner.
    Far,
}

impl Zoom {
    pub const NAMES: [&'static str; 2] = ["Near", "Far"];

    /// How much more of the picture than it asks for ([`crate::wrist_live::Spot::zoom`]).
    pub fn share(self) -> f32 {
        match self {
            Zoom::Near => 1.0,
            Zoom::Far => 2.0,
        }
    }

    pub fn name(self) -> &'static str {
        Zoom::NAMES[self as usize]
    }

    pub fn of(name: &str) -> Option<Zoom> {
        match name {
            "Near" => Some(Zoom::Near),
            "Far" => Some(Zoom::Far),
            _ => None,
        }
    }
}

/// The opt-in, and what the live view asks for this frame.
#[derive(Resource, Debug, Default)]
pub struct WristLive {
    /// The settings page's own row (Gameplay › Pip-Boy › Glance at the Pip-Boy), off
    /// until the player turns it on.
    pub on: bool,
    /// `NVF_WRIST_LIVE=1`: a developer's override that turns it on whatever the row says.
    pub forced: bool,
    /// `NVF_WRIST_FORCE_RAISE=1`: a developer's override that asks for the glance with no
    /// key at all, so a screenshot can be taken of what it draws. Not a row of the
    /// settings page and nothing a player meets: it only stands in for the key, so every
    /// blocker still closes the glance exactly as it does for a held key.
    pub held_up: bool,
    /// How its key works ([`Raise`]), the settings page's second row.
    pub style: Raise,
    /// Which picture its screen shows ([`Style`]), the settings page's third row.
    pub map_style: Style,
    /// Where in the frame it holds the device ([`Position`]), the settings page's fourth
    /// row.
    pub map_position: Position,
    /// How much of the map its screen shows ([`Zoom`]), the settings page's fifth row.
    pub map_zoom: Zoom,
    /// Which way up it holds the map ([`Turning`]), the settings page's sixth row.
    pub map_rotate: Turning,
    /// The player is asking for the glance now: the key is held past its moment
    /// ([`GLANCE_SECONDS`]) in Hold style, or the latch is on in Toggle style.
    pub asked: bool,
    /// The arm pose it asks for, while it owns the wrist.
    pub open: Option<Key>,
    /// The picture its screen should show, until the next sample replaces it. `None`
    /// means give the screen back.
    pub shown: Option<Shown>,
    /// Which page its screen shows ([`Page`]), remembered between glances and across a
    /// load: the player comes back to the page they left [design: the owner, 2026-11-21].
    pub page: Page,
    /// The rows that page draws ([`crate::wrist_page::rows`]), laid out at the glance's
    /// own rate and never per frame; empty on the MAP page, whose picture is the glass.
    pub rows: Vec<Row>,
    /// Seconds since the page was flipped, for the screen's flicker; `None` once the
    /// flicker is over or the glance is down.
    pub flick: Option<f32>,
    /// Bumped whenever the loaded game is replaced, so no binding outlives its session.
    session: u64,
    /// Seconds the arm has been coming up, as [`crate::pipboy::HOLD_SECONDS`] measures
    /// the menus' raise.
    arm: f32,
    /// When the glance may raise again after the real Pip-Boy went down.
    ready_at: Option<Duration>,
    live: Live,
}

/// One sample, as the screen's owner needs it: owned, so it outlives the frame that read
/// the world.
#[derive(Debug, Clone)]
pub struct Shown {
    pub key: Key,
    /// The picture the load order owns for this worldspace, as the interface names it, or
    /// the name of one laid out here ([`Shown::local`]).
    pub texture: String,
    /// The player's place on it, and the way they face (clockwise from north, picture y
    /// down).
    pub uv: [f32; 2],
    pub forward: [f32; 2],
    /// The share of the picture from the middle to an edge the screen shows
    /// (`wrist_live::MapPicture::half_window`).
    pub half: f32,
    /// The texels of the Local Map's outline when that is the base
    /// ([`crate::wrist_local`]); `None` for the worldspace's own picture.
    pub local: Option<LocalPic>,
    /// What is marked on the picture, in its own coordinates: the active quest's targets,
    /// the map markers and the doors the HUD compass's last look found
    /// ([`crate::compass::CompassNear`]), at most [`crate::wrist_live::MARKS_MOST`].
    pub marks: Vec<Mark>,
}

impl PartialEq for Shown {
    /// The same picture of the same place: the outline's by its texels rather than by
    /// every sample of them ([`LocalPic::is`]).
    fn eq(&self, other: &Self) -> bool {
        (self.key, &self.texture, self.uv, self.forward, self.half, &self.marks) == (other.key, &other.texture, other.uv, other.forward, other.half, &other.marks)
            && match (&self.local, &other.local) {
                (Some(a), Some(b)) => a.is(b),
                (a, b) => a.is_none() == b.is_none(),
            }
    }
}

/// Take the glance key's style by name; false: not one of ours.
fn take_style(world: &mut World, name: &str) -> bool {
    match Raise::of(name) {
        Some(style) => {
            world.resource_mut::<WristLive>().style = style;
            true
        }
        None => false,
    }
}

/// Take the picture the glance shows by name; false: not one of ours.
fn take_map_style(world: &mut World, name: &str) -> bool {
    match Style::of(name) {
        Some(style) => {
            world.resource_mut::<WristLive>().map_style = style;
            true
        }
        None => false,
    }
}

/// Take how much of the map the glance shows by name; false: not one of ours.
fn take_zoom(world: &mut World, name: &str) -> bool {
    match Zoom::of(name) {
        Some(z) => {
            world.resource_mut::<WristLive>().map_zoom = z;
            true
        }
        None => false,
    }
}

/// Take which way up the glance holds its map by name; false: not one of ours.
fn take_rotate(world: &mut World, name: &str) -> bool {
    match Turning::of(name) {
        Some(t) => {
            world.resource_mut::<WristLive>().map_rotate = t;
            true
        }
        None => false,
    }
}

/// Take where the glance holds the device by name; false: not one of ours.
fn take_position(world: &mut World, name: &str) -> bool {
    match Position::of(name) {
        Some(at) => {
            world.resource_mut::<WristLive>().map_position = at;
            true
        }
        None => false,
    }
}

/// How long the Pip-Boy key is held before the glance takes it instead of the menus: long
/// enough that an ordinary tap still opens the Pip-Boy, short enough to feel like a
/// glance [design: the owner, 2026-11-21].
pub const GLANCE_SECONDS: f32 = 0.25;

/// Whether the glance holds the hands' controls now, for `bindings::ControlInput::held`.
///
/// A global and not a resource, as `bindings::set_keys_with_pad` is: the view of the
/// input that [`decide`] itself reads is the same `ControlInput` every other system
/// reads, and one system cannot both write the glance's state and hold a view of the
/// input that reads it (Bevy's own B0002). [`decide`] clears it while it reads the
/// frame's keys — so a press of the hands' own controls is seen on the very frame it is
/// made, as the behaviour notes ask — and sets it again from the frame's decision, before
/// anything else in [`crate::PlaySet::Input`] or after it reads the input.
static GLANCE_HOLDS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn glance_holds() -> bool {
    GLANCE_HOLDS.load(std::sync::atomic::Ordering::Relaxed)
}

fn set_glance_holds(on: bool) {
    GLANCE_HOLDS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The controls the glance holds while the arm is up, so an attack cannot fire or draw a
/// weapon through it and the sights cannot zoom [design: the behaviour notes' own list].
/// The Pip-Boy's key is not among them: it is the glance's own key, and the menus must
/// stay reachable. Derived from [`WristLive::open`] every frame and never latched, so the
/// controls are back the frame the arm goes down.
/// The D-Pad's own two controls are among them while the arm is up: left and right turn
/// the device's pages instead of working the hotkey and the ammo swap they are bound to
/// ([`page_turn`]). Taken the same way as the rest — derived from the open intent, never
/// latched — so the frame the arm goes down they do their own job again.
pub fn holds(c: Control) -> bool {
    matches!(c, Control::Use | Control::Block | Control::ReadyItem | Control::Activate | Control::Vats | Control::AmmoSwap | Control::Hotkey4)
}

/// How long the screen flickers after a page is flipped: a scan across the glass and a
/// dimming, as a screen that has just been switched over [design: the owner's "a quick
/// screen flicker/scan transition", 2026-11-21].
pub const FLICK_SECONDS: f32 = 0.33;

/// The page flip the player asked for this frame: the device's own D-Pad left and right
/// (`GamepadButton::DPadLeft`, `DPadRight`, whatever they are bound to — the glance holds
/// those controls while it owns the wrist), and the arrow keys for a keyboard, which no
/// play control uses. Read raw and only on a frame the glance owns the wrist, so nothing
/// is swallowed while the arm is down [design: the owner, 2026-11-21].
fn page_turn(input: &ControlInput) -> i32 {
    let pad = |b: GamepadButton| input.pads.iter().any(|g| g.just_pressed(b));
    let left = pad(GamepadButton::DPadLeft) || input.keys.just_pressed(KeyCode::ArrowLeft);
    let right = pad(GamepadButton::DPadRight) || input.keys.just_pressed(KeyCode::ArrowRight);
    i32::from(right) - i32::from(left)
}

/// What the player does with their hands: pressed or just pressed, any of these ends the
/// glance on that frame, before the press is acted on. Attack and block must not fire or
/// zoom through a raised arm, readying must not draw it, and the Pip-Boy's own key is left
/// out on purpose — it has to keep working so the menus can still be opened.
const ACTIONS: [Control; 5] = [Control::Use, Control::Block, Control::ReadyItem, Control::Activate, Control::Vats];

/// How long the glance waits after the real Pip-Boy goes down [design: about the time the
/// menus take to lower, so one press does one thing].
const COOLDOWN: Duration = Duration::from_millis(600);

/// What of the HUD compass's last look the glance's map marks, and as what: the active
/// quest's targets and the player's own marker, the map markers of the places about them,
/// and the doors when the player asked the compass for those. The living — actors,
/// companions, whatever is hostile — is left off: the glance draws a map, not a radar
/// [design: the owner, 2026-11-21]. The compass finds them itself, ten times a second
/// (`crate::compass::gather`), so nothing here looks at the world.
fn marked(m: crate::compass::Mark) -> Option<MarkKind> {
    use crate::compass::Mark as C;
    match m {
        C::Quest => Some(MarkKind::Quest),
        C::Player => Some(MarkKind::Player),
        C::Place => Some(MarkKind::Place),
        C::Found => Some(MarkKind::Found),
        C::Door => Some(MarkKind::Door),
        C::Actor | C::Hostile | C::Companion => None,
    }
}

/// Where the glance's raise stands after `dt` seconds: up to the hold the menus use, and
/// straight back to nothing the frame it is let go. Nothing here touches input, so a menu
/// or a conversation opening in the middle of a raise leaves no control held and no pose
/// owed — the arm's own owner has it again on that frame [design: the behaviour notes in
/// spikes/001-live-wrist-world/README.md].
pub fn raised(arm: f32, dt: f32, open: bool) -> f32 {
    if open {
        (arm + dt.max(0.0)).clamp(0.0, crate::pipboy::HOLD_SECONDS)
    } else {
        0.0
    }
}

impl WristLive {
    /// Whether the mode is on at all: the settings row, or a developer's override.
    pub fn live(&self) -> bool {
        self.on || self.forced
    }

    /// What the screen should wear, or `None` when it should wear its own material: the
    /// one place that joins the opt-in, the open intent and the last sample.
    pub fn wants(&self) -> Option<&Shown> {
        self.shown.as_ref().filter(|_| self.live() && self.open.is_some())
    }

    /// The arm's hold-pose time while the live view owns it, for nvforever's body; `None`
    /// leaves the pose to its normal owner.
    pub fn arm_clip_time(&self) -> Option<f32> {
        self.open.filter(|_| self.live()).map(|_| self.arm)
    }

    /// Whether the glance holds the device under the middle of the view rather than low on
    /// the left (`wrist-map-position`), for nvforever's body.
    pub fn centred(&self) -> bool {
        self.map_position == Position::Centre
    }

    /// Whether the close picture is wanted now: the glance holds the arm, the player
    /// chose Local, and the MAP page is the one up, so [`crate::wrist_local`] casts its
    /// rays only for a frame that will show them.
    pub fn wants_local(&self) -> bool {
        self.live() && self.open.is_some() && self.map_style == Style::Local && self.page == Page::Map
    }

    /// The screen the glance dresses this frame, whatever page it shows: the MAP page
    /// needs a sample of the world to show anything, the other two need only the arm.
    /// `None` means give the shape its own material back.
    pub fn screen_key(&self) -> Option<Key> {
        match self.page {
            Page::Map => self.wants().map(|s| s.key),
            _ => self.open.filter(|_| self.live()),
        }
    }

    /// Whether the map turns with the player (`wrist-map-rotate`), for nvforever's
    /// screen: the picture turns under a mark that points up the glass, and the compass
    /// letters orbit its edge.
    pub fn rotate_map(&self) -> bool {
        self.map_rotate == Turning::WithMe
    }

    /// How far the flicker of a page flip has run, 0 to 1; `None` while none runs.
    pub fn flicker(&self) -> Option<f32> {
        self.flick.map(|t| (t / FLICK_SECONDS).clamp(0.0, 1.0))
    }
}

impl WristLive {
    /// Test aid: the resource as it stands while the glance holds the arm (`held`) or
    /// after it has let go, for nvforever's screen tests.
    pub fn held_for_test(held: bool, shown: Option<Shown>) -> WristLive {
        WristLive {
            on: true,
            open: shown.as_ref().map(|s| s.key).filter(|_| held),
            shown,
            arm: if held { crate::pipboy::HOLD_SECONDS } else { 0.0 },
            ..default()
        }
    }
}

/// What only nvforever's body knows, written there each frame and read by [`decide`] on
/// the frame after. Both fields fail closed: without them the mode stays shut.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct WristBody {
    /// The first-person arms are drawn (`PlayerBody::first_person_drawn`).
    pub first_person_drawn: bool,
    /// The arm model's screen shape (`PipBoyPart::Screen`), by its entity, so a rebuilt
    /// rig is a different screen.
    pub screen: Option<u64>,
    /// Another owner has the arm's materials: the Stealth Boy's field, which replaces
    /// every mesh's material with a see-through copy of its own
    /// (nvforever `stealth_field.rs`). The live view lets go rather than fight it.
    pub taken: bool,
}

/// The worldspace's picture and projection, read once per worldspace as the paused page's
/// `data::map_for` caches it.
#[derive(Default)]
pub(crate) struct Maps {
    key: Option<(u64, u32)>,
    map: Option<WorldMap>,
}

pub struct WristLivePlugin;

impl Plugin for WristLivePlugin {
    fn build(&self, app: &mut App) {
        let forced = std::env::var_os("NVF_WRIST_LIVE").is_some_and(|v| v == "1");
        if forced {
            info!("wrist: the glance is forced on (NVF_WRIST_LIVE=1), whatever the settings page says");
        }
        let held_up = std::env::var_os("NVF_WRIST_FORCE_RAISE").is_some_and(|v| v == "1");
        if held_up {
            info!("wrist: the glance is held up (NVF_WRIST_FORCE_RAISE=1, a developer's switch for screenshots), as though its key were never let go");
        }
        // A developer's switch for a screenshot of one page: not a row, and named nowhere
        // a player looks.
        let page = std::env::var_os("NVF_WRIST_PAGE").and_then(|v| Page::of(&v.to_string_lossy())).unwrap_or_default();
        if page != Page::default() {
            info!("wrist: the glance opens on the {} page (NVF_WRIST_PAGE, a developer's switch)", page.name());
        }
        app.insert_resource(WristLive { forced, held_up, page, ..default() })
            .init_resource::<WristBody>()
            // After the Menu Mode key's hold is read, which is what asks for the glance
            // (`crate::flashlight::menu_key`).
            .add_systems(Update, (decide, read_pages, foley).chain().in_set(crate::PlaySet::Input).after(crate::flashlight::menu_key));
        let row = |key, label, note| Setting { key, group: crate::pause::GROUP_MENUS, label, note, switches: "", get: |_| false, set: |_, _| {}, slider: None };
        add_setting(
            app,
            Setting {
                get: |w| w.resource::<WristLive>().on,
                set: |w, on| w.resource_mut::<WristLive>().on = on,
                ..row(
                    "wrist-map",
                    "Glance at the Pip-Boy",
                    "On: holding the Pip-Boy key or button raises the arm with the world map live on its screen, without opening the menus; a short tap opens the Pip-Boy as usual.",
                )
            },
        );
        add_choice_list(
            app,
            row("wrist-map-raise", "Glance key", "Hold: the arm is up while the key is held. Toggle: a hold raises it and the next hold puts it down."),
            ChoiceList {
                names: |_| Raise::NAMES.iter().map(|n| n.to_string()).collect(),
                chosen: |w| w.resource::<WristLive>().style.name().to_string(),
                pick: |w, n| take_style(w, n),
                save: |w| w.resource::<WristLive>().style.name().to_string(),
                load: |w, t| take_style(w, t),
                ask_keep: false,
            },
        );
        add_choice_list(
            app,
            row(
                "wrist-map-style",
                "Glance map",
                "Local: the place round you, drawn from what is there — the roads, the buildings and the walls about them. World: the worldspace's own map picture, as the World Map page shows it.",
            ),
            ChoiceList {
                names: |_| Style::NAMES.iter().map(|n| n.to_string()).collect(),
                chosen: |w| w.resource::<WristLive>().map_style.name().to_string(),
                pick: |w, n| take_map_style(w, n),
                save: |w| w.resource::<WristLive>().map_style.name().to_string(),
                load: |w, t| take_map_style(w, t),
                ask_keep: false,
            },
        );
        add_choice_list(
            app,
            row(
                "wrist-map-position",
                "Glance position",
                "Lower left: the arm comes up low on the left, leaving the middle of the picture clear to walk through. Centre: under the middle of the view, where the menus hold it.",
            ),
            ChoiceList {
                names: |_| Position::NAMES.iter().map(|n| n.to_string()).collect(),
                chosen: |w| w.resource::<WristLive>().map_position.name().to_string(),
                pick: |w, n| take_position(w, n),
                save: |w| w.resource::<WristLive>().map_position.name().to_string(),
                load: |w, t| take_position(w, t),
                ask_keep: false,
            },
        );
        add_choice_list(
            app,
            row("wrist-map-rotate", "Glance map turns", "With me: the map turns as you do, your mark points up the glass and N, E, S and W ride round its edge. North up: north stays at the top and your mark turns."),
            ChoiceList {
                names: |_| Turning::NAMES.iter().map(|n| n.to_string()).collect(),
                chosen: |w| w.resource::<WristLive>().map_rotate.name().to_string(),
                pick: |w, n| take_rotate(w, n),
                save: |w| w.resource::<WristLive>().map_rotate.name().to_string(),
                load: |w, t| take_rotate(w, t),
                ask_keep: false,
            },
        );
        add_choice_list(
            app,
            row("wrist-map-zoom", "Glance zoom", "Near: the place about you, a couple of hundred metres across the glass. Far: twice as much of the map either way."),
            ChoiceList {
                names: |_| Zoom::NAMES.iter().map(|n| n.to_string()).collect(),
                chosen: |w| w.resource::<WristLive>().map_zoom.name().to_string(),
                pick: |w, n| take_zoom(w, n),
                save: |w| w.resource::<WristLive>().map_zoom.name().to_string(),
                load: |w, t| take_zoom(w, t),
                ask_keep: false,
            },
        );
    }
}

/// The Pip-Boy's own raise and lower foley for the glance: the very SOUN records the
/// menus play when the device comes up and goes down (`crate::ui_sounds`:
/// `UIPipBoyAccessUp`, `UIPipBoyAccessDown`), at the listener, one on each change of the
/// arm's state [design: the clean-room notes §8; the owner, 2026-11-21]. Nothing of its
/// own is added — the names are the load order's — and nothing sounds while the mode is
/// off, since the arm is never up then.
pub(crate) fn foley(
    wrist: Res<WristLive>,
    sfx: Option<Res<crate::sfx::Sfx>>,
    mut out: MessageWriter<crate::sfx::PlaySound>,
    mut was: Local<bool>,
    mut page: Local<Page>,
    mut names: Local<Option<std::collections::HashMap<String, fnv_sim::FormId>>>,
) {
    let up = wrist.live() && wrist.open.is_some();
    // The device's own tab click when the page is flipped while the arm is up
    // (`crate::pipboy::sounds`' `UIPipBoyTab`, the very record the menus' tabs play).
    let turned = up && *was && *page != wrist.page;
    *page = wrist.page;
    if up == *was && !turned {
        return;
    }
    *was = up;
    let Some(sfx) = sfx else { return };
    let name = if turned {
        "UIPipBoyTab"
    } else if up {
        "UIPipBoyAccessUp"
    } else {
        "UIPipBoyAccessDown"
    };
    let sounds = names.get_or_insert_with(|| sfx.forms.sounds.iter().filter_map(|(&id, s)| s.edid.clone().map(|e| (e, id))).collect());
    match sounds.get(name) {
        Some(&sound) => {
            info!("wrist: the glance's foley: {name}");
            out.write(crate::sfx::PlaySound { sound, at: None });
        }
        None => warn!("wrist: no SOUN {name} for the glance"),
    }
}

/// The STATUS and QUESTS pages' facts, read off state the engine already keeps, at the
/// live view's own rate ([`crate::wrist_live::INTERVAL`], ten times a second) and only
/// while the glance holds the arm on a page that needs them. A closed wrist, the MAP page
/// and the mode off all read nothing at all.
///
/// The numbers are the HUD's own ([`crate::hud::HudView`]'s shares against the whole the
/// game keeps), so the page and the meters it replaces can never differ.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_pages(
    time: Res<Time<Real>>,
    mut wrist: ResMut<WristLive>,
    view: Option<Res<crate::hud::HudView>>,
    limbs: Option<Res<crate::combat::PlayerLimbs>>,
    data: Option<Res<crate::pipboy::data::DataState>>,
    script: Option<ResMut<crate::script::ScriptState>>,
    mut last: Local<Option<Duration>>,
    mut was: Local<Page>,
    mut facts: Local<Facts>,
) {
    if !wrist.live() || wrist.open.is_none() || wrist.page == Page::Map {
        if !wrist.rows.is_empty() {
            wrist.rows.clear();
        }
        // The next page that wants them reads at once rather than waiting its turn.
        *last = None;
        return;
    }
    let now = time.elapsed();
    let flipped = *was != wrist.page;
    *was = wrist.page;
    if !flipped && wrist.rows.is_empty() {
        // The first frame of a page: read now.
    } else if !flipped && last.is_some_and(|t| now.saturating_sub(t) < crate::wrist_live::INTERVAL) {
        return;
    }
    *last = Some(now);
    *facts = read_facts(script, view.as_deref(), limbs.as_deref(), data.as_deref());
    let rows = crate::wrist_page::rows(wrist.page, &facts);
    if wrist.rows != rows {
        wrist.rows = rows;
    }
}

/// The player's condition codes, as [`crate::wrist_page::LIMBS`] lists them.
const LIMB_CODES: [u16; 6] = [25, 26, 27, 28, 29, 30];

/// Total rads, the actor value the Pip-Boy's own dial shows (`pipboy_controls.rs`).
const RADS: u16 = 54;

/// What the pages show, from what the engine already keeps. Nothing is simulated here and
/// nothing is written: the game is borrowed mutably only because reading an actor value
/// is a mutable call.
fn read_facts(
    script: Option<ResMut<crate::script::ScriptState>>,
    view: Option<&crate::hud::HudView>,
    limbs: Option<&crate::combat::PlayerLimbs>,
    data: Option<&crate::pipboy::data::DataState>,
) -> Facts {
    let mut f = Facts::default();
    if let Some(v) = view {
        f.condition = v.weapon.condition;
        f.ammo = v.weapon.ammo;
        f.ammo_label.clone_from(&v.weapon.abbrev);
    }
    if let Some(l) = limbs {
        f.crippled = LIMB_CODES.map(|c| l.crippled(c));
    }
    let Some(mut script) = script else { return f };
    // Reading needs the game mutably; nothing is changed.
    let Some(rt) = script.bypass_change_detection().rt.as_mut() else { return f };
    let me = fnv_sim::FormId::PLAYER_REF;
    let temporary = |g: &fnv_game::Game, code| fnv_game::functions::actor_value_temporary(g, me, code);
    // The whole each bar is a share of, as `crate::hud::gather` measures it: an aid item's
    // temporary modifier raises the maximum with the value.
    let health_whole = rt.game.max_health(me) + temporary(&rt.game, fnv_game::actor_values::HEALTH);
    f.health = view.map(|v| ((f64::from(v.health) * health_whole).max(0.0), health_whole));
    if let Ok([base, modifier, _]) = fnv_game::functions::actor_value_parts(&mut rt.game, me, fnv_game::actor_values::ACTION_POINTS) {
        let whole = base + modifier + temporary(&rt.game, fnv_game::actor_values::ACTION_POINTS);
        f.action_points = view.map(|v| ((f64::from(v.action_points) * whole).max(0.0), whole));
    }
    if let Ok([base, modifier, damage]) = fnv_game::functions::actor_value_parts(&mut rt.game, me, RADS) {
        f.rads = Some((base + modifier + temporary(&rt.game, RADS) - damage).max(0.0));
    }
    // The active quest and the objective the player is on: the latest one shown and not
    // done, as the paused DATA page orders them (`pipboy::data::model::sort_objectives`).
    let Some(quest) = data.and_then(|d| d.active_quest(Some(&rt.game))) else { return f };
    let id = fnv_sim::FormId(quest);
    let Some(def) = rt.game.data.quests.get(&id) else { return f };
    f.quest.clone_from(&def.name);
    let Some(state) = rt.game.quests.get(&id) else { return f };
    let mut best: Option<(i32, String)> = None;
    for (i, o) in &state.objectives {
        if !o.displayed || o.completed || def.objectives.get(i).is_none() {
            continue;
        }
        if best.as_ref().is_some_and(|(at, _)| at >= i) {
            continue;
        }
        // SetObjectiveText's own words first (`fnv_game::nvse_jip10`), the load order's
        // otherwise; nothing of the original's screen is transcribed here.
        if let Some(text) = rt.game.objective_text(id, *i) {
            let text = rt.game.expand_codes(&text);
            if !text.trim().is_empty() {
                best = Some((*i, text.trim().to_string()));
            }
        }
    }
    f.objective = best.map(|(_, text)| text);
    f
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn decide(
    time: Res<Time<Real>>,
    mut wrist: ResMut<WristLive>,
    body: Res<WristBody>,
    close: Option<Res<crate::wrist_local::WristMap>>,
    state: Res<State<crate::PlayState>>,
    input: ControlInput,
    (pip, control, active): (Option<Res<crate::pipboy::PipBoy>>, Option<Res<PlayerControl>>, Option<Res<crate::menu::ActiveMenu>>),
    (location, world, player): (Option<Res<crate::Location>>, Option<Res<crate::PlayWorld>>, Option<Res<Player>>),
    (vats, kill_cam): (Option<Res<crate::vats_mode::VatsMode>>, Option<Res<crate::kill_cam::KillCam>>),
    (hold, near): (Option<Res<crate::flashlight::MenuKeyHold>>, Option<Res<crate::compass::CompassNear>>),
    mut maps: Local<Maps>,
    mut was_menus: Local<bool>,
    mut latched: Local<bool>,
    mut marks: Local<Vec<(MarkKind, [f32; 2])>>,
    mut refused: Local<bool>,
) {
    // Nothing is held while the frame's own keys are read, so a press of the hands' own
    // controls does its job on the frame it is made and only then ends the glance.
    set_glance_holds(false);
    if !wrist.live() {
        // Off: it asks for nothing, and anything it asked for before is withdrawn. The
        // Pip-Boy's key is left entirely alone, so the menus behave as they always did.
        if wrist.open.is_some() || wrist.shown.is_some() || wrist.asked || *latched {
            (wrist.open, wrist.shown, wrist.arm, wrist.asked, wrist.live) = (None, None, 0.0, false, Live::default());
            (wrist.flick, *latched, *refused) = (None, false, false);
            wrist.rows.clear();
        }
        return;
    }
    // A loaded game replaces the world: its bindings go with it.
    if world.as_ref().is_some_and(|w| w.is_added()) {
        wrist.session = wrist.session.wrapping_add(1);
    }
    let now = time.elapsed();
    // The real Pip-Boy going down starts a short wait, so the press that closed it cannot
    // raise the glance in the same breath (and the glance cannot fight the lowering arm).
    let menus = pip.as_deref().is_some_and(|p| p.shown() || p.warming.is_some());
    if *was_menus && !menus {
        wrist.ready_at = Some(now + COOLDOWN);
    }
    *was_menus = menus;
    // What the player asks for: their own key if they bound one, else the Pip-Boy key's
    // hold, which `crate::flashlight::menu_key`already measures for the light. A tap of that
    // key still opens the Pip-Boy; only a hold past `GLANCE_SECONDS` comes here.
    let bound = input.bound(Control::GlancePipBoy);
    let asking = if bound { input.pressed(Control::GlancePipBoy) } else { hold.as_deref().is_some_and(|h| h.glancing()) };
    let began = if bound { input.just_pressed(Control::GlancePipBoy) } else { asking && !wrist.asked };
    wrist.asked = asking;
    // The developer's switch stands in for the key being held down and nothing else.
    let want_open = wrist.held_up
        || match wrist.style {
            Raise::Hold => asking,
            // A hold flips the latch; letting go leaves it where it is.
            Raise::Toggle => {
                if began {
                    *latched = !*latched;
                }
                *latched
            }
        };
    let exterior = location.as_deref().filter(|l| l.interior.is_none()).map(|l| l.world);
    let play = Play {
        session: wrist.session,
        world: exterior,
        screen: body.screen,
        playing: *state.get() == crate::PlayState::Playing,
        first_person: body.first_person_drawn,
        input_suppressed: input.suppressed(),
        // Up, coming up, or going down: the menus own the arm through all of it.
        pipboy: menus,
        menu: active.as_deref().is_some_and(|a| a.0.is_some()),
        vats: vats.as_deref().is_some_and(|v| v.holds_input()) || kill_cam.as_deref().is_some_and(|k| k.active()),
        // A drawn weapon is no longer a blocker: the Pip-Boy clip poses both arms, so the
        // weapon hand is held down while the arm is up exactly as the menus' own raise
        // holds it (the owner, 2026-11-21). The sights still are.
        aiming: control.as_deref().is_some_and(|c| c.aiming),
        // Read afresh from this frame's input and never kept: the press is not swallowed,
        // the glance simply lets the arm go before the press is acted on
        // [design: the behaviour notes in spikes/001-live-wrist-world/README.md].
        acting: ACTIONS.iter().any(|&c| input.just_pressed(c) || input.pressed(c)),
        cooldown: wrist.ready_at.is_some_and(|t| now < t),
        // The Stealth Boy's field owns every material on the arm while it is on; the
        // screen's own owner checks the material it installed as well (`wrist_screen`).
        taken: body.taken,
    };
    let want = exterior.map(|w| (wrist.session, w));
    if maps.key != want {
        maps.key = want;
        maps.map = want.and_then(|(_, w)| world.as_deref().and_then(|d| read::world_map(d.lo, d.schema, w)));
    }
    let map = maps.map.as_ref();
    let here = player.as_deref().map(|p| ([p.feet[0], p.feet[1]], p.heading));
    // The base: the Local Map's outline of the place round the player where
    // `crate::wrist_local` has one that covers the window, else the picture the load
    // order owns for the worldspace, as the paused World Map page shows it.
    let outline = match (exterior, here, wrist.map_style) {
        (Some(w), Some((feet, _)), Style::Local) => close.as_deref().and_then(|c| c.ready.as_ref()).filter(|r| r.covers(None, w, feet)),
        _ => None,
    };
    // The key is the press: the rules open on its edge and close the moment it is
    // let go (`enabled` below), so nothing raises the arm by itself any more.
    let press = want_open && wrist.live.showing().is_none();
    let was = wrist.open;
    // What the compass found last, as the glance marks it: the same buffer every sample,
    // so no list of places is allocated per frame.
    marks.clear();
    if let Some(found) = near.as_deref().filter(|_| want_open) {
        marks.extend(found.0.iter().filter_map(|n| Some((marked(n.mark)?, n.at))).take(MARKS_MOST * 2));
    }
    let marks = marks.as_slice();
    let zoom = wrist.map_zoom.share();
    let frame = wrist.live.step(now, want_open, press, &play, || {
        let (feet, heading) = here?;
        let base: &dyn MapPicture = match outline {
            Some(close) => close,
            None => map?,
        };
        Some(Spot { map: base, feet, heading, marks, zoom })
    });
    if frame.clear.is_some() {
        wrist.shown = None;
    }
    if let Some(draw) = frame.draw {
        wrist.shown = Some(Shown { key: draw.key, texture: draw.texture.to_string(), uv: draw.uv, forward: draw.forward, half: draw.half, local: draw.local.cloned(), marks: draw.marks });
    }
    if frame.pose.is_none() {
        // An interruption (a menu, a conversation, a weapon) drops the latch too, so the
        // next hold opens again rather than closing something already gone.
        *latched = false;
    }
    wrist.open = frame.pose;
    wrist.arm = raised(wrist.arm, time.delta_secs(), frame.pose.is_some());
    // Pages are flipped only while the glance owns the wrist, and the flicker runs itself
    // out in real time: nothing here is latched, so the arm going down takes both with it.
    let dt = time.delta_secs();
    if frame.pose.is_some() {
        let turn = page_turn(&input);
        if turn != 0 {
            wrist.page = wrist.page.step(turn);
            wrist.flick = Some(0.0);
            info!("wrist: the glance turned to the {} page", wrist.page.name());
        } else {
            wrist.flick = wrist.flick.map(|t| t + dt.max(0.0)).filter(|t| *t < FLICK_SECONDS);
        }
    } else if wrist.flick.is_some() || !wrist.rows.is_empty() {
        wrist.flick = None;
        wrist.rows.clear();
    }
    // What the glance holds for the rest of this frame, from its own decision.
    set_glance_holds(frame.pose.is_some());
    // One line on each change, so a log says when the live view held the arm and why it
    // let go; nothing at all while it is closed.
    match (was, frame.pose) {
        (None, Some(key)) => info!("wrist: the live World Map is on the arm (worldspace {:#010x})", key.world),
        (Some(_), None) => info!("wrist: the live World Map let go of the arm: {}", play.eligible().err().map_or("nothing to show", |b| b.why())),
        _ => {}
    }
    // A key asked for the glance and nothing came up: say why, once for that press and
    // never again until it is let go, so a player's log shows what stopped it instead of
    // nothing at all [design: the owner, 2026-11-21]. Only for a glance that never opened
    // — one that is interrupted while it is up says so on the line above.
    if !want_open {
        *refused = false;
    } else if !*refused && was.is_none() && frame.pose.is_none() {
        *refused = true;
        info!("wrist: the glance was asked for and did not open: {}", play.eligible().err().map_or("nothing to show", |b| b.why()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Key {
        Key { session: 1, world: 0x0000_003C, screen: 7 }
    }

    /// A sample of the worldspace's own picture, or of an outline laid out here.
    fn shown_for(local: Option<LocalPic>) -> Shown {
        Shown {
            key: key(),
            texture: "map.dds".to_string(),
            uv: [0.5; 2],
            forward: [0.0, -1.0],
            half: crate::wrist_live::HALF_WINDOW,
            local,
            marks: Vec::new(),
        }
    }

    /// Without the opt-in nothing asks for the arm or its screen, whatever the resource
    /// holds: both nvforever systems and `player_body`'s pose go through these two.
    #[test]
    fn the_mode_asks_for_nothing_at_all_while_it_is_off() {
        let off = WristLive {
            on: false,
            open: Some(key()),
            shown: Some(shown_for(None)),
            ..default()
        };
        assert_eq!(off.wants(), None);
        assert_eq!(off.arm_clip_time(), None);
        // The default (what the plugin inserts without `NVF_WRIST_LIVE=1`) likewise.
        let fresh = WristLive::default();
        assert!(!fresh.live() && fresh.wants().is_none() && fresh.arm_clip_time().is_none());
    }

    /// A menu, a conversation or a weapon coming out in the middle of a raise: the raise
    /// is given up whole on that frame, so nothing is left half held.
    #[test]
    fn the_raise_goes_back_to_nothing_the_frame_it_is_let_go() {
        let mut arm = 0.0;
        for _ in 0..3 {
            arm = raised(arm, 1.0 / 60.0, true);
        }
        assert!(arm > 0.0 && arm < crate::pipboy::HOLD_SECONDS, "mid-raise, not held: {arm}");
        assert_eq!(raised(arm, 1.0 / 60.0, false), 0.0);
        // Held: it stops at the menus' own hold and stays there.
        let mut arm = 0.0;
        for _ in 0..60 {
            arm = raised(arm, 1.0 / 60.0, true);
        }
        assert_eq!(arm, crate::pipboy::HOLD_SECONDS);
        assert_eq!(raised(arm, 1.0, true), crate::pipboy::HOLD_SECONDS);
    }


    /// The glance holds the hands' controls while the arm is up, and holds nothing the
    /// moment it is down: the rule is read from the open intent, never remembered, so an
    /// interruption cannot leave a control stuck (the behaviour notes' own bug class).
    #[test]
    fn the_hands_controls_are_held_only_while_the_arm_is_up() {
        for c in [Control::Use, Control::Block, Control::ReadyItem, Control::Activate, Control::Vats] {
            assert!(holds(c), "{c:?} is the glance's to hold while looking");
        }
        // Never the Pip-Boy's own key (it is the glance's key), nor moving about.
        for c in [Control::MenuMode, Control::Forward, Control::Jump, Control::CrouchSneak, Control::QuickSave] {
            assert!(!holds(c), "{c:?} must keep working while looking");
        }
        // Nothing is held until a frame's decision says so, and nothing is remembered:
        // `decide` clears it while it reads the frame's keys and sets it from its own
        // decision (the controls are read through `bindings::ControlInput::held`).
        set_glance_holds(false);
        assert!(!glance_holds(), "a frame that has not decided yet holds nothing");
        set_glance_holds(true);
        assert!(glance_holds());
        set_glance_holds(false);
        // And the whole rule hangs on the open intent: with no arm up there is nothing to
        // hold, whatever was held a frame ago.
        let up = WristLive::held_for_test(true, None);
        let down = WristLive::held_for_test(false, None);
        assert!(up.live() && up.open.is_none(), "no picture yet is still no arm");
        assert!(down.open.is_none() && !down.asked);
    }

    /// Every system of the glance, with the one that follows it, built into one schedule:
    /// a parameter that holds something another holds (Bevy's B0002) panics here, in the
    /// tests, instead of on the first frame of a shipped build — which is how the port's
    /// `ControlInput` reading `WristLive` while `decide` wrote it was found (the spike's
    /// notes, 2026-11-21). **Every system this mode adds belongs in this list.**
    #[test]
    fn the_glances_systems_have_no_conflicting_parameters() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems((decide, read_pages, foley, crate::wrist_local::build).chain());
        schedule.initialize(&mut world).expect("the schedule builds");
    }

    /// The D-Pad's two controls are the glance's while the arm is up, so a flip does not
    /// also swap ammo or fire a hotkey; the moment it is down they are theirs again.
    #[test]
    fn the_dpad_turns_the_pages_only_while_the_arm_is_up() {
        for c in [Control::AmmoSwap, Control::Hotkey4] {
            assert!(holds(c), "{c:?} turns a page while looking");
        }
        // The pages themselves run round and the glance remembers the one it was left on.
        let mut wrist = WristLive { on: true, open: Some(key()), ..default() };
        assert_eq!(wrist.page, Page::Map);
        wrist.page = wrist.page.step(1);
        assert_eq!(wrist.page, Page::Status);
        // The close picture is cast for the MAP page alone: a page of letters needs none.
        assert!(!wrist.wants_local());
        wrist.page = Page::Map;
        assert!(wrist.wants_local());
    }

    /// The STATUS and QUESTS pages dress the screen on the arm alone; the MAP page waits
    /// for a sample of the world, as it always did.
    #[test]
    fn a_page_of_letters_needs_no_sample_of_the_world() {
        let open = WristLive { on: true, open: Some(key()), ..default() };
        assert_eq!(open.screen_key(), None, "the MAP page with nothing sampled shows nothing");
        let with_map = WristLive { on: true, open: Some(key()), shown: Some(shown_for(None)), ..default() };
        assert_eq!(with_map.screen_key(), Some(key()));
        let status = WristLive { on: true, open: Some(key()), page: Page::Status, ..default() };
        assert_eq!(status.screen_key(), Some(key()), "a page of letters dresses the screen at once");
        // Nothing at all while the arm is down or the mode is off.
        assert_eq!(WristLive { on: true, page: Page::Status, ..default() }.screen_key(), None);
        assert_eq!(WristLive { open: Some(key()), page: Page::Status, ..default() }.screen_key(), None);
    }

    /// The flicker of a page flip runs itself out in real time and is never latched.
    #[test]
    fn the_flip_flickers_once_and_runs_out() {
        let fresh = WristLive { flick: Some(0.0), ..default() };
        assert_eq!(fresh.flicker(), Some(0.0));
        assert_eq!(WristLive { flick: Some(FLICK_SECONDS / 2.0), ..default() }.flicker(), Some(0.5));
        assert_eq!(WristLive { flick: Some(FLICK_SECONDS * 2.0), ..default() }.flicker(), Some(1.0));
        assert_eq!(WristLive::default().flicker(), None);
    }

    /// How much of the map the glance shows is the player's: Near by default (the window
    /// the picture itself asks for), Far twice as much of it either way.
    #[test]
    fn the_glance_shows_the_near_window_until_the_player_asks_for_more() {
        assert_eq!(Zoom::default(), Zoom::Near);
        assert_eq!((Zoom::Near.share(), Zoom::Far.share()), (1.0, 2.0));
        assert_eq!((Zoom::Near.name(), Zoom::Far.name()), ("Near", "Far"));
        assert_eq!(Zoom::of("Far"), Some(Zoom::Far));
        assert_eq!(Zoom::of("Furthest"), None);
    }

    /// Where the device is held is the player's: Lower left by default, Centre on asking,
    /// and nothing else is a name of this row.
    #[test]
    fn the_glance_holds_the_device_lower_left_until_the_player_asks_for_the_middle() {
        assert_eq!(Position::default(), Position::LowerLeft);
        assert_eq!((Position::LowerLeft.name(), Position::Centre.name()), ("Lower left", "Centre"));
        assert_eq!(Position::of("Centre"), Some(Position::Centre));
        assert_eq!(Position::of("Lower right"), None);
        assert!(!WristLive::default().centred());
        assert!(WristLive { map_position: Position::Centre, ..default() }.centred());
    }

    /// Hold style asks while the key is held; Toggle style flips on each hold and an
    /// interruption drops the latch.
    #[test]
    fn hold_asks_while_held_and_toggle_flips() {
        let (hold, toggle) = (Raise::Hold, Raise::Toggle);
        assert_eq!(hold.name(), "Hold");
        assert_eq!(Raise::of("Toggle"), Some(toggle));
        assert_eq!(Raise::of("Whenever"), None);
        // Hold: asking follows the key.
        let want = |style: Raise, asking: bool, began: bool, latch: &mut bool| match style {
            Raise::Hold => asking,
            Raise::Toggle => {
                if began {
                    *latch = !*latch;
                }
                *latch
            }
        };
        let mut latch = false;
        assert!(want(hold, true, true, &mut latch));
        assert!(!want(hold, false, false, &mut latch));
        // Toggle: the first hold opens, the key going up leaves it open, the next closes.
        assert!(want(toggle, true, true, &mut latch), "the first hold opens it");
        assert!(want(toggle, false, false, &mut latch), "letting go leaves it up");
        assert!(!want(toggle, true, true, &mut latch), "the next hold puts it down");
    }

    /// With the opt-in, the screen follows the open intent: no pose, no picture.
    #[test]
    fn the_screen_is_asked_for_only_while_the_arm_is_held() {
        let shown = shown_for(None);
        let closed = WristLive { on: true, open: None, shown: Some(shown.clone()), ..default() };
        assert_eq!((closed.wants(), closed.arm_clip_time()), (None, None));
        let open = WristLive { on: true, open: Some(key()), shown: Some(shown.clone()), arm: 0.25, ..default() };
        assert_eq!((open.wants(), open.arm_clip_time()), (Some(&shown), Some(0.25)));
    }

    /// The rays of the close picture are cast only for a frame that will show it: a
    /// closed wrist, or the mode off, asks `crate::wrist_local` for nothing at all.
    #[test]
    fn the_close_picture_is_asked_for_only_while_the_arm_is_held() {
        assert!(WristLive { on: true, open: Some(key()), ..default() }.wants_local());
        assert!(WristLive { forced: true, open: Some(key()), ..default() }.wants_local());
        for quiet in [WristLive { on: true, open: None, ..default() }, WristLive { on: false, open: Some(key()), ..default() }, WristLive::default()] {
            assert!(!quiet.wants_local());
        }
    }

    /// The picture the player chose: Local by default, and the close picture is made and
    /// shown only for it.
    #[test]
    fn the_glance_shows_the_local_map_until_the_player_asks_for_the_world() {
        assert_eq!(Style::default(), Style::Local);
        assert_eq!((Style::Local.name(), Style::World.name()), ("Local", "World"));
        assert_eq!(Style::of("World"), Some(Style::World));
        assert_eq!(Style::of("Wherever"), None);
        assert!(WristLive { on: true, open: Some(key()), ..default() }.wants_local());
        assert!(!WristLive { on: true, open: Some(key()), map_style: Style::World, ..default() }.wants_local());
        // Which picture is shown is all it changes: the glance itself still opens, holds
        // the arm and shows the worldspace's own picture either way.
        let world = WristLive { on: true, open: Some(key()), map_style: Style::World, shown: Some(shown_for(None)), arm: 0.25, ..default() };
        assert_eq!((world.wants(), world.arm_clip_time()), (Some(&shown_for(None)), Some(0.25)));
    }

    /// Two samples of the same outline are the same sample (the texels are compared by
    /// what they are, not by every byte), and one of another outline is not.
    #[test]
    fn a_sample_of_the_same_outline_is_the_same_sample() {
        let pic = || LocalPic { alpha: std::sync::Arc::new(vec![3; 64]), size: [8, 8] };
        let (a, b) = (pic(), pic());
        assert_eq!(shown_for(Some(a.clone())), shown_for(Some(a.clone())));
        assert_ne!(shown_for(Some(a.clone())), shown_for(Some(b)), "another outline, however alike");
        assert_ne!(shown_for(Some(a)), shown_for(None), "the worldspace's own picture is not it");
    }
}
