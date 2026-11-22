// NVForever for Android (Milestone 1). Open THIS folder (android/) in Android Studio.
// The Rust side lives in ../engine/nvf-android; app/build.gradle.kts builds it with cargo-ndk.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "NVForever"
include(":app")
