use serde_json::Value as J;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn trace(level: &str, keys: &str, label: &str) -> (J, Vec<u8>, J) {
    let trace = env::temp_dir().join(format!("star-{}-{label}.bin", std::process::id()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
        .args(["replay", level, "--trace"])
        .arg(&trace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(keys.as_bytes())
        .unwrap();
    let run = child.wait_with_output().unwrap();
    assert!(run.status.success(), "{level}: {keys}");
    let result = serde_json::from_slice(&run.stdout).unwrap();
    let bytes = fs::read(&trace).unwrap();
    let sidecar = trace.with_extension("bin.json");
    let layout = serde_json::from_slice(&fs::read(&sidecar).unwrap()).unwrap();
    fs::remove_file(sidecar).unwrap();
    fs::remove_file(trace).unwrap();
    (result, bytes, layout)
}

#[test]
fn idle_traces_match_the_original_reference() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let load = |name: &str| -> J {
        let path = root.join("solving/results").join(name);
        let data = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_slice(&data).unwrap()
    };
    let hashes = load("original-steam-full-trace-hashes.json");
    let expected = load("original-rosetta-benchmark.json");
    // The historical solution hashes include waits that interrupt pending moves.
    // Idle routes still reproduce the original timing exactly.
    for (level, v) in expected["levels"].as_object().unwrap() {
        let n = v["idle"]["frames"].as_u64().unwrap();
        let (result, bytes, _) = trace(level, &format!("T{n}"), "idle-reference");
        assert_eq!(result["frames"], n);
        assert_eq!(result["won"], false);
        let digest = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(hashes[level]["idle"], digest, "{level}/idle");
    }
}

#[test]
fn trace_t0_records_completed_movement() {
    let (result, bytes, layout) = trace("heroes1_1", "RT0", "settled-reference");
    assert_eq!(layout["queue_counts"], serde_json::json!([1, 0]));
    assert_eq!(result["frames"], 9);
    let sizes = layout["ranges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|range| range[1].as_u64().unwrap() as usize)
        .sum::<usize>();
    let ticks = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    assert_eq!(ticks(28), 0);
    assert_eq!(ticks(28 + 12 + sizes), 9);
}

#[test]
fn replay_rejects_removed_wait_opcode() {
    let mut run = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
        .args(["replay", "heroes1_1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    run.stdin.take().unwrap().write_all(b"P1").unwrap();
    let output = run.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let result: J = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"], "invalid key 0x50 at byte 0");
}
