//! Packs the schemas (`schemas/` at the repository root: record, NIF and script-function
//! descriptions the game reads at start-up) into the library, so the APK needs no separate
//! asset step. `src/paths.rs` writes them into the app's folder on the first start.
//!
//! Generates `$OUT_DIR/schemas_embedded.rs`: `pub static FILES: &[(&str, &[u8])]`, each entry
//! the path below `schemas/` (with `/`) and the file's bytes.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            collect(&p, root, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            // Only the JSON the game reads; the READMEs stay out.
            let rel = p.strip_prefix(root).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            out.push((rel, p));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let schemas = manifest.join("..").join("..").join("schemas");
    println!("cargo:rerun-if-changed={}", schemas.display());
    println!("cargo:rerun-if-changed=build.rs");
    let mut files = Vec::new();
    collect(&schemas, &schemas, &mut files);
    assert!(!files.is_empty(), "no schema files found in {}", schemas.display());
    let mut code = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (rel, path) in &files {
        let abs = path.clone();
        // `{:?}` quotes and escapes the path (backslashes on Windows) as a Rust string.
        let _ = writeln!(code, "    ({rel:?}, include_bytes!({:?})),", abs.display().to_string());
        println!("cargo:rerun-if-changed={}", path.display());
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("schemas_embedded.rs");
    std::fs::write(out, code).expect("writing schemas_embedded.rs");
}
