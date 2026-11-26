use serde_json::{Value as J, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub type Res<T> = Result<T, Box<dyn std::error::Error>>;
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new() -> Res<Self> {
        let p = std::env::temp_dir().join(format!(
            "star-film-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
/// Milliseconds spent inside the native worker; these intervals are nested
/// inside the parent draw/submit timings and must not be added to them.
#[derive(Default)]
pub struct WorkerTimings(std::collections::BTreeMap<String, (usize, f64)>);
impl WorkerTimings {
    pub fn record(&mut self, worker: &Worker) -> std::io::Result<()> {
        let reader = BufReader::new(fs::File::open(&worker.log)?);
        for line in reader.split(b'\n') {
            let line = line?;
            let Some(fields) = line
                .strip_prefix(b"SceneProfile ")
                .or_else(|| line.strip_prefix(b"FrameProfile "))
            else {
                continue;
            };
            if let Ok(fields) = std::str::from_utf8(fields) {
                for field in fields.split_ascii_whitespace() {
                    if let Some((name, value)) = field.split_once('=') {
                        if let Ok(ms) = value.parse::<f64>() {
                            let row = self.0.entry(name.to_owned()).or_default();
                            row.0 += 1;
                            row.1 += ms;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn mean_ms(&self) -> J {
        self.0
            .iter()
            .map(|(name, (n, total))| (name.clone(), json!(total / *n as f64)))
            .collect::<serde_json::Map<_, _>>()
            .into()
    }
}
pub struct Worker {
    child: Child,
    input: ChildStdin,
    output: Receiver<std::io::Result<String>>,
    log: PathBuf,
    failure_log: PathBuf,
}
impl Worker {
    pub fn spawn(c: &mut Command, dir: &Path, out: &Path) -> Res<Self> {
        let log = dir.join("worker.log");
        let mut child = c
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(fs::File::create(&log)?)
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        let (tx, output) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            input: child.stdin.take().unwrap(),
            output,
            child,
            log,
            failure_log: out.with_extension("worker.log"),
        })
    }
    fn failed(&mut self, cause: impl std::fmt::Display) -> Box<dyn std::error::Error> {
        // Stop a stalled worker before taking the final copy of its stderr.
        let _ = self.child.kill();
        let status = self.child.wait();
        let detail = match fs::copy(&self.log, &self.failure_log) {
            Ok(_) => format!("see {}", self.failure_log.display()),
            Err(e) => format!("could not save worker log: {e}"),
        };
        format!(
            "GPU worker: {cause}; {}; {detail}",
            status.map_or_else(|e| e.to_string(), |s| s.to_string())
        )
        .into()
    }
    pub fn send(&mut self, s: &str) -> Res<()> {
        writeln!(self.input, "{s}")
            .and_then(|()| self.input.flush())
            .map_err(|e| self.failed(e))
    }
    pub fn until(&mut self, marker: &str) -> Res<()> {
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        loop {
            match self
                .output
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            {
                Ok(Ok(line)) if line == marker => return Ok(()),
                Ok(Ok(_)) => (),
                Ok(Err(e)) => return Err(self.failed(e)),
                Err(e) => return Err(self.failed(format!("waiting for {marker}: {e}"))),
            }
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub fn put_f(b: &mut Vec<u8>, f: f64) {
    b.extend((f as f32).to_le_bytes())
}
pub fn put_u(b: &mut Vec<u8>, u: u64) {
    b.extend((u as u32).to_le_bytes())
}
pub fn val(v: &J) -> f64 {
    v.as_f64().unwrap_or(0.)
}
pub fn num(v: &J) -> u64 {
    v.as_u64().unwrap_or(0)
}
pub fn vecj(v: &J) -> &[J] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn b64(s: &str) -> Vec<u8> {
    let (mut out, mut buf, mut bits) = (Vec::new(), 0u32, 0);
    for c in s.bytes() {
        let n = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => panic!("bad base64"),
        };
        buf = (buf << 6) | n as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8)
        }
    }
    out
}
pub fn atom(p: &Path, j: &J) -> Res<()> {
    let tmp = p.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp, serde_json::to_vec(j)?)?;
    fs::rename(tmp, p)?;
    Ok(())
}
pub fn source() -> J {
    json!({"backend":"rust-renderer","game_build":env!("STAR_GAME_BUILD"),"source_sha256":env!("STAR_SOURCE_SHA256")})
}
