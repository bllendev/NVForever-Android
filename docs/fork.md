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
| a debug variant beside the release one | `android/app/build.gradle` (`applicationIdSuffix '.debug'`) | a test build with its own saves |
| the signed, verified, published APK | `.github/workflows/fork-android.yml` | upstream publishes an unsigned release and a debug-signed test APK, and builds the Rust at `opt-level = 1` |
| 16 KB ELF load alignment | `tools/android/verify_apk.py` | upstream's check accepts 4 KB, which does not load on a 16 KB-page device (Android 15) |
| the updater pointed at this repository | `crates/nvf-launcher/src/odin.rs` | upstream's fails closed on a different package name and signer, which is exactly what this fork is |
| the rebase workflows | `.github/workflows/upstream-{sync,promote}.yml` | the stack is replayed, so it needs replaying every day and promoting by hand |
| the live glance at the Pip-Boy | `engine/nvf-play/src/wrist_{arm,hud,live,local,page}.rs`, `engine/nvforever/src/wrist_screen.rs` | the owner's live wrist map: the device raised on the arm with the game still running, which is what he plays with on the Odin |
| Quick Loot scrolled with the D-pad | `engine/nvf-play/src/quick_loot_dpad.rs` | on a controller the list could only be scrolled with the look stick |
| Hide HUD Key: Keep Quick Loot | the `tweak-hide-hud-quick-loot` row | "just the HUD gone but Quick Loot to stay" |

Phases 2 to 4, with the old stack's commits to port from, are in
`.hermes/plans/android-base-phases.md`.

`docs/android.md` and `docs/design/android.md` are upstream's and are deliberately left
alone, so they describe upstream's defaults: the `org.nvforever.android` package, the
`files/home` engine home and the importer. Where they differ from what this fork builds, this
file is the one that is right.

## The fork-owned module rule

The cost of carrying this fork is the number of **upstream lines** it changes, because every
one of them is a line a rebase can conflict on. So:

- the fork's code lives in files of its own: `engine/nvforever/src/odin/`,
  `engine/nvf-play/src/odin/`, `crates/nvf-launcher/src/odin.rs`, `android/.../OdinFork.java`,
  `android/.../OdinStorage.java`, `android/.../OdinLauncher.java`,
  `android/.../OdinFolderPicker.java`, `android/.../SaveBackup.java`,
  `android/.../DebugReport.java`, `android/tests/{OdinFork,SaveBackup,DebugReport}Test.java`,
  `tools/android/tests/test_fork.py`, `.github/workflows/fork-android.yml`;
- an upstream file gets a **hook**: one or two lines that call into those files;
- a hook that grows a body is moved back into a fork-owned file, leaving the call behind.

The hooks, in full, as of this stack:

| upstream file | lines | the hook |
| --- | :-: | --- |
| `engine/nvforever/src/main.rs` | 3 | `pub mod odin;`, `odin::mem_log::MemoryLogPlugin` in the Android plugin list, and `odin::wrist_screen::WristScreenPlugin` in the play branch's |
| `engine/nvforever/src/android.rs` | 1 | `NVFOREVER_HOME` joins `odin::home::DIR` |
| `engine/nvforever/src/crash.rs` | 2 blocks | `SaysWhenEmpty` wraps `odin::alloc::BACKEND`; `odin::proc_mem::dump_once` on a failed allocation |
| `engine/nvforever/Cargo.toml` | 2 blocks | `libc` and the optional `mimalloc` on Android, the `odin-mimalloc` feature |
| `engine/nvforever/src/touch/gyro.rs` | 4 blocks | `odin::controller_gyro::install`, its `poll` parameter, and three terms of `poll` |
| `engine/nvf-play/src/lib.rs` | 1 | `pub mod odin;` |
| `engine/nvf-play/src/player.rs` | 2 | `odin::controller_gyro::look_allows` in the look route's device gate |
| `crates/nvf-settings-layout/src/lib.rs` | 3 | `odin-controller-gyro-v1` in two sections and in `NEEDS` |
| `crates/nvf-launcher/src/lib.rs` | 1 | `pub mod odin;` |
| `crates/nvf-launcher/src/mobile.rs` | 4 blocks | `odin::adopt(home)` at the top of `request`; three fork fields in `State`; `Request::Odin` and its arm; `odin::chosen_save` in `Start` |
| `android/.../MobileLauncher.java` | 11 words, 9 lines | `OdinFork.home`; upstream's builders, palette and pages stop being `private`; the `odin` field; one call per page, the focus ring and the two render hooks |
| `android/.../LauncherActivity.java` | 4 blocks | `OdinFork.home` twice, `OdinStorage.promptIfNeeded`, the three key and motion overrides, the export result, the save a start loads |
| `crates/nvf-launcher/src/news.rs` | 1 | `RELEASES` is `odin::RELEASES` |
| `android/app/build.gradle` | 4 blocks | the application id property, the debug suffix, the `-odin.<n>` revision |
| `android/app/src/main/AndroidManifest.xml` | 3 | the storage permissions |
| `tools/android/build.py` | 2 blocks | `--profile` and `--variant` |
| `tools/android/verify_apk.py` | 2 blocks | the 16 KB alignment |
| `engine/nvforever/src/player_body.rs` | 4 blocks | the glance's arms view, drop and side; `place_lens` takes the view's own frame; `drive_body` reads `wrist_arm::WristLive` |
| `engine/nvforever/src/view_sway.rs` | 1 block | `faded`, the pose held rigidly while the glance is up |
| `engine/nvforever/src/pipboy_controls.rs` | 1 block | the tab knob follows the glance's page |
| `engine/nvf-play/src/lib.rs` | 2 blocks | the `wrist_*` and `quick_loot_dpad` modules, and the three glance plugins |
| `engine/nvf-play/src/bindings.rs` | 4 blocks | `Control::GlancePipBoy`, `held` and `bound` for the glance, `pressed_with_pad` for the list's D-pad |
| `engine/nvf-play/src/flashlight.rs` | 2 blocks | `MenuKeyHold` hands a hold past the glance's moment to the glance |
| `engine/nvf-play/src/hud.rs` | 3 blocks | the `Meter` markers the glance's HUD row hides by, `HudHidden::quick_loot` and `hides_quick_loot` |
| `engine/nvf-play/src/quick_loot.rs` | 1 field, 6 lines | `owns_vertical_dpad`, the calls into `quick_loot_dpad`, and `hides_quick_loot` in `follow` and `draw` |
| `engine/nvf-play/src/weapon_wheel.rs` | 2 | one resource and `quick_loot_dpad::binds` round the hotkeys' presses |
| `engine/nvf-play/src/pipboy/data{,/model,/outline}.rs` | 4 blocks | `outline::cast` shared with `wrist_local`, `MapPicture` for `WorldMap`, two visibility widenings |
| `engine/nvf-play/src/reticle.rs` | 1 | the crosshair goes while the glance is up |
| `engine/nvf-play/src/interface.rs` | 1 | `own_texture` is public, for the glance's map picture |

`git diff upstream/main -- engine crates android tools` should stay about that size. When one
of those grows, move its body into a fork-owned file and leave the hook.

## Conflict hotspots

By how much of an upstream file the fork touches, likeliest first:

1. `android/app/build.gradle` and `android/gradle.properties`. Upstream edits these whenever
   the port's SDK, assets or version scheme change. Small, but they conflict often. Resolve in
   upstream's favour and re-add the application id property, the debug suffix and the
   `-odin.<n>` revision.
2. `engine/nvforever/src/crash.rs`. The allocator block is upstream's code with the backing
   allocator swapped; if upstream rewrites `SaysWhenEmpty`, keep upstream's version and put
   `BACKEND` and the `dump_once` line back into it.
3. `engine/nvforever/src/main.rs`. Two lines, and the Android plugin tuple is a line upstream
   edits whenever it adds a plugin. Keep upstream's tuple and add ours back at the end.
4. `crates/nvf-launcher/src/mobile.rs` and `news.rs`. One line each; `mobile.rs` is churned a
   lot, so expect to re-place rather than re-apply.
5. `tools/android/{build.py,verify_apk.py}`. Upstream's own tooling. The alignment constant and
   the two build options are worth offering upstream (see below), which would remove them from
   this list.
6. `engine/Cargo.lock`. Never merge it: take upstream's and build once, which adds `mimalloc`
   back.
7. `engine/nvforever/src/android.rs`. One line, rarely touched.

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
5. Later, from phases 3 and 4: the GPU flight recorder and breadcrumbs, the Turnip driver
   choice, Quick Loot D-pad scrolling, gamepad navigation in the launcher, launcher string
   resources.
