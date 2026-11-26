// Run explicitly on a Mac with D3DMetal, Rosetta (on Apple silicon) and a GPU.
// The exported binary must not rely on the source tree or programs on PATH.
use std::{env, fs, path::PathBuf, process::Command};
#[test]
#[ignore = "requires a working D3DMetal installation and HEVC encoder"]
fn export_from_empty_directory() {
    let dir = env::temp_dir().join(format!("renderer-portable-test-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    struct Clean(PathBuf);
    impl Drop for Clean {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _clean = Clean(dir.clone());
    let binary = dir.join("sinking-star");
    fs::copy(env!("CARGO_BIN_EXE_sinking-star"), &binary).unwrap();
    fs::write(
        dir.join("route.txt"),
        include_bytes!("../solutions/heroes1_1.txt"),
    )
    .unwrap();
    let output = Command::new(binary)
        .current_dir(&dir)
        .env("PATH", "/nonexistent")
        .args([
            "render",
            "heroes1_1",
            "-i",
            "route.txt",
            "--resolution",
            "854x480",
            "--fps",
            "20",
            "-o",
            "test.mp4",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["won"], true);
    assert_eq!(result["frames"], 44);
    assert_eq!(result["ticks"], 106);
    assert_eq!(result["fps"], 20);
    assert_eq!(result["width"], 854);
    assert_eq!(result["height"], 480);
    assert_eq!(result["render_ticks"], 127);
    assert_eq!(result["settled"], true);
    assert_eq!(
        result,
        serde_json::from_slice::<serde_json::Value>(&fs::read(dir.join("test.json")).unwrap())
            .unwrap()
    );
    assert!(fs::metadata(dir.join("test.mp4")).unwrap().len() > 1000);
}
