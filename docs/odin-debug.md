# NVForever Odin debug builds

This is an experimental native ARM64 Android port of upstream NVForever v0.8.1. The Android shell and controller/save handoff come from [martin2844's Retroid port](https://github.com/martin2844/NVForever-Android/tree/android-v0.6), originally based on RedZeroGotcha's M6 shell. No GameNative or Windows compatibility layer is needed.

## On the Odin

1. Download `NVForever-Odin-arm64.apk` from [Releases](https://github.com/bllendev/NVForever-Android/releases) and open it from Downloads. This APK is a separately installable, debuggable app, `org.nvforever.android.odin`. Later builds signed with the same key can update it in place. Do not uninstall between tests if you want to keep its private saves.
2. Put the `Data` folder from your own unmodified Fallout: New Vegas install in readable storage, e.g. `Internal storage/NVForever/Data`. `FalloutNV.esm` and the game's BSAs should be directly inside `Data`. The APK does not include any Fallout game data. Another app's private `Android/data` or `Android/obb` folder will not work.
3. Open NVForever Odin Debug, grant the requested all-files permission, select `Data`, enable physical controls, and start a **new** game. Start without mods. This engine does not load PC NVSE plugin DLLs or use GameNative's saves.
4. For a smoke test, confirm the main menu, left stick and camera, then save and reload. For audio, graphics, or startup issues, return to the launcher and tap **Export debug log**. Save the `.txt` to Downloads or a writable SD card folder, review it, and attach it in Discord. The file contains build/device information, the latest engine run log (bounded to 224 KiB of input per file), and a matching crash report if present. It does not export saves, configuration files, or device-wide logs, and nothing is uploaded automatically. **Copy debug summary** is still available for a quick log tail.

### Where the crash tests moved to

They are no longer on the first screen. The launcher opens on Home, and every crash-test switch is on a **Debug** screen, reached two ways: hold the build chip in the top right of Home for about a second, or Settings, then the **Debug** row. Each switch keeps its key and its file (`NVForever/nvforever-controls.txt`) exactly as below; only where you turn it on has changed. Home shows "N tests on" in amber next to the build while any of them is on, and the Debug screen carries the same count, so nobody plays for a week wondering why the game stutters. **Turn every crash test off** is one action on that screen.

**Export debug log** and **Copy debug summary** are now in release builds too, on the Debug screen under Reports and on Home's status card, and they read the active data profile's `logs/` folder. When a run ends unexpectedly, Home says so and links straight to them: that is the path a player with an actual problem takes, so the hidden menu never hides the thing they need.

## Crash test: Vulkan validation (slow)

The launcher has an off-by-default switch, **Crash test: Vulkan validation (slow)**, for the Odin 3's `DeviceLost`. On, it writes `vulkan-validation true` to `NVForever/nvforever-controls.txt`, and the next game start loads the Khronos validation layer and asks wgpu for it (`InstanceFlags::VALIDATION|DEBUG`). Validation messages go through wgpu-hal's debug-utils messenger into the run log (and `adb logcat -s NVForever`) at warn/error; the run log's filter turns `wgpu_hal` up to `debug` for the run, because Bevy's default holds everything under `wgpu` at error. One line at startup says the switch is on and whether the layer library loaded. Expect a much slower game: leave the switch off for anything but this test.

- The layer binary is `libVkLayer_khronos_validation.so` from the Khronos [Vulkan-ValidationLayers release `vulkan-sdk-1.4.363.1`](https://github.com/KhronosGroup/Vulkan-ValidationLayers/releases/tag/vulkan-sdk-1.4.363.1) (asset `android-binaries-1.4.363.1.zip`, sha256 `2c17b00a95b18fd63244b27332ef86f45c9269769f798a6261ce80b869c41af4`; the arm64-v8a library inside it, sha256 `7d36b66e72fd11270fba96de10002256ba95c8408047a8167e636673c16760ae`). It is **licensed Apache-2.0 by the Khronos Group and its contributors**, is not part of NVForever and is never committed: the Odin CI workflow downloads it, checks both hashes and unpacks only that one library into `android/app/src/main/jniLibs/arm64-v8a/` before Gradle packages the APK. Its licence travels with the Khronos release.
- How the layer is found on the device: the APK is debuggable, so Android's Vulkan loader may list layers from the app's own native library folder. For that folder to hold files, the app packages its libraries the legacy way (`extractNativeLibs=true`, `app/build.gradle.kts`), so the installer extracts them. Belt and braces, `nvf-android` also loads the library by name itself before the engine starts, the way the pre-Android-9 recipe did. Both libraries' ELF `LOAD` segments are 16 KB-page aligned, which CI checks for each of them.
- If a run says the layer did not load or wgpu-hal says `unable to find layer`, enable it from outside the app instead and start the game again:
  `adb shell settings put global enable_gpu_debug_layers 1`, `adb shell settings put global gpu_debug_app org.nvforever.android.odin`, `adb shell settings put global gpu_debug_layers VK_LAYER_KHRONOS_validation`. Undo with `adb shell settings delete global enable_gpu_debug_layers`.
- A local `gradle -p android assembleDebug` packages whatever is in `jniLibs/arm64-v8a/`; without the download step the switch simply finds no layer and says so.

### The deeper checks: two more switches

The core checks found nothing before the loss, so two further switches ask the same layer for what core validation cannot see. Both need **Crash test: Vulkan validation (slow)** on as well; on their own they do nothing, because the layer that would read their settings is not loaded.

- **Crash test: Vulkan sync validation** writes `vulkan-validation-sync true` and asks for synchronization validation: a missing or wrong barrier between passes, which a tiler forgives until it does not.
- **Crash test: Vulkan GPU-assisted validation (very slow)** writes `vulkan-validation-gpuav true` and asks for GPU-assisted validation, which instruments the shaders so an out-of-bounds descriptor or buffer read is caught on the GPU. It is slower again than the core checks and a save load may not finish at all on the phone, which is why it is its own switch rather than part of the one above.

Both become `NVF_VULKAN_VALIDATION_DEEP=sync,gpuav` for the engine, which chains a `VkLayerSettingsCreateInfoEXT` onto the Vulkan instance naming `khronos_validation.validate_core`, `.validate_sync` and `.gpuav_enable` [documented: the Khronos validation layer's layer settings; a setting name the layer does not know is ignored, so it simply keeps its defaults]. GPU-assisted validation also sets wgpu's own `GPU_BASED_VALIDATION` instance flag, so wgpu-hal chains the older `VkValidationFeaturesEXT` for it too. The startup line says which checks were asked for.

## Crash test: GPU breadcrumbs

**Crash test: GPU breadcrumbs** (off by default, `gpu-breadcrumbs true`) answers the one question the flight recorder cannot: which render-graph node the GPU was executing when it stopped. On, every node of the render graph — the prepass, shadows, opaque, transparent, post-processing, UI and the game's own nodes — writes a four-byte marker into host-visible memory before anything it records and another after the last of it. A lost device leaves that memory readable, so the dump beside the flight recorder's says, for each of the last frames, the last node the GPU began, the last it ended, and any node it began and never ended: that last is the node that hung.

- It is independent of the validation switches and far cheaper than them: 60-120 extra one-command command buffers a frame, no extra memory worth counting (4 KiB) and nothing drawn differently. [observed 2026-11-21, one pair of desktop runs, Goodsprings 12:00, the `dev` profile: 9.2 ms a frame with the switch on against 8.5 ms with it off — an order of magnitude, not a measurement.] It is still a diagnostic, so it is off unless asked for.
- On a desktop run, `NVF_GPU_BREADCRUMBS=1` does the same thing.
- What it cannot promise: the markers carry no barriers, so "the GPU reached this node's begin" is not "every command before it retired", and a value reaches the CPU when the driver flushes it, which on a hung queue is best effort. That is why the dump prints the whole window rather than one answer.

### Finer breadcrumbs: the transparent pass in chunks

The breadcrumbs named the pass: on a save load, frames 2 to 13 completed and frame 14 began
`bevy_core_pipeline::core_3d::main_transparent_pass_3d_node::main_transparent_pass_3d` and
never ended (the transmissive pass before it ended) [observed 2026-11-22]. That one pass
draws every blended thing in the world, so the Debug screen's **GPU breadcrumbs inside the
transparent pass** (`gpu-breadcrumb-chunks true`, greyed until the breadcrumbs switch above it
is on) cuts it up: a
run of consecutive sorted transparent items sharing a draw function and a pipeline is drawn in
a render pass of its own, with a marker before and after it, up to 32 chunks a frame.

- A marker is a `vkCmdFillBuffer`, which Vulkan forbids inside a render pass, so a marker
  cannot sit between two draws of one pass. One render pass per chunk is the way to get a
  marker between the draws, and it is also the cost: each chunk loads and stores the colour
  and depth attachments, which on a tiler is the expensive thing. Leave the switch off for
  anything but this test.
- Nothing is drawn differently: the phase keeps its sorted order and a cut only ever falls on
  a batch boundary, so the same draws happen in the same order.
- The dump beside the node breadcrumbs then says which chunk the GPU began and never ended,
  with its item range, draw count, pipeline id, the pipeline's own label and shader file, and
  what is drawn in it: particles, effect particles and decals tag their drawn entity with
  their form or model while the switch is on, so the line can read
  `particles meshes\effects\ambient\fxsprayjet01.nif pSteam07`. The chunk tables of the last
  24 frames are kept, so the frame the markers point at is still explained.
- `NVF_GPU_BREADCRUMB_CHUNKS=1` with `NVF_GPU_BREADCRUMBS=1` does the same on a desktop run.
- Engine code: `engine/nvforever/src/gpu_recorder/transparent_chunks.rs`; the marker itself is
  `RenderContext::gpu_breadcrumb` in the vendored `bevy_render`.

### Crash tests: one drawn feature each

Four switches on the Debug screen, under "Leave something out of the picture", each off by
default and each gating only its own feature, for the suspects in that pass. They are
independent of the breadcrumbs and of each other.

| Launcher switch | `nvforever-controls.txt` | Environment | What it does |
| --- | --- | --- | --- |
| No soft particles | `no-soft-particles true` | `NVF_NO_SOFT_PARTICLES=1` | A particle gets no soft-intersection distance, so it has a hard edge where it passes into a wall. A NIF material without a soft distance is then built without the soft-intersection path at all, so its fragment shader reads the view's depth prepass nowhere. |
| No particles | `no-particles true` | `NVF_NO_PARTICLES=1` | No particle system and no effect particle is drawn. They are still simulated, so nothing else in the game changes. |
| No decals | `no-decals true` | `NVF_NO_DECALS=1` | No decal is placed (blood, bullet holes). The shots and their impacts are unchanged. |
| No heat haze | `no-heat-haze true` | `NVF_NO_HEAT_HAZE=1` | The heat-haze pass records nothing. It is a post-process pass, not a transparent draw, and is off in every preset anyway. |

## Iteration

- Pushing `odin/**` builds an APK and uploads a GitHub Actions artifact; it does not publish a release. `odin-v*` tags build, verify and publish a GitHub prerelease. Create each new tag from the tested commit, never move a published tag.
- CI uses JDK 17, Gradle 8.11.1, Android SDK 35, NDK 28 and Rust with `aarch64-linux-android`. For local work, set `ANDROID_HOME` and `ANDROID_NDK_HOME`, install `cargo-ndk`, and run `gradle -p android assembleDebug`. Use `-PprebuiltRust` only when a matching native library already exists in `android/app/src/main/jniLibs/arm64-v8a/`.
- The signing key lives outside Git and is restored from the `ODIN_KEYSTORE_B64` Actions secret only for packaging. Never commit a keystore or its encoded contents. Local builds using a different key cannot update the CI-signed package.
- With an authorized USB device, `adb install -r android/app/build/outputs/apk/debug/app-debug.apk` installs a local build when keys match. `adb logcat -s NVForever` is useful for native startup failures. Do not use GameNative's private files as scratch space.
