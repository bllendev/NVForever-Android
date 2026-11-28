# The Android fork of NVForever

This repository (`bllendev/NVForever-Android`) is a fork of
[LootJunkieAzza/NVForever](https://github.com/LootJunkieAzza/NVForever). Upstream now carries
its own ARM64 Android port (jojokengly, issue #2436), and this fork **takes that port as its
base** rather than carrying one of its own. What is left is a short stack of commits that adds
what the owner's device needs and upstream does not have, replayed onto the newest upstream,
never merged into it.

The comparison the decision came from is `nvf-side-by-side.md` (read-only, 2026-11-25): the
upstream port is larger than the fork's old one in every direction, and the fork was better
only where the device hurts, so the fork keeps those parts and drops the rest.

## What we take from upstream

Everything, including every part the fork used to have of its own:

- the engine as a library over `src/main.rs` with the binary in `src/bin/nvforever.rs`, the
  `NativeActivity` subclass, the vendored winit controller admission patch;
- the process restart on load (`RestartActivity`, `HandoffCoordinator`, the Java loading
  cover, the ticket and lease protocol), which is better than the fork's old file poll;
- the whole touch control surface, the gyro, the thermal and performance governor, the Mobile
  settings tab, the mods page, the in-app APK updater, the SAF importer;
- `--android-player-start`, reports off on Android, the Windows-only `ureq`, the staged
  licence notices and `tools/android/verify_apk.py`.

## What we add

| what | where | why |
| --- | --- | --- |
| the application id `org.nvforever.android.odin` | `android/gradle.properties` (`nvfApplicationId`) | the owner's install upgrades in place, and upstream's APK and this one never overwrite each other |
| the engine home `<internal files>/NVForever` | `engine/nvforever/src/odin/home.rs`, `OdinFork.HOME` | every install of the fork keeps its saves, `NVForever.ini`, `plugins.txt` and logs there |
| the game folder read in place | `crates/nvf-launcher/src/odin.rs`, `OdinStorage`, `OdinFolderPicker`, the manifest's `MANAGE_EXTERNAL_STORAGE` | upstream's importer needs free space equal to the game folder and hides the copy from every file manager |
| save backup and restore to `Download/NVForever-backups/<date>/` | `SaveBackup.java`, `OdinLauncher` | saves live where no file app can reach them and uninstalling deletes them; upstream's own doc says to export them and gives no way to |
| Export debug log, Copy debug summary, and the line about a run that ended unexpectedly | `DebugReport.java`, `OdinLauncher`, `odin::last_run_note` | without it a tester needs `adb` to send anything at all |
| a Load save list | `odin::saves`, `odin::chosen_save`, `OdinLauncher` | upstream has Continue and a preview card but no way to pick an older save |
| a gamepad that works the whole launcher | `OdinLauncher` | upstream's launcher has no `KeyEvent`, `onKeyDown` or focus handling: it is touch only |
| the device gyro with a controller in hand | `engine/nvf-play/src/odin/controller_gyro.rs` | upstream's gyro is a touch feature that a selected controller cancels, so on a handheld with sticks it does nothing (the owner, 2026-11-26) |
| the memory diagnostics | `engine/nvforever/src/odin/{proc_mem,alloc,mem_log}.rs` | the owner's 2026-11-24 run died on a 67,600-byte allocation with 10 GB free; nothing upstream has can see a mapping-count ceiling |
| the crash-test switches | `engine/nvforever/src/odin/crash_tests.rs`, `OdinCrashTests.java`, `OdinAdvanced.java` | the device loses its GPU in play, and narrowing that down means leaving one drawn feature out at a time, on the device (docs/odin-debug.md) |
| the GPU flight recorder, breadcrumbs and the device-fault query | `engine/nvforever/src/gpu_recorder/`, the vendored `bevy_render` | upstream's loss says one line and nothing about which node the GPU was in |
| the Khronos validation layer in the APK | `.github/workflows/fork-android.yml`, `engine/nvforever/src/odin/vulkan_validation.rs` | the only thing that can say whether the API use before the loss was valid |
| a Vulkan driver the player installed (Mesa Turnip through libadrenotools) | `engine/nvforever/src/odin/gpu_driver.rs`, the vendored `wgpu-hal`, `GpuDrivers.java`, `android/app/src/main/cpp/CMakeLists.txt` | the vendor driver is the one suspect behind the device's loss that no crash test can rule out (docs/odin-drivers.md) |
| a debug variant beside the release one | `android/app/build.gradle` (`applicationIdSuffix '.debug'`) | a test build with its own saves |
| the signed, verified, published APK | `.github/workflows/fork-android.yml` | upstream publishes an unsigned release and a debug-signed test APK, and builds the Rust at `opt-level = 1` |
| 16 KB ELF load alignment | `tools/android/verify_apk.py` | upstream's check accepts 4 KB, which does not load on a 16 KB-page device (Android 15) |
| the updater pointed at this repository | `crates/nvf-launcher/src/odin.rs` | upstream's fails closed on a different package name and signer, which is exactly what this fork is |
| the rebase workflows | `.github/workflows/upstream-{sync,promote}.yml` | the stack is replayed, so it needs replaying every day and promoting by hand |
| the live glance at the Pip-Boy | `engine/nvf-play/src/wrist_{arm,hud,live,local,page}.rs`, `engine/nvforever/src/wrist_screen.rs` (its `framing` module holds the arms view, the drop and the side) | the owner's live wrist map: the device raised on the arm with the game still running, which is what he plays with on the Odin |
| Quick Loot scrolled with the D-pad | `engine/nvf-play/src/quick_loot_dpad.rs` | on a controller the list could only be scrolled with the look stick |
| Hide HUD Key: Keep Quick Loot | the `tweak-hide-hud-quick-loot` row | "just the HUD gone but Quick Loot to stay" |
| the compact density on a handheld | `OdinCompact.java`, `OdinLauncher` (the card columns, the slim plugin rows) | upstream draws for a phone held upright: four rows fill the Odin's screen and everything else is under the fold ("the menu is too bulky and shows very little", 2026-11-27) |
| missing masters, and the one tap that clears them | `crates/nvf-launcher/src/odin/masters.rs`, `OdinLauncher` | a plugin whose master is not loaded takes the game down in the load, which is what the owner's mods did; the start is refused instead |
| the start log | `engine/nvforever/src/odin/start_log.rs`, `DebugReport.java` | a start the engine refuses returns before the run log is open, so Export debug log had nothing at all to export |

Phases 2 to 4, with the old stack's commits to port from, are in
`.hermes/plans/android-base-phases.md`.

`docs/odin-debug.md` is the fork's own page for the crash tests and the GPU diagnostics:
what to turn on when the device dies again, and what the run log then says.
`docs/odin-drivers.md` is the page for a Vulkan driver the player installed (Mesa Turnip
through libadrenotools): the launcher's rows, how a driver is opened, and the fallback that
takes the next start back to the system driver.

`docs/android.md` and `docs/design/android.md` are upstream's and are deliberately left
alone, so they describe upstream's defaults: the `org.nvforever.android` package, the
`files/home` engine home and the importer. Where they differ from what this fork builds, this
file is the one that is right.

## The fork-owned module rule

The cost of carrying this fork is the number of **upstream lines** it changes, because every
one of them is a line a rebase can conflict on. So:

- the fork's code lives in files of its own: `engine/nvforever/src/odin/`,
  `engine/nvf-play/src/odin/`, `engine/nvforever/src/gpu_recorder/` and
  `engine/nvforever/src/wrist_screen.rs` (both declared from `odin/mod.rs` with a `#[path]`,
  kept by those names because their log lines and docs use them),
  `engine/nvf-play/src/wrist_{arm,hud,live,local,page}.rs`,
  `engine/nvf-play/src/quick_loot_dpad.rs`, `crates/nvf-launcher/src/odin.rs`,
  `android/.../OdinFork.java`, `android/.../OdinStorage.java`, `android/.../OdinLauncher.java`,
  `android/.../OdinFolderPicker.java`, `android/.../SaveBackup.java`, `android/.../DebugReport.java`,
  `android/.../OdinCrashTests.java`, `android/.../OdinAdvanced.java`, `android/.../GpuDrivers.java`,
  `android/.../OdinCompact.java`, `crates/nvf-launcher/src/odin/masters.rs`,
  `engine/nvforever/src/odin/start_log.rs`,
  `android/app/src/main/cpp/CMakeLists.txt`, `android/tests/*Test.java` (the fork's),
  `tools/android/tests/test_fork.py`, `.github/workflows/fork-android.yml`;
- an upstream file gets a **hook**: one or two lines that call into those files;
- a hook that grows a body is moved back into a fork-owned file, leaving the call behind.

The hooks, in full, as of this stack (rebased onto upstream `6b7015970`, 2026-11-27). "lines"
counts production lines only: where the fork also added tests to an upstream file's own
`mod tests`, the row says so, because those lines are ours and are never what a rebase
conflicts on.

| upstream file | lines | the hook |
| --- | :-: | --- |
| `engine/nvforever/src/main.rs` | 3 | `pub mod odin;`, `odin::mem_log::MemoryLogPlugin` in the Android plugin list, and `odin::wrist_screen::WristScreenPlugin` in the play branch's |
| `engine/nvforever/src/android.rs` | 1 block, 1 line | `NVFOREVER_HOME` joins `odin::home::DIR`, `odin::start_log::capture` on the same home, `odin::crash_tests::apply` and `odin::gpu_driver::start` on the files folder; `odin::gpu_driver::restarting` in `restart` |
| `engine/nvforever/src/crash.rs` | 6 blocks | `SaysWhenEmpty` wraps `odin::alloc::BACKEND`; `odin::proc_mem::dump_once` on a failed allocation; `odin::gpu_recorder`'s `note_lost`, `log_dump_once` and `report_section` on the two lost-device paths |
| `engine/nvforever/src/dlss/mod.rs` | 1 | the renderer's instance flags go through `odin::vulkan_validation::instance_flags` |
| `engine/nvforever/src/reports/mod.rs` | 1 block | `odin::vulkan_validation::log_directives` joins the run log's filter |
| `vats_highlight.rs`, `particles.rs`, `effect_particles.rs`, `decals.rs`, `heat_haze.rs`, `pipboy_arm.rs` | 1 gate each | `odin::crash_tests` |
| `particles.rs`, `effect_particles.rs`, `decals.rs` | 1 call each | `odin::gpu_recorder::transparent_chunks::tag` |
| `cloud_shadows.rs`, `minimap_render.rs`, `reflection.rs`, `ssao.rs`, `sun_heightfield.rs`, `terrain_shadows.rs`, `vats_highlight.rs` | 1 `mark` each (2 in `ssao.rs` and `terrain_shadows.rs`) | `odin::gpu_recorder::mark` |
| `engine/nvf-assets/src/{lib,nif_material,particles}.rs` | 3 blocks | `nif_material::no_soft_particles`, the crash test `nvf_assets` cannot reach `odin` for; in `lib.rs` two names added to upstream's one-line `pub use nif_material::{…}`, which is why that line conflicts on every upstream addition to it |
| the vendored `bevy_render` | 6 blocks | the breadcrumbs, the upload and readback counters and the readback label (`engine/vendor/README.md`) |
| the vendored `wgpu-hal` | 2 blocks | `set_android_vk_get_instance_proc_addr` and the entry the Vulkan instance is built from (`engine/vendor/README.md`) |
| `engine/nvforever/Cargo.toml` | 2 blocks | `libc` and the optional `mimalloc` on Android, the `odin-mimalloc` feature |
| `engine/nvforever/src/touch/gyro.rs` | 4 blocks | `odin::controller_gyro::install`, its `poll` parameter, and three terms of `poll` |
| `engine/nvf-play/src/lib.rs` | 1 | `pub mod odin;` |
| `engine/nvf-play/src/player.rs` | 2 | `odin::controller_gyro::look_allows` in the look route's device gate |
| `crates/nvf-settings-layout/src/lib.rs` | 3 | `odin-controller-gyro-v1` in two sections and in `NEEDS`, and the glance's eight `wrist-*` rows |
| `engine/nvf-play/src/settings_layout.rs` | 8 rows | the glance's rows in `KNOWN`, and `hide-hud-quick-loot` in the tweak-order string |
| `engine/nvf-play/src/tweaks.rs` | 4 blocks | `hide_hud_keeps_quick_loot`: the field, its default, its `switch!` row and the `HudHidden` term it feeds |
| `engine/nvf-android/Cargo.toml` | 1 block | the `odin-mimalloc` feature passed through to `nvforever` |
| `crates/nvf-launcher/src/lib.rs` | 1 | `pub mod odin;` |
| `crates/nvf-launcher/src/mobile.rs` | 7 blocks | `odin::adopt(home)` at the top of `request`; three fork fields in `State`; `Request::Odin` and its arm; `Request::Start`'s `save` field and `odin::chosen_save`; `odin::masters::refuse_start` in the same arm; `REPO_DOWNLOADS` is `odin::DOWNLOADS`; `odin::apk_for_this_app` in the asset filter |
| `android/.../MobileLauncher.java` | 12 words, 18 lines | `OdinFork.home`; upstream's builders, palette and pages stop being `private`; the `odin` field; one call per page, the focus ring, the two render hooks and `OdinAdvanced.attach` on the build line; every size in `label`, `tap`, `space`, `add`, `addBar`, `card`, the header and the save preview goes through `OdinCompact`; `card` adds into `odin.cardParent`, a settings heading into `odin.heading`; the plugin loop and the start refusal are one call each |
| `android/.../LauncherActivity.java` | 4 blocks | `OdinFork.home` twice, `OdinStorage.promptIfNeeded`, the three key and motion overrides, the export and driver-zip results, the save a start loads |
| `crates/nvf-launcher/src/news.rs` | 1 | `RELEASES` is `odin::RELEASES` |
| `android/app/build.gradle` | 5 blocks | the application id property, the debug suffix, the `-odin.<n>` revision, and libadrenotools' CMake build when its sources are there |
| `android/app/src/main/AndroidManifest.xml` | 3 | the storage permissions |
| `tools/android/build.py` | 2 blocks | `--profile` and `--variant` |
| `tools/android/verify_apk.py` | 2 blocks | the 16 KB alignment |
| `engine/nvforever/src/player_body.rs` | 3 blocks, 1 use, + 53 test lines | `use crate::odin::wrist_screen::framing` and two calls on it (the arms view, the drop and the side; the numbers themselves are fork-owned since 2026-11-27); `place_lens` takes the view's own frame; `drive_body` reads `wrist_arm::WristLive` out of its last parameter tuple |
| `engine/nvforever/src/view_sway.rs` | 1 fn, + 26 test lines | `faded`, the pose held rigidly while the glance is up. Kept here, not moved: eight lines of code on upstream's own `SwayPose`, tested beside upstream's sway tests with their `busy` helper |
| `engine/nvforever/src/pipboy_controls.rs` | 1 block | the tab knob follows the glance's page |
| `engine/nvf-play/src/lib.rs` | 2 blocks | the `wrist_*` and `quick_loot_dpad` modules, and the three glance plugins |
| `engine/nvf-play/src/bindings.rs` | 4 blocks | `Control::GlancePipBoy` and its label, one row in `GAME_CONTROLS` (whose length is in its own type, so the count changes with it), `held` and `bound` for the glance, `pressed_with_pad` for the list's D-pad |
| `engine/nvf-play/src/flashlight.rs` | 2 blocks, + 40 test lines | `MenuKeyHold` hands a hold past the glance's moment to the glance |
| `engine/nvf-play/src/hud.rs` | 3 blocks, + 7 test lines | the `Meter` markers the glance's HUD row hides by, `HudHidden::quick_loot` and `hides_quick_loot` |
| `engine/nvf-play/src/quick_loot.rs` | 1 field, 6 lines, + 200 test lines | `owns_vertical_dpad`, the calls into `quick_loot_dpad`, and `hides_quick_loot` in `follow` and `draw` |
| `engine/nvf-play/src/weapon_wheel.rs` | 2, + 64 test lines | one term in upstream's `Extras` type alias and its destructure, and `quick_loot_dpad::binds` round the hotkeys' presses |
| `engine/nvf-play/src/pipboy/data{,/model,/outline}.rs` | 4 blocks | `outline::cast` shared with `wrist_local`, `MapPicture` for `WorldMap`, two visibility widenings |
| `engine/nvf-play/src/reticle.rs` | 1 | the crosshair goes while the glance is up |
| `engine/nvf-play/src/interface.rs` | 1 | `own_texture` is public, for the glance's map picture |

`git diff upstream/main -- engine crates android tools` should stay about that size. When one
of those grows, move its body into a fork-owned file and leave the hook.

## Conflict hotspots

Ranked by what actually conflicts, from the rebases run so far. The 2026-11-27 rebase of this
stack across 612 upstream commits (upstream `43a17d451` to `6b7015970`, which carried a mod
manager, named mod profiles, a Co-op page and report deletion) hit **five** conflicts, all of
them in the top class below, and none in `android/`:

1. **A one-line list or a counted array an upstream file keeps in one place.** This is the
   whole class that conflicted, and it will keep conflicting, because both sides add to the
   same line:
   - `engine/nvf-assets/src/lib.rs`, the single-line `pub use nif_material::{…}` re-export;
   - `engine/nvf-play/src/weapon_wheel.rs`, the `Extras<'w>` type alias and its destructure
     (upstream turned four loose parameters into that alias, so the fork's parameter became a
     term of it);
   - `engine/nvforever/src/player_body.rs`, `drive_body`'s last parameter tuple, which both
     sides append to because a Bevy system takes at most 16 parameters;
   - `engine/nvf-play/src/bindings.rs`, `GAME_CONTROLS`, whose length is written into its type:
     resolving the doc comment in upstream's favour and keeping upstream's count is the trap,
     and it fails to compile rather than silently, so take upstream's prose and add 1.
   Resolve each by taking upstream's line and adding the fork's term to the end of it.
2. **A doc comment that lists every row or feature.** `engine/nvf-play/src/settings_layout.rs`
   `KNOWN`'s comment and `bindings.rs`'s `GAME_CONTROLS` comment. Keep upstream's sentence and
   append the fork's clause; never drop upstream's additions to it.
3. `engine/nvforever/src/crash.rs`. The allocator block is upstream's code with the backing
   allocator swapped; if upstream rewrites `SaysWhenEmpty`, keep upstream's version and put
   `BACKEND` and the `dump_once` line back into it. It did not conflict this time.
4. `android/app/build.gradle` and `android/gradle.properties`. Upstream edits these whenever
   the port's SDK, assets or version scheme change, and the fork rewrites the version
   derivation. Resolve in upstream's favour and re-add the application id property, the debug
   suffix and the `-odin.<n>` revision. Quiet since 2026-11-25.
5. `android/.../MobileLauncher.java` and `LauncherActivity.java`. The fork's largest hook by
   word count, and so far the luckiest: upstream's 2026-11 launcher work was all in the
   **desktop** launcher (`crates/nvf-launcher/src/{app,fomod,modlist}.rs`, 8,200 lines) and
   touched neither Java file nor `mobile.rs`. When it does touch them, the hook to re-place is
   one call per page in `render()`, the two render hooks, the three key and motion overrides
   and the widened visibility of upstream's builders and palette; everything behind them is in
   `OdinLauncher`.
6. `crates/nvf-launcher/src/mobile.rs` and `news.rs`. One line each; expect to re-place rather
   than re-apply.
7. `tools/android/{build.py,verify_apk.py}`. Upstream's own tooling. The alignment constant and
   the two build options are worth offering upstream (see below), which would remove them from
   this list.
8. `engine/Cargo.lock`. Never merge it: take upstream's and build once, which adds `mimalloc`
   back.
9. `engine/nvforever/src/android.rs`. One line, rarely touched.

What does **not** conflict, and is worth keeping that way: every one of the fork's own files,
the vendored `bevy_render` patches (upstream vendors the same tree and edits other parts of
it), and the fork's tests inside upstream files' `mod tests`, which sit at the end of the file
where upstream adds tests of its own without meeting them.

## Identity, saves and the game folder

- **Application id** `org.nvforever.android.odin`, one Gradle property. The namespace and the
  Java package stay `org.nvforever.android`, so every class path and the
  `${applicationId}.updates` provider authority follow. The debug variant adds `.debug`.
- **Saves** stay in `<internal files>/NVForever`: `saves/`, `NVForever.ini`, `plugins.txt`,
  `logs/`, `cache/`. Nothing is moved or converted. `tools/android/tests/test_fork.py` fails if
  the Java and Rust sides of that name ever disagree.
- **The game folder** is read where it is. `<internal files>/data-path.txt` names it (one line,
  absolute); with no such file, `/storage/emulated/0/NVForever/Data` is tried, and failing both,
  upstream's importer is what the launcher offers. Whatever is found becomes `[Game] Path` in
  `NVForever.ini`, which is how upstream's launcher and the engine already find the game, so
  the engine is handed no `--data` and an Android start stays a player's start (the home's
  `plugins.txt` is the load order).
- **Nothing is ever written into the game folder.** The game files are opened read-only and the
  home is in private app storage, so `fnv_install::check_separation` can never be broken by the
  player's choice.

## Debug and release

- **release**: Gradle's `release` build type, application id `org.nvforever.android.odin`,
  signed in CI with the fork's keystore.
- **debug**: Gradle's `debug` build type, application id `org.nvforever.android.odin.debug`,
  `debuggable`, signed with the **same** key, installed beside the release app with app
  storage, and therefore saves, of its own.
- **both package the same native library**, built with the same Cargo profile. Upstream's only
  Android build path is `--profile android-dev`, which is `opt-level = 1`; Bevy at that
  optimisation runs at a few frames a second, so the fork never ships it. A tag builds
  `--release`; a branch or a pull request builds `release-quick`, which is `release` with
  incremental compilation (`engine/Cargo.toml`).

## The release scheme

The version name carries the fork's revision: `<semver>-odin.<n>`, for example
`0.8.1-odin.32`. That `<n>` is what raises the Android `versionCode` through upstream's
derivation, which the fork extended to read `-odin.<n>` exactly as it reads its own
`-android-r<n>`: `0.8.1-odin.32` is versionCode 801032. The owner's installed build is
versionCode 36, and the APK build refuses to publish anything that does not beat it.

Tags:

| tag | what it publishes |
| --- | --- |
| `android-v0.8.1-odin.32` | a release |
| `android-v0.8.1-odin.32-test.1` | a prerelease (so does `-rc.1`) |

Assets, named for any ARM64 Android device:

```
NVForever-<version>-arm64.apk           the app to play
NVForever-<version>-arm64-debug.apk     the same build as a separate app
SHA256SUMS.txt                          both APKs (the in-app updater needs this file)
signer.txt                              the signing certificate, both APKs
package.txt                             what both APKs declare
LICENSE.txt LEGAL.txt THIRD-PARTY-LICENSES.txt NDK-NOTICE.txt
NDK-NOTICE.toolchain.txt SOURCE.txt     the notices upstream stages into the APK
LICENSE.libadrenotools.txt
LICENSE.liblinkernsbypass.txt           BSD-2-Clause, beside the loader they cover
```

Every build, tagged or not, is checked before it is kept: `zipalign -c -P 16 4`, the signing
certificate against a pinned SHA-256, 16 KB ELF LOAD alignment for **every** packaged library,
`arm64-v8a` only, the declared package name, the versionCode floor, and upstream's own
`verify_apk.py` (no game data, no LAN uploader, archive CRCs, cleartext traffic off).

## The workflows

- **Fork Android APK** (`fork-android.yml`): every push to `odin/**`, every pull request, every
  `android-v*` tag. Builds both variants, signs, verifies, uploads artifacts; a tag also
  publishes the release. Upstream's `android.yml` is left untouched and keeps running on its own.
- **Upstream sync** (`upstream-sync.yml`), daily at 09:17 UTC and on demand: replays the stack
  from `SYNC_BASE` (default `odin/android-base`) onto `upstream/main` on `odin/upstream-sync`,
  force-pushes it, asks for an APK build of it, and opens or refreshes a draft pull request. It
  never moves the base branch. A rebase it cannot finish is aborted, nothing is pushed, and the
  whole report is in the run summary.
- **Upstream promote** (`upstream-promote.yml`), manual, with a typed confirmation, after the
  APK has been installed on the device: saves the current base as
  `backup/<base>-<date>-<sha>`, then moves the base to the sync branch's commit. Do not use
  GitHub's merge button on the sync pull request: that branch is a rewrite of the base, not
  commits on top of it.

## Resolving a sync conflict by hand

```sh
git remote add upstream https://github.com/LootJunkieAzza/NVForever.git   # once
git fetch --no-tags upstream main
git switch -C odin/upstream-sync odin/android-base
git rebase upstream/main
# for each conflicted file: keep upstream's version, put our hook back on top of it
git add <files> && git rebase --continue
git push --force-with-lease origin odin/upstream-sync
```

Rules of thumb:

- Resolve in upstream's favour, then re-apply the fork's intent. If upstream rewrote the system
  a hook sat in, put the hook into upstream's new version; never keep the fork's old copy of
  the system.
- A conflict in a file the hook table above lists means the hook has to be re-placed. If the
  conflict is large, the hook has grown a body: move the body into a fork-owned file.
- `engine/Cargo.lock` is upstream's with `mimalloc` added. Take upstream's and build once.
- If a commit of ours has become upstream's own work, drop it from the stack and say so in the
  sync pull request.
- Checks before promoting: `cargo check -p nvf-android --target aarch64-linux-android`,
  `cargo test -p nvforever --lib -- odin`,
  `cargo test -p nvf-launcher --no-default-features --features mobile --lib`,
  `python -m unittest discover -s tools/android/tests`, the Java controls the APK workflow
  runs, and a desktop `cargo build -p nvforever --bin nvforever` with a real run of it.

## What could be offered upstream

Nothing goes out without the owner's word, and upstream asks for an issue first, then a draft
pull request with the whole clean-room checklist ticked (its `CLAUDE.md` and
`.github/pull_request_template.md`). Ranked by value against argument:

1. **16 KB ELF load alignment in `verify_apk.py`.** A threshold they already compute.
2. **The memory diagnostics and the OOM dump.** A real Android failure their budget code
   cannot see.
3. **Reading the game folder in place, as an option beside the importer.** Their own doc admits
   the double-space problem; the all files permission is a Play Store blocker for them and not
   for a sideloaded fork, so it has to be framed as an option.
4. **A release build path that is not `opt-level = 1`.**
5. **The GPU flight recorder, the breadcrumbs and the readback label** (phase 3, on this
   branch now). The readback label and the upload counters are small and platform-neutral;
   the breadcrumbs are a larger argument, and the crash-test switches are the fork's own
   problem, not upstream's.
6. **The Turnip driver choice** (phase 3.2, on this branch now). The wgpu-hal entry-point
   hook is small and is worth taking to wgpu itself rather than to upstream; the rest is an
   Adreno argument the owner's device is the only evidence for.
7. Later, from phase 4: Quick Loot D-pad scrolling, gamepad navigation in the launcher,
   launcher string resources.
