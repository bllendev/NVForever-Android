# A Vulkan driver of your own on Android (Mesa Turnip)

The Android fork can run the game on a Vulkan driver the player installed, instead of the one
in the phone's system image. On Adreno that means a Mesa **Turnip** build, loaded through
**libadrenotools**. This page is what it does, how to use it, and every way a run goes back to
the system driver.

**NVForever ships no driver, hosts none and mirrors none.** A driver zip is one the player
supplies. The APK carries only the loader (libadrenotools and its four hook libraries,
BSD-2-Clause, LEGAL.md).

Why it exists: the owner's AYN Odin 3 (Adreno 830, Android 15) loses its Vulkan device in play
and the vendor driver is the first suspect the crash tests cannot rule out
([docs/odin-debug.md](odin-debug.md)). Running the same build on Turnip is the measurement.

## Using it

Settings, then **Graphics**, then the **Graphics driver** card:

| row | what the next start does |
| --- | --- |
| Automatic | a driver you installed if there is one, the system driver if there is not |
| System | the phone's own driver, always there and never removed |
| each installed driver | that one, by its `meta.json` name and version |
| Install driver zip... | the document picker; the zip is unpacked into this app's storage |

Hold an installed driver's row to remove it. Every row is a focusable row, so a gamepad reaches
them with the D-pad or the left stick and chooses with A, like the rest of the launcher.

A driver zip is the format the Android emulators use: one 64-bit ARM `.so`, optionally with a
`meta.json` naming it (`name`, `driverVersion`, `libraryName`). The install refuses anything
else at install time rather than at the next start: it reads the ELF header, so a 32-bit or
junk file is rejected while there is still a message to show.

The Home page says, when it happened, that the last start used the system driver and why.

## How a driver is opened

Qualcomm's driver is part of the system image and cannot be replaced, but an app may load
another one into itself. [libadrenotools](https://github.com/bylaws/libadrenotools)
(BSD-2-Clause) opens the system Vulkan loader in a linker namespace of its own, with a hook
that redirects the loader's search for the vendor driver to a library in the app's private
storage.

1. `android::prepare` calls `odin::gpu_driver::start`, before the engine builds its app and so
   before any Vulkan instance exists.
2. It reads `files/gpu_drivers/selected`. **With no such file nothing happens at all**: no
   library is opened, no variable is set, no file is written.
3. `dlopen("libadrenotools.so")` from the app's own library folder, then
   `adrenotools_open_libvulkan(RTLD_NOW|RTLD_LOCAL, ADRENOTOOLS_DRIVER_CUSTOM, ...)` with the
   app's native library folder as the hook folder and the driver's folder and soname.
4. `dlsym("vkGetInstanceProcAddr")` on what comes back goes to the vendored wgpu-hal's Android
   hook (`set_android_vk_get_instance_proc_addr`), whose Vulkan backend then builds its
   `ash::Entry` from that pointer instead of loading `libvulkan.so` itself
   (`engine/vendor/README.md`).

A driver must live in private app storage: a library any other app could rewrite is one
`dlopen` refuses. The APK keeps `useLegacyPackaging = true`, so the hook libraries are
extracted at install time and can be opened by path.

```text
<internal files>/gpu_drivers/
    <id>/                 one installed driver: its .so, and meta.json if the zip had one
    selected              folder, library and label; absent for the system driver
    mode                  the launcher's own row: "automatic", else absent
    renderer_starting     written just before the renderer starts, removed after 300 frames
    last_fallback         why the last start did not use the selected driver
    native_lib_dir        ApplicationInfo.nativeLibraryDir, for the engine's fallback
```

## The crash-safe fallback

A driver that cannot draw would otherwise make the game unstartable, with the launcher's own
choice behind the start that dies. So:

- `start` writes `renderer_starting` just before the renderer comes up, and sets
  `NVF_RENDERER_MARKER` to it;
- the engine removes that file once the run has **drawn 300 frames** (about five seconds at 60
  frames a second: past instance and device creation, past the first swapchain, past the menu's
  first draws, and well past the frame or two in which the Odin 3's loss happens);
- a start that dies before then leaves the file. The next start sees it and runs on the system
  driver, keeping the player's choice, and writes why into `last_fallback`, which the launcher
  shows on Home and in the Graphics driver card.

Starting again tries the driver once more: the marker is removed when it has been read. A run
that asks the launcher to start another one (loading a save from the main menu, which can
happen inside those 300 frames) removes its own marker first, because a run well enough to ask
says nothing about the driver (`android::restart`).

Three things put a run back on the system driver while keeping the selection:

| what | the reason the launcher shows |
| --- | --- |
| the last start left its marker | "the last start on X did not reach 300 drawn frames" |
| the Vulkan validation crash test is on | "which runs on the system driver" |
| libadrenotools refused the driver | the loader's own words, or what was missing |

Validation and a driver of your own are **mutually exclusive on purpose**: the layer chain
under libadrenotools' private namespace is not something this port has checked, and a crash
test that cannot be trusted is worse than none. The Advanced page says so while a driver is
chosen.

## What the run log says

One line at startup, at `warn`, whenever a driver was asked for:

```
gpu driver: requested Turnip 25.0.3; the adapter reports Turnip Adreno (TM) 830 (Vulkan, driver 25.0.3 ...)
```

and one at `info` when the start has survived:

```
gpu driver: this run drew 300 frames; the start marker .../renderer_starting is removed
```

A fallback is a line of its own before the engine's log is up, so it reaches logcat and the
exported debug report:

```
gpu driver: Turnip 25.0.3 is selected but not used: the last start ... did not reach 300 drawn frames ...
```

An ordinary run, with no driver selected, says none of these.

## Test aids

| variable | what it does |
| --- | --- |
| `NVF_GPU_DRIVER` | the words the startup line says were asked for; the Android shell sets it |
| `NVF_RENDERER_MARKER` | the marker file to remove once the frames are drawn |
| `NVF_RENDERER_MARKER_FRAMES` | fewer frames than 300, to see the removal without waiting |

## What CI builds, and what is not proven

The fork's workflow (`.github/workflows/fork-android.yml`) fetches libadrenotools and its
`liblinkernsbypass` submodule at pinned commits, checks both against pinned SHA-256 hashes,
copies their BSD-2-Clause licence texts beside the APK, and lets Gradle's CMake
(`android/app/src/main/cpp/CMakeLists.txt`) build them into `lib/arm64-v8a/`. Neither source
tree is committed. The verify step then requires `libadrenotools.so` and all four hook
libraries in the APK, with 16 KB load alignment like every other packaged library, and fails if
`libnvf_android.so` links the system `libvulkan.so` (once the system loader is in the process a
custom driver cannot be used).

Without those sources Gradle configures no native build at all, so a local
`gradle assembleDebug` works exactly as it did and the APK simply carries no libadrenotools;
the launcher then says the driver could not be opened.

Not yet run on a device: this port has never opened a Turnip build on the owner's Odin 3. The
CMake, Gradle and NDK steps run only in CI.
