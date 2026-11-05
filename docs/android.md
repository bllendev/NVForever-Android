# Android / Retroid build

This branch ports upstream v0.6.0 (Bevy 0.20.1) to ARM64 Android. It uses the
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
The desktop program remains available through its small binary entry point;
the same engine now also exposes `run_with` for platform plugins.

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
