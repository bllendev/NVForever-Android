# Android / Retroid build

This branch ports upstream NVForever (Bevy 0.20.1) to ARM64 Android. It uses the
author's published M6 Android shell as its starting point, with native controller
input and activity-based save loading. It includes no game files.

## Build

Install Rust stable, the `aarch64-linux-android` target, `cargo-ndk`, JDK 17,
Android SDK 35 and an Android NDK. Gradle 8.11.1 or compatible 8.x is required.
Set `ANDROID_HOME` and `ANDROID_NDK_HOME` for your machine, then run:

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
gradle -p android assembleDebug
```

The app is `android/app/build/outputs/apk/debug/app-debug.apk`. Gradle compiles
the release Rust library before packaging it. `-PprebuiltRust` can skip that
step when the matching library is already in `app/src/main/jniLibs/arm64-v8a/`.
The desktop program remains available through its small binary entry point
(`engine/nvforever/src/desktop.rs`); the same engine, whose crate root is upstream's
own `src/main.rs`, also exposes `odin::run_with` for platform plugins (fork.md).

This build uses the separate application ID `org.nvforever.android.retroid`.
It preserves that app's private settings and saves when installed with the same
signing key. Keep a backup before changing signing keys or engine versions.
SDK paths, signing keys, APKs, saves, captures and build products are not committed.

## Data and controls

Copy your own Steam game's Data files to `/sdcard/NVForever/Data`, or choose an
accessible Data directory in the launcher. Grant file access there.
The launcher has independent switches for physical controllers and touch controls.
They apply on the next game start and persist in
`/sdcard/NVForever/nvforever-controls.txt`:

```text
controller true
touch false
```

The native adapter feeds Bevy's existing Gamepad bindings. The left stick moves
and navigates menus, the right stick looks, and upstream's menu-specific controller
actions are retained. D-pad Up changes ammunition. Only a complete old default
D-pad profile is automatically migrated; custom bindings are preserved.
Optional `diagnostics true` in that file logs native input events.

## The launcher and data profiles

The launcher is six screens: Setup (all-files access, the Data folder, the file
check), Home (CONTINUE, PLAY, the status card, the active profile), Data (the
profile list), Settings (launcher options only), About, and a hidden Debug screen
with the crash-test switches. Its words are Android string resources
(`android/app/src/main/res/values*/strings.xml`, English the default; see
`android/README-translations.md`), and its one typeface is bundled
(`android/THIRD-PARTY.md`).

A **data profile** is a name, a Data folder, and a home folder. Everything else
follows from the home, because `fnv_install::EngineHome` derives the plugin list,
the saves, the logs and the mods folder from its root and `EngineHome::default_root`
takes `NVFOREVER_HOME` first.

```text
/data/user/0/org.nvforever.android/files/
    data-path.txt        the active profile's Data folder        (unchanged contract)
    profile.txt          the active profile's home, relative     (read by paths.rs)
    profiles.txt         the launcher's own list                 (the engine never reads it)
    android-continue.txt one start's save, for CONTINUE          (consumed once)
    NVForever/           the "Vanilla" profile's home: NVForever.ini, plugins.txt,
                         saves/, logs/, cache/, mods/
    profiles/<id>/       any other profile's home, the same files again
```

The first profile is always Vanilla and keeps `files/NVForever`, the home every
install has had, so an existing install's saves, settings and logs stay exactly
where they are. Nothing is moved or renamed, and `files/profiles/` is created only
when a second profile is added. Switching a profile writes `data-path.txt` and
`profile.txt`, and nothing else. "Forget" removes a row from the list and touches
no folder, save or log.

**Saves are per profile**, because a save made with mods, loaded without them,
fails late and looks like an engine bug. The launcher offers to copy saves into a
new profile; the folder copied from is left as it was.

**Settings are shared across profiles** (owner decision): a profile switch must not
change resolution, difficulty or anything else on the game's settings page. The INI
cannot be linked, because every writer replaces it whole through a temporary file
and a rename (`fnv_install::write_whole`), which would break a hard link and orphan
a symlink. So `paths::share_settings` copies the newest `NVForever.ini` among all
the profile homes into the active one before the game starts. Only one profile runs
at a time and the launcher never writes the file, so "newest wins" has exactly one
candidate: whichever profile ran last.

**Mod plugins load on Android from this build on.** `developer_data` in
`engine/nvforever/src/lib.rs` used to be `args.data.is_some()`, and every Android
start passes `--data`, so the branch that reads the home's `plugins.txt` never ran
and the load order was always the base game plus the DLC masters. The Android shell
now passes `--player-start` as well, which says "a player's start even so", and the
profile's own `plugins.txt` is the load order. A developer or smoke run, which
passes `--data` without that flag, is unchanged.

A Mods screen for editing that list is phase 2; the Data area shows the row as
"coming soon" rather than hiding it.

## Graphics drivers

`org.nvforever.android.GpuDrivers` is the contract between the launcher and the
custom Vulkan driver support (Turnip through libadrenotools), which is being built
separately. Six static methods: `installed(Context)` as `{id, label}` pairs,
`selected(Context)` (null means the system driver), `select(Context, idOrNull)`,
`install(Context, Uri)` returning a new id, `delete(Context, id)`, and
`lastStartFellBack(Context)`, a short reason or null, which Home shows. The body in
this build is a stub: the list is empty, the selection is the system driver, and
installing throws with a message fit to show.

## Save loading on Android

The desktop handoff starts a fresh executable. NativeActivity requires a fresh
Android game process instead. After pending saves finish and the loading cover is
up, the Android handoff writes a JSON request into the app's private engine home.
GameActivity notices it and opens the launcher. The launcher consumes the request,
checks the old process's PID, UID and name, ends only that game's process, waits
for it to disappear and starts GameActivity again. The Rust entry consumes the
private command line exactly once. JSON preserves spaces in save paths.

This supports both load requests and return-to-main-menu requests without trying
to spawn `app_process` as a desktop game. The public launcher Intent carries only
a wake-up flag; arguments and the process ID are read from private storage.
A missing activity acknowledgement times out and removes the loading cover.

## Checks

```sh
cargo test --manifest-path engine/Cargo.toml -p nvf-android-input
cargo clippy --manifest-path engine/Cargo.toml --release --target aarch64-linux-android -p nvf-android --lib --tests -- -D warnings
cargo test --manifest-path engine/Cargo.toml --release --target aarch64-linux-android -p nvf-android --lib -- --test-threads=1
```

The last command needs a Cargo target runner which copies the test executable to
an authorized Android device and runs it through `adb shell`. Adapter tests use
headless Bevy and require no game files. Device checks must additionally cover
Continue, movement/look, Pip-Boy, pause/save/load and returning to the main menu.
Keep all game-derived evidence outside this repository.

## Device validation (2026-11-05)

On the owner's RP5 running Android 13, the release APK was installed over the
earlier Retroid build, preserving its data. Continue loaded the existing v0.5.1
autosave into Goodsprings through a fresh Android game process. Saves created from
the pause menu were then loaded successfully, including a save path with a space.
Native Android input injection exercised both sticks, pause and Pip-Boy buttons,
and LT/RT section changes. Reopening through the launcher after Home preserved
the running game process rather than initializing a second Bevy instance.
The RP5's physical ABXY order was separately recorded from the owner's presses.

Eight ARM64 tests ran on the actual device and passed; the std-only input test and
Android-target clippy with warnings denied also passed. This is a device smoke
check, not a complete playthrough or a full console-parity certification.

The checked profile retains HDR glow and eye adaptation. The previous M6 profile
with `glow=0` produced unreadable first-person rendering on this RP5; restoring
`glow=1` and `eye-adaptation=1` made the 3D Pip-Boy readable and corrected exterior
exposure. Neon glow remains independently optional. Avoid carrying over the old
`--graphics-only no-glow,no-eye-adaptation,no-neon-glow` tuning unchanged.

Android does not require desktop pointer capture for controller/touch camera input.
The player uses virtual input without capture, and the vendored window backend
accepts releasing an uncaptured pointer as a no-op; real capture remains unsupported.
