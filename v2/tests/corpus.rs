use serde_json::{Value as J, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
const EXE: &str = env!("CARGO_BIN_EXE_sinking-star");

fn corpus(root: &Path, path: &str) -> J {
    let path = root.join(path);
    serde_json::from_reader(BufReader::new(
        fs::File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    ))
    .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
#[ignore = "replays the full repository corpus and writes a report"]
fn generate_report() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap();
    let path = env::var_os("STAR_CORPUS_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("target/original-rust-key-verification.json"));
    let jobs = env::var_os("STAR_CORPUS_WORKERS").map_or(4, |n| {
        n.to_str()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| (1..=16).contains(n))
            .expect("STAR_CORPUS_WORKERS must be an integer between 1 and 16")
    });
    let mut cases = vec![];
    let verified = corpus(root, "replay/verified.json");
    let published = corpus(root, "solving/results/wasm-solutions.json");
    for (k, v) in published["solutions"].as_object().unwrap() {
        assert!(v["verified"] == true);
        cases.push((
            "published",
            k.clone(),
            0,
            fs::read(root.join(format!("replay/solutions/{k}.txt"))).unwrap(),
            None,
        ));
    }
    let certified = corpus(root, "solving/results/original-x64-remaining.json");
    for (k, v) in certified["results"].as_object().unwrap() {
        if v["win"] == true {
            cases.push((
                "certified",
                k.clone(),
                0,
                fs::read(root.join(format!("replay/solutions/{k}.txt"))).unwrap(),
                verified[k].as_u64(),
            ));
        }
    }
    let known: BTreeSet<_> = cases.iter().map(|(_, k, ..)| k.clone()).collect();
    for (k, v) in verified.as_object().unwrap() {
        if !known.contains(k) {
            cases.push((
                "recovered",
                k.clone(),
                0,
                fs::read(root.join(format!("replay/solutions/{k}.txt"))).unwrap(),
                Some(v.as_u64().expect("verified frame count")),
            ));
        }
    }
    let ix = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results = Mutex::new(BTreeMap::new());
    if let Some(p) = path.parent() {
        if !p.as_os_str().is_empty() {
            fs::create_dir_all(p).unwrap();
        }
    }
    let journal = Mutex::new(fs::File::create(path.with_extension("jsonl")).unwrap());
    eprintln!("Testing {} routes, {} workers", cases.len(), jobs);
    thread::scope(|scope| {
        for _ in 0..jobs {
            scope.spawn(|| loop {
                let i = ix.fetch_add(1, Ordering::Relaxed);
                let Some((group,key,variant,data,expected)) = cases.get(i) else { break };
                let mut child = Command::new(EXE)
                    .args(["replay", key.as_str()])
                    .env("STAR_BATCH","1").stdin(Stdio::piped())
                    .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
                let mut input = child.stdin.take().unwrap();
                // A failing initializer may have already closed its end of the pipe.
                let _ = input.write_all(data);
                drop(input);
                let start = Instant::now();
                let mut timeout = false;
                while child.try_wait().unwrap().is_none() {
                    if start.elapsed() > Duration::from_secs(120) {
                        let _ = child.kill();
                        timeout = true;
                        break;
                    }
                    thread::sleep(Duration::from_millis(15));
                }
                let output = child.wait_with_output().unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let result: J = serde_json::from_str(&stdout).unwrap_or(J::Null);
                let frames=result["frames"].as_u64();
                let win=result["won"]==true;
                let status = if timeout { "timeout" }
                    else if !output.status.success() || frames.is_none() {
                        "error"
                    } else if !win { "no-victory" }
                    else if expected.is_some() && *expected != frames { "frame-mismatch" }
                    else { "pass" };
                let mut record = json!({"source":group,"level":key,"variant":variant,"mode":"full","status":status,"frames":frames});
                if let Some(n)=expected { record["expected_frames"]=json!(n); }
                if status=="error" {
                    record["detail"]=json!(format!("{} {}",output.status,stderr.trim()).chars().take(450).collect::<String>());
                }
                writeln!(journal.lock().unwrap(),"{record}").unwrap();
                results.lock().unwrap().insert(i,record);
                let n=done.fetch_add(1,Ordering::Relaxed)+1;
                if n.is_multiple_of(100) { eprintln!("{n}/{}",cases.len()); }
            });
        }
    });
    let results = results
        .into_inner()
        .unwrap()
        .into_values()
        .collect::<Vec<_>>();
    let mut totals: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for v in &results {
        *totals
            .entry(v["source"].as_str().unwrap().into())
            .or_default()
            .entry(v["status"].as_str().unwrap().into())
            .or_default() += 1;
    }
    let report = json!({"source_sha256":env!("STAR_SOURCE_SHA256"),"checked":results.len(),"totals":totals,"results":results});
    fs::write(&path, serde_json::to_string_pretty(&report).unwrap() + "\n").unwrap();
    println!("{}", report["totals"]);
}
