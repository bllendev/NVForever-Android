//! The HUD on the Pip-Boy: the Glance's rows that take the health, action points,
//! condition and ammo meters off the screen, so the player reads them on the device's own
//! STATUS page instead [design: the owner, 2026-11-21: "the point of this mod is a more
//! immersive HUD through the realtime Pip-Boy"].
//!
//! Two rows, both under Gameplay › Pip-Boy and both needing the glance itself
//! (`wrist-map`):
//!
//! - **`wrist-hud`, HUD on the Pip-Boy** (off by default): while it is on, the four
//!   meters ([`crate::hud::Meter`]) are hidden. The compass, the crosshair, the sneak
//!   word and every message stay exactly as they are: only the numbers the device's own
//!   page now carries go.
//! - **`wrist-hud-flash`, Flash a hidden meter** (on by default): a safety net. A hidden
//!   meter comes back whole for [`FLASH_SECONDS`] when it changes in a way the player
//!   needs to know about at once (damage taken, action points spent, a round fired, the
//!   weapon's condition dropping) and fades out over the last [`FADE_SECONDS`]. A purist
//!   turns the row off and the meters never come back [design: the owner, 2026-11-21].
//!
//! Nothing here is latched: [`WristHud::step`] is a fresh decision every frame from the
//! HUD's own reading ([`crate::hud::HudView`]), and the frame the rows go off every meter
//! is given back whole ([`WristHud::step`]'s `hiding` false). With the glance off
//! (`wrist-map`) the rows do nothing at all, so a player cannot hide their health with no
//! Pip-Boy to read it on.
//!
//! The tile-drawn vanilla HUD (`crate::tiles::vanilla`, `[UI] TileMenus=HUDMainMenu`)
//! draws its own corners and hides these nodes itself; the rows act on the hand-built HUD,
//! which is what the engine draws unless that switch names the HUD.

use crate::hud::{HudView, Meter};
use crate::settings::{add_setting, Setting};
use crate::wrist_arm::WristLive;
use bevy::prelude::*;

/// How long a hidden meter stays back after a change worth knowing about, and how much of
/// that time it spends fading out [design: the owner's "about 3 s, then fades"].
pub const FLASH_SECONDS: f32 = 3.0;
pub const FADE_SECONDS: f32 = 0.6;

/// How much a share has to move to count as a change at all, so a bar creeping back with
/// a stimpak or a rested action point does not flash the HUD on every frame [design].
const EPSILON: f32 = 0.002;

/// What the HUD shows of the four meters, as [`crate::hud::HudView`] has it: the health
/// and action points shares, the equipped weapon's condition (1 with bare hands: nothing
/// to lose) and the rounds in the magazine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub health: f32,
    pub action_points: f32,
    pub condition: f32,
    pub clip: u32,
}

impl Default for Reading {
    fn default() -> Self {
        Reading { health: 1.0, action_points: 1.0, condition: 1.0, clip: 0 }
    }
}

impl Reading {
    /// The reading of a frame's HUD.
    pub fn of(view: &HudView) -> Reading {
        Reading {
            health: view.health,
            action_points: view.action_points,
            condition: view.weapon.condition.unwrap_or(1.0),
            clip: view.weapon.ammo.map_or(0, |(clip, _)| clip),
        }
    }
}

/// Which of the four meters changed in a way the player has to be told about at once:
/// only the way that hurts: health or action points falling, the weapon wearing, a round
/// leaving the magazine. Healing, resting, reloading and a weapon repaired are good news
/// and the device's page is soon enough for them [design: the owner's own list, "damage
/// taken, AP spent, ammo low, condition drop"].
pub fn worth_a_flash(now: Reading, was: Reading) -> [bool; 4] {
    [
        now.health < was.health - EPSILON,
        now.action_points < was.action_points - EPSILON,
        now.condition < was.condition - EPSILON,
        now.clip < was.clip,
    ]
}

/// How strongly a meter with `left` seconds of its flash to run is drawn: whole until the
/// last [`FADE_SECONDS`], then fading to nothing. 0 is hidden.
pub fn alpha(left: f32) -> f32 {
    if left <= 0.0 || !left.is_finite() {
        0.0
    } else {
        (left / FADE_SECONDS).min(1.0)
    }
}

/// The two rows and the flashes' clocks.
#[derive(Resource, Debug)]
pub struct WristHud {
    /// `wrist-hud`: the meters are the Pip-Boy's.
    pub on: bool,
    /// `wrist-hud-flash`: a hidden meter flashes back when it changes for the worse.
    pub flash: bool,
    /// Seconds of flash left for each of [`MeterKind::ALL`].
    left: [f32; 4],
    /// The reading the flashes were judged against; `None` until the first frame.
    was: Option<Reading>,
    /// Something was hidden or dimmed and has to be given back when the rows go off.
    touched: bool,
}

impl Default for WristHud {
    fn default() -> Self {
        WristHud { on: false, flash: true, left: [0.0; 4], was: None, touched: false }
    }
}

impl WristHud {
    /// One frame: how strongly each of [`MeterKind::ALL`] is drawn, 1 for the whole meter
    /// and 0 for a hidden one. `hiding` is the rows' own decision (the glance on and
    /// `wrist-hud` on); while it is false everything is whole and no flash is counted, so
    /// turning the rows off gives the HUD back at once.
    pub fn step(&mut self, now: Reading, dt: f32, hiding: bool) -> [f32; 4] {
        if !hiding {
            self.was = Some(now);
            self.left = [0.0; 4];
            return [1.0; 4];
        }
        let fresh = self.was.replace(now).map_or([false; 4], |was| worth_a_flash(now, was));
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        for (left, fresh) in self.left.iter_mut().zip(fresh) {
            *left = if fresh && self.flash { FLASH_SECONDS } else { (*left - dt).max(0.0) };
        }
        self.left.map(alpha)
    }

    /// The seconds left, for the log line and the tests.
    pub fn left(&self) -> [f32; 4] {
        self.left
    }
}

pub struct WristHudPlugin;

impl Plugin for WristHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WristHud>().add_systems(
            PostUpdate,
            // After the drawers of `crate::hud` (they run in `Update`) and before the
            // tiles' own hiding, so this is the last word on a meter's visibility.
            hide_meters
                .before(crate::tiles::vanilla::hide_hand_built)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
        let row = |key, label, note| Setting { key, group: crate::pause::GROUP_MENUS, label, note, switches: "", get: |_| false, set: |_, _| {}, slider: None };
        add_setting(
            app,
            Setting {
                get: |w| w.resource::<WristHud>().on,
                set: |w, on| w.resource_mut::<WristHud>().on = on,
                ..row(
                    "wrist-hud",
                    "HUD on the Pip-Boy",
                    "On: the health, action points, condition and ammo meters leave the screen and are read on the Pip-Boy's own page instead. The compass, crosshair and messages stay.",
                )
            },
        );
        add_setting(
            app,
            Setting {
                get: |w| w.resource::<WristHud>().flash,
                set: |w, on| w.resource_mut::<WristHud>().flash = on,
                ..row(
                    "wrist-hud-flash",
                    "Flash a hidden meter",
                    "On: a meter the row above hides comes back for a few seconds when it drops: damage taken, action points spent, a round fired, the weapon wearing. Off: it never comes back.",
                )
            },
        );
    }
}

/// Hide the four meters while the rows ask for it, and give back the ones that are
/// flashing. After every drawer has set its own visibility and colours (the condition
/// row's is `crate::hud::draw`'s, the ticks' `crate::hud::pips`'), so this is the last
/// word on them for the frame and nothing has to be changed in either.
#[allow(clippy::too_many_arguments)]
fn hide_meters(
    wrist: Option<Res<WristLive>>,
    mut hud: ResMut<WristHud>,
    view: Option<Res<HudView>>,
    time: Res<Time<Real>>,
    meters: Query<(Entity, &Meter)>,
    kids: Query<&Children>,
    mut visible: Query<&mut Visibility>,
    mut images: Query<&mut ImageNode>,
    mut fills: Query<&mut BackgroundColor>,
    mut texts: Query<&mut TextColor>,
) {
    let hiding = hud.on && wrist.as_deref().is_some_and(|w| w.live());
    if !hiding && !hud.touched {
        // Nothing of ours is on the screen and nothing was ever hidden: the cheapest frame.
        hud.was = view.as_deref().map(Reading::of);
        return;
    }
    let now = view.as_deref().map_or_else(Reading::default, Reading::of);
    let alphas = hud.step(now, time.delta_secs(), hiding);
    hud.touched = hiding;
    for (e, meter) in &meters {
        let a = alphas[meter.0.at()];
        if let Ok(mut v) = visible.get_mut(e) {
            v.set_if_neq(if a > 0.0 { Visibility::Inherited } else { Visibility::Hidden });
        }
        if a > 0.0 {
            tint(e, a, &kids, &mut images, &mut fills, &mut texts);
        }
    }
}

/// Draw a meter's whole subtree at `a` of its strength: its pictures, its blocks and its
/// letters, whatever each drawer set this frame.
fn tint(e: Entity, a: f32, kids: &Query<&Children>, images: &mut Query<&mut ImageNode>, fills: &mut Query<&mut BackgroundColor>, texts: &mut Query<&mut TextColor>) {
    if let Ok(mut image) = images.get_mut(e) {
        if image.color.alpha() != a {
            image.color.set_alpha(a);
        }
    }
    if let Ok(mut fill) = fills.get_mut(e) {
        // A node's own transparent background stays transparent (`hud::enemy_colours`
        // keeps the same rule): only a block drawn as a meter's own ink is dimmed.
        if fill.0.alpha() > 0.0 && fill.0.alpha() != a {
            fill.0.set_alpha(a);
        }
    }
    if let Ok(mut text) = texts.get_mut(e) {
        if text.0.alpha() != a {
            text.0.set_alpha(a);
        }
    }
    if let Ok(children) = kids.get(e) {
        for child in children {
            tint(*child, a, kids, images, fills, texts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(health: f32, action_points: f32, condition: f32, clip: u32) -> Reading {
        Reading { health, action_points, condition, clip }
    }

    /// Only a change for the worse flashes a meter back: healing, resting, repairing and
    /// reloading are the page's own news.
    #[test]
    fn a_meter_flashes_back_only_when_it_drops() {
        let whole = r(1.0, 1.0, 1.0, 10);
        assert_eq!(worth_a_flash(whole, whole), [false; 4]);
        assert_eq!(worth_a_flash(r(0.6, 1.0, 1.0, 10), whole), [true, false, false, false]);
        assert_eq!(worth_a_flash(r(1.0, 0.4, 1.0, 10), whole), [false, true, false, false]);
        assert_eq!(worth_a_flash(r(1.0, 1.0, 0.7, 10), whole), [false, false, true, false]);
        assert_eq!(worth_a_flash(r(1.0, 1.0, 1.0, 9), whole), [false, false, false, true]);
        // Back up: nothing flashes.
        assert_eq!(worth_a_flash(whole, r(0.6, 0.4, 0.7, 9)), [false; 4]);
        // A share creeping up by a hair is no change at all.
        assert_eq!(worth_a_flash(r(1.0 - EPSILON / 2.0, 1.0, 1.0, 10), whole), [false; 4]);
        // Everything at once.
        assert_eq!(worth_a_flash(r(0.6, 0.4, 0.7, 9), whole), [true; 4]);
    }

    /// The flash runs its time, fades out over its last part and leaves the meter hidden.
    #[test]
    fn the_flash_runs_its_time_and_fades_out() {
        let mut hud = WristHud { on: true, ..default() };
        let whole = r(1.0, 1.0, 1.0, 10);
        assert_eq!(hud.step(whole, 0.0, true), [0.0; 4], "a hidden meter starts hidden");
        // Damage taken: the health meter comes back whole.
        assert_eq!(hud.step(r(0.6, 1.0, 1.0, 10), 1.0 / 60.0, true)[0], 1.0);
        // It stays whole until the fade, then goes out.
        let mut left = FLASH_SECONDS;
        while left > FADE_SECONDS + 0.05 {
            assert_eq!(hud.step(r(0.6, 1.0, 1.0, 10), 0.05, true)[0], 1.0, "{left} s left");
            left -= 0.05;
        }
        let fading = hud.step(r(0.6, 1.0, 1.0, 10), 0.05, true)[0];
        assert!(fading > 0.0 && fading < 1.0, "it fades: {fading}");
        for _ in 0..100 {
            hud.step(r(0.6, 1.0, 1.0, 10), 0.05, true);
        }
        assert_eq!(hud.step(r(0.6, 1.0, 1.0, 10), 0.05, true), [0.0; 4], "and goes out again");
    }

    /// With the rows off nothing is ever hidden, whatever the meters do; with the flash
    /// row off a hidden meter never comes back.
    #[test]
    fn the_rows_decide_and_nothing_is_latched() {
        let mut hud = WristHud { on: true, ..default() };
        assert_eq!(hud.step(r(0.6, 1.0, 1.0, 10), 0.016, false), [1.0; 4], "the rows off: the HUD is whole");
        // And the frame after a drop it is still whole, with no flash counted.
        assert_eq!(hud.step(r(0.2, 1.0, 1.0, 10), 0.016, false), [1.0; 4]);
        assert_eq!(hud.left(), [0.0; 4]);
        // The flash row off: a drop hides the meter all the same.
        let mut purist = WristHud { on: true, flash: false, ..default() };
        purist.step(r(1.0, 1.0, 1.0, 10), 0.016, true);
        assert_eq!(purist.step(r(0.2, 1.0, 1.0, 10), 0.016, true), [0.0; 4]);
    }

    /// The reading is the HUD's own, so the Pip-Boy page and the meters can never differ.
    #[test]
    fn the_reading_is_the_huds_own() {
        let view = HudView { health: 0.5, action_points: 0.25, weapon: crate::combat::HudWeapon { condition: Some(0.8), ammo: Some((7, 50)), abbrev: None }, ..default() };
        assert_eq!(Reading::of(&view), r(0.5, 0.25, 0.8, 7));
        // Bare hands: nothing to lose, so the condition never flashes.
        let fists = HudView::default();
        assert_eq!(Reading::of(&fists), r(1.0, 1.0, 1.0, 0));
    }

    /// Every system this file adds, in one schedule: a parameter that holds what another
    /// holds (Bevy's B0002) panics here rather than on the first frame of a shipped build.
    /// **Every system this file adds belongs in this list.**
    #[test]
    fn the_hud_rows_systems_have_no_conflicting_parameters() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(hide_meters);
        schedule.initialize(&mut world).expect("the schedule builds");
    }
}
