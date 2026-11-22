"""Narrow checks for the live wrist World Map, without building the Bevy workspace.

Two stages, both plain `rustc --test` on checked-in engine source (no Cargo, no game
data, no copied engine logic):

1. `engine/nvf-play/src/wrist_live.rs` on its own: the module's own unit tests, the same
   ones `cargo test -p nvf-play` runs in the crate.
2. `projection.rs`: the same module driven through the engine's real World Map projection,
   extracted from `engine/nvf-play/src/pipboy/data/model.rs`, with the same glue
   implementation that `model.rs` carries. It pins the orientation the module assumes
   (east to the right, north up the picture) to the projection the paused page uses.

Use `--nocapture` to see the printed samples. `RUSTC` overrides the compiler;
`CLIPPY=1` runs `clippy-driver` over stage 1 as well (same lints as CI's `-D warnings`).
"""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

here = Path(__file__).resolve().parent
repo = here.parents[1]
module = repo / "engine/nvf-play/src/wrist_live.rs"
model = repo / "engine/nvf-play/src/pipboy/data/model.rs"

# Compile the engine's own projection, not a second implementation of its map maths.
# Fail loudly if the extraction boundaries moved instead of silently checking nothing.
text = model.read_text(encoding="utf-8")
cell = "pub const CELL: f32 = "
start = "/// A worldspace's map:"
end = "/// Where a map picture"
if any(text.count(marker) != 1 for marker in (cell, start, end)):
    raise SystemExit("WorldMap extraction boundaries changed; review model.rs")
projection = text[text.index(start) : text.index(end)]
if "impl WorldMap {" not in projection or "pub fn uv(" not in projection:
    raise SystemExit("Expected WorldMap::uv in the extracted model")
if "MapPicture for WorldMap" not in text:
    raise SystemExit("model.rs no longer implements wrist_live::MapPicture for WorldMap")
source = text[text.index(cell) :].splitlines()[0] + "\n" + projection

rustc = os.environ.get("RUSTC") or shutil.which("rustc") or str(Path.home() / ".cargo" / "bin" / ("rustc.exe" if os.name == "nt" else "rustc"))
scratch = Path(os.environ["LOCALAPPDATA"]) / "hermes/cache/scratch" if os.name == "nt" else repo
scratch.mkdir(parents=True, exist_ok=True)
failed = []
with tempfile.TemporaryDirectory(prefix="wrist-world-", dir=scratch) as tmp:
    out = Path(tmp)
    (out / "world_map.rs").write_text(source, encoding="utf-8")
    env = {
        **os.environ,
        "WRIST_WORLD_MAP_RS": str(out / "world_map.rs"),
        "TMP": str(out),
        "TEMP": str(out),
        "TMPDIR": str(out),
    }
    stages = [("engine module", module), ("engine module through the real projection", here / "projection.rs")]
    for name, entry in stages:
        binary = out / (f"{entry.stem}.exe" if os.name == "nt" else entry.stem)
        print(f"== {name}: {entry.relative_to(repo).as_posix()}", flush=True)
        build = subprocess.run([rustc, "--edition=2021", "--test", "-Dwarnings", str(entry), "-o", str(binary)], env=env, check=False)
        if build.returncode != 0:
            failed.append(f"{name} (build)")
            continue
        if subprocess.run([str(binary), *sys.argv[1:]], check=False).returncode != 0:
            failed.append(name)
    if os.environ.get("CLIPPY") == "1":
        clippy = os.environ.get("CLIPPY_DRIVER") or shutil.which("clippy-driver") or str(Path.home() / ".cargo" / "bin" / ("clippy-driver.exe" if os.name == "nt" else "clippy-driver"))
        print("== clippy-driver over the engine module", flush=True)
        # The component may be absent (rustup component add clippy). Say so instead of
        # reporting a lint failure the module did not cause.
        installed = subprocess.run([clippy, "-Vv"], env=env, check=False, capture_output=True)
        if installed.returncode != 0:
            print("skipped: clippy-driver is not installed (rustup component add clippy)", flush=True)
        else:
            lint = subprocess.run(
                [clippy, "--edition=2021", "--test", "--emit=metadata", "-Dwarnings", "-Dclippy::all", str(module), "-o", str(out / "lint.rmeta")],
                env=env,
                check=False,
            )
            if lint.returncode != 0:
                failed.append("clippy")
print("FAILED: " + ", ".join(failed) if failed else "all stages passed")
sys.exit(1 if failed else 0)
