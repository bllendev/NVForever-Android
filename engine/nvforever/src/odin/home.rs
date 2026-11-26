//! Where the Android fork keeps the player's own files, and why it is not upstream's folder.
//!
//! Upstream's `android::prepare` sets `NVFOREVER_HOME` to `<internal files>/home`. Every
//! install of this fork, including the owner's, has it at `<internal files>/NVForever`
//! instead: that folder holds the saves, `NVForever.ini`, `plugins.txt` and the run logs of
//! builds going back to the fork's first APK, and an application id that upgrades in place
//! would otherwise find an empty home and look like a lost save game.
//!
//! So the fork's whole change to the home is this name, used in two places that must agree:
//!
//! - the engine, through the one line of `android.rs` that joins it onto the files directory;
//! - the Java launcher, through `OdinFork.HOME` (`android/app/src/main/java/.../OdinFork.java`),
//!   which is where the launcher reads the settings, the plugin list and the saves from and
//!   where the engine's `lang` files are unpacked to.
//!
//! Nothing else about the saves is touched: they are not moved, copied, converted or renamed.
//!
//! The game folder is a separate question ([`super`]'s module list and the launcher's
//! `odin::adopt`, `crates/nvf-launcher/src/odin.rs`): the home is always in private app
//! storage, which keeps it outside the game folder whatever the player chose
//! (`fnv_install::check_separation`).

/// The engine home's folder name inside the app's private files directory.
///
/// Keep this the same as `OdinFork.HOME` in the Java launcher.
pub const DIR: &str = "NVForever";

#[cfg(test)]
mod tests {
    /// The fork's home is the folder the owner's install already has, not upstream's `home`.
    /// A change here is a change to where every existing install's saves are looked for, so
    /// the test states the value rather than deriving it.
    #[test]
    fn the_home_is_the_folder_every_install_of_the_fork_already_has() {
        assert_eq!(super::DIR, "NVForever");
        assert_ne!(super::DIR, "home", "upstream's name would hide the player's saves");
    }
}
