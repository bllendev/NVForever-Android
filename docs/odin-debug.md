# NVForever Odin debug builds

This is an experimental native ARM64 Android port of upstream NVForever v0.8.1. The Android shell and controller/save handoff come from [martin2844's Retroid port](https://github.com/martin2844/NVForever-Android/tree/android-v0.6), originally based on RedZeroGotcha's M6 shell. No GameNative or Windows compatibility layer is needed.

## On the Odin

1. Download `NVForever-Odin-arm64.apk` from [Releases](https://github.com/bllendev/NVForever-Android/releases) and open it from Downloads. This APK is a separately installable, debuggable app, `org.nvforever.android.odin`. Later builds signed with the same key can update it in place. Do not uninstall between tests if you want to keep its private saves.
2. Put the `Data` folder from your own unmodified Fallout: New Vegas install in readable storage, e.g. `Internal storage/NVForever/Data`. `FalloutNV.esm` and the game's BSAs should be directly inside `Data`. The APK does not include any Fallout game data. Another app's private `Android/data` or `Android/obb` folder will not work.
3. Open NVForever Odin Debug, grant the requested all-files permission, select `Data`, enable physical controls, and start a **new** game. Start without mods. This engine does not load PC NVSE plugin DLLs or use GameNative's saves.
4. For a smoke test, confirm the main menu, left stick and camera, then save and reload. For a startup failure, reopen the launcher and tap **Copy debug summary**. It copies build/device information and a short log tail locally; review before pasting into Discord. Nothing is uploaded by that button.

## Iteration

- Pushing `odin/**` builds an APK and uploads a GitHub Actions artifact; it does not publish a release. `odin-v*` tags build, verify and publish a GitHub prerelease. Create each new tag from the tested commit, never move a published tag.
- CI uses JDK 17, Gradle 8.11.1, Android SDK 35, NDK 28 and Rust with `aarch64-linux-android`. For local work, set `ANDROID_HOME` and `ANDROID_NDK_HOME`, install `cargo-ndk`, and run `gradle -p android assembleDebug`. Use `-PprebuiltRust` only when a matching native library already exists in `android/app/src/main/jniLibs/arm64-v8a/`.
- The signing key lives outside Git and is restored from the `ODIN_KEYSTORE_B64` Actions secret only for packaging. Never commit a keystore or its encoded contents. Local builds using a different key cannot update the CI-signed package.
- With an authorized USB device, `adb install -r android/app/build/outputs/apk/debug/app-debug.apk` installs a local build when keys match. `adb logcat -s NVForever` is useful for native startup failures. Do not use GameNative's private files as scratch space.
