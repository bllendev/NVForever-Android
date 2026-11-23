# The APK's one piece of C/C++: libadrenotools and its hook libraries, for the launcher's
# optional "GPU driver" choice (a Mesa Turnip build the player supplies; docs/odin-debug.md).
# The game itself is Rust, built by cargo-ndk, and is not touched by any of this.
#
# libadrenotools (BSD-2-Clause, bylaws/libadrenotools) and its liblinkernsbypass submodule
# (BSD-2-Clause, bylaws/liblinkernsbypass) are NOT committed here: the Odin workflow downloads
# both at pinned commits, checks their hashes and unpacks them into `libadrenotools/` beside
# this file (.github/workflows/odin-android.yml). Without that step the folder is empty, Gradle
# adds no native build at all (app/build.gradle.kts), and the launcher simply finds no
# libadrenotools and says so.
cmake_minimum_required(VERSION 3.22.1)

project(nvforever_native LANGUAGES C CXX)

set(ADRENOTOOLS_DIR "${CMAKE_CURRENT_SOURCE_DIR}/libadrenotools")

if(NOT EXISTS "${ADRENOTOOLS_DIR}/CMakeLists.txt")
    message(FATAL_ERROR "libadrenotools sources are missing from ${ADRENOTOOLS_DIR}. \
Gradle only configures this build when they are there, so reaching this means the download \
step ran but left the folder incomplete.")
endif()
if(NOT EXISTS "${ADRENOTOOLS_DIR}/lib/linkernsbypass/CMakeLists.txt")
    message(FATAL_ERROR "libadrenotools' liblinkernsbypass submodule is missing from \
${ADRENOTOOLS_DIR}/lib/linkernsbypass; libadrenotools cannot build without it.")
endif()

# libadrenotools' own CMakeLists reads this (`add_library(adrenotools ${LIB_SOURCES})` and an
# `if (${BUILD_SHARED_LIBS})` that would be an error with the variable undefined). Shared,
# because the game dlopens libadrenotools.so by name at startup rather than linking it: the
# Rust build stays exactly as it was. liblinkernsbypass declares itself STATIC, so it is still
# linked into libadrenotools.so and is not packaged on its own.
set(BUILD_SHARED_LIBS ON CACHE BOOL "" FORCE)

# -> libadrenotools.so, libhook_impl.so, libmain_hook.so, libfile_redirect_hook.so and
# libgsl_alloc_hook.so, all of which Gradle packages into lib/arm64-v8a/ of the APK. The hooks
# must be there as files, which is why the APK keeps useLegacyPackaging = true.
add_subdirectory("${ADRENOTOOLS_DIR}" adrenotools)
