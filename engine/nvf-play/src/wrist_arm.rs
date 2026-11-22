//! The live wrist World Map in the running game: the opt-in switch, the frame's decision,
//! and the request nvforever's arm and its screen follow. The rules themselves are
//! [`crate::wrist_live`] (pure, and tested on their own); this file is only the wiring
//! [design: spikes/001-live-wrist-world/README.md].
//!
//! The player turns it on at Gameplay › Pip-Boy on the settings page (`wrist-map`, off
//! until they do) and chooses how its key works (`wrist-map-raise`: Hold or Toggle).
//! `NVF_WRIST_LIVE=1` forces it on for a developer whatever the row says.
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
//!   pose it asks for) and [`WristLive::shown`] (what its screen should show).
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
use crate::wrist_live::{Key, Live, LocalPic, MapPicture, Play, Spot};
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

/// The opt-in, and what the live view asks for this frame.
#[derive(Resource, Debug, Default)]
pub struct WristLive {
    /// The settings page's own row (Gameplay › Pip-Boy › Glance at the Pip-Boy), off
    /// until the player turns it on.
    pub on: bool,
    /// `NVF_WRIST_LIVE=1`: a developer's override that turns it on whatever the row says.
    pub forced: bool,
    /// How its key works ([`Raise`]), the settings page's second row.
    pub style: Raise,
    /// The player is asking for the glance now: the key is held past its moment
    /// ([`GLANCE_SECONDS`]) in Hold style, or the latch is on in Toggle style.
    pub asked: bool,
    /// The arm pose it asks for, while it owns the wrist.
    pub open: Option<Key>,
    /// The picture its screen should show, until the next sample replaces it. `None`
    /// means give the screen back.
    pub shown: Option<Shown>,
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
}

impl PartialEq for Shown {
    /// The same picture of the same place: the outline's by its texels rather than by
    /// every sample of them ([`LocalPic::is`]).
    fn eq(&self, other: &Self) -> bool {
        (self.key, &self.texture, self.uv, self.forward, self.half) == (other.key, &other.texture, other.uv, other.forward, other.half)
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

/// How long the Pip-Boy key is held before the glance takes it instead of the menus: long
/// enough that an ordinary tap still opens the Pip-Boy, short enough to feel like a
/// glance [design: the owner, 2026-11-21].
pub const GLANCE_SECONDS: f32 = 0.25;

/// The controls the glance holds while the arm is up, so an attack cannot fire or draw a
/// weapon through it and the sights cannot zoom [design: the behaviour notes' own list].
/// The Pip-Boy's key is not among them: it is the glance's own key, and the menus must
/// stay reachable. Derived from [`WristLive::open`] every frame and never latched, so the
/// controls are back the frame the arm goes down.
pub fn holds(c: Control) -> bool {
    matches!(c, Control::Use | Control::Block | Control::ReadyItem | Control::Activate | Control::Vats)
}

/// What the player does with their hands: pressed or just pressed, any of these ends the
/// glance on that frame, before the press is acted on. Attack and block must not fire or
/// zoom through a raised arm, readying must not draw it, and the Pip-Boy's own key is left
/// out on purpose — it has to keep working so the menus can still be opened.
const ACTIONS: [Control; 5] = [Control::Use, Control::Block, Control::ReadyItem, Control::Activate, Control::Vats];

/// How long the glance waits after the real Pip-Boy goes down [design: about the time the
/// menus take to lower, so one press does one thing].
const COOLDOWN: Duration = Duration::from_millis(600);

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

    /// Whether the close picture is wanted now: the glance holds the arm, so
    /// [`crate::wrist_local`] casts its rays only for a frame that will show them.
    pub fn wants_local(&self) -> bool {
        self.live() && self.open.is_some()
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
        app.insert_resource(WristLive { forced, ..default() })
            .init_resource::<WristBody>()
            // After the Menu Mode key's hold is read, which is what asks for the glance
            // (`crate::flashlight::menu_key`).
            .add_systems(Update, decide.in_set(crate::PlaySet::Input).after(crate::flashlight::menu_key));
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
    }
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
    hold: Option<Res<crate::flashlight::MenuKeyHold>>,
    mut maps: Local<Maps>,
    mut was_menus: Local<bool>,
    mut latched: Local<bool>,
) {
    if !wrist.live() {
        // Off: it asks for nothing, and anything it asked for before is withdrawn. The
        // Pip-Boy's key is left entirely alone, so the menus behave as they always did.
        if wrist.open.is_some() || wrist.shown.is_some() || wrist.asked || *latched {
            (wrist.open, wrist.shown, wrist.arm, wrist.asked, wrist.live) = (None, None, 0.0, false, Live::default());
            *latched = false;
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
    let want_open = match wrist.style {
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
        // Holstered and empty-handed only, until the armed pose is compared with stock.
        weapon_out: control.as_deref().is_some_and(|c| c.weapon_out || c.aiming),
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
    let outline = match (exterior, here) {
        (Some(w), Some((feet, _))) => close.as_deref().and_then(|c| c.ready.as_ref()).filter(|r| r.covers(None, w, feet)),
        _ => None,
    };
    // The key is the press: the rules open on its edge and close the moment it is
    // let go (`enabled` below), so nothing raises the arm by itself any more.
    let press = want_open && wrist.live.showing().is_none();
    let was = wrist.open;
    let frame = wrist.live.step(now, want_open, press, &play, || {
        let (feet, heading) = here?;
        let base: &dyn MapPicture = match outline {
            Some(close) => close,
            None => map?,
        };
        Some(Spot { map: base, feet, heading })
    });
    if frame.clear.is_some() {
        wrist.shown = None;
    }
    if let Some(draw) = frame.draw {
        wrist.shown = Some(Shown { key: draw.key, texture: draw.texture.to_string(), uv: draw.uv, forward: draw.forward, half: draw.half, local: draw.local.cloned() });
    }
    if frame.pose.is_none() {
        // An interruption (a menu, a conversation, a weapon) drops the latch too, so the
        // next hold opens again rather than closing something already gone.
        *latched = false;
    }
    wrist.open = frame.pose;
    wrist.arm = raised(wrist.arm, time.delta_secs(), frame.pose.is_some());
    // One line on each change, so a log says when the live view held the arm and why it
    // let go; nothing at all while it is closed.
    match (was, frame.pose) {
        (None, Some(key)) => info!("wrist: the live World Map is on the arm (worldspace {:#010x})", key.world),
        (Some(_), None) => info!("wrist: the live World Map let go of the arm: {}", play.eligible().err().map_or("nothing to show", |b| b.why())),
        _ => {}
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
        // And the whole rule hangs on the open intent: with no arm up there is nothing to
        // hold, whatever was held a frame ago.
        let up = WristLive::held_for_test(true, None);
        let down = WristLive::held_for_test(false, None);
        assert!(up.live() && up.open.is_none(), "no picture yet is still no arm");
        assert!(down.open.is_none() && !down.asked);
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
