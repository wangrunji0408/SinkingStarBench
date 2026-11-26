// Each world owns fixed guest addresses and starts in a fresh replay process.
use libtest_mimic::{Arguments, Failed, Trial};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn check(level: &str, route: &Path, expected_frames: Option<u64>) -> Result<(), Failed> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sinking-star"));
    command.args(["replay", level]);
    if level == "overworld" {
        command.arg("--instant-solve");
    }
    let mut child = command
        .arg("-i")
        .arg(route)
        .env("STAR_BATCH", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() >= Duration::from_secs(120) {
            let _ = child.kill();
            child.wait()?;
            return Err("replay exceeded 120 seconds".into());
        }
        thread::sleep(Duration::from_millis(15));
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "{}: {} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        )
        .into());
    }
    let result: Value = serde_json::from_slice(&output.stdout)?;
    if level == "overworld" {
        let expected: Vec<String> =
            serde_json::from_slice(&fs::read(route.with_extension("json"))?)?;
        if !expected.is_empty()
            && result["won"] == false
            && result["target_level"].is_null()
            && result["standing_level"].as_str() == expected.last().map(String::as_str)
            && result["visited"] == serde_json::json!(expected)
        {
            return Ok(());
        }
        return Err(format!("overworld visit sequence or endpoint differed: {result}").into());
    }
    if level == "intro" {
        if result["won"] == false && result["target_level"] == "overworld" {
            return Ok(());
        }
        return Err(format!("did not exit to overworld: {result}").into());
    }
    if result["won"] != true || !result["target_level"].is_null() {
        return Err(format!("did not win: {result}").into());
    }
    if result["frames"].as_u64() != expected_frames {
        return Err(
            format!("expected {expected_frames:?} frames in verified.json: {result}").into(),
        );
    }
    Ok(())
}

fn main() {
    let mut args = Arguments::from_args();
    args.test_threads
        .get_or_insert_with(|| thread::available_parallelism().map_or(1, |n| n.get().min(8)));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("solutions");
    let mut routes = fs::read_dir(root)
        .expect("missing solutions directory")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect::<Vec<_>>();
    routes.sort();
    assert!(!routes.is_empty(), "empty solutions directory");
    let verified: Value = serde_json::from_str(include_str!("../verified.json")).unwrap();
    let tests = routes
        .into_iter()
        .map(|route| {
            let level = route.file_stem().unwrap().to_str().unwrap().to_owned();
            let expected_frames = verified[&level].as_u64();
            Trial::test(level.clone(), move || {
                check(&level, &route, expected_frames)
            })
        })
        .collect();
    libtest_mimic::run(&args, tests).exit();
}
