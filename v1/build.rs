use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let levels_dir = manifest_dir.join("levels");
    println!("cargo:rerun-if-changed={}", levels_dir.display());

    let mut files: Vec<(u32, u32, String, PathBuf)> = fs::read_dir(&levels_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", levels_dir.display()))
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if path.extension().and_then(|value| value.to_str()) != Some("txt") {
                return None;
            }
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let (chapter, level) = parse_level_name(&name).unwrap_or_else(|| {
                panic!("level file name must be <digit>-<digit>.txt: {}", path.display())
            });
            Some((chapter, level, name, path))
        })
        .collect();
    files.sort_by_key(|(chapter, level, _, _)| (*chapter, *level));
    assert!(!files.is_empty(), "no .txt level files found in levels/");

    let mut generated = String::from("const EMBEDDED_LEVEL_SOURCES: &[(&str, &str)] = &[\n");
    for (_, _, name, path) in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let path = absolute(&path);
        writeln!(
            generated,
            "    ({name:?}, include_str!({path:?})),",
            path = path.to_string_lossy()
        )
        .unwrap();
    }
    generated.push_str("];\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("embedded_levels.rs");
    fs::write(&output, generated)
        .unwrap_or_else(|error| panic!("failed to generate {}: {error}", output.display()));
}

fn parse_level_name(name: &str) -> Option<(u32, u32)> {
    let (chapter, level) = name.split_once('-')?;
    Some((chapter.parse().ok()?, level.parse().ok()?))
}

fn absolute(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|error| panic!("failed to resolve {}: {error}", path.display()))
}
