//! The World Map live on the wrist while the game runs (opt-in; first slice of
//! spikes/001-live-wrist-world).
//!
//! The ordinary Pip-Boy pauses the game, poses the arm and draws its menus into
//! [`crate::pipboy::PipBoyScreen`]. This module holds the rules of a second, opt-in mode:
//! the player's own key puts the picture the load order already owns for the worldspace
//! they are standing in on the arm's screen shape, with their position and heading on it,
//! without opening a menu, pausing, or touching the normal Pip-Boy's state
//! [design: spikes/001-live-wrist-world/README.md].
//!
//! It keeps no Bevy state on purpose. Everything here is a decision made from plain
//! values, so the rules are tested without a renderer, a window or game data, and the
//! systems that will feed it (the key's edge, the arm pose, the screen material) stay the
//! only part that needs the game to judge. Those systems are **not written yet**: nothing
//! in the engine calls [`Live::step`] so far, and the spike's README says what that
//! adapter still has to prove on the real rig.
//!
//! The rules [design]:
//!
//! - Off by default, and closed when off. The mode opens only on a rising edge of its own
//!   action, in a frame that is eligible ([`Play::eligible`]): ordinary first-person play,
//!   weapon away, an exterior worldspace, and an arm with a screen shape of its own. The
//!   same press closes it again.
//! - Every frame is judged afresh and fails closed. An ineligible frame (a menu, the
//!   normal Pip-Boy, a conversation, a loading screen, V.A.T.S., suppressed input, third
//!   person, a rig or session that was replaced) gives the pose and the screen back and
//!   disarms the mode: it takes another press to come back. An interruption in the same
//!   frame as a press wins.
//! - At most one sample per [`INTERVAL`], counted across closing and reopening, so the
//!   live view costs a bounded amount of work and never catches up on missed frames.
//! - A sample that cannot be used (no picture, a position off the picture, a heading or a
//!   position that is not a number) clears what was drawn rather than leaving a stale one.
//! - Travel to another worldspace in the same session clears at once but keeps the
//!   player's open intent, so the wrist goes blank until the next allowed sample instead
//!   of showing the map of where they were. Crossing a cell boundary inside one
//!   worldspace changes nothing.
//! - The caller applies [`Frame::clear`] before [`Frame::draw`], and restores only what
//!   [`Live::owns`] still says is its own.

use std::time::Duration;

/// How often the live view may look at the world [design: 10 Hz is enough for a map
/// arrow and bounds the cost of the mode; the picture itself never changes].
pub const INTERVAL: Duration = Duration::from_millis(100);

/// What the live view is bound to: a binding survives nothing of this changing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    /// The loaded game's generation: a new game, a load or a reload is a new number.
    pub session: u64,
    /// The exterior worldspace (WRLD FormID) the player is in.
    pub world: u32,
    /// The arm's screen shape this binding was made for.
    pub screen: u64,
}

/// Why the live view may not own the wrist this frame. The first reason in
/// [`Play::eligible`]'s order is the one reported, so a log line is stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    NotPlaying,
    InputSuppressed,
    Menu,
    PipBoy,
    Vats,
    NotFirstPerson,
    WeaponOut,
    Acting,
    Taken,
    Cooldown,
    Interior,
    NoScreen,
}

impl Block {
    /// NVForever's own words for a log line (never the original's).
    pub fn why(&self) -> &'static str {
        match self {
            Block::NotPlaying => "the game is not in play",
            Block::InputSuppressed => "the player's controls are off",
            Block::Menu => "a menu owns the screen",
            Block::PipBoy => "the Pip-Boy menus own the arm",
            Block::Vats => "V.A.T.S. owns the camera",
            Block::NotFirstPerson => "the first-person arms are not drawn",
            Block::WeaponOut => "a weapon is in hand",
            Block::Acting => "the player is doing something with their hands",
            Block::Taken => "another owner holds the arm",
            Block::Cooldown => "the Pip-Boy has only just gone down",
            Block::Interior => "there is no exterior worldspace here",
            Block::NoScreen => "the arm's model has no screen shape",
        }
    }
}

/// The frame as the adapter reads it off existing state. Every field is a fact the engine
/// already keeps; none of it is a second pause or menu policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct Play {
    /// The loaded game's generation (see [`Key::session`]).
    pub session: u64,
    /// The exterior worldspace the player is in (nvf-play `pipboy::data`'s location),
    /// `None` in an interior.
    pub world: Option<u32>,
    /// The first-person arm's screen shape (`PipBoyPart::Screen`, nvforever
    /// `pipboy_arm.rs`), as a number that changes when the rig is rebuilt; `None` when
    /// the arm has no screen shape.
    pub screen: Option<u64>,
    /// [`crate::PlayState::Playing`]: no menu, no conversation, no loading screen.
    pub playing: bool,
    /// The first-person body is drawn (nvforever `PlayerBody::first_person_drawn`).
    pub first_person: bool,
    /// Input is suppressed (a script's `DisablePlayerControls`;
    /// `bindings::ControlInput::suppressed`).
    pub input_suppressed: bool,
    /// The ordinary Pip-Boy is up, coming up or going down (`pipboy::PipBoy::on_arm`,
    /// `PipBoy::warming`).
    pub pipboy: bool,
    /// Another menu or a pause owns the screen or the cursor.
    pub menu: bool,
    /// V.A.T.S. or a kill cam owns the camera (`vats_mode`, `kill_cam`).
    pub vats: bool,
    /// A weapon is drawn or the sights are up. The first pass is holstered, empty-handed
    /// first person only, until the armed pose has been compared against stock.
    pub weapon_out: bool,
    /// Another owner holds the arm pose or the screen material this frame.
    pub taken: bool,
    /// The player is acting this very frame: attack, ready or reload, block or aim,
    /// activate, V.A.T.S. — or an animation the game drives (a reload, an equip, an idle
    /// or a scripted move). The glance lets go before the action is read, so the press
    /// does its own job on the frame it is made and never through a raised arm.
    pub acting: bool,
    /// The real Pip-Boy has only just gone down: the press that closed it must not raise
    /// the glance again in the same breath.
    pub cooldown: bool,
}

impl Play {
    /// The binding this frame allows, or the first reason it allows none.
    pub fn eligible(&self) -> Result<Key, Block> {
        let blockers = [
            (!self.playing, Block::NotPlaying),
            (self.input_suppressed, Block::InputSuppressed),
            (self.menu, Block::Menu),
            (self.pipboy, Block::PipBoy),
            (self.vats, Block::Vats),
            (!self.first_person, Block::NotFirstPerson),
            (self.weapon_out, Block::WeaponOut),
            // Before the frame's action is read, so the press acts on its own frame.
            (self.acting, Block::Acting),
            (self.taken, Block::Taken),
            (self.cooldown, Block::Cooldown),
        ];
        if let Some(why) = blockers.into_iter().find_map(|(blocked, why)| blocked.then_some(why)) {
            return Err(why);
        }
        Ok(Key { session: self.session, world: self.world.ok_or(Block::Interior)?, screen: self.screen.ok_or(Block::NoScreen)? })
    }
}

/// A worldspace's map picture and its projection, as the live view needs them.
/// `nvf-play`'s `pipboy::data::model::WorldMap` implements it with the projection the
/// paused World Map page already uses; the live view adds no map maths of its own.
pub trait MapPicture {
    /// The picture as the interface names it (under `textures\`).
    fn map_texture(&self) -> &str;
    /// Where a position in this worldspace falls on the picture, in texture coordinates.
    fn map_uv(&self, x: f32, y: f32) -> [f32; 2];
}

/// Where the player is, for one sample: their feet in game space (x east, y north) and
/// their compass heading in radians (0 north, as `player::Player::heading` keeps it).
pub struct Spot<'a> {
    pub map: &'a dyn MapPicture,
    pub feet: [f32; 2],
    pub heading: f32,
}

impl<'a> Spot<'a> {
    fn draw(self, key: Key) -> Option<Draw<'a>> {
        let texture = self.map.map_texture();
        // A broken or unloaded record, or a position the physics left as NaN: draw nothing
        // rather than a picture of nowhere.
        if texture.is_empty() || !self.heading.is_finite() || !self.feet.iter().all(|v| v.is_finite()) {
            return None;
        }
        let uv = self.map.map_uv(self.feet[0], self.feet[1]);
        if !uv.iter().all(|v| (0.0..=1.0).contains(v)) {
            return None;
        }
        // Clockwise from north on a picture whose y runs down, so the arrow points the way
        // the player faces.
        let (sin, cos) = self.heading.sin_cos();
        Some(Draw { key, texture, uv, forward: [sin, -cos] })
    }
}

/// What to put on the arm's screen: the owned picture, the player's place on it, and the
/// way they face.
#[derive(Debug, PartialEq)]
pub struct Draw<'a> {
    pub key: Key,
    pub texture: &'a str,
    pub uv: [f32; 2],
    pub forward: [f32; 2],
}

/// One frame's requests, in the order the caller applies them.
#[derive(Debug, Default, PartialEq)]
pub struct Frame<'a> {
    /// Ask for the arm's hold pose and keep the live screen, for as long as it is open.
    /// Asked on every open frame, not only on the frames that sample.
    pub pose: Option<Key>,
    /// Give back the pose and the screen material this key installed. Applied first.
    pub clear: Option<Key>,
    /// The new picture to show, at most one per [`INTERVAL`].
    pub draw: Option<Draw<'a>>,
}

/// The share of the map picture the wrist shows from the middle to an edge [design: a
/// legible slice of the worldspace at the screen's size, where the paused page shows the
/// whole picture].
pub const HALF_WINDOW: f32 = 0.08;

/// How the arm's screen samples the map picture: `sample = matrix · mesh_uv + offset`,
/// `matrix` by columns, which is what a material's UV transform (`Affine2`) carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub matrix: [f32; 4],
    pub offset: [f32; 2],
}

impl Window {
    /// Where the shape's own texture coordinate `mesh` falls on the map picture.
    pub fn at(&self, mesh: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d] = self.matrix;
        [a * mesh[0] + c * mesh[1] + self.offset[0], b * mesh[0] + d * mesh[1] + self.offset[1]]
    }
}

/// The window that puts the player's place `uv` in the middle of a screen whose own
/// texture coordinates run to `extent` (the arm's screen shape, nvforever
/// `pipboy_arm::UV_EXTENT`), the way they face ([`Draw::forward`]) up the screen, and
/// `half` of the picture from the middle to an edge. Numbers it cannot use fall back to a
/// north-up window, so a broken record cannot turn the picture inside out.
pub fn window(uv: [f32; 2], forward: [f32; 2], half: f32, extent: f32) -> Window {
    let extent = if extent.is_finite() && extent > 0.0 { extent } else { 1.0 };
    let half = if half.is_finite() && half > 0.0 { half } else { 0.5 };
    let len = (forward[0] * forward[0] + forward[1] * forward[1]).sqrt();
    let [fx, fy] = if len.is_finite() && len > 1e-6 { [forward[0] / len, forward[1] / len] } else { [0.0, -1.0] };
    // The turn that takes the screen's up to the way the player faces: a rotation whose
    // sine is `fx` and cosine `-fy`, so a northward facing leaves the picture north up.
    let k = 2.0 * half / extent;
    let matrix = [k * -fy, k * fx, k * -fx, k * -fy];
    let corner = [-fy - fx, fx - fy];
    Window { matrix, offset: [uv[0] - half * corner[0], uv[1] - half * corner[1]] }
}

/// The live view's whole state: an open intent, what is on the screen, and when the world
/// was last looked at.
#[derive(Debug, Default)]
pub struct Live {
    open: Option<Key>,
    bound: Option<Key>,
    sampled: Option<Duration>,
}

impl Live {
    /// One frame. `now` is monotonic real time (the mode does not follow the game clock),
    /// `enabled` the player's opt-in, `pressed` a rising edge of the mode's own action,
    /// `play` this frame's state, and `spot` a lazy look at the world that is called at
    /// most once per [`INTERVAL`] and never while closed or ineligible.
    pub fn step<'a>(&mut self, now: Duration, enabled: bool, pressed: bool, play: &Play, spot: impl FnOnce() -> Option<Spot<'a>>) -> Frame<'a> {
        let eligible = play.eligible().ok().filter(|_| enabled);
        self.open = match (self.open, eligible, pressed) {
            // Open: the same session and the same screen shape keep it, through a change
            // of worldspace (which the binding below then clears).
            (Some(open), Some(key), false) if (open.session, open.screen) == (key.session, key.screen) => Some(key),
            (None, Some(key), true) => Some(key),
            _ => None,
        };
        let mut frame = Frame { pose: self.open, ..Frame::default() };
        if self.bound != self.open {
            frame.clear = self.bound.take();
        }
        let Some(key) = self.open else { return frame };
        if self.sampled.is_some_and(|last| now.saturating_sub(last) < INTERVAL) {
            return frame;
        }
        self.sampled = Some(now);
        frame.draw = spot().and_then(|spot| spot.draw(key));
        if frame.draw.is_some() {
            self.bound = Some(key);
        } else if frame.clear.is_none() {
            frame.clear = self.bound.take();
        }
        frame
    }

    /// Whether `key`'s picture is the one the live view put on the screen: the caller
    /// restores a shape only while this still holds (another owner may have replaced it).
    pub fn owns(&self, key: Key) -> bool {
        self.bound == Some(key)
    }

    /// The player's open intent, for a log line and for the pose's owner.
    pub fn showing(&self) -> Option<Key> {
        self.open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::f32::consts::{FRAC_PI_2, PI};

    const KEY: Key = Key { session: 1, world: 42, screen: 7 };

    /// An authored stand-in for a worldspace's picture: a cell (4096 units) to a quarter
    /// of the texture, east to the right and north up, the player's cell in the middle.
    /// The real projection's orientation and crop are checked against
    /// `pipboy::data::model::WorldMap` itself by `spikes/001-live-wrist-world/run.py`, so
    /// no map maths is copied here.
    struct Patch(&'static str);

    /// One cell east or north on [`Patch`], as `model::CELL` measures a cell.
    const CELL: f32 = 4096.0;

    impl MapPicture for Patch {
        fn map_texture(&self) -> &str {
            self.0
        }
        fn map_uv(&self, x: f32, y: f32) -> [f32; 2] {
            [0.5 + x / (4.0 * CELL), 0.5 - y / (4.0 * CELL)]
        }
    }

    fn t(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    /// Ordinary first-person play, weapon away, in an exterior, on an arm with a screen.
    fn play() -> Play {
        Play { session: 1, world: Some(42), screen: Some(7), playing: true, first_person: true, ..Play::default() }
    }

    fn at(map: &Patch, x: f32, y: f32, heading: f32) -> Option<Spot<'_>> {
        Some(Spot { map, feet: [x, y], heading })
    }

    #[test]
    fn off_and_closed_never_look_at_the_world() {
        let mut live = Live::default();
        let map = Patch("synthetic-owned-map.dds");
        for (enabled, pressed) in [(false, true), (false, false), (true, false)] {
            let frame = live.step(t(0), enabled, pressed, &play(), || panic!("a closed wrist sampled"));
            assert_eq!(frame, Frame::default(), "enabled {enabled}, pressed {pressed}");
        }
        // The press that does open it is the first look at the world.
        assert!(live.step(t(0), true, true, &play(), || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
    }

    #[test]
    fn every_blocker_keeps_it_closed_with_its_own_reason() {
        let cases: [(Play, Block); 10] = [
            (Play { playing: false, ..play() }, Block::NotPlaying),
            (Play { input_suppressed: true, ..play() }, Block::InputSuppressed),
            (Play { menu: true, ..play() }, Block::Menu),
            (Play { pipboy: true, ..play() }, Block::PipBoy),
            (Play { vats: true, ..play() }, Block::Vats),
            (Play { first_person: false, ..play() }, Block::NotFirstPerson),
            (Play { weapon_out: true, ..play() }, Block::WeaponOut),
            (Play { taken: true, ..play() }, Block::Taken),
            (Play { world: None, ..play() }, Block::Interior),
            (Play { screen: None, ..play() }, Block::NoScreen),
        ];
        for (blocked, why) in cases {
            assert_eq!(blocked.eligible(), Err(why), "{why:?} should block");
            let mut live = Live::default();
            let frame = live.step(t(0), true, true, &blocked, || panic!("{why:?} sampled"));
            assert_eq!(frame, Frame::default(), "{why:?} opened the wrist");
            assert!(!why.why().is_empty());
        }
        assert_eq!(play().eligible(), Ok(KEY));
    }

    #[test]
    fn the_world_map_follows_the_player_at_ten_hz() {
        let mut live = Live::default();
        let map = Patch("synthetic-owned-map.dds");
        let looks = Cell::new(0);
        let sample = |x, y, heading| {
            looks.set(looks.get() + 1);
            at(&map, x, y, heading)
        };
        let first = live.step(t(0), true, true, &play(), || sample(0.0, 0.0, 0.0)).draw.unwrap();
        assert_eq!((first.key, first.texture, first.uv, first.forward), (KEY, "synthetic-owned-map.dds", [0.5, 0.5], [0.0, -1.0]));
        // Nothing in the 99 ms after it, however far the player walks.
        for ms in 1..100 {
            let frame = live.step(t(ms), true, false, &play(), || sample(CELL, CELL, 0.0));
            assert_eq!((frame.pose, frame.clear, frame.draw), (Some(KEY), None, None), "{ms} ms");
        }
        let moved = live.step(t(100), true, false, &play(), || sample(CELL, CELL, FRAC_PI_2)).draw.unwrap();
        assert_eq!(moved.uv, [0.75, 0.25], "east and north move the arrow right and up the picture");
        assert!((moved.forward[0] - 1.0).abs() < 1e-6 && moved.forward[1].abs() < 1e-6, "east is {:?}", moved.forward);
        assert_eq!(looks.get(), 2, "one look per 100 ms");
        // South and west, to pin the turn's direction on a picture whose y runs down.
        let south = live.step(t(200), true, false, &play(), || sample(0.0, 0.0, PI)).draw.unwrap();
        assert!(south.forward[0].abs() < 1e-6 && (south.forward[1] - 1.0).abs() < 1e-6, "south is {:?}", south.forward);
        let west = live.step(t(300), true, false, &play(), || sample(0.0, 0.0, -FRAC_PI_2)).draw.unwrap();
        assert!((west.forward[0] + 1.0).abs() < 1e-6 && west.forward[1].abs() < 1e-6, "west is {:?}", west.forward);
    }

    #[test]
    fn the_pose_is_asked_every_open_frame_and_given_back_when_it_closes() {
        let mut live = Live::default();
        let map = Patch("synthetic-owned-map.dds");
        assert_eq!(live.step(t(0), true, true, &play(), || at(&map, 0.0, 0.0, 0.0)).pose, Some(KEY));
        assert_eq!(live.showing(), Some(KEY));
        assert!(live.owns(KEY));
        // A frame inside the sample budget still holds the pose.
        let held = live.step(t(50), true, false, &play(), || panic!("sampled inside the budget"));
        assert_eq!((held.pose, held.clear, held.draw), (Some(KEY), None, None));
        // The same press closes it: pose and screen go back in one frame.
        let closed = live.step(t(60), true, true, &play(), || panic!("the closing frame sampled"));
        assert_eq!((closed.pose, closed.clear, closed.draw), (None, Some(KEY), None));
        assert!(!live.owns(KEY) && live.showing().is_none());
    }

    #[test]
    fn an_interruption_gives_it_back_and_takes_another_press() {
        let map = Patch("synthetic-owned-map.dds");
        // The frame a menu opens, a conversation or a loading screen starts, V.A.T.S. is
        // entered, a weapon comes out, a door leads inside, the rig is rebuilt, or the
        // game is reloaded: the pose and the screen go back in that same frame.
        let replacements = [
            Play { menu: true, ..play() },
            Play { pipboy: true, ..play() },
            Play { playing: false, ..play() },
            Play { vats: true, ..play() },
            Play { weapon_out: true, ..play() },
            Play { input_suppressed: true, ..play() },
            Play { first_person: false, ..play() },
            Play { world: None, ..play() },
            Play { screen: None, ..play() },
            Play { screen: Some(8), ..play() },
            Play { session: 2, ..play() },
        ];
        for next in replacements {
            let mut live = Live::default();
            assert!(live.step(t(0), true, true, &play(), || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
            // An interruption in the same frame as a press wins over the press.
            let gone = live.step(t(1), true, true, &next, || panic!("an interrupted frame sampled"));
            assert_eq!((gone.pose, gone.clear, gone.draw), (None, Some(KEY), None));
            // It stays closed once the frame is eligible again: no press, no view.
            let back = if next.eligible().is_ok() { next } else { play() };
            assert_eq!(live.step(t(200), true, false, &back, || panic!("reopened by itself")), Frame::default());
            assert!(live.step(t(201), true, true, &back, || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
        }
    }

    #[test]
    fn travel_to_another_worldspace_clears_at_once_and_keeps_the_intent() {
        let mut live = Live::default();
        let map = Patch("synthetic-owned-map.dds");
        assert!(live.step(t(0), true, true, &play(), || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
        // A cell boundary inside the same worldspace changes nothing.
        let crossed = live.step(t(100), true, false, &play(), || at(&map, CELL, 0.0, 0.0));
        assert_eq!((crossed.clear, crossed.draw.map(|d| d.uv)), (None, Some([0.75, 0.5])));
        let elsewhere = Play { world: Some(43), ..play() };
        let travelled = live.step(t(150), true, false, &elsewhere, || panic!("sampled before its time"));
        assert_eq!((travelled.pose, travelled.clear, travelled.draw), (Some(Key { world: 43, ..KEY }), Some(KEY), None));
        let after = live.step(t(200), true, false, &elsewhere, || at(&map, 0.0, 0.0, 0.0)).draw.unwrap();
        assert_eq!(after.key, Key { world: 43, ..KEY });
    }

    #[test]
    fn a_sample_it_cannot_use_clears_what_was_drawn() {
        let mut live = Live::default();
        let map = Patch("synthetic-owned-map.dds");
        static NO_PICTURE: Patch = Patch("");
        let mut ms = 0;
        for bad in 0..5 {
            assert!(live.step(t(ms), true, true, &play(), || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
            ms += 100;
            let cleared = live.step(t(ms), true, false, &play(), || match bad {
                // No sample at all (the worldspace's record is not read yet).
                0 => None,
                1 => at(&map, f32::NAN, 0.0, 0.0),
                2 => at(&map, 0.0, 0.0, f32::INFINITY),
                // Off the picture (another worldspace's bounds, or a mod's broken record).
                3 => at(&map, 9_000_000.0, 0.0, 0.0),
                _ => at(&NO_PICTURE, 0.0, 0.0, 0.0),
            });
            assert_eq!((cleared.pose, cleared.clear, cleared.draw), (Some(KEY), Some(KEY), None));
            assert!(!live.owns(KEY), "a cleared screen is not ours any more");
            // Nothing is cleared twice, and it stays open waiting for a usable sample.
            ms += 100;
            let again = live.step(t(ms), true, false, &play(), || None);
            assert_eq!((again.pose, again.clear, again.draw), (Some(KEY), None, None));
            ms += 100;
            live.step(t(ms), true, true, &play(), || panic!("the closing frame sampled"));
        }
    }

    #[test]
    fn closing_and_reopening_does_not_catch_up_on_samples() {
        let mut live = Live::default();
        let map = Patch("synthetic-owned-map.dds");
        assert!(live.step(t(0), true, true, &play(), || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
        live.step(t(10), true, true, &play(), || panic!("the closing frame sampled"));
        assert!(live.step(t(20), true, true, &play(), || panic!("a reopened wrist sampled early")).draw.is_none());
        assert!(live.step(t(99), true, false, &play(), || panic!("still inside the budget")).draw.is_none());
        assert!(live.step(t(100), true, false, &play(), || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
        // Time that goes backwards (a reload's clock) waits instead of sampling at once.
        assert!(live.step(t(0), true, false, &play(), || panic!("sampled on a clock that went back")).draw.is_none());
    }

    /// How the adapter arms the mode until it has a key of its own (`wrist_arm::decide`):
    /// every closed frame is a press, so it opens itself and comes back after a block.
    fn auto(live: &mut Live, ms: u64, play: &Play, map: &'static Patch) -> Frame<'static> {
        let press = live.showing().is_none();
        live.step(t(ms), true, press, play, || at(map, 0.0, 0.0, 0.0))
    }

    #[test]
    fn the_arming_adapter_yields_while_blocked_and_comes_back_after() {
        static MAP: Patch = Patch("synthetic-owned-map.dds");
        let mut live = Live::default();
        assert!(auto(&mut live, 0, &play(), &MAP).draw.is_some());
        // The ordinary Pip-Boy coming up owns the arm: the live view goes in that frame.
        let menus = Play { pipboy: true, ..play() };
        let gone = auto(&mut live, 100, &menus, &MAP);
        assert_eq!((gone.pose, gone.clear, gone.draw), (None, Some(KEY), None));
        // While it is blocked it asks for nothing at all, however long it lasts.
        for ms in 101..400 {
            assert_eq!(auto(&mut live, ms, &menus, &MAP), Frame::default(), "{ms} ms");
        }
        // Back in ordinary play it opens again, under the same budget.
        let back = auto(&mut live, 400, &play(), &MAP);
        assert_eq!((back.pose, back.clear, back.draw.is_some()), (Some(KEY), None, true));
    }

    #[test]
    fn the_window_puts_the_player_in_the_middle_and_their_facing_up() {
        let (uv, half, extent) = ([0.25, 0.75], 0.1, 0.8);
        let about = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6;
        for forward in [[0.0, -1.0], [1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]] {
            let w = window(uv, forward, half, extent);
            assert!(about(w.at([extent / 2.0, extent / 2.0]), uv), "facing {forward:?}: {:?}", w.at([extent / 2.0, extent / 2.0]));
            // Up the screen is the way they face, half a window away.
            let up = w.at([extent / 2.0, 0.0]);
            assert!(about(up, [uv[0] + forward[0] * half, uv[1] + forward[1] * half]), "facing {forward:?} up the screen is {up:?}");
        }
        // North up: east is to the right of the picture and north above it.
        let w = window(uv, [0.0, -1.0], half, extent);
        assert!(about(w.at([extent, extent / 2.0]), [uv[0] + half, uv[1]]));
        assert!(about(w.at([extent / 2.0, extent]), [uv[0], uv[1] + half]));
    }

    #[test]
    fn an_unusable_window_falls_back_to_north_up_and_the_whole_picture() {
        let plain = window([0.5; 2], [0.0, -1.0], 0.5, 1.0);
        for bad in [window([0.5; 2], [0.0, 0.0], f32::NAN, 0.0), window([0.5; 2], [f32::NAN, 1.0], -1.0, -3.0)] {
            assert_eq!(bad, plain);
        }
        // The whole picture, corner to corner.
        assert_eq!((plain.at([0.0; 2]), plain.at([1.0; 2])), ([0.0; 2], [1.0; 2]));
    }


    /// The behaviour notes' own regressions: a conversation, a menu, a reload or an attack
    /// in the middle of a raise gives the arm and the screen back in that frame, and
    /// nothing is left half held. There is no lowering animation to be caught in: the
    /// frame after is already the arm's own owner's.
    #[test]
    fn an_interruption_mid_raise_or_just_after_gives_everything_back_at_once() {
        static MAP: Patch = Patch("synthetic-owned-map.dds");
        for (ms, interrupted) in [(0, Play { playing: false, ..play() }), (0, Play { menu: true, ..play() }), (0, Play { acting: true, ..play() }), (0, Play { cooldown: true, ..play() })] {
            let mut live = Live::default();
            // Up, with a picture on the screen.
            let up = live.step(t(ms), true, true, &play(), || at(&MAP, 0.0, 0.0, 0.0));
            assert!(up.pose.is_some() && up.draw.is_some());
            // The interruption lands: the pose goes, the screen is cleared, and the frame
            // asks for nothing else.
            let cut = live.step(t(ms + 16), true, false, &interrupted, || at(&MAP, 0.0, 0.0, 0.0));
            assert_eq!((cut.pose, cut.clear, cut.draw.is_some()), (None, Some(KEY), false), "{:?}", interrupted.eligible());
            // And the frame after it still asks for nothing, with or without a press:
            // nothing is owed, so no control and no material can be left held.
            let after = live.step(t(ms + 32), true, false, &interrupted, || at(&MAP, 0.0, 0.0, 0.0));
            assert_eq!(after, Frame::default());
        }
    }

    #[test]
    fn a_press_of_the_hands_own_controls_is_the_first_reason_it_closes() {
        // The action is read before another owner's hold, so the log says what the player
        // did rather than what followed from it.
        assert_eq!(Play { acting: true, ..play() }.eligible(), Err(Block::Acting));
        assert_eq!(Play { acting: true, taken: true, ..play() }.eligible(), Err(Block::Acting));
        assert_eq!(Play { cooldown: true, ..play() }.eligible(), Err(Block::Cooldown));
        // Neither is remembered: the next frame without them opens again.
        assert!(play().eligible().is_ok());
    }

    #[test]
    fn turning_the_player_off_closes_it_wherever_it_was() {
        let mut live = Live::default();
        let map = Patch("synthetic-owned-map.dds");
        assert!(live.step(t(0), true, true, &play(), || at(&map, 0.0, 0.0, 0.0)).draw.is_some());
        let off = live.step(t(100), false, false, &play(), || panic!("sampled with the mode off"));
        assert_eq!((off.pose, off.clear, off.draw), (None, Some(KEY), None));
        assert_eq!(live.step(t(200), false, true, &play(), || panic!("opened with the mode off")), Frame::default());
    }
}
