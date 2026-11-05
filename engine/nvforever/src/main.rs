//! The NVForever desktop program. The game itself is the `nvforever` library beside this
//! file (`src/lib.rs`), so the Android activity can start exactly the same code
//! (`engine/nvf-android`); this binary only hands it the command line. What the program
//! does with that line, and the keys it reads, are in the library's own notes.

fn main() -> bevy::app::AppExit {
    nvforever::run(std::env::args().collect())
}
