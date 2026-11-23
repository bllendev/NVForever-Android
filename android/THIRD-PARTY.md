# Third-party files in the Android app

Everything bundled in the APK that is not NVForever's own code, where it came from and under
what licence. The app's About screen shows the same list and opens each full text from
`app/src/main/assets/licences/`.

| File in the repository | What it is | Licence | Licence text in the app |
|---|---|---|---|
| `app/src/main/res/font/jetbrains_mono_regular.ttf`, `jetbrains_mono_bold.ttf` | JetBrains Mono 2.304, the launcher's only typeface | SIL Open Font License 1.1 | `assets/licences/jetbrains-mono-ofl-1.1.txt` |
| (not committed; CI downloads it) `app/src/main/jniLibs/arm64-v8a/libVkLayer_khronos_validation.so` | the Khronos Vulkan validation layer, for the Debug screen's validation switches | Apache-2.0 | `assets/licences/vulkan-validation-apache-2.0.txt` |
| the engine library built from this repository | NVForever itself | GPL-3.0-or-later | `assets/licences/gpl-3.0.txt` |
| the Rust crate graph the engine is built on | Bevy and several hundred crates | MIT or Apache-2.0, enforced by `deny.toml` | `assets/licences/rust-crates.txt` |

## JetBrains Mono

Downloaded from the project's own release, `JetBrainsMono-2.304.zip`
(github.com/JetBrains/JetBrainsMono, release v2.304), and the two static TTFs plus `OFL.txt`
taken out of it unchanged. The file names are lower case because Android resource names must
be; the font data is byte for byte the release's.

```
fonts/ttf/JetBrainsMono-Regular.ttf  273900 bytes  sha256 a0bf60ef0f83c5ed...
fonts/ttf/JetBrainsMono-Bold.ttf     277828 bytes  sha256 5590990c82e09739...
OFL.txt                                4399 bytes  sha256 30f0c136e3c88e42...
```

It is used because it is not the game's font and not a lookalike: the launcher must never
ship Monofonto, Overseer, or anything made for or shipped with Fallout: New Vegas
(`LEGAL.md`). Two weights only; headings differ by size, letter spacing and colour.

## The Apache-2.0 text

`assets/licences/vulkan-validation-apache-2.0.txt` is the standard Apache License, version
2.0, taken from a copy already in this repository (`engine/vendor/bevy_pbr/LICENSE-APACHE`).
It is the licence of the Khronos validation layer the debug build packages, and of the half
of the Rust crate graph that is used under Apache-2.0.
