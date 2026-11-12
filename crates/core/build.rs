//! Builds the shipped `presets/*.txt` into the library, for builds without a filesystem (WASM).

use std::fmt::Write;
use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../presets");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect())
        .unwrap_or_default();
    files.sort();
    let mut out = String::from("pub static PRESETS: &[(&str, &str)] = &[\n");
    for f in files {
        println!("cargo:rerun-if-changed={}", f.display());
        let id = f.file_stem().unwrap().to_string_lossy();
        // same rule as `presets::valid_id`
        if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            continue;
        }
        let path = f.canonicalize().unwrap();
        writeln!(out, "    ({id:?}, include_str!({:?})),", path.display().to_string()).unwrap();
    }
    out.push_str("];\n");
    std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded_presets.rs"), out).unwrap();
}
