# Re-layering the fork on upstream's Android base: phases 2 to 4

Written 2026-11-25, at the end of phase 1. The decision and the sizing come from
`nvf-side-by-side.md` (read-only comparison of `odin/style-review` @ `ecb0c3ca7` against
upstream's port). Phase 1 is on this branch: see `docs/fork.md`.

The old stack is branch **`odin/style-review`** (`ecb0c3ca7` = upstream `ae5028f5a` plus 33
commits). Every hash below is a commit on that branch; read it with
`git show <hash>` or `git show <hash>:<path>`.

Sizes are the side-by-side document's: S under about 8 worker-hours, M about 8 to 25, L about
25 to 45.

---

## Phase 2: the device is usable (about 45 to 70 hours)

The point of phase 2 is that the owner stops needing a PC to get anything off the device or
to point it at a different game folder.

### 2.1 Read in place, as a thing the launcher does (M)

Phase 1 adopts a `data-path.txt` that is already there. What is missing is choosing the folder
on a device that has none, and saying so in the launcher.

- Port from: `75f85cc20` (the old launcher's Data screen and `DataCheck.java`), `d0dc75f38`
  (the engine side, `paths.rs` `data_dir`, `chosen_data_dir`).
- Build on: `crates/nvf-launcher/src/odin.rs` (`configured_data_dir`, `adopt`) and
  `OdinStorage` from phase 1.
- Shape: a row on upstream's Game folder page, beside their importer, that asks for all files
  access, lets the player type or pick a folder, validates it with `GameInstall::open` plus
  `missing_files`, writes `data-path.txt` and re-runs `odin::adopt`. Keep their importer as the
  other choice on the same page.
- Care: upstream's `removeImport` and their shared preference `imports/active` must not try to
  delete a folder that was never an import (`InstallConfig.ownedImport` already refuses, so
  check that path rather than trusting it).

### 2.2 Save backup and restore (S/M)

- Port from: `c5399c424` (`SaveBackup.java`, `SaveBackupTest.java`, both self-contained).
- Shape: copy the home's `saves/` to `Download/NVForever-backups/<profile>-<date>/` and back.
  Two invariants the test already covers: nothing is ever deleted, and a save a restore would
  overwrite is copied to `<name>.bak` (`.bak-2` upward) first, after telling the player how
  many names clash.
- The rows have to be built into upstream's Java launcher (`MobileLauncher`), which is where
  the work is. Upstream's own doc tells the player to export before uninstalling and gives
  them no tool to do it with, so this is also the best phase 2 candidate to offer upstream.

### 2.3 Debug log export and Report a problem (S/M)

- Port from: `c772db0a5` (`DebugReport.java`, `DebugReportTest.java`,
  `nvf-android/src/logcat.rs`).
- Shape: Export debug log, Copy debug summary and a Report a problem row, reading the home's
  `logs/`, bounded to 224 KiB per file, with build and device information, the latest run log
  and a matching crash report; no saves, no config, no device-wide logs, and it uploads nothing.
- This is what makes the memory diagnostics of phase 1 reachable without `adb`: it is the
  difference between a tester who can send something and one who cannot.

### 2.4 A Load save list (S)

- Port from: `75f85cc20` (the old launcher's Load save screen).
- Upstream has Continue and a save preview card but no list. It writes the same one start
  request their Continue does (`mobile::Request::Start { resume }` with a chosen path, which
  needs a small addition to their request shape).

### 2.5 Gamepad navigation in upstream's launcher (M)

- Port from: `0d70719ef` (design v2: the focus model, the amber focus bar, the controller
  legend), `75f85cc20` (the key handling).
- Upstream's `MobileLauncher.java` has no `KeyEvent`, `onKeyDown` or `setFocusable` at all: it
  is touch only. On a handheld with buttons that is a daily annoyance.
- Keep their look. Add: focusability and a focus order per page, A select, B back, LB and RB
  to change page, Start plays, D-pad navigation, and the legend row.
- The single most offerable item in phase 2 after 2.2.

---

## Phase 3: knowing why the GPU dies (about 55 to 80 hours)

Do not start phase 3 until phase 1 has been played on the device: it only matters if the
device still loses its graphics device on upstream's base.

### 3.1 The GPU diagnostics suite (L)

- Port from: `e0dfc212a` (readback attribution, the bounded flight recorder, GPU breadcrumbs
  round every render-graph node), `28e665fbb` (breadcrumbs inside the transparent pass, cut
  into up to 32 single-pass chunks with item range, draw count, pipeline id and label; five
  crash-test switches), `bb2d105d2` (the Khronos validation layer, packaged from a pinned
  release with two hashes checked in CI, plus sync and GPU-assisted validation as separate
  switches), `dc7f67070` and `65c578de1` (the fork-owned shape all of it ended up in).
- It patches vendored `bevy_render` (`renderer/render_context.rs`, `renderer/mod.rs`,
  `gpu_readback.rs`), which upstream also vendors, so the patches re-apply rather than
  conflict.
- The crash-test switches need a home: upstream's Mobile settings tab is the natural one, or
  the fork's own file as before (`nvforever-controls.txt`).
- `odin/gpu_recorder/` is also where `proc_mem`'s figures used to be printed from; phase 1
  prints them from `odin/mem_log.rs` instead, so when the recorder arrives, fold the one line
  into its summary and delete `mem_log`.
- Cost measured on desktop, 2026-11-21, one pair of runs: 9.2 ms a frame with breadcrumbs
  against 8.5 ms without. An order of magnitude, not a measurement.

### 3.2 Turnip through libadrenotools (L)

- Port from: `fc5c580bc` (the engine side: the private linker namespace, handing wgpu-hal the
  driver's own `vkGetInstanceProcAddr`, the `renderer_starting` marker that falls back to the
  system driver after a start that dies), `3a690cc29` (Gradle and CI build `libadrenotools.so`
  and its four hook libraries from pinned commits, hash-checked, neither source tree
  committed), `740c43333` (`GpuDrivers.java`, the launcher row), `4cf717ac5` (the Automatic
  row), `dc2bc6da0` (the docs, the licences and two smoke scenarios).
- The Gradle side was Kotlin DSL and upstream's is Groovy, so the CMake and NDK plumbing is a
  rewrite, not a copy.
- This is the fork's whole reason for existing if the device's `DeviceLost` turns out to be
  the vendor driver's. Validation and a custom driver stay mutually exclusive on purpose.

---

## Phase 4: the rest of the old fork (about 50 to 80 hours)

### 4.1 Data profiles (M/L): **partly redundant** after upstream's named mod profiles

Checked against upstream `6b7015970` on the 2026-11-27 rebase, which brought a mod manager and
named mod profiles (`crates/nvf-launcher/src/{app,modlist}.rs`, about 8,200 new lines).

What upstream's profiles are: a folder `<home>/profiles/<name>/` holding `modlist.txt`,
`plugins.txt`, `loadorder.txt`, a locks side file, archive names and a note; Add profile,
Load, Save, Delete and a right-click menu; the last one used is remembered and restored on the
next run (`restore_remembered_profile`), a profile outside the home can be pointed at from
settings (`profiles_dir_from_settings`), and two of them can be compared
(`modlist::profile_comparison`). Loading one "only puts the lists on the page. It does not
link or copy mod files" (`load_chosen_profile`).

So, of the three things the fork's data profiles were for:

- **the mod list and the load order per profile: redundant.** Upstream's is better than the
  fork's planned version (history, locks, comparison, a remembered pick) and there is no case
  for a second mechanism over the same `plugins.txt`.
- **a Data folder per profile: still needed, and untouched.** Upstream's profile holds no game
  folder at all. One `[Game] Path` is still the only one, so a player with vanilla in one
  folder and a TTW install in another still cannot keep both ready. That is the half the owner
  asked for and the half this fork's `data-path.txt` already half-answers.
- **saves, `NVForever.ini`, `logs/` and `cache/` per profile: still needed.** Upstream's
  profiles all live *inside* one home and share every one of those. Saves in particular are
  what makes a modded playthrough and a vanilla one separable.

And one thing that is new: **none of it is on Android.** Upstream's profiles are drawn by the
desktop egui launcher (`app.rs`); `crates/nvf-launcher/src/mobile.rs` has not one mention of a
profile, and neither has upstream's Java launcher. On the device the feature does not exist
yet.

**Verdict: partly redundant. Keep 4.1, cut it down, and re-scope it.** Drop the mod-list and
load-order half entirely. What is left is a smaller job than the M/L this was sized at: a
profile is a **name, a Data folder and a home folder**, and the plumbing is
`odin::adopt` taking a chosen home instead of the one constant, plus rows in `OdinLauncher`.
Build it so a profile's home is where upstream's `profiles/` folder then lives, so upstream's
mod profiles nest inside the fork's data profiles rather than competing with them; if upstream
ever brings its profile rows to `mobile.rs`, that nesting is what keeps both working.
Re-sizing: S/M, from M/L. Still not offerable upstream (it is a design argument, not a patch),
and now additionally overlapping a feature they have chosen a different shape for.

- Port from: `d0dc75f38` (the engine side: `NVFOREVER_HOME` per profile, `share_settings`,
  `profile.txt`, `profiles.txt`), `75f85cc20` (the launcher side). Read both for the home
  switching only; their mod-list screens are superseded.
- A profile is a name, a Data folder and a home folder; everything else falls out of the home.
  The first profile is always Vanilla and keeps `files/NVForever`, so nothing moves. Saves are
  per profile on purpose. `NVForever.ini` is shared by copying the newest one before a start,
  because every writer replaces the file whole.
- Upstream's `android.rs` hardcodes the home's name (phase 1 made that one constant), so the
  engine side is small; the launcher side has to be built twice over, in their Java UI and in
  `mobile.rs`.
- Keep this out of anything offered upstream: it is a design argument, not a patch.

### 4.2 The wrist live map, Glance at Pip-Boy (M): done 2026-11-27

Ported onto upstream's base as three commits on `odin/p4-wrist`. What was re-placed
rather than re-applied, and what is left:

- the hooks are listed in docs/fork.md's table, which is now the record of where each one
  sits; `wrist_screen` is declared from `odin/mod.rs` with a `#[path]` and its plugin goes
  in from one line in `main.rs`'s play branch (upstream has no `before_run`).
- **Left as follow-up: a touch trigger of its own.** Upstream's touch controls are a
  `BUTTONS` table in `engine/nvforever/src/touch.rs` indexed by position, with named
  indices (`PAUSE_BUTTON`, `TAB_BUTTON`, `gyro::BUTTON`) and a couple of dozen tests that
  index it by number, and no per-button switch to hang an off-by-default row on. Adding a
  Glance button means appending an entry, a row in `touch_ui_options`, and re-checking
  every index: more than the "if small" this was given. Meanwhile the glance **is** already
  reachable by touch: upstream's Pip-Boy button pushes `Control::MenuMode` while it is
  held, and the glance follows a hold of Menu Mode wherever it has no binding of its own
  (`flashlight::MenuKeyHold::glancing`), so a long press of that button raises the arm and
  a tap opens the Pip-Boy, exactly as on the controller.

- Ported from: `bc1b4ed92`, with `3a1b86ca4`'s `take_choice` refactor folded in.
- Hooks in `player_body.rs`, `hud.rs`, `bindings.rs`, `pipboy/data*.rs`, `flashlight.rs`,
  `weapon_wheel.rs`, with the bodies in fork-owned files (`wrist_lens.rs`, `sway_fade.rs`,
  `wrist_meters.rs`, `wrist_menu_key.rs`). Every one of those upstream files is churned by
  their touch work, so expect to re-place the hooks rather than re-apply the diff.
- `3a1b86ca4` is the later refactor of its five settings rows down to one `take_choice`.

### 4.3 Quick Loot D-pad, and Hide HUD that keeps Quick Loot (S): done 2026-11-27

The body is in `engine/nvf-play/src/quick_loot_dpad.rs` as planned; `quick_loot.rs` keeps
one field and six lines, `weapon_wheel.rs` two. Still worth offering upstream.

- Ported from: `2af65959d` and `7826e6cdd`.
- One field, three lines, the body in a `#[path]` child (`quick_loot_dpad.rs`). Their touch
  work moved `quick_loot.rs` and `hud.rs`, so the hooks need re-placing.
- Small, platform-neutral and upstream-shaped: they already have five other `hide-hud-*` tweak
  rows. Worth offering.

### 4.4 Launcher string resources and translations (M)

- Port from: `75f85cc20` (`res/values/strings.xml`), `958bc947c` and the `values-es` and
  `values-pt-rBR` files, `android/tools/check_strings.py`, `android/README-translations.md`.
- Upstream's launcher is hardcoded English Java, so this means adding the resource layer
  first, which is most of the work. `check_strings.py` is what caught the aapt2 apostrophe
  (`7f4cf4b7a`) and would enforce the no-dash house rule on their text too.

---

## Dropped on purpose (upstream's is better or the same)

Recorded so no later session ports them by mistake:

- the Android handoff file poll (`8c8617112`): upstream's `RestartActivity` and
  `HandoffCoordinator` are strictly better.
- the winit Android controller admission patch: upstream's is equivalent and documented in
  `engine/vendor/README.md`.
- `--player-start` and the mods load-order fix (`d0dc75f38`, part of it): upstream has
  `--android-player-start`.
- the Windows-only `ureq` in `nvf-dlss-files`, and the reports-off-on-Android stubs: upstream
  made both changes.
- the old `touch.rs`, `nvf-android-input` and `controller.rs` as the input path: superseded
  many times over. Keep only the recorded ABXY order for the owner's older device and the
  D-pad Up ammunition change, if they are still wanted.
- the schema embedding in `nvf-android/build.rs`: upstream stages schemas from APK assets.
- `Ui.java`, the seven-screen `LauncherActivity` and the bundled Inter and JetBrains Mono
  (`c19b61f81`, `0d70719ef`, `1d51c072f`, `b0681a3b7`): genuinely nice and genuinely
  redundant. Keep their launcher, add rows to it.

## Risks carried into phase 2

- Upstream's own port says it is **not device-qualified** and its recent cycles record failing
  engine tests. Phase 1 is the measurement of whether their base runs better than the old fork
  on the device; phases 3 and 4 should not be started before that answer.
- No upstream evidence of any Adreno 8-series handheld being tested.
- `memory_budget.rs:343` returns `None` for an integrated GPU, so on an Adreno their VRAM
  give-back and allocator-block sizing probably sit idle and only their RAM check applies.
  That is a hypothesis from reading the code, not something anyone ran on the device.
