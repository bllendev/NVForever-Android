# The Android fork's crash tests and GPU diagnostics

This is the fork's own page for the owner's AYN Odin 3 (Adreno 830, Android 15), whose
graphics device dies in play. `docs/fork.md` is what the fork is and why; this is what to
turn on when it dies again, and what the run log will then say.

Nothing here is a setting. Every switch either leaves something out of the picture or turns
on a diagnostic that makes the game slower, and every one of them is **off by default**. A
game with none of them on draws exactly what it drew before, down to the pipelines built.

## Where the switches are

They are not on any page a player scrolls past. The launcher opens on Home; **long press the
build line** ("Experimental build (...)") and the Advanced page opens. While any switch is on,
that same line says so in amber: "3 crash tests on (long press)". **Turn every crash test
off** is one action on the page.

Each row writes a `key value` line into `<engine home>/odin-advanced.txt`. The engine reads
that file before it starts any threads and exports the variables its gates read
(`engine/nvforever/src/odin/crash_tests.rs`, the one table both sides follow;
`OdinCrashTests.java` is the launcher's copy of it, and `tools/android/tests/test_fork.py`
fails if the two ever disagree). A desktop run sets the same variables itself.

| The row | the file key | the variable | what it does |
| --- | --- | --- | --- |
| No V.A.T.S. target outline | `no-vats-outline` | `NVF_NO_VATS_OUTLINE` | The selected body part is not lit on the target, so the outline's plugin registers nothing and its extra mask camera never exists. Choosing targets and the V.A.T.S. menu are unchanged. |
| No soft particles | `no-soft-particles` | `NVF_NO_SOFT_PARTICLES` | A particle is given no soft-intersection distance, so it has a hard edge where it passes into a wall; no material is then built with the soft-fade path, and the one read the main pass takes of the view's depth prepass goes with it. |
| No particles | `no-particles` | `NVF_NO_PARTICLES` | No particle system and no effect particle is drawn. They are still simulated, so nothing else in the game changes. |
| No decals | `no-decals` | `NVF_NO_DECALS` | No decal is placed (blood, bullet holes). The shots and their impacts are unchanged. |
| No heat haze | `no-heat-haze` | `NVF_NO_HEAT_HAZE` | The heat-haze pass records nothing. It is a post-process pass, not a transparent draw, and is off in every preset anyway. |
| Skip the Pip-Boy warm-up during loads | `no-pipboy-warm` | `NVF_NO_PIPBOY_WARM` | The hidden raise that warms the Pip-Boy's materials behind a loading screen is left out. It is the one thing drawing while a load streams. |
| GPU breadcrumbs | `gpu-breadcrumbs` | `NVF_GPU_BREADCRUMBS` | Every render-graph node writes a marker before and after its work (below). |
| GPU breadcrumbs inside the transparent pass | `gpu-breadcrumb-chunks` | `NVF_GPU_BREADCRUMB_CHUNKS` | Cuts that one pass into up to 32 render passes with a marker between them (below). Greyed until GPU breadcrumbs is on. |
| Vulkan validation (slow) | `vulkan-validation` | `NVF_VULKAN_VALIDATION` | The Khronos validation layer checks every call (below). |
| Vulkan sync validation | `vulkan-validation-sync` | `NVF_VULKAN_VALIDATION_DEEP=sync` | Synchronization validation as well. Greyed until the layer is on. |
| Vulkan GPU-assisted validation (very slow) | `vulkan-validation-gpuav` | `NVF_VULKAN_VALIDATION_DEEP=gpuav` | Instrumented shaders. Greyed until the layer is on. |

A row that needs another is greyed until that one is on, and turning the one it needs off
turns it off too, rather than leaving it on behind it. The last two share one variable and
become `NVF_VULKAN_VALIDATION_DEEP=sync,gpuav`.

The crash tests are **not test aids**: `nvf_play::test_aid::drives_run` does not see these
names, so a run with one on is still an ordinary attended run, and the Pip-Boy row leaves
upstream's own `NVF_PLAY_TEST_NO_PIPBOY_WARM` branch untouched.

## What every run already says: the ten-second summary

The GPU flight recorder (`engine/nvforever/src/gpu_recorder/`) is always on and costs
nothing worth measuring: per frame an atomic swap a counter, one uncontended lock and a
60-byte write into a fixed array. There is no switch for it. Every ten seconds it writes one
line:

```
gpu recorder: last 128 frames: 10.8 ms mean, 13.6 ms worst; uploads 5600 writes, 22.1 MB;
pipelines busy at most 62; readbacks at most 1; ran load; gpu allocated 836 MB, reserved
1050 MB in 20 blocks, 1320 live allocations; VRAM 1141 of 7450 MB; resident 2646 MB;
VmSize 9122 MB, VmRSS 3993 MB (anon 3310 MB, file 683 MB); 48213 mappings of 65530;
malloc in use 1420 MB, mapped 1902 MB (read 0.0 s ago)
```

(one line in the log; wrapped here, and the last three clauses are Android's alone). Each
start and end of a loading screen also gets a line of its own with the process's resident
memory, because a load is where the main memory moves most.

What a frame records: Bevy's frame index, the time since the last loading screen came up, the
frame's wall time, which of the engine's heavy render features ran (the water reflection,
both terrain-shadow paths, the sun heightfield, the V.A.T.S. mask, the minimap, cloud shadows,
both ambient-occlusion passes), the bytes and writes uploaded through the render queue, the
pipelines queued or compiling, and the readbacks whose buffer map has not come back. The
uploads and readbacks are counted in the vendored `bevy_render` (`engine/vendor/README.md`),
because the recorder cannot be a dependency of that crate.

The Android memory figures (`odin/proc_mem.rs`) are the ones the out-of-memory abort of
2026-11-24 is read back against: a 64 KB allocation failed with about 10 GB of the phone
free, which only the mapping count against `vm.max_map_count` and the allocator's own figures
can explain.

## What a lost device prints

The moment the loss is known the driver is asked what faulted, while the device is still
there, and then the whole window is written once: into the run log, and into the crash report
under the loss (both the panic path and the no-panic one). A run that keeps its device writes
none of this.

```
gpu recorder: the graphics device was lost (reported by wgpu)
gpu recorder: the last 128 of 4821 frames of this run, oldest first:
       frame  load s  frame ms writes  up MB  busy    asked   rb  features
        4694     2.1      16.8      7   0.42      0      912     1  load,water-reflection
        ...
        4821     7.6     124.0      0   0.00      0      912     2  vats-mask,ao-lighting
gpu recorder: gpu allocated 844 MB, reserved 1235 MB in 22 blocks, 3441 live allocations;
  resident 3993 MB; 48213 mappings of 65530 (read 3.2 s ago)
gpu recorder: VK_EXT_device_fault: GPU hang
gpu recorder: VK_EXT_device_fault address: write to an invalid address at 0x00000000c0ffee00
  (precision 0x1000)
gpu recorder: VK_EXT_device_fault vendor: KGSL fault (code 0x3, data 0x0)
```

`VK_EXT_device_fault` is asked for at device creation, and only when the adapter offers both
the extension and its `deviceFault` feature; the vendor binary blob is never asked for. A
driver without it says so at the start of the run, and the dump reads
`gpu recorder: VK_EXT_device_fault: nothing to report`. `NVF_VK_DEVICE_FAULT=0` never asks for
it, as a way out if a driver misbehaves with it enabled.

With **GPU breadcrumbs** on, the same dump ends with the one thing the frames cannot say:

```
gpu breadcrumbs: 248 markers in the window, 171 of them reached by the GPU
gpu breadcrumbs: the last marker the GPU reached: frame 14 began
  bevy_core_pipeline::core_3d::main_transparent_pass_3d_node::main_transparent_pass_3d
gpu breadcrumbs: the next marker it never reached: frame 14 end of
  bevy_core_pipeline::core_3d::main_transparent_pass_3d_node::main_transparent_pass_3d
gpu breadcrumbs: frame 13: 36 markers, 36 reached; last began ui, last ended ui;
  began and never ended: none
gpu breadcrumbs: frame 14: 19 markers, 17 reached; last began main_transparent_pass_3d,
  last ended main_transmissive_pass_3d; began and never ended: main_transparent_pass_3d
```

The last begin with no matching end is the node that hung. That is exactly what the old base
said on this device [observed 2026-11-22: frames 2 to 13 of a save load completed, frame 14
began `main_transparent_pass_3d` and never ended].

With **the chunks** on as well, the transparent pass is cut up and the dump names the draws:

```
gpu breadcrumbs: the transparent pass was cut into chunks (NVF_GPU_BREADCRUMB_CHUNKS):
  24 frame(s) of chunk tables kept
gpu breadcrumbs: began and never ended: frame 14 chunk 01 items 4..9, 5 draw(s),
  pipeline 212, draw fn 1 [transparent_mesh_pipeline nif_material.wesl;
  particles meshes\effects\ambient\fxsprayjet01.nif pSteam07]
gpu breadcrumbs: frame 14 chunk 00 items 0..4, 4 draw(s), pipeline 190, draw fn 0 [...]
```

Particles, effect particles and decals tag their drawn entity with their model while that
switch is on, which is where the names in the brackets come from.

### How a marker works, and what it cannot promise

A marker is a `vkCmdFillBuffer` of a monotonic sequence number into one slot of 1024 words of
host-visible, host-coherent memory, mapped for the whole run (the vendored `bevy_render`'s
`gpu_breadcrumbs.rs`). After a lost device the mapping still reads: a slot holding the number
its fill was told to write is a marker the GPU reached. Each marker is a command buffer of its
own, because wgpu-core 30 panics as soon as an encoder touched by `as_hal_mut` is used with an
ordinary wgpu call; the queue is given marker, node, marker in that order.

- No barrier orders a marker against the work around it, so "the GPU reached this node's
  begin" is not "every command before it retired", and a value reaches the CPU when the driver
  flushes it, which on a hung queue is best effort. That is why the dump prints the whole
  window rather than one answer.
- The chunks change nothing about what is drawn: the phase keeps its sorted order and a cut
  falls only on a batch boundary. What changes is that each chunk is a render pass that loads
  and stores the colour and depth attachments, which on a tiler is the whole cost and a large
  one. A `vkCmdFillBuffer` is not allowed inside a render pass, which is why the chunks exist
  at all: a marker cannot otherwise sit between two draws of one pass.
- Cost of the node markers alone: 60 to 120 extra one-command command buffers a frame and
  4 KiB of memory. [observed 2026-11-21, one pair of desktop runs, Goodsprings 12:00, the
  `dev` profile: 9.2 ms a frame with the switch on against 8.5 ms with it off. An order of
  magnitude, not a measurement.]
- `NVF_GPU_BREADCRUMBS=1` and `NVF_GPU_BREADCRUMB_CHUNKS=1` do the same on a desktop run.

## Vulkan validation

The layer binary is `libVkLayer_khronos_validation.so` from the Khronos
[Vulkan-ValidationLayers release `vulkan-sdk-1.4.363.1`](https://github.com/KhronosGroup/Vulkan-ValidationLayers/releases/tag/vulkan-sdk-1.4.363.1)
(asset `android-binaries-1.4.363.1.zip`, sha256
`2c17b00a95b18fd63244b27332ef86f45c9269769f798a6261ce80b869c41af4`; the arm64-v8a library
inside it, sha256 `7d36b66e72fd11270fba96de10002256ba95c8408047a8167e636673c16760ae`). It is
**licensed Apache-2.0 by the Khronos Group and its contributors**, is not part of NVForever
and is never committed: the fork's APK workflow downloads it, checks both hashes and unpacks
only that one library before Gradle packages the APK (LEGAL.md).

With the switch on, the instance asks for `VK_LAYER_KHRONOS_validation` with
`VALIDATION|DEBUG`, and the run log's filter turns `wgpu_hal` up to `debug` for the run,
because Bevy's default holds everything under `wgpu` at error. One line at startup says the
switch is on, which checks were asked for, and whether the layer library loaded. Expect a much
slower game.

How the layer is found on the device: the APK extracts its native libraries at install time
(`useLegacyPackaging`), so Android's Vulkan loader can list layers from the app's own library
folder; CI fails the build if that library did not reach the APK. Belt and braces, the engine
also loads it by name itself before the renderer exists, the way the pre-Android-9 recipe did.

If a run says the layer did not load, or wgpu-hal says `unable to find layer`, enable it from
outside the app and start again:

```
adb shell settings put global enable_gpu_debug_layers 1
adb shell settings put global gpu_debug_app org.nvforever.android.odin
adb shell settings put global gpu_debug_layers VK_LAYER_KHRONOS_validation
adb shell settings delete global enable_gpu_debug_layers   # to undo
```

The two deeper rows do nothing on their own: the layer that would read their settings is not
loaded. Together they become `NVF_VULKAN_VALIDATION_DEEP=sync,gpuav`, which chains a
`VkLayerSettingsCreateInfoEXT` onto the instance naming `khronos_validation.validate_core`,
`.validate_sync` and `.gpuav_enable` [documented: the Khronos validation layer's layer
settings; a setting name the layer does not know is ignored, so it simply keeps its defaults].
GPU-assisted validation also sets wgpu's own `GPU_BASED_VALIDATION` instance flag, so wgpu-hal
chains the older `VkValidationFeaturesEXT` for it too.

## Narrowing a loss down, in order

1. Play as usual. The ten-second line is already in the run log, and a loss already prints the
   window and the driver's fault info. Send that first.
2. If the loss is in V.A.T.S. or just after it, turn **No V.A.T.S. target outline** on. That
   is the outline's extra mask camera gone entirely.
3. If it is a few seconds after a load, turn **GPU breadcrumbs** on and play until it happens.
   The dump then names the node.
4. If the node is the transparent pass, turn **GPU breadcrumbs inside the transparent pass**
   on as well, and read the chunk the GPU began and never ended.
5. The chunk's own name says what was in it: turn that one feature off (**No particles**,
   **No decals**, **No soft particles**) and see whether the run survives.
6. Only then **Vulkan validation**, and only then its two deeper rows. They are the slowest
   thing here and a load may not finish with GPU-assisted validation on.

## What is not here

Turnip through libadrenotools (a Vulkan driver of your own) is phase 3.2 of
`.hermes/plans/android-base-phases.md` and is not on this branch. Validation and a custom
driver were deliberately mutually exclusive on the old stack, so whichever lands second has to
say so again.
