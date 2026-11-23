//! The NVForever desktop program. The game itself is the crate's library (`src/main.rs`,
//! upstream's file, whose `main` this calls through `odin::desktop`), so the Android activity
//! can start exactly the same code (`engine/nvf-android`, docs/fork.md). What the program does
//! with its command line, and the keys it reads, are in the library's own notes.

fn main() -> bevy::app::AppExit {
    nvforever::odin::desktop()
}
