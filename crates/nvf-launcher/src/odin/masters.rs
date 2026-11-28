//! Missing masters: what a start is refused for, and the one tap that clears it
//! (docs/fork.md).
//!
//! A plugin's TES4 header names the files it was built on top of, one `MAST` subrecord each.
//! When one of those is not in the load order, or is in it but turned off, the engine cannot
//! map that plugin's FormIDs and the game dies somewhere in the load, which on the owner's
//! device looked like "I added mods and it crashed" with nothing to send.
//!
//! The header is read with the launcher's own reader, [`plugins::master_names`], which walks
//! the 24-byte record header and its subrecords (docs/spec-notes/esm.md). Nothing here parses
//! a byte of its own.
//!
//! What this deliberately does **not** check: a plugin that wants a script extender plugin.
//! Nothing in a plugin's header says so (the need is in its scripts), so there is no cheap
//! test for it and a guess would turn off mods that work.

use fnv_install::{plugins, EngineHome, GameInstall};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The master the game always loads, whatever the list says (`mobile::Request::Plugin`).
const BASE: &str = "FalloutNV.esm";

/// How many plugins a refusal names before it says how many more there are.
const NAMED: usize = 3;

/// One enabled plugin that cannot load, and the masters it names that nothing will load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The plugin, as the load order spells it.
    pub plugin: String,
    /// Its missing masters, in the plugin's own order: not in the list, turned off, or in
    /// the list with no file to open.
    pub missing: Vec<String>,
}

/// The enabled plugins whose masters are not all loaded, in load order.
///
/// `entries` is the load order as the Mods page has it, `data` the game's `Data` folder and
/// `mods` the home's mods folder when there is one. A plugin whose own file cannot be found
/// is not judged here: it is not loaded either way, and upstream's own sync says so.
pub fn problems(entries: &[(String, bool)], data: &Path, mods: Option<&Path>) -> Vec<Problem> {
    let file_of = |name: &str| plugins::plugin_path(data, mods, name);
    let mut on: Vec<String> = entries
        .iter()
        .filter(|(_, active)| *active)
        .map(|(name, _)| name.to_ascii_lowercase())
        .collect();
    let base = BASE.to_ascii_lowercase();
    if !on.contains(&base) {
        on.push(base);
    }
    let loaded = |name: &str| {
        let key = name.to_ascii_lowercase();
        on.iter().any(|n| *n == key) && file_of(name).is_file()
    };
    entries
        .iter()
        .filter(|(_, active)| *active)
        .filter_map(|(name, _)| {
            let path = file_of(name);
            if !path.is_file() {
                return None;
            }
            let mut missing: Vec<String> = Vec::new();
            for master in plugins::master_names(&path) {
                if !loaded(&master) && !missing.iter().any(|m: &String| m.eq_ignore_ascii_case(&master)) {
                    missing.push(master);
                }
            }
            (!missing.is_empty()).then(|| Problem { plugin: name.clone(), missing })
        })
        .collect()
}

/// The load order this home starts with, the game folder it reads and its mods folder.
/// `None` when there is no complete game folder, in which case nothing can be judged and the
/// start is refused for want of a game folder instead.
fn load_order(home: &EngineHome) -> Option<(Vec<(String, bool)>, GameInstall, PathBuf)> {
    let ini = std::fs::read_to_string(home.config_file()).unwrap_or_default();
    let game = crate::launch::game_from_ini(&ini).filter(|g| g.missing_files().is_empty())?;
    let text = std::fs::read_to_string(home.plugins_file()).unwrap_or_default();
    let list = plugins::PluginList::parse(&text);
    let entries = list.entries.into_iter().map(|e| (e.name, e.active)).collect();
    Some((entries, game, home.mods()))
}

/// This home's problems, for the Mods page and for the refusal.
pub fn of_home(home: &Path) -> Vec<Problem> {
    let home = EngineHome::new(home);
    let Some((entries, game, mods)) = load_order(&home) else { return Vec::new() };
    problems(&entries, &game.data, Some(&mods))
}

/// The Mods page's own ask: one entry per plugin that cannot load.
pub fn as_json(home: &Path) -> Value {
    let list: Vec<Value> = of_home(home)
        .into_iter()
        .map(|p| json!({"plugin": p.plugin, "missing": p.missing}))
        .collect();
    json!({"problems": list})
}

/// What the player is told instead of a start. One sentence per named plugin, so it reads
/// the same in the status line and in the dialog the launcher raises.
pub fn sentence(problems: &[Problem]) -> String {
    let named: Vec<String> = problems
        .iter()
        .take(NAMED)
        .map(|p| format!("{} needs {}", p.plugin, p.missing.join(" and ")))
        .collect();
    let rest = problems.len().saturating_sub(NAMED);
    let more = match rest {
        0 => String::new(),
        1 => ", and one more plugin".into(),
        n => format!(", and {n} more plugins"),
    };
    format!(
        "{}{}. {} not in the load order, or turned off. Turn off the plugins with missing masters, or enable the masters, then start again.",
        named.join("; "),
        more,
        if problems.len() == 1 && problems[0].missing.len() == 1 { "That master is" } else { "Those masters are" }
    )
}

/// The check `mobile::Request::Start` makes before it hands out a command line: a start whose
/// load order cannot work is refused with the sentence, rather than a game that dies in the
/// load with nothing to send.
pub fn refuse_start(home: &Path) -> Result<(), String> {
    let problems = of_home(home);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(sentence(&problems))
    }
}

/// Turn off every plugin with a missing master, which is the launcher's one tap. Turning one
/// off can leave another without **its** master, so this settles rather than passing once.
/// Returns the plugins turned off, in load order.
pub fn turn_off(home: &Path) -> Result<Vec<String>, String> {
    let engine = EngineHome::new(home);
    let Some((mut entries, game, mods)) = load_order(&engine) else {
        return Err("Set up a game folder first.".into());
    };
    let mut off: Vec<String> = Vec::new();
    for _ in 0..entries.len().max(1) {
        let found = problems(&entries, &game.data, Some(&mods));
        if found.is_empty() {
            break;
        }
        for problem in found {
            if let Some(entry) = entries.iter_mut().find(|(name, _)| *name == problem.plugin) {
                entry.1 = false;
                off.push(problem.plugin);
            }
        }
    }
    if off.is_empty() {
        return Ok(off);
    }
    let text = std::fs::read_to_string(engine.plugins_file()).unwrap_or_default();
    let mut list = plugins::PluginList::parse(&text);
    for entry in &mut list.entries {
        if off.iter().any(|name| *name == entry.name) {
            entry.active = false;
        }
    }
    let path = engine.plugins_file();
    fnv_install::guard_write(&path, Some(&game)).map_err(|e| e.to_string())?;
    fnv_install::write_whole(&path, list.to_text().as_bytes()).map_err(|e| e.to_string())?;
    Ok(off)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin file: a TES4 record whose header names `masters`, as
    /// `plugins::master_names` reads one (docs/spec-notes/esm.md).
    fn plugin(path: &Path, masters: &[&str]) {
        let mut body = Vec::new();
        for master in masters {
            let mut name = master.as_bytes().to_vec();
            name.push(0);
            body.extend(b"MAST");
            body.extend((name.len() as u16).to_le_bytes());
            body.extend(&name);
            body.extend(b"DATA");
            body.extend(8u16.to_le_bytes());
            body.extend(0u64.to_le_bytes());
        }
        let mut file = Vec::new();
        file.extend(b"TES4");
        file.extend((body.len() as u32).to_le_bytes());
        file.extend(0u32.to_le_bytes()); // flags
        file.extend(0u32.to_le_bytes()); // FormID
        file.extend(0u32.to_le_bytes()); // version control
        file.extend(0u16.to_le_bytes()); // form version
        file.extend(0u16.to_le_bytes()); // version control 2
        file.extend(&body);
        std::fs::write(path, file).unwrap();
    }

    fn temp(tail: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nvf-odin-masters-{tail}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn order(entries: &[(&str, bool)]) -> Vec<(String, bool)> {
        entries.iter().map(|(n, a)| ((*n).to_string(), *a)).collect()
    }

    /// A master that is not in the list at all, one that is in it but turned off, and one
    /// that is enabled with no file: each is missing, and nothing else is.
    #[test]
    fn a_master_that_will_not_load_is_named_with_the_plugin_that_wants_it() {
        let dir = temp("missing");
        let data = dir.join("Data");
        std::fs::create_dir_all(&data).unwrap();
        plugin(&data.join("FalloutNV.esm"), &[]);
        plugin(&data.join("Working.esp"), &["FalloutNV.esm"]);
        plugin(&data.join("Orphan.esp"), &["Gone.esm"]);
        plugin(&data.join("TurnedOff.esp"), &["Sleeping.esm"]);
        plugin(&data.join("Sleeping.esm"), &[]);
        plugin(&data.join("NoFile.esp"), &["Promised.esm"]);

        let entries = order(&[
            ("FalloutNV.esm", true),
            ("Sleeping.esm", false),
            ("Promised.esm", true),
            ("Working.esp", true),
            ("Orphan.esp", true),
            ("TurnedOff.esp", true),
            ("NoFile.esp", true),
        ]);
        let found = problems(&entries, &data, None);
        assert_eq!(
            found,
            vec![
                Problem { plugin: "Orphan.esp".into(), missing: vec!["Gone.esm".into()] },
                Problem { plugin: "TurnedOff.esp".into(), missing: vec!["Sleeping.esm".into()] },
                Problem { plugin: "NoFile.esp".into(), missing: vec!["Promised.esm".into()] },
            ],
            "Working.esp loads, and a plugin that is off is not judged"
        );

        // Turning the master on clears its plugin, and FalloutNV.esm counts as loaded even
        // when the list leaves it out, because the game always loads it.
        let entries = order(&[("Sleeping.esm", true), ("TurnedOff.esp", true), ("Working.esp", true)]);
        assert_eq!(problems(&entries, &data, None), Vec::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The plugin's own name is read whatever case either side spells it in, and a master
    /// named twice is said once.
    #[test]
    fn the_case_of_a_name_is_not_the_test() {
        let dir = temp("case");
        let data = dir.join("Data");
        std::fs::create_dir_all(&data).unwrap();
        plugin(&data.join("FalloutNV.esm"), &[]);
        plugin(&data.join("Base.esm"), &[]);
        plugin(&data.join("Patch.esp"), &["BASE.esm", "base.esm"]);
        let entries = order(&[("base.ESM", true), ("Patch.esp", true)]);
        assert_eq!(problems(&entries, &data, None), Vec::new());

        let entries = order(&[("base.ESM", false), ("Patch.esp", true)]);
        assert_eq!(
            problems(&entries, &data, None),
            vec![Problem { plugin: "Patch.esp".into(), missing: vec!["BASE.esm".into()] }],
            "one line per missing master, however many times it is named"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A plugin in the mods folder is opened from there, as a start opens it.
    #[test]
    fn a_plugin_in_the_mods_folder_is_read_from_there() {
        let dir = temp("mods");
        let data = dir.join("Data");
        let mods = dir.join("mods");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&mods).unwrap();
        plugin(&data.join("FalloutNV.esm"), &[]);
        plugin(&data.join("Mod.esp"), &[]);
        plugin(&mods.join("Mod.esp"), &["Missing.esm"]);
        let entries = order(&[("Mod.esp", true)]);
        assert_eq!(problems(&entries, &data, None), Vec::new(), "Data's copy names no master");
        assert_eq!(
            problems(&entries, &data, Some(&mods)),
            vec![Problem { plugin: "Mod.esp".into(), missing: vec!["Missing.esm".into()] }],
            "the mods folder's copy is the one that would load"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The sentence names the plugins and what each one wants, and says how many it did not
    /// name. It is what the status line and the dialog both show.
    #[test]
    fn the_refusal_says_which_plugin_wants_which_master() {
        let one = vec![Problem { plugin: "Orphan.esp".into(), missing: vec!["Gone.esm".into()] }];
        let said = sentence(&one);
        assert!(said.starts_with("Orphan.esp needs Gone.esm."), "{said}");
        assert!(said.contains("That master is"), "{said}");
        assert!(said.contains("Turn off the plugins with missing masters"), "{said}");

        let many: Vec<Problem> = (0..5)
            .map(|i| Problem { plugin: format!("Mod{i}.esp"), missing: vec!["Gone.esm".into()] })
            .collect();
        let said = sentence(&many);
        assert!(said.contains("Mod2.esp needs Gone.esm, and 2 more plugins."), "{said}");
        assert!(!said.contains("Mod3.esp"), "only the first three are named: {said}");
    }

    /// The one tap: every plugin with a missing master is turned off, including one that is
    /// only broken once another is, and the file keeps its order.
    #[test]
    fn turning_them_off_settles_rather_than_passing_once() {
        let dir = temp("turn-off");
        let home = dir.join("NVForever");
        let data = dir.join("game/Data");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        for name in fnv_install::REQUIRED_DATA_FILES {
            std::fs::write(data.join(name), b"x").unwrap();
        }
        plugin(&data.join("FalloutNV.esm"), &[]);
        plugin(&data.join("Chain.esm"), &["Gone.esm"]);
        plugin(&data.join("OnChain.esp"), &["Chain.esm"]);
        plugin(&data.join("Fine.esp"), &["FalloutNV.esm"]);
        std::fs::write(
            home.join("NVForever.ini"),
            format!("[Game]\nPath={}\n", dir.join("game").display()),
        )
        .unwrap();
        std::fs::write(home.join("plugins.txt"), "*FalloutNV.esm\n*Chain.esm\n*OnChain.esp\n*Fine.esp\n").unwrap();

        assert!(refuse_start(&home).is_err(), "a start is refused while the list cannot work");
        let mut off = turn_off(&home).unwrap();
        off.sort();
        assert_eq!(off, vec!["Chain.esm".to_string(), "OnChain.esp".to_string()]);
        let text = std::fs::read_to_string(home.join("plugins.txt")).unwrap();
        let list = plugins::PluginList::parse(&text);
        let state: Vec<(String, bool)> = list.entries.iter().map(|e| (e.name.clone(), e.active)).collect();
        assert_eq!(
            state,
            vec![
                ("FalloutNV.esm".into(), true),
                ("Chain.esm".into(), false),
                ("OnChain.esp".into(), false),
                ("Fine.esp".into(), true),
            ],
            "the order is kept and only the broken plugins go off"
        );
        assert_eq!(refuse_start(&home), Ok(()), "and the start is allowed again");
        assert_eq!(turn_off(&home).unwrap(), Vec::<String>::new(), "a second tap changes nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With no game folder there is nothing to judge, so a start is not refused for this.
    #[test]
    fn nothing_is_judged_without_a_game_folder() {
        let dir = temp("no-game");
        let home = dir.join("NVForever");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(of_home(&home), Vec::new());
        assert_eq!(refuse_start(&home), Ok(()));
        assert_eq!(as_json(&home)["problems"].as_array().map(Vec::len), Some(0));
        assert!(turn_off(&home).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
