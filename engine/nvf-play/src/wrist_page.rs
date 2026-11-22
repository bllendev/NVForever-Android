//! The glance's pages: what the Pip-Boy's screen shows while the arm is up, and the rows
//! each page is made of [design: the owner, 2026-11-21].
//!
//! Three pages, in the order the device's own tab knob turns through them:
//!
//! - **MAP** — the map and its quest markers, as [`crate::wrist_live`] samples it. It is
//!   the only page that needs a look at the world; the other two are read off state the
//!   engine already keeps.
//! - **STATUS** — health, action points, the weapon's condition, the rounds in the
//!   magazine and carried, rads, and the limbs that are crippled.
//! - **QUESTS** — the active quest's name and the objective the player is on.
//!
//! This module is plain values and has no engine behind it: the pages' rules are tested
//! without a renderer, a window or game data. [`crate::wrist_arm`] fills [`Facts`] from
//! the game at the glance's own rate (never per frame) and keeps the [`Row`]s the screen
//! draws; `nvforever`'s `wrist_screen` rasterises them with the game's own Pip-Boy font.
//!
//! The words are NVForever's own or the data's: the row labels are abbreviations the HUD
//! and the Pip-Boy pages already use in our own drawing, and a quest's name and objective
//! are the load order's own text (LEGAL.md).

/// Which page the glance's screen shows.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    #[default]
    Map,
    Status,
    Quests,
}

impl Page {
    /// The three, in the order D-Pad Right turns through them (and the tab knob with
    /// them).
    pub const ALL: [Page; 3] = [Page::Map, Page::Status, Page::Quests];
    /// The heading each page draws at its top. Our own words [design].
    pub const NAMES: [&'static str; 3] = ["MAP", "STATUS", "QUESTS"];

    pub fn name(self) -> &'static str {
        Page::NAMES[self.at()]
    }

    pub fn at(self) -> usize {
        match self {
            Page::Map => 0,
            Page::Status => 1,
            Page::Quests => 2,
        }
    }

    /// `by` pages on, round the three: the flip never runs out of pages at either end.
    pub fn step(self, by: i32) -> Page {
        let n = Page::ALL.len() as i32;
        Page::ALL[(self.at() as i32 + by).rem_euclid(n) as usize]
    }

    /// A page by name, for the developer's `NVF_WRIST_PAGE` switch (case is ignored).
    pub fn of(name: &str) -> Option<Page> {
        let want = name.trim();
        Page::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(want))
    }
}

/// The limbs, in the order the STATUS page lists them, as the condition codes 25–30 run
/// (the brain, 31, has no picture on the HUD either). Our own words [design].
pub const LIMBS: [&str; 6] = ["Head", "Torso", "Left arm", "Right arm", "Left leg", "Right leg"];

/// What the pages show, read off the game at the glance's own rate. Every field is
/// optional: a page draws what it has and says so plainly when it has nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// Health and action points now and at their whole, as the HUD's own bars measure
    /// them (`crate::hud::gather`).
    pub health: Option<(f64, f64)>,
    pub action_points: Option<(f64, f64)>,
    /// The equipped weapon's condition, 0 to 1; `None` with bare hands.
    pub condition: Option<f32>,
    /// Rounds in the magazine and carried besides.
    pub ammo: Option<(u32, u32)>,
    /// The ammo type's abbreviation (AMMO `QNAM`), where it has one.
    pub ammo_label: Option<String>,
    /// Total rads, the actor value the STATS page's dial shows.
    pub rads: Option<f64>,
    /// Which of [`LIMBS`] are crippled.
    pub crippled: [bool; 6],
    /// The active quest's name and the objective the player is on, both the load order's
    /// own text.
    pub quest: Option<String>,
    pub objective: Option<String>,
}

/// A row of a page, as the screen draws it.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// The page's heading, with a rule under it.
    Head(String),
    /// A label, a bar filled to `share` (0 to 1) and the reading at the right.
    Bar(String, f32, String),
    /// A label at the left and its reading at the right.
    Pair(String, String),
    /// A line of text across the page; the drawer wraps it to the glass.
    Text(String),
    /// A blank line.
    Gap,
}

/// A whole number for the page, never in exponent form.
fn whole(v: f64) -> String {
    if v.is_finite() {
        format!("{:.0}", v.clamp(-9_999_999.0, 9_999_999.0))
    } else {
        "-".to_string()
    }
}

/// A share as whole per cent.
fn percent(v: f32) -> String {
    if v.is_finite() {
        format!("{}%", (v.clamp(0.0, 1.0) * 100.0).round())
    } else {
        "-".to_string()
    }
}

/// A share that is a number and within its bounds, 0 where it is not.
fn held(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn share(now: f64, whole: f64) -> f32 {
    if now.is_finite() && whole.is_finite() && whole > 0.0 {
        (now / whole).clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

/// The rows of a page. MAP has none: its own picture fills the glass.
pub fn rows(page: Page, f: &Facts) -> Vec<Row> {
    match page {
        Page::Map => Vec::new(),
        Page::Status => status_rows(f),
        Page::Quests => quest_rows(f),
    }
}

fn status_rows(f: &Facts) -> Vec<Row> {
    let mut out = vec![Row::Head(Page::Status.name().to_string())];
    match f.health {
        Some((now, all)) => out.push(Row::Bar("HP".into(), share(now, all), format!("{}/{}", whole(now), whole(all)))),
        None => out.push(Row::Pair("HP".into(), "-".into())),
    }
    match f.action_points {
        Some((now, all)) => out.push(Row::Bar("AP".into(), share(now, all), format!("{}/{}", whole(now), whole(all)))),
        None => out.push(Row::Pair("AP".into(), "-".into())),
    }
    match f.condition {
        Some(c) => out.push(Row::Bar("CND".into(), held(c), percent(c))),
        // Bare hands: nothing to wear out, and no magazine either.
        None => out.push(Row::Pair("CND".into(), "-".into())),
    }
    let ammo = match (f.ammo, f.ammo_label.as_deref()) {
        (Some((clip, carried)), Some(label)) if !label.trim().is_empty() => format!("{clip}/{carried} {}", label.trim()),
        (Some((clip, carried)), _) => format!("{clip}/{carried}"),
        (None, _) => "-".to_string(),
    };
    out.push(Row::Pair("AMMO".into(), ammo));
    out.push(Row::Pair("RADS".into(), f.rads.map_or_else(|| "-".to_string(), whole)));
    let hurt: Vec<&str> = LIMBS.iter().zip(f.crippled).filter_map(|(name, bad)| bad.then_some(*name)).collect();
    if !hurt.is_empty() {
        out.push(Row::Gap);
        out.push(Row::Head("CRIPPLED".into()));
        for name in hurt {
            out.push(Row::Text(name.to_string()));
        }
    }
    out
}

fn quest_rows(f: &Facts) -> Vec<Row> {
    let mut out = vec![Row::Head(Page::Quests.name().to_string())];
    match f.quest.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(name) => {
            out.push(Row::Text(name.to_string()));
            out.push(Row::Gap);
            match f.objective.as_deref().map(str::trim).filter(|o| !o.is_empty()) {
                Some(objective) => out.push(Row::Text(objective.to_string())),
                // Our own words: the page says what it has rather than nothing [design].
                None => out.push(Row::Text("No objective".to_string())),
            }
        }
        None => out.push(Row::Text("No active quest".to_string())),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pages_turn_round_in_their_own_order() {
        assert_eq!(Page::default(), Page::Map);
        assert_eq!((Page::Map.name(), Page::Status.name(), Page::Quests.name()), ("MAP", "STATUS", "QUESTS"));
        assert_eq!(Page::Map.step(1), Page::Status);
        assert_eq!(Page::Status.step(1), Page::Quests);
        assert_eq!(Page::Quests.step(1), Page::Map, "the flip never runs out of pages");
        assert_eq!(Page::Map.step(-1), Page::Quests);
        assert_eq!(Page::Map.step(0), Page::Map);
        assert_eq!(Page::Map.step(7), Page::Status, "any number of steps lands on a page");
        assert_eq!(Page::of("status"), Some(Page::Status));
        assert_eq!(Page::of(" QUESTS "), Some(Page::Quests));
        assert_eq!(Page::of("items"), None);
    }

    fn facts() -> Facts {
        Facts {
            health: Some((120.0, 150.0)),
            action_points: Some((45.0, 75.0)),
            condition: Some(0.92),
            ammo: Some((12, 240)),
            ammo_label: Some("JHP".into()),
            rads: Some(48.0),
            crippled: [false; 6],
            quest: Some("Back in the Saddle".into()),
            objective: Some("Talk to Sunny Smiles".into()),
        }
    }

    /// The STATUS page shows what the HUD's meters did, as numbers and bars.
    #[test]
    fn the_status_page_shows_the_meters_the_hud_gave_up() {
        let rows = rows(Page::Status, &facts());
        assert_eq!(rows[0], Row::Head("STATUS".into()));
        assert_eq!(rows[1], Row::Bar("HP".into(), 0.8, "120/150".into()));
        assert_eq!(rows[2], Row::Bar("AP".into(), 0.6, "45/75".into()));
        assert_eq!(rows[3], Row::Bar("CND".into(), 0.92, "92%".into()));
        assert_eq!(rows[4], Row::Pair("AMMO".into(), "12/240 JHP".into()));
        assert_eq!(rows[5], Row::Pair("RADS".into(), "48".into()));
        assert_eq!(rows.len(), 6, "nothing is crippled, so no list of limbs");
    }

    /// Nothing read is nothing invented: every row still stands, with a dash in it.
    #[test]
    fn a_page_with_nothing_read_says_so_rather_than_guess() {
        let rows = rows(Page::Status, &Facts::default());
        assert_eq!(rows[1], Row::Pair("HP".into(), "-".into()));
        assert_eq!(rows[2], Row::Pair("AP".into(), "-".into()));
        assert_eq!(rows[3], Row::Pair("CND".into(), "-".into()), "bare hands wear nothing out");
        assert_eq!(rows[4], Row::Pair("AMMO".into(), "-".into()));
        assert_eq!(rows[5], Row::Pair("RADS".into(), "-".into()));
        // A number that is not one draws a dash rather than an exponent.
        let odd = Facts { health: Some((f64::NAN, f64::INFINITY)), condition: Some(f32::NAN), ..Facts::default() };
        assert_eq!(super::rows(Page::Status, &odd)[1], Row::Bar("HP".into(), 0.0, "-/-".into()));
        assert_eq!(super::rows(Page::Status, &odd)[3], Row::Bar("CND".into(), 0.0, "-".into()));
    }

    /// The crippled limbs are listed under their own heading, and only when there are any.
    #[test]
    fn the_crippled_limbs_are_listed_under_their_own_heading() {
        let mut f = facts();
        f.crippled = [false, false, true, false, false, true];
        let rows = rows(Page::Status, &f);
        assert_eq!(rows[6], Row::Gap);
        assert_eq!(rows[7], Row::Head("CRIPPLED".into()));
        assert_eq!(rows[8], Row::Text("Left arm".into()));
        assert_eq!(rows[9], Row::Text("Right leg".into()));
        assert_eq!(rows.len(), 10);
    }

    /// The QUESTS page is the active quest's name and the objective under it, both the
    /// load order's own words; with none it says so in ours.
    #[test]
    fn the_quests_page_shows_the_active_quest_and_its_objective() {
        let rows = rows(Page::Quests, &facts());
        assert_eq!(rows[0], Row::Head("QUESTS".into()));
        assert_eq!(rows[1], Row::Text("Back in the Saddle".into()));
        assert_eq!(rows[2], Row::Gap);
        assert_eq!(rows[3], Row::Text("Talk to Sunny Smiles".into()));
        let none = Facts { quest: Some("  ".into()), ..Facts::default() };
        assert_eq!(super::rows(Page::Quests, &none)[1], Row::Text("No active quest".into()));
        let quiet = Facts { quest: Some("Ghost Town Gunfight".into()), objective: None, ..Facts::default() };
        assert_eq!(super::rows(Page::Quests, &quiet)[3], Row::Text("No objective".into()));
    }

    /// The MAP page draws no rows at all: its picture is the whole glass.
    #[test]
    fn the_map_page_draws_no_rows() {
        assert!(rows(Page::Map, &facts()).is_empty());
    }

    /// The ammo reads clip over carried, with its type where the record names one.
    #[test]
    fn the_ammo_reads_the_clip_over_what_is_carried() {
        let plain = Facts { ammo_label: None, ..facts() };
        assert_eq!(rows(Page::Status, &plain)[4], Row::Pair("AMMO".into(), "12/240".into()));
        let blank = Facts { ammo_label: Some("  ".into()), ..facts() };
        assert_eq!(rows(Page::Status, &blank)[4], Row::Pair("AMMO".into(), "12/240".into()));
    }
}
