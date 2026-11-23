// Milestone 2: the whole game. The app module: packages libnvf_android.so (built from Rust by cargo-ndk) into an APK that
// runs it through a NativeActivity (GameActivity.java). The only Java is the launcher: the permission, the folder picker,
// the data profiles and the debug screen (docs/android.md). It uses the framework alone: no AndroidX, no Kotlin, no
// dependency at all, which is also what keeps its string resources and its bundled font library-free.
plugins {
    id("com.android.application")
}

// Whether the libadrenotools sources are in place (see the externalNativeBuild block below).
val adrenotoolsPresent = file("src/main/cpp/libadrenotools/CMakeLists.txt").exists()

android {
    // The game data is read from any folder the player picks (default /storage/emulated/0/NVForever/Data).
    namespace = "org.nvforever.android"
    compileSdk = 35
    buildFeatures { buildConfig = true }

    defaultConfig {
        applicationId = "org.nvforever.android.odin"
        // 26 = Android 8.0. Vulkan 1.1 phones are the target; the manifest declares the
        // requirement, so the Play Store / installer hides the app from phones without it.
        minSdk = 26
        targetSdk = 35
        val buildNumber = providers.environmentVariable("GITHUB_RUN_NUMBER").orNull
        versionCode = buildNumber?.toInt() ?: 1
        versionName = "0.8.1-odin.${buildNumber ?: "local"}"
        // 64-bit ARM only (every phone sold in years). x86_64 (emulator) can be added later
        // with `-t x86_64` in the cargo-ndk call below and here.
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    // libadrenotools and its hook libraries, for the launcher's optional "GPU driver" choice
    // (src/main/cpp/CMakeLists.txt, docs/odin-debug.md). Its sources are never committed: CI
    // downloads them at pinned commits into src/main/cpp/libadrenotools/ before this build
    // runs. Without them there is no native build at all, so a local `gradle assembleDebug`
    // works exactly as it did and the APK simply carries no libadrenotools.
    if (adrenotoolsPresent) {
        // The NDK AGP builds the C/C++ with. Without this AGP wants its own default version,
        // which is not the one CI installs (NDK_VERSION in .github/workflows/odin-android.yml).
        // It matters only here: with no native build AGP never looks for an NDK at all.
        providers.environmentVariable("NDK_VERSION").orNull?.let { ndkVersion = it }
        externalNativeBuild {
            cmake {
                path = file("src/main/cpp/CMakeLists.txt")
                version = "3.22.1"
            }
        }
        defaultConfig {
            externalNativeBuild {
                cmake {
                    // The NDK's default for CMake, named rather than assumed: libadrenotools
                    // and its hook library each carry their own copy, which is what the
                    // private linker namespace the hooks live in needs.
                    arguments += "-DANDROID_STL=c++_static"
                    // 16 KB pages, as the engine's own library already is and as CI checks
                    // every packaged library for. NDK 28 does this anyway; asked for by name
                    // so the build does not depend on that staying the default.
                    arguments += "-DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON"
                }
            }
        }
    }

    buildTypes {
        // Both build types package the RELEASE Rust library (see cargoNdkBuild): Bevy compiled
        // without optimisation runs at a few frames per second, which tests nothing useful.
        // The "debug" APK is still debuggable and signed with the debug key.
        release {
            isMinifyEnabled = false // a handful of small Java classes; the engine is the APK's size
            signingConfig = signingConfigs.getByName("debug") // M1: sideload only, no store key yet
        }
    }

    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")

    // The debug APK also carries the Khronos Vulkan validation layer when CI put it in
    // jniLibs (the launcher's "Crash test: Vulkan validation (slow)"; docs/odin-debug.md).
    // Android's Vulkan loader finds a layer as a *file* in the app's native library folder,
    // so the libraries are extracted at install time (extractNativeLibs=true) instead of
    // being mapped straight out of the APK, which would leave that folder empty. Nothing
    // about the game changes; both libraries are 16 KB-page aligned either way.
    // libadrenotools needs the same thing for the same reason: its hook libraries are opened
    // by path out of that folder, and its own header says the app must package them this way.
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
}

// ---------------------------------------------------------------------------------------
// Rust build. Needs (once): `rustup target add aarch64-linux-android`, `cargo install
// cargo-ndk`, and the Android NDK (Android Studio: Settings > SDK Manager > SDK Tools >
// "NDK (Side by side)"). cargo-ndk finds the NDK through ANDROID_NDK_HOME, or through the
// ANDROID_HOME/ndk folder. Details: docs/android.md.
// ---------------------------------------------------------------------------------------
val cargoNdkBuild by tasks.registering(Exec::class) {
    onlyIf { !project.hasProperty("prebuiltRust") }
    group = "nvforever"
    description = "Builds libnvf_android.so for arm64-v8a with cargo-ndk"
    workingDir = file("../../engine")
    // No symbols or debug info in the packaged library: with them it is several times larger.
    // (Environment instead of a Cargo profile, so the desktop build is left as it is.)
    environment("CARGO_PROFILE_RELEASE_STRIP", "symbols")
    environment("CARGO_PROFILE_RELEASE_DEBUG", "false")
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a",
        "--platform", "26", // AAudio (libaaudio, used by cpal) exists only from API 26; cargo-ndk defaults to 21
        "-o", file("src/main/jniLibs").absolutePath, // -> jniLibs/arm64-v8a/libnvf_android.so
        "build", "--release", "-p", "nvf-android",
    )
}

// Build the Rust library before Android packages the APK.
tasks.named("preBuild") { dependsOn(cargoNdkBuild) }
