// Milestone 2: the whole game. The app module: packages libnvf_android.so (built from Rust by cargo-ndk) into an APK that
// runs it through a NativeActivity (GameActivity.java). The only Java is the launcher (folder permission and picker).
plugins {
    id("com.android.application")
}

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

    buildTypes {
        // Both build types package the RELEASE Rust library (see cargoNdkBuild): Bevy compiled
        // without optimisation runs at a few frames per second, which tests nothing useful.
        // The "debug" APK is still debuggable and signed with the debug key.
        release {
            isMinifyEnabled = false // only two tiny Java classes
            signingConfig = signingConfigs.getByName("debug") // M1: sideload only, no store key yet
        }
    }

    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")
}

// ---------------------------------------------------------------------------------------
// Rust build. Needs (once): `rustup target add aarch64-linux-android`, `cargo install
// cargo-ndk`, and the Android NDK (Android Studio: Settings > SDK Manager > SDK Tools >
// "NDK (Side by side)"). cargo-ndk finds the NDK through ANDROID_NDK_HOME, or through the
// ANDROID_HOME/ndk folder. Details: docs/android/milestone-2.md.
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
