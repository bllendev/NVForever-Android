// NVForever for Android (Milestone 1). Open THIS folder (android/) in Android Studio.
// The Rust side lives in ../engine/nvf-android; app/build.gradle.kts builds it with cargo-ndk.
pluginManagement {
    repositories {
        google()
        maven("https://maven-central.storage-download.googleapis.com/maven2/") // Google's mirror of Maven Central: CI runners get HTTP 429 from Central
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        maven("https://maven-central.storage-download.googleapis.com/maven2/") // Google's mirror of Maven Central: CI runners get HTTP 429 from Central
        mavenCentral()
    }
}
rootProject.name = "NVForever"
include(":app")
