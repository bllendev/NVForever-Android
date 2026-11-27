//! The fork's crash-test switches: one table, one reader.
//!
//! The owner's AYN Odin 3 (Adreno 830, Android 15) loses its graphics device in play, and the
//! only way to narrow that down is from the device: turn one drawn feature off at a time and
//! see which run survives. So the launcher offers a switch per suspect and the engine reads
//! them here.
//!
//! Every switch is **off by default** and gates **only its own feature**. A run with none of
//! them on draws exactly what it drew before: the gates are early returns and one key field
//! that is `false` for every material, so even the pipelines built are the same ones.
//!
//! The shape is the one the old fork's stack ended at: [`SWITCHES`] names each switch once
//! (its file key, the variable the engine reads, what the launcher calls it, and which switch
//! it needs), and everything else is a lookup in that table rather than a branch per switch.
//! Adding a switch is a row here, a gate in the feature, and nothing else.
//!
//! How a switch reaches the engine:
//!
//! 1. the launcher's Advanced page writes `<home>/odin-advanced.txt`, one `key value` line per
//!    switch that is on (`OdinAdvanced.java`);
//! 2. [`apply`] reads that file before the engine starts any threads and exports the variables
//!    ([`environment`]), from the one hook in upstream's `android.rs`;
//! 3. the feature asks [`on`] (or `nvf_assets::nif_material::no_soft_particles`, which cannot
//!    reach this module) at the point where it would do the work.
//!
//! A desktop run sets the same variables itself; there is no file and no launcher, which is
//! why step 2 is the only Android-only part of this file.

use std::collections::BTreeMap;

/// One crash-test switch: a row of [`SWITCHES`], and the only place its names are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashTest {
    /// What the launcher writes in `odin-advanced.txt` and what the engine's log calls it.
    pub key: &'static str,
    /// The variable the engine reads. Switches may share one, and then their [`CrashTest::value`]s
    /// are joined with commas ([`environment`]).
    pub env: &'static str,
    /// What the variable is set to when this switch is on.
    pub value: &'static str,
    /// What the launcher's Advanced page calls the row.
    pub label: &'static str,
    /// The one line under the label: what the switch does, in the player's words.
    pub note: &'static str,
    /// The [`CrashTest::key`] of the switch this one needs: the launcher greys this row until
    /// that one is on, because on its own it does nothing.
    pub needs: Option<&'static str>,
}

/// Every crash-test switch the fork has, in the order the Advanced page lists them: the
/// suspects first, then the diagnostics that narrow a loss down, then the checks that are too
/// slow to leave on.
pub const SWITCHES: &[CrashTest] = &[
    CrashTest {
        key: "no-vats-outline",
        env: "NVF_NO_VATS_OUTLINE",
        value: "1",
        label: "No V.A.T.S. target outline",
        // The owner's crash of 2026-11-27 was in V.A.T.S. in combat, which is what the old
        // base's loss looked like too: about two seconds after the outline's mask camera
        // started drawing (`vats_highlight`).
        note: "The selected body part is not lit on the target, so the outline's extra camera never draws. Targeting and the V.A.T.S. menu are unchanged.",
        needs: None,
    },
    CrashTest {
        key: "no-soft-particles",
        env: "NVF_NO_SOFT_PARTICLES",
        value: "1",
        label: "No soft particles",
        note: "A particle has a hard edge where it passes into a wall, and its shader reads the depth prepass nowhere.",
        needs: None,
    },
    CrashTest {
        key: "no-particles",
        env: "NVF_NO_PARTICLES",
        value: "1",
        label: "No particles",
        note: "No particle system and no effect particle is drawn. They are still simulated, so nothing else in the game changes.",
        needs: None,
    },
    CrashTest {
        key: "no-decals",
        env: "NVF_NO_DECALS",
        value: "1",
        label: "No decals",
        note: "No decal is placed (blood, bullet holes). The shots and their impacts are unchanged.",
        needs: None,
    },
    CrashTest {
        key: "no-heat-haze",
        env: "NVF_NO_HEAT_HAZE",
        value: "1",
        label: "No heat haze",
        note: "The heat-haze pass records nothing.",
        needs: None,
    },
    CrashTest {
        key: "no-pipboy-warm",
        env: "NVF_NO_PIPBOY_WARM",
        value: "1",
        label: "Skip the Pip-Boy warm-up during loads",
        note: "The hidden raise that warms the Pip-Boy's materials behind a loading screen is left out. It is the one thing drawing while a load streams.",
        needs: None,
    },
    CrashTest {
        key: "gpu-breadcrumbs",
        env: "NVF_GPU_BREADCRUMBS",
        value: "1",
        label: "GPU breadcrumbs",
        note: "Every render-graph node writes a marker before and after its work, so a lost device names the node the GPU hung in. A little slower.",
        needs: None,
    },
    CrashTest {
        key: "gpu-breadcrumb-chunks",
        env: "NVF_GPU_BREADCRUMB_CHUNKS",
        value: "1",
        label: "GPU breadcrumbs inside the transparent pass",
        note: "Cuts the transparent pass into up to 32 render passes with a marker between them, so a loss names the draws rather than the pass. Much slower.",
        needs: Some("gpu-breadcrumbs"),
    },
    CrashTest {
        key: "vulkan-validation",
        env: "NVF_VULKAN_VALIDATION",
        value: "1",
        label: "Vulkan validation (slow)",
        note: "The Khronos validation layer checks every call and writes what it finds to the run log. Expect a much slower game.",
        needs: None,
    },
    CrashTest {
        key: "vulkan-validation-sync",
        env: "NVF_VULKAN_VALIDATION_DEEP",
        value: "sync",
        label: "Vulkan sync validation",
        note: "Asks the layer for synchronization validation as well: a missing or wrong barrier between passes.",
        needs: Some("vulkan-validation"),
    },
    CrashTest {
        key: "vulkan-validation-gpuav",
        env: "NVF_VULKAN_VALIDATION_DEEP",
        value: "gpuav",
        label: "Vulkan GPU-assisted validation (very slow)",
        note: "Asks the layer to instrument the shaders, so an out-of-bounds descriptor or buffer read is caught on the GPU. A save load may not finish.",
        needs: Some("vulkan-validation"),
    },
];

/// The file in the engine home the launcher's Advanced page writes, one `key value` line per
/// switch (`OdinAdvanced.java`). Android only: a desktop run sets the variables itself.
pub const FILE: &str = "odin-advanced.txt";

/// The row `key` names, or `None` for a key no build of the fork has.
pub fn switch(key: &str) -> Option<&'static CrashTest> {
    SWITCHES.iter().find(|s| s.key == key)
}

/// The variable the switch `key` is read from, for a log line that says how to turn it on.
/// Panics for a key no row has, which a test of the table catches before any run does.
pub fn env(key: &str) -> &'static str {
    switch(key).expect("a caller names a row of SWITCHES").env
}

/// The switches `text` turns on, in [`SWITCHES`] order, from the `key value` lines the
/// launcher writes.
///
/// A value of `true` or `1` is on; anything else, including a key the launcher no longer
/// writes, is off. A line without a value, a blank line and a `#` comment are ignored, so an
/// older or newer launcher's file is read for what this build understands and no more.
pub fn requested(text: &str) -> Vec<&'static CrashTest> {
    let mut keys: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else { continue };
        if matches!(parts.next(), Some("true" | "1")) {
            keys.push(key);
        }
    }
    SWITCHES.iter().filter(|s| keys.contains(&s.key)).collect()
}

/// The switches of `requested` whose [`CrashTest::needs`] is satisfied, which is what a run
/// actually honours: a deeper check whose own switch is off would otherwise export a variable
/// nothing reads.
pub fn effective(text: &str) -> Vec<&'static CrashTest> {
    let asked = requested(text);
    let keys: Vec<&str> = asked.iter().map(|s| s.key).collect();
    asked.iter().copied().filter(|s| s.needs.is_none_or(|n| keys.contains(&n))).collect()
}

/// The variables `on` asks for, in name order, each switch's [`CrashTest::value`] and the
/// values of the switches that share a variable joined with commas in [`SWITCHES`] order.
pub fn environment(on: &[&'static CrashTest]) -> Vec<(&'static str, String)> {
    let mut grouped: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
    for switch in on {
        grouped.entry(switch.env).or_default().push(switch.value);
    }
    grouped.into_iter().map(|(env, values)| (env, values.join(","))).collect()
}

/// The one line the run log gets when any switch is on, so no run is ever read without
/// knowing what was left out of it; `None` when the run is an ordinary one.
pub fn describe(on: &[&'static CrashTest]) -> Option<String> {
    if on.is_empty() {
        return None;
    }
    let keys: Vec<&str> = on.iter().map(|s| s.key).collect();
    Some(format!("crash tests on ({}): {}", on.len(), keys.join(", ")))
}

/// Whether `switch` is on in this run: the variable it names is set to anything but nothing,
/// `0` or `false`.
///
/// The engine's gates ask this, not `std::env` directly, so a switch has one spelling.
pub fn on(switch: &CrashTest) -> bool {
    std::env::var(switch.env).is_ok_and(|v| value_on(&v, switch.value))
}

/// Whether a variable holding `value` turns a switch whose own value is `wanted` on: the
/// switches that share a variable hold a comma list, so each looks for its own word in it.
fn value_on(value: &str, wanted: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value == "0" || value == "false" {
        return false;
    }
    if wanted == "1" {
        return true;
    }
    value == "1" || value.split(',').any(|v| v.trim() == wanted)
}

/// [`on`] read once a run, for a gate that is asked every frame.
fn once(cell: &'static std::sync::OnceLock<bool>, switch: &CrashTest) -> bool {
    *cell.get_or_init(|| on(switch))
}

/// Shorthand for a switch's row and a cell of its own, as the gates want it.
macro_rules! gate {
    ($name:ident, $key:literal) => {
        /// Whether this run was asked for the switch named in the function's name. Read once.
        pub fn $name() -> bool {
            static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            static ROW: std::sync::OnceLock<&'static CrashTest> = std::sync::OnceLock::new();
            let row = ROW.get_or_init(|| switch($key).expect("a gate names a row of SWITCHES"));
            once(&ON, row)
        }
    };
}

gate!(no_vats_outline, "no-vats-outline");
gate!(gpu_breadcrumbs, "gpu-breadcrumbs");
gate!(gpu_breadcrumb_chunks, "gpu-breadcrumb-chunks");
gate!(no_particles, "no-particles");
gate!(no_decals, "no-decals");
gate!(no_heat_haze, "no-heat-haze");
gate!(no_pipboy_warm, "no-pipboy-warm");

/// Read the launcher's file in `home` and export what it asks for, before the engine starts
/// any threads (the one hook in upstream's `android.rs`). Returns [`describe`]'s line, which
/// the recorder's summary carries, or `None` for an ordinary run.
///
/// No file, an unreadable file or an empty one is an ordinary run: a crash test is never on
/// because something went wrong.
pub fn apply(home: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(home.join(FILE)).unwrap_or_default();
    let on = effective(&text);
    for (name, value) in environment(&on) {
        std::env::set_var(name, value);
    }
    describe(&on)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_switch_is_named_once_and_needs_a_switch_that_exists() {
        for (i, s) in SWITCHES.iter().enumerate() {
            assert!(SWITCHES[..i].iter().all(|o| o.key != s.key), "{} is listed twice", s.key);
            assert!(s.env.starts_with("NVF_"), "{} reads {}, which is not one of the engine's", s.key, s.env);
            assert!(!s.label.is_empty() && !s.note.is_empty(), "{} has no words for the launcher", s.key);
            assert!(s.note.len() <= 200, "{}'s note is {} characters, which the page cannot fit", s.key, s.note.len());
            if let Some(needs) = s.needs {
                assert!(switch(needs).is_some(), "{} needs {needs}, which is no switch", s.key);
            }
            // Two switches may share a variable, and then neither may be the plain `1` that
            // `value_on` reads as "every switch on this variable".
            let shared = SWITCHES.iter().filter(|o| o.env == s.env).count();
            assert_eq!(shared > 1, s.value != "1", "{} shares {} with another switch", s.key, s.env);
        }
        assert_eq!(SWITCHES[0].key, "no-vats-outline", "the page lists the suspect of the day first");
    }

    /// `nvf_assets` cannot reach this module, so it spells the name itself and this is what
    /// keeps the launcher's row and the material's gate pointed at one variable.
    #[test]
    fn the_soft_particles_switch_is_the_variable_nvf_assets_reads() {
        assert_eq!(switch("no-soft-particles").unwrap().env, nvf_assets::NO_SOFT_PARTICLES_ENV);
    }

    #[test]
    fn the_launcher_s_file_turns_on_what_it_names_and_nothing_else() {
        let text = "no-vats-outline true\nno-decals false\ngpu-breadcrumbs 1\nno-such-switch true\n# a comment\n\nno-particles\n";
        let on = requested(text);
        assert_eq!(on.iter().map(|s| s.key).collect::<Vec<_>>(), ["no-vats-outline", "gpu-breadcrumbs"]);
        assert!(requested("").is_empty(), "no file is an ordinary run");
        assert!(requested("no-decals 0\n").is_empty());
    }

    #[test]
    fn a_deeper_check_does_nothing_without_the_switch_it_needs() {
        let alone = "vulkan-validation-sync true\ngpu-breadcrumb-chunks true\n";
        assert_eq!(requested(alone).len(), 2, "the file asked for both");
        assert!(effective(alone).is_empty(), "neither has the switch it needs, so neither is exported");
        let with = "vulkan-validation true\nvulkan-validation-sync true\n";
        assert_eq!(effective(with).iter().map(|s| s.key).collect::<Vec<_>>(), ["vulkan-validation", "vulkan-validation-sync"]);
    }

    #[test]
    fn the_switches_that_share_a_variable_are_one_comma_list() {
        let both = effective("vulkan-validation true\nvulkan-validation-gpuav true\nvulkan-validation-sync true\n");
        assert_eq!(
            environment(&both),
            [("NVF_VULKAN_VALIDATION", "1".to_string()), ("NVF_VULKAN_VALIDATION_DEEP", "sync,gpuav".to_string())],
            "the deeper checks are one variable in SWITCHES order, whatever order the file named them"
        );
        assert!(environment(&[]).is_empty(), "an ordinary run exports nothing");
    }

    #[test]
    fn a_shared_variable_is_read_for_the_switch_s_own_word() {
        let sync = switch("vulkan-validation-sync").unwrap();
        let gpuav = switch("vulkan-validation-gpuav").unwrap();
        let vats = switch("no-vats-outline").unwrap();
        assert!(value_on("sync,gpuav", sync.value) && value_on("sync,gpuav", gpuav.value));
        assert!(value_on("sync", sync.value) && !value_on("sync", gpuav.value));
        assert!(value_on("1", sync.value) && value_on("1", gpuav.value), "1 is both, as the docs say");
        assert!(value_on("1", vats.value) && value_on("anything", vats.value));
        for off in ["", " ", "0", "false"] {
            assert!(!value_on(off, vats.value), "{off:?} is off");
            assert!(!value_on(off, sync.value));
        }
    }

    #[test]
    fn the_log_says_what_was_left_out_of_the_run() {
        assert_eq!(describe(&[]), None, "an ordinary run says nothing");
        let on = effective("no-vats-outline true\nno-decals true\n");
        assert_eq!(describe(&on).unwrap(), "crash tests on (2): no-vats-outline, no-decals");
    }
}
