# 001 — Native live wrist World Map

**Verdict: it works on the desktop with vanilla data — the map is legible on the arm; the player's arrow is not drawn yet.** The opt-in live World Map
has all three of its parts in engine source — the rules, the arm pose and the screen
material — `cargo test -p nvf-play --lib` and `cargo check -p nvforever --all-targets`
both pass with no warnings, and the rules and the off-switch are covered by unit tests.
It has been seen on a screen (below): the device is held low and facing the player with the
worldspace's map on it in the interface's amber, north up, and it lets go as it should. The
one piece still missing from the picture is the player's own arrow.
What is unproven is listed under **Blockers** at the bottom; nothing in this spike claims
rendered pixels.

Turned on at **Gameplay › Pip-Boy › Glance at the Pip-Boy** (off by default), with
**Glance key** choosing Hold or Toggle; `NVF_WRIST_LIVE=1` still forces it on for a
developer. Its key is the rebindable **Glance at Pip-Boy** control, unbound by default so
that a *hold* of the Pip-Boy key raises it and a tap of that key opens the Pip-Boy exactly
as before. Off (the default) every system below leaves after one comparison: no resource
is read, no material is copied, the Pip-Boy key is untouched, and the paused Pip-Boy, the
bindings, the HUD and the pose are exactly as they were. While the arm is up the glance
holds attack, aim/block, ready/reload, activate and V.A.T.S. — derived each frame from the
open intent, never latched.

## What the engine carries

| File | Change |
| --- | --- |
| `engine/nvf-play/src/wrist_live.rs` | New (pure, no Bevy). `Play`/`Block` (what makes a frame eligible and the first reason it is not), `Key`, `Live::step` (open intent, the 100 ms sample budget, clear-before-draw), `MapPicture`/`Spot`/`Draw`/`Frame`, `window` (the screen's UV window), and 12 unit tests. |
| `engine/nvf-play/src/wrist_arm.rs` | New (Bevy). `WristLive` (the opt-in and the frame's request), `WristBody` (what only nvforever's body knows), `decide` in `PlaySet::Input`, `WristLivePlugin`. |
| `engine/nvf-play/src/pipboy/data/model.rs` | `impl crate::wrist_live::MapPicture for WorldMap` (6 lines): the picture's name and `WorldMap::uv`. |
| `engine/nvf-play/src/pipboy/data.rs` | `mod model` → `pub(crate) mod model` (the live map reads `WorldMap` too). |
| `engine/nvf-play/src/interface.rs` | `own_texture` is `pub` (nvforever loads a worldspace's own picture). |
| `engine/nvf-play/src/lib.rs` | `pub mod wrist_arm;`, `pub mod wrist_live;`, `WristLivePlugin`. |
| `engine/nvforever/src/wrist_screen.rs` | New. `wear_map` (the map on `pipboyscreen:0`, before `show_screen`), `hide_lit` (after it), `feed` (the body's facts back to nvf-play), `WristScreenPlugin`. |
| `engine/nvforever/src/player_body.rs` | One line in `drive_body`: the first-person Pip-Boy hold also comes from `WristLive::arm_clip_time`, plus the resource in an existing parameter tuple. |
| `engine/nvforever/src/main.rs` | `mod wrist_screen;` and `WristScreenPlugin`. |

## The pose, and who owns the screen

**Pose.** `drive_body` already picks the first-person `ClipKey::PipBoy` layer, times it by
`PipBoy::clip_time`, and drives rig visibility, sway, sights and arm magnification from it.
The live view supplies the same number from its own clock (0 → `pipboy::HOLD_SECONDS`, the
hold where the screen faces the player), so the arm raises over a third of a second on the
same rig with the same clip and holds there. It never writes `PipBoy`, `ActiveMenu`,
`PlayState` or a pause clock, and never sends `OpenInventory`. Letting go sets the time back
to none in that frame, so the normal owner has the pose again at once (the menus' own
lowering half of the clip is not played — the live view snaps down).

**Screen.** `pipboy_arm::show_screen` wears the menus' render target on `pipboyscreen:0`
through a copy of that shape's own material. `wear_map` does the same with a different
picture: a copy of the shape's own `NifMaterial` whose base texture is the map the load
order owns for this worldspace, `pipboy_screen` set (so it is tinted, scanned and kept out
of the image-space grade like the real screen), and whose UV transform is
`wrist_live::window` — the player's place in the middle of the screen, the way they face up
it, `HALF_WINDOW` of the picture from the middle to an edge. The shader already samples
`uv_transform * uv` and clamps inside the picture, so there is no render target, no second
camera and no readback.

Ownership is arranged so the normal behaviour always wins:

- `wear_map` runs **before** `show_screen`. In the frame a menu, a conversation, V.A.T.S.
  or the ordinary Pip-Boy takes over, the live view has already put the shape's own
  material back, so `show_screen` copies the real one and never ours.
- It restores only while the shape still wears the copy it installed; if another owner
  replaced it, it lets that owner have it.
- Under a Stealth Boy (`stealth_field::StealthField::on`) the live view yields the arm
  altogether rather than swap materials against the field each frame.
- `hide_lit` runs **after** `show_screen`, which sets `ScreenLit`'s visibility every frame:
  the lit copy over the screen is hidden only while the live view owns the screen, and
  comes back by itself on the frame it lets go.
- A frame is judged afresh from state the engine already keeps: `PlayState::Playing`,
  `ControlInput::suppressed`, `PipBoy::shown`/`warming`, `ActiveMenu`, `VatsMode::holds_input`,
  `KillCam::active`, `PlayerControl::weapon_out`/`aiming`, `PlayerBody::first_person_drawn`,
  the exterior worldspace of `Location`, and the `PipBoyPart::Screen` entity. Anything
  missing means closed.

**Deliberate difference from the brief**: crossing an exterior *cell* boundary inside one
worldspace keeps the picture (the map is per worldspace, and 4096 units apart would blank
it constantly). Changing worldspace clears the screen in that frame and keeps the open
intent; a door into an interior closes the mode (there is no exterior map there). Both are
pinned by tests.

## Checks

Narrow, no Cargo and no Bevy (Python 3 and `rustc`; from the worktree root):

```sh
python spikes/001-live-wrist-world/run.py --nocapture
```

Stage 1 compiles `engine/nvf-play/src/wrist_live.rs` on its own with `rustc --test
-Dwarnings` — the same tests `cargo test -p nvf-play --lib -- wrist_live` runs in the crate.
Stage 2 (`projection.rs`) compiles the module together with the
`CELL`/`WorldMap`/`MapTransform` section extracted from the checked-in `model.rs` and the
same glue `model.rs` carries, which pins the orientation the module assumes to the real
projection. `CLIPPY=1` adds `clippy-driver` over stage 1 when that component is installed.

Local result (Windows, 2026-11-21): stage 1 **12 passed**, stage 2 **16 passed** (the
module's 12 plus 4 projection checks). One cell east and north moves the reported UV from
`[0.5, 0.5]` to `[0.765625, 0.23437503]` facing `[1.0, 4.4e-8]` (east); the screen's window
at `[0.56640625, 0.5]` facing north puts the player dead centre and the top of the screen at
`[0.56640625, 0.42]`, north of them on the real picture.

Cargo, run 2026-11-21 in this worktree (its own target dir, `--locked --offline`):

```
cargo test  --manifest-path engine/Cargo.toml -p nvf-play --lib -- wrist   # 14 passed
cargo check --manifest-path engine/Cargo.toml -p nvforever --bin nvforever # no warnings
cargo test  --manifest-path engine/Cargo.toml -p nvf-play --lib            # 2148 passed,
            # 1 failed, 5 ignored — the failure is phys::tests::two_runs_are_identical, a
            # rapier3d island panic that already fails on main on this machine
```

The 14 are the module's 12 plus two in `wrist_arm`: with the opt-in off (the plugin's
default) `WristLive::wants` and `arm_clip_time` — the two gates nvforever's systems and
`drive_body` go through — give nothing whatever the resource holds; with it on, the screen
is asked for only while the arm is held. `tests::every_plugin_is_added` in nvf-play covers
`WristLivePlugin` being registered.

Not run in this pass: clippy over the Bevy workspace, nvforever's own test suite, and every
smoke scenario.

## What the first desktop run showed (2026-11-21, Goodsprings)

It engages: `wrist: the live World Map is on the arm (worldspace 0x000da726)` and
`wrist: the live World Map is on the arm's screen
(textures\interface\worldmap\wasteland_nv_1024_no_map.dds)`, and an earlier run let go
of the arm by itself for a DLC message box. Two things the pixels settled:

- **The glance needed its own view.** Through the menus' 47 the lens draws the rig so far
  across the view that only the gloved hand is in the picture and the Pip-Boy is past the
  left edge; at 85 the device is cut by it; at 110 (`WRIST_FOV`, `player_body.rs`) the
  whole device is in the lower left with the road ahead clear.
- **The device's screen faces away from the player at the Pip-Boy clip's hold, and that is
  not this spike's doing.** With the menus opened instead (`NVF_PLAY_TEST_PIPBOY=4:open`)
  at the same spot the arm is in the very same pose, only magnified further, so the paused
  Pip-Boy shows the player a hand and no menus at all. The eye sees the device's cuff side
  whatever the view pitch, because `place_lens` carries the rig with the view; no turn of
  the rig about the eye can help (a rigid turn about the eye leaves the angle the eye sees
  a surface at unchanged — tried on the desktop and dropped). Making the screen face the
  player means re-posing the forearm, or re-measuring the clip's hold
  (`nvf_play::pipboy::HOLD_SECONDS`, 0.333 s of a 0.733 s clip; sweeping the clip showed
  the arm is up only in a narrow band around it). That is a baseline Pip-Boy fix the owner
  should rule on, not a change this spike should smuggle in: **the map is on the far side
  of the device, so it is not legible yet.**

Screenshots of that run (not committed; game-derived):
`%LOCALAPPDATA%\hermes\cache\scratch
vf-wrist-try\` — `final110.png` (the glance as
it ships), `crop110.png` (the device close up, cuff side to the eye), `ref_menu.png` (the
paused Pip-Boy, same pose, device off the picture), `ref_plain.png` (no arm at all with the
mode off).

## Polish pass (2026-11-21, vanilla data view)

With the vanilla load order the arm's hold pose is right: the device faces the player, so
the earlier "screen facing away" was the modded install's, not the engine's.

- **The map reads as a Pip-Boy map.** It is drawn in the interface's own amber at the
  picture's own alpha, as the paused page draws the very same picture
  (`wrist_screen::map_ink`, `pipboy::data::MAP_ALPHA`): laid on raw it came out grey,
  because the screen's own tint (`SCREEN_TINT`) is near neutral — the menus' amber is in
  their drawing, not in the tint.
- **The map is north up** and the player's arrow turns instead, which is how a map on a
  wrist reads; the window maths already had the north-up case under test.
- **The glance holds the arm lower than a menu** (`player_body::WRIST_DROP`, 3 units under
  the eye) so the road ahead is read over the device.
- **The player's arrow is written but not yet drawn.** `wrist_screen::show_arrow` places a
  small amber dart on the posed screen shape — the maths is under test (middle of the map,
  turning with the player, both ways round a mirrored screen, nothing on a degenerate
  shape) and the entity is spawned and moved each frame at a sane place and size, but
  nothing of it reaches the picture. Not a pipeline error (the log is clean), not the
  material (`StandardMaterial` and `NifMaterial` both), not the layer (the arms' and the
  world's both), not the lift (a step along the eye's own ray, up to a sixth of the way to
  the eye). The next thing to try is the arms' own pass: a quad that is not part of the rig
  may simply not be in it, in which case the arrow belongs under the lens nodes
  (`player_body::place_lens`) or in the screen's own material.

## Marks, and the rules the behaviour notes ask for (2026-11-21)

- **The marks are in the picture now, not over it.** The 3D dart is gone: a loose quad is
  never drawn by the pass that draws the first-person rig. The live view lays out its own
  256-square picture at each sample — the map's window, the player's mark turned the way
  they face, and the compass letters, which stand still because the map is north up — and
  the screen's material wears that picture whole. The map's texels are decoded once per
  worldspace (`read_picture`), because the graphics card keeps block-compressed pictures
  compressed and they cannot be read back. Seen on the desktop at Goodsprings: facing east
  the mark points right, facing north it points up, with N, E, S and W at the sides.
- **Two more blockers, read afresh every frame and never latched**: the player acting with
  their hands (attack, block or aim, ready or reload, activate, V.A.T.S.) ends the glance
  on the frame of the press, before the press is acted on; and the real Pip-Boy going down
  starts a short wait so one press does one thing. The Pip-Boy's own key is deliberately
  not blocked — it must keep working, and the menus block the glance by themselves.
- **Done since: the green local map** (see the next section).
- **Also not done from the spec**: the derived control suppression belongs with a real
  hold/toggle key — with the current self-opening mode, holding Attack or Ready would trap
  the player, so the glance yields to those presses instead. Device presets (the glass's
  own UV rectangle), the DATA lamp, knobs, the rad needle and the up/down foley are
  untouched.

## The green local map (2026-11-21)

**It works on the desktop with vanilla data**: at Goodsprings the glass shows the place
round the player in green, the buildings and the road's banks as outlines, the player's
mark in the middle and N, E and W at the sides (`local-a.png` of the try folder; the
screen's colour was set to Green in `NVForever.ini` for the shot).

- **The base is the Local Map's own outline**, option 2 of the clean-room spec's §9: no
  second camera, no readback, and the picture a plan of the collision the physics world
  holds. The ray casting the Local Map page did inline is `pipboy::data::outline::cast`
  now, and `nvf-play` `wrist_local` asks it for a grid of the 5×5 cells round the player
  (246 × 246 samples 96 units apart) whenever the glance holds the arm: 1.5 ms of a frame,
  64 rays at a time. At Goodsprings the whole grid was made in about a second of looking
  (`wrist: the local map's outline is made for the glance: 246x246 samples 96 apart, 60516
  landed`) and the screen changed from the worldspace picture to the outline in the same
  breath. The grid is kept for a lattice of anchors, so walking about keeps it; the
  finished one stays on the screen while the next is cast.
- **The window is close**: about 175 m across the glass (`wrist_local::LOCAL_SPAN`), two
  texels a sample, where the worldspace picture shows a slice of the whole map. The
  picture is laid out once per grid and shared behind an `Arc`; the screen lays its canvas
  out again only when the window, the facing or the base itself changed.
- **The ink is the Pip-Boy screens' own colour** (`interface::pipboy_rgb`), so a green
  device draws a green map, and it is at full strength: the dimming moved into the base
  (`Pic::gain`), which leaves the player's mark and the compass letters the brightest
  things on the glass.
- **The player chooses** at Gameplay › Pip-Boy: `wrist-map-style`, Local (the default) or
  World. Where there is no outline yet — the first second of a glance, after travel, or
  where the player has walked off the one that is made — the worldspace's own picture
  stands in, so the glass is never blank.
- **A crash that was in the port**: `ControlInput` read `WristLive` while `decide` writes
  it, which Bevy refuses (B0002), so every run panicked on the first frame of `Update`.
  The hold is a global now (`wrist_arm::glance_holds`), cleared while the frame's keys are
  read and set from the frame's own decision — which is also what the behaviour notes ask
  for, since the press of an attack must be seen on the frame it is made.
- **`NVF_WRIST_FORCE_RAISE=1`** holds the glance up with no key, for screenshots. A
  developer's switch; it stands in for the key and nothing else.
- **Still not done**: interiors (the glance does not open in one yet, though the producer
  draws a plan of the rooms), the marker and door icons over the outline, and everything
  of the spec's §8 (lamp, knobs, needle, foley).

## The owner's list after the first on-device run (2026-11-22)

The first Odin 3 photo of build map.12 (1080p-ish 16:9 handheld) showed the mode working:
the amber map on the glass with N/E/W/S and the player's triangle, the arm up in the
middle right of the frame. What the owner asked for next, and where it stands:

1. **Done — the glance opens with a weapon drawn.** `Play::weapon_out` is
   `Play::aiming` now (`Block::WeaponOut` → `Block::Aiming`): the sights block it, a
   weapon merely in hand does not. Nothing extra lowers the weapon, because the Pip-Boy
   clip poses *both* arms at the hold and its tracks outrank the weapon's — which is
   exactly what the menus' own raise does with a drawn weapon. The glance already holds
   attack, aim, ready, activate and V.A.T.S. while the arm is up, so nothing fires or
   zooms through it, and a press of any of them ends the glance on the frame of the
   press, before the press is acted on. Nothing is holstered, so nothing has to be drawn
   again when it goes down. **Not yet seen with a weapon in hand on the desktop.**
2. **Done — every compass letter is whole inside the glass.** The shape the picture is
   worn on runs under the device's own frame, so the S stood on the bottom edge with its
   lower half under the button bar. `wrist_screen::letter_ring` places the four letters
   inside a glass rect (`GLASS_INSET`, a tenth of the picture each side) with
   `LETTER_MARGIN` to spare, across and down separately, never further out than the old
   `LETTER_RING`. Seen at Goodsprings: N, W, E and S all clear of the frame.
3. **Done — quest targets, map markers and doors on the map.** The HUD compass's own last
   look (`compass::CompassNear`, ten times a second) rides on the sample as
   `wrist_live::Mark`s in the picture's own coordinates; actors are left off. The screen
   draws a diamond where one falls inside the window and a chevron held at the glass's
   edge where it does not (`Window::of`, the window taken the other way about). Same CPU
   overlay, same canvas, same sample cap, held to `MARKS_MOST`; the canvas is laid out
   again only when the window, the facing, the base or the marks changed.
4. **Done — the device is held low on the left.** `place_lens` takes a side offset beside
   its drop (moving the lens node by `d` across the view leaves the rig `(1 − k)·d` from
   where it was, and the glance's arms view has `k` under 1, so a positive side is to the
   right and the glance passes a negative one). `WRIST_SIDE` is 14 units, fitted at
   1920 × 1080 at Goodsprings: at 26 the device ran off the left edge; at 14 it is whole
   in the lower left with the road ahead clear. `NVF_WRIST_SIDE` overrides it. The row
   **Glance position** (`wrist-map-position`) chooses Lower left (the default) or Centre.
5. **Done — a refused glance says why.** One info line per press, not repeated until the
   key is let go, naming `Block::why()`: seen as
   `wrist: the glance was asked for and did not open: the first-person arms are not drawn`
   in the first second of a run.
6. **Partly done — quality of life.** The menus' own raise and lower foley
   (`UIPipBoyAccessUp` / `UIPipBoyAccessDown`, the load order's own SOUN records) plays
   with the glance (`wrist_arm::foley`), and **Glance zoom** (`wrist-map-zoom`) shows
   Near (the window the picture asks for) or Far (twice as much either way). **Interiors
   are still not allowed** (`Block::Interior`): the outline producer draws a plan of the
   rooms, but the glance's key is the worldspace and an interior would need the cell in
   its place, with the worldspace picture gone as the base — a change of its own.

Still open from this pass: the letters and the marks are drawn in the same ink as the
outline and the outline's walls are brighter, so a letter over a building is hard to read;
the device presets, the DATA lamp, the knobs and the rad needle of the notes' §8.

## Blockers for a desktop try

1. **No clippy over the Bevy workspace yet.** Both crates compile warning-free under
   `cargo check`/`cargo test`, but `cargo clippy --release --all-targets -- -D warnings` in
   `engine/` has not been run on this branch; it is one of the push checks.
2. **No pixels have been seen.** Unproven on the real rig, in this order: that the map
   texture reads as a map at the screen's size and `UV_EXTENT` crop (the shader samples mip
   0 and clamps inside the picture, so a map of 2048² on a screen of about 200 px will
   alias); that the tint and scanlines leave it legible; that `HALF_WINDOW = 0.08` is a
   useful magnification; and that the arm actually draws the Pip-Boy shapes with live
   locomotion when the pose comes from here rather than from the menus.
3. **The player marker is the middle of the screen, not a drawn arrow.** The window puts
   the player there and turns the map under them, which is the whole of "you are here" in
   this slice. A glyph needs either a shader addition, a small child quad on the screen
   shape, or a CPU-composed picture; none of those is in this pass.
4. **No switch and no key.** `NVF_WRIST_LIVE=1` is the opt-in. A settings row for a
   gameplay change this size belongs on **NVF Features**, which needs the owner's word
   (docs/design/settings-page.md §0), and a toggle key means a new `bindings::Control` with
   its INI name and controls-page row. Until then the mode raises itself whenever the frame
   is eligible, which is what `pressed` means to the rules.
5. **`WristBody` is a frame old.** The body's facts (arms drawn, which shape is the screen)
   are written after `BodySet` and read by `decide` in the next frame's `PlaySet::Input`. It
   can only delay opening or closing by a frame, and `wear_map` refuses to dress a shape the
   request does not name, but it has not been watched in a real rig rebuild or a load.
6. **Armed poses are untried.** The mode is holstered and empty-handed only (`weapon_out`,
   `aiming`), to be compared with stock before anything else; #2862 already owns a baseline
   first-person visibility issue.

**Stop/go gate** (unchanged): desktop, owned Data, a separate profile; compare disabled,
enabled-closed and open on the same exterior route; pixels only on the wrist; watch heading,
cell crossings and a world change; menu takeover and pose/input restoration on toggle,
pause, ordinary Pip-Boy, dialogue, V.A.T.S., load/reload and a camera or rig change; a log
that shows no sampling while closed (`wrist:` lines say when it took and let go of the arm
and why); and the normal `tiles-data-world`, `pipboy-first-open` and
`pipboy-screen-menu-unworn` smoke scenarios. No desktop renderer/play or Odin result yet.
No Local Map, settings UI, extra markers, generic projection, mod adapter or Android work in
this spike.

Read-only overlap check (no upstream changes or claims): #2418 (Local Map) and #2862
(visibility) remain open under `jojokengly`; #3268 is about marker overrides for existing
maps and the HUD, not this single arrow. The earlier standalone harness here (`wrist.rs`,
`tests.rs`, a copy of the state machine) is gone: the engine module and `projection.rs`
replace it.

## Behavior reference: Realtime Pip-Boy Map (Stentorious, Nexus 92277, v1.4.2)

Studied for behavior only. Its permissions forbid reuse of its assets, uploads,
forks or AI-assisted modification without the author's consent, so nothing from
it (scripts, meshes, the Pip-Boy check animation, INI presets) is copied or
converted here. It is also a PC xNVSE/JIP/kNVSE stack, not an ARM64 input.

Features worth matching independently:
- Hold/toggle key bind with controller support; optional "walk while looking".
- Screen refresh-rate cap, brightness, scanline toggle, depth-of-field blur on
  the world while looking.
- Raise/lower foley sounds; Pip-Boy knobs move with map interaction.
- Detection/combat indicator on the map border.
- Per-Pip-Boy screen presets (modded devices place the screen differently).

Its changelog's bug classes, as regression checks for our slice:
- Controls must never stay disabled when a menu or dialogue opens while
  raising or lowering the arm.
- Attack must not draw the weapon while looking; scoped zoom must not engage.
- Pip-Boy light/screen glow must reset on exit (incl. third person).
- The view mode (third person) is restored after exit.
- No map overlap at the world map's border.
- Missing Pip-Boy mesh or screen nodes: feature inert, no crash.
