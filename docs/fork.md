# The Android fork of NVForever

This repository (`bllendev/NVForever-Android`) is a fork of
[LootJunkieAzza/NVForever](https://github.com/LootJunkieAzza/NVForever) that adds an ARM64
Android build (the AYN Odin handhelds) and a few engine parts of its own. Upstream moves fast,
so the fork is kept as a **short stack of topic commits replayed onto the newest upstream**,
never as a long-lived merge.

## What the stack is

`main` is upstream's `main` plus our commits, in this order:

1. `android: the engine as a library, an Android adapter crate and its Gradle app`
2. `ci(android): the Odin APK workflow, signed, verified and publishable`
3. `android: export bounded game diagnostics from the launcher`
4. `diag(gpu): readback attribution, a bounded flight recorder and GPU breadcrumbs`
5. `android: the launcher's crash-test switches, and packaging the validation layer`
6. `feat(quick-loot): D-pad Up and Down scroll the Quick Loot list on a controller`
7. `feat(wrist): a live glance at the Pip-Boy: its map, its pages and the HUD on it`
8. `feat(hud): a Hide HUD part that keeps Quick Loot, on by default`
9. `ci: keep the fork rebased on upstream NVForever`
10. `feat(android): data profiles in the engine, and mods can load at last`
11. `feat(android): the launcher rewritten, with data profiles and a hidden Debug screen`
12. `diag(odin): breadcrumbs inside the transparent pass, and five crash-test switches`
13. `feat(android): the renderer can start on a Vulkan driver the player supplied`
14. `build(android): Gradle and CI build libadrenotools and its hook libraries`
15. `feat(android): GpuDrivers, the real body behind the launcher's Graphics driver row`
16. `docs(android): the custom GPU driver, its licences and two smoke scenarios`
17. `refactor(fork): fork logic out of upstream files, into fork-owned modules`

`git log --oneline upstream/main..main` is the live list. Keep it a stack: new work is a
commit on top, not a merge of upstream into it.

## Why `engine/nvforever/src/main.rs` is untouched

That file is the engine's crate root: 3,000 lines, every `mod` declaration, every plugin the
game adds, and the file upstream edits most. An earlier shape of commit 1 moved it whole to
`src/lib.rs` so the Android shell could start the game as a library. Git saw a renamed file
with 236 commits of upstream edits to merge into it, and **every** sync conflicted on the
whole file.

So the library and the binary are the same file instead:

```toml
# engine/nvforever/Cargo.toml
autobins = false

[lib]
name = "nvforever"
path = "src/main.rs"

[[bin]]
name = "nvforever"
path = "src/desktop.rs"
```

- `src/main.rs` stays upstream's file. `fn main()` is still the whole program; as a library
  root it is simply a private function the crate's own code can call.
- `src/desktop.rs` is the binary: three lines that call `nvforever::odin::desktop()`.
- `src/odin/` is everything the fork adds, declared from there rather than from `main.rs`
  (the fork's own engine parts keep their files beside `main.rs`, reached with `#[path]`).

`main.rs` therefore differs from upstream by one `pub mod odin;`, two changed lines and two
added calls:

| hook | where | why |
| --- | --- | --- |
| `pub mod odin;` | above `fn main` | the fork's module |
| `odin::args()` instead of `std::env::args().collect()` | first lines of `main` | an Android activity has no argv, so the shell hands the game a command line |
| `odin::init(&mut app);` | after `anti_lag::AntiLagInitPlugin` | what the Vulkan instance and device must be asked for *before* `DefaultPlugins`, which is where the renderer reads those settings |
| `odin::before_run(&mut app);` | just before `app.run()` | the fork's own plugins, then the platform shell's |
| `odin::developer_data(args.data.is_some())` instead of `args.data.is_some()` | `let developer_data` in `main` | every Android start names `--data` (the active data profile's folder) and is still a player's start, so the profile's own plugins.txt is the load order |

Keep it that way. `git diff upstream/main -- engine/nvforever/src/main.rs` should stay around
ten lines; those four regions have been touched twice each in upstream's whole history, so a
sync almost never has to merge anything there.

New fork code goes in `src/odin/` (or in a file declared from it), never into `main.rs`'s
`mod` list or plugin chains. A fork change to an *existing* upstream file is fine: those
conflicts are small and local.

## Conflict hotspots

The upstream files the stack still edits, by how many lines of them it changes
(`git diff --stat upstream/main..HEAD -- engine crates`, leaving out `src/odin/`, which is
ours). The longer the entry, the likelier a sync has to merge it by hand: when one grows,
move its body into a fork-owned file and leave a hook, as the sync of 2026-11-23 did (the
fork files named below).

| lines | file | what the fork does there |
| ---: | --- | --- |
| 88 | `engine/vendor/bevy_render/src/renderer/render_context.rs` | the GPU flight recorder's breadcrumbs round every graph node |
| 79 | `engine/vendor/wgpu-hal/src/vulkan/instance.rs` | a Vulkan driver the player supplied (libadrenotools), the validation layer |
| 67 | `engine/nvf-play/src/quick_loot.rs` | the list's D-pad scrolling and Keep Quick Loot (body in `quick_loot_dpad.rs`) |
| 66 | `engine/nvforever/src/player_body.rs` | the glance's hold of the arm (body in `wrist_lens.rs`, `sway_fade.rs`) |
| 65 | `engine/vendor/README.md` | why winit, wgpu-hal and bevy_render are patched here |
| 52 | `engine/nvforever/src/handoff.rs` | Android has no second process to hand off to |
| 49 | `engine/nvf-assets/src/nif_material.rs` | the crash tests: no soft particles, no decals, no heat haze |
| 43 | `engine/nvf-play/src/bindings.rs` | the Glance at Pip-Boy control, `bound`, the pad-allowing presses |
| 42 | `engine/nvf-play/src/hud.rs` | the meter tags the glance hides (bodies in `wrist_meters.rs`) |
| 41 | `engine/nvforever/Cargo.toml` | the library target, the Android dependencies |
| 40 | `engine/vendor/bevy_render/src/renderer/mod.rs` | readback attribution and the device-lost report |
| 39 | `engine/nvf-play/src/pipboy/data/outline.rs` | the local map's outline for the glance |
| 36 | `engine/nvf-play/src/pipboy/data.rs` | the glance reads the map pages without the menus |
| 34 | `engine/vendor/bevy_render/src/gpu_readback.rs` | which readback a stall belongs to |
| 26 | `engine/nvf-play/src/tweaks.rs` | Hide HUD Key: Keep Quick Loot |
| 24 | `engine/nvforever/src/reports/upload.rs` | Android's own upload path |
| 23 | `engine/nvf-play/src/settings_layout.rs` | the fork's rows in the two counted lists |
| 23 | `engine/Cargo.lock` | the fork's crates (take upstream's lock and build, never merge it) |
| 21 | `engine/nvf-play/src/flashlight.rs` | the glance owns a hold of the Pip-Boy key (body in `wrist_menu_key.rs`) |
| 20 | `engine/nvf-play/src/weapon_wheel.rs` | the hotkeys give up the vertical D-pad (test in `weapon_wheel_dpad.rs`) |
| 20 | `engine/nvforever/src/crash.rs` | the Android crash file |
| 16 | `crates/nvf-settings-layout/src/lib.rs` | the fork's settings rows and their place |
| 15 | `engine/nvforever/src/reports/mod.rs` | reports off on Android unless asked for |
| 14 | `engine/nvforever/src/pipboy_controls.rs` | the glance's pages |
| 11 | `engine/nvforever/src/main.rs` | the five hooks of the table above |
| 11 | `engine/nvf-play/src/lib.rs` | the fork's modules and their plugins |

The rest are ten lines or fewer each: `particles.rs`, `decals.rs`, `effect_particles.rs`,
`reflection.rs`, `vats_highlight.rs`, `dlss/mod.rs`, `heat_haze.rs`, `terrain_shadows.rs`,
`ssao.rs`, `sun_heightfield.rs`, `minimap_render.rs`, `cloud_shadows.rs`, `pipboy_arm.rs`,
`reticle.rs`, `player.rs`, `interface.rs`, `pipboy/data/model.rs`, the two `nif_surface_blend`
and `nif_material.wesl` lines, `engine/Cargo.toml`, `crates/nvf-dlss-files/Cargo.toml` and
`engine/nvf-assets/src/lib.rs` (mostly a crash-test switch or one `cfg` for Android).

### Starting the game

| caller | entry |
| --- | --- |
| desktop binary | `odin::desktop()` → `crate::main()`, which reads this process's argv |
| Android activity (`engine/nvf-android`) | `odin::run(line)` / `odin::run_with(line, extend)`: stores the command line and the shell's plugins, then `crate::main()` |

`extend` is handed the configured `App` by `odin::before_run`, after every plugin and system
the game adds and just before `App::run`: that is where Android's touch and controller input
plugins go.

## The workflows

- **Upstream sync** (`.github/workflows/upstream-sync.yml`), daily at 09:17 UTC and on demand.
  Fetches upstream, replays the stack from `SYNC_BASE` (default `main`) onto `upstream/main` on
  the bot branch `odin/upstream-sync`, force-pushes it, starts an **Odin Android debug** APK
  build of it, and opens or refreshes a draft pull request. It never moves the base branch. A
  rebase it cannot finish is aborted, nothing is pushed, and the job fails with an `::error::`
  annotation and the whole report (conflicting files, the commit it stopped at, how to resolve)
  in the run summary. Issues are disabled on this repository, so the summary and the failed run
  are the report; where issues are enabled it also opens or refreshes one labelled
  `upstream-sync`.
- **Upstream promote** (`.github/workflows/upstream-promote.yml`), manual, after the APK has been
  tested on the device. Saves the current base as `backup/<base>-<date>-<sha>` and moves the base
  branch to the sync branch's commit. Do not use GitHub's merge button on the sync pull request:
  that branch is a rewrite of the base, not commits on top of it.
- **Odin Android debug** (`.github/workflows/odin-android.yml`) builds and signs the APK.

## Resolving a sync conflict by hand

```sh
git remote add upstream https://github.com/LootJunkieAzza/NVForever.git   # once
git fetch --no-tags upstream main
git switch -C odin/upstream-sync main
git rebase upstream/main
# for each conflicted file: keep upstream's version, put our change back on top of it
git add <files> && git rebase --continue
git push --force-with-lease origin odin/upstream-sync
```

Then run **Odin Android debug** on the branch, test the APK on the device, and run **Upstream
promote**.

Rules of thumb:

- Resolve in upstream's favour, then re-apply the fork's intent. Upstream rewrote the system
  our line sat in: put the fork's line into upstream's new version, do not keep the fork's old
  copy of the system.
- A conflict in `engine/nvforever/src/main.rs` means the shape above is being lost. Keep
  upstream's file and re-add the hooks from the table; never take the fork's whole file.
- Builds to run before promoting (the lane names are local): the Android target,
  `cargo check -p nvf-android --target aarch64-linux-android`, and a desktop
  `cargo build -p nvforever --bin nvforever` with a real run of it. `engine/Cargo.lock` is
  upstream's with the fork's crates added; take upstream's lock and re-run a build to add them
  back rather than merging the file by hand.
- If a commit of ours has become upstream's own work, drop it from the stack and say so in the
  sync pull request.
