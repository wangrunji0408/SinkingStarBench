use crate::cli::{DEFAULT_FPS, Dimensions, GAME_FPS, Render};
mod assets;
mod frame_map;
mod image;
pub mod io;
mod video;
use video::probe;
mod poses;
mod snapshot;
use self::io::*;
use serde_json::{Value as J, json};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};
use std::{
    io::Write,
    process::{Child, ChildStdout, Stdio},
    sync::{Arc, Mutex},
};

struct Trace(Child);
impl Trace {
    fn reader(&mut self) -> ChildStdout {
        self.0.stdout.take().unwrap()
    }
    fn wait(&mut self) -> std::io::Result<bool> {
        Ok(self.0.wait()?.success())
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Partial(PathBuf);
impl Drop for Partial {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn d3dmetal_library(path: &Path) -> Option<PathBuf> {
    [
        path.to_owned(),
        path.join("D3DMetal"),
        path.join("Versions/A/D3DMetal"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

fn runtime() -> Res<PathBuf> {
    for name in ["D3DMETAL_PATH", "CX_D3DM"] {
        if let Some(path) = env::var_os(name).filter(|p| !p.is_empty()) {
            let path = PathBuf::from(path);
            let library = d3dmetal_library(&path).ok_or_else(|| {
                format!(
                    "{name} does not point to a D3DMetal framework or library: {}",
                    path.display()
                )
            })?;
            return Ok(library.canonicalize()?);
        }
    }

    let apps = std::iter::once(PathBuf::from("/Applications")).chain(
        env::var_os("HOME")
            .filter(|p| !p.is_empty())
            .map(|p| PathBuf::from(p).join("Applications")),
    );
    for dir in apps {
        let gptk = dir.join("CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk");
        for relative in ["external/D3DMetal.framework", "D3DMetal.framework"] {
            if let Some(library) = d3dmetal_library(&gptk.join(relative)) {
                return Ok(library.canonicalize()?);
            }
        }
    }
    if let Some(library) = d3dmetal_library(Path::new("/Library/Apple/usr/lib/D3DMetal.framework"))
    {
        return Ok(library.canonicalize()?);
    }
    Err("D3DMetal was not found in /Applications/CrossOver.app, ~/Applications/CrossOver.app, or /Library/Apple/usr/lib; set D3DMETAL_PATH to its framework or library".into())
}
fn game() -> Res<PathBuf> {
    let h = PathBuf::from(env::var_os("HOME").unwrap_or_default());
    let p = if let Some(p) = env::var_os("GAME_ROOT") {
        PathBuf::from(p)
    } else {
        [
            "Library/Application Support/Steam",
            "Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam",
        ]
        .iter()
        .map(|s| h.join(s).join("steamapps/common/Order of the Sinking Star"))
        .find(|p| p.join("sinking_star.exe").is_file())
        .ok_or("set GAME_ROOT to the Steam install")?
    };
    for s in ["sinking_star.exe", "data/animation.package", "data-pc"] {
        if !p.join(s).exists() {
            return Err(format!("GAME_ROOT missing {s}: {}", p.display()).into());
        }
    }
    if format!(
        "{:x}",
        Sha256::digest(fs::read(p.join("sinking_star.exe"))?)
    ) != env!("STAR_SOURCE_SHA256")
    {
        return Err(
            "GAME_ROOT executable differs from this renderer; rebuild for the installed version"
                .into(),
        );
    }
    Ok(p.canonicalize()?)
}
fn trace(level: &str, ops: &str, mode: &str, fps: Option<u32>) -> Res<Trace> {
    let mut command = Command::new(env::current_exe()?);
    command.args(["render", level, mode]);
    if crate::overworld::enabled() {
        command.arg("--instant-solve");
    }
    if let Some(fps) = fps {
        command.args(["--fps", &fps.to_string()]);
    }
    let mut c = Trace(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
    );
    c.0.stdin.take().unwrap().write_all(ops.as_bytes())?;
    Ok(c)
}
fn log(p: &Path) -> Res<fs::File> {
    Ok(fs::OpenOptions::new().create(true).append(true).open(p)?)
}
fn lock(p: &Path, nonblock: bool) -> Res<fs::File> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    let f = log(p)?;
    if unsafe { flock(f.as_raw_fd(), if nonblock { 6 } else { 2 }) } != 0 {
        return Err(format!(
            "cannot lock {}: {}",
            p.display(),
            std::io::Error::last_os_error()
        )
        .into());
    }
    Ok(f)
}
fn install_worker(dir: &Path) -> Res<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gpu-worker"));
    let local = dir.join("gpu-worker");
    let install = || -> Res<()> {
        fs::write(&local, IMAGE)?;
        fs::set_permissions(&local, fs::Permissions::from_mode(0o700))?;
        Ok(())
    };

    // A stable executable path lets Metal reuse compiled pipelines between
    // independent CLI invocations. Only share the executable, never scene data.
    let cached = || -> Res<PathBuf> {
        unsafe extern "C" {
            fn geteuid() -> u32;
        }
        let uid = unsafe { geteuid() };
        let private_dir = |p: &Path| -> Res<()> {
            let m = fs::symlink_metadata(p)?;
            if !m.is_dir() || m.uid() != uid || m.mode() & 0o022 != 0 {
                return Err("not a private user cache".into());
            }
            Ok(())
        };
        let parent = dir.parent().ok_or("missing temporary directory")?;
        private_dir(parent)?;
        let cache = parent.join(format!("sinking-star-gpu-{:x}", Sha256::digest(IMAGE)));
        match fs::DirBuilder::new().mode(0o700).create(&cache) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            other => other?,
        }
        private_dir(&cache)?;
        let path = cache.join("gpu-worker");
        if !path.try_exists()? {
            install()?;
            // Publish only a complete executable. Concurrent renders can race
            // here; both publish the same bytes, and neither replaces a runner.
            match fs::hard_link(&local, &path) {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                other => other?,
            }
        }
        let m = fs::symlink_metadata(&path)?;
        if !m.is_file()
            || m.uid() != uid
            || m.mode() & 0o7777 != 0o700
            || m.len() != IMAGE.len() as u64
            || fs::read(&path)? != IMAGE
        {
            return Err("invalid cached GPU worker".into());
        }
        Ok(path)
    };
    if let Ok(path) = cached() {
        return Ok(path);
    }
    install()?;
    Ok(local)
}
fn start_worker(
    host: &Path,
    dir: &Path,
    level: &str,
    runtime: &Path,
    game: &Path,
    resources: &J,
    out: &Path,
    size: Dimensions,
) -> Res<Worker> {
    let mut g = Worker::spawn(
        Command::new(host)
            .current_dir(dir)
            .arg(game.join("sinking_star.exe"))
            .arg(dir.join("shaders.tsv"))
            .arg(runtime)
            .arg(&resources["textures"].as_str().ok_or("no textures")?)
            .arg(dir.join("materials.bin"))
            .arg(resources["meshes"].as_str().ok_or("no meshes")?)
            .arg(dir.join("world.bin"))
            .env(
                "DYLD_LIBRARY_PATH",
                runtime.parent().unwrap().join("Resources"),
            )
            .env("MTL_SHADER_VALIDATION", "0")
            .env("MTL_DEBUG_LAYER", "0")
            .env("GAME_ROOT", game)
            .env("SINKING_VALIDATE", "0")
            .env("SINKING_SERVE", "1")
            .env("SINKING_MOVIE", "1")
            .env("SINKING_RENDER_FRAME", "1")
            .env("SINKING_RENDER_WIDTH", size.width.to_string())
            .env("SINKING_RENDER_HEIGHT", size.height.to_string())
            .env("SINKING_WARM_FRAMES", "auto")
            .env("SINKING_LEVEL", level)
            .env("SINKING_WORLD", dir.join("world.bin"))
            .env(
                "SINKING_TEXTURE_CATALOG",
                resources["textures"].as_str().ok_or("no textures")?,
            )
            .env(
                "SINKING_MESH_CATALOG",
                resources["meshes"].as_str().ok_or("no meshes")?,
            ),
        dir,
        out,
    )?;
    g.until("READY")?;
    Ok(g)
}
fn draw(g: &mut Worker, dir: &Path, output: &Path) -> Res<()> {
    g.send(&format!(
        "{}\t{}",
        dir.join("world.snapshot").display(),
        output.display()
    ))
}
fn key_trace(level: &str, ops: &str, mode: &str, expected: Option<u64>) -> Res<J> {
    let mut c = trace(level, ops, mode, None)?;
    let t: J = serde_json::from_reader(c.reader())?;
    if !c.wait()? || expected.is_some_and(|x| t["won"] != true || t["ticks"] != x) {
        return Err("key trace failed victory check".into());
    }
    Ok(t)
}
fn movie(
    level: &str,
    ops: &str,
    out: &Path,
    expected: Option<u64>,
    runtime: &Path,
    game: &Path,
    size: Dimensions,
    fps: u32,
    quality: u8,
) -> Res<J> {
    let step = u64::from(GAME_FPS / fps);
    let fps64 = u64::from(fps);
    crate::routes::parse(ops.as_bytes())?;
    let _output_lock = lock(&out.with_extension("render.lock"), false)?;
    let start = Instant::now();
    let profile = env::var_os("SINKING_FRAME_PROFILE").is_some();
    let mut phases = std::collections::BTreeMap::<&str, f64>::new();
    let mut mark = start;
    let mut lap = |phase| {
        if profile {
            let now = Instant::now();
            *phases.entry(phase).or_default() += (now - mark).as_secs_f64();
            mark = now;
        }
    };
    let mut starts = Vec::new();
    let mut worker_timings = WorkerTimings::default();
    let identity = source();
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            [
                fs::read(env::current_exe()?)?.as_slice(),
                level.as_bytes(),
                ops.as_bytes(),
                game.as_os_str().as_encoded_bytes(),
                &size.width.to_le_bytes(),
                &size.height.to_le_bytes(),
                &[crate::overworld::enabled() as u8],
                &fps.to_le_bytes(),
                &[quality]
            ]
            .concat()
        )
    );
    let meta = out.with_extension("json");
    if let Ok(b) = fs::read(&meta) {
        if let Ok(j) = serde_json::from_slice::<J>(&b) {
            if !profile
                && j["fingerprint"] == fingerprint
                && expected.is_none_or(|t| j["won"] == true && j["ticks"] == t)
                && probe(out, size, fps).ok() == j["frames"].as_u64()
            {
                return Ok(j);
            }
        }
    }
    let part = Partial(out.with_extension("partial.mp4"));
    if part.0.exists() {
        fs::remove_file(&part.0)?;
    }
    let tmp = Temp::new()?;
    let d = &tmp.0;
    let t = key_trace(level, ops, "--movie-keys", expected)?;
    let mut c = trace(level, ops, "--movie-stream", Some(fps))?;
    let mut lines = BufReader::new(c.reader()).lines();
    let h: J = serde_json::from_str(&lines.next().ok_or("missing header")??)?;
    lap("trace_startup");
    let mut assets = assets::Assets::new(game, d, level)?;
    if level == "overworld" {
        assets.prepare_overworld_scenery()?;
    }
    let setup = assets.setup(&h["initial"], &t)?;
    let mut scene = snapshot::Scene::new(&h["initial"], &setup);
    let mut poses = poses::Poses::default();
    lap("assets_setup");
    // While the worker draws into one slot, the parent submits the previous
    // slot to AVFoundation. Only DONE hands a new slot back to the parent.
    let paths = [d.join("even.rgba"), d.join("odd.rgba")];
    let slots = [
        frame_map::FrameMap::new(&paths[0], size)?,
        frame_map::FrameMap::new(&paths[1], size)?,
    ];
    let ticks = num(&t["ticks"]);
    let tail = num(&t["render_ticks"]).saturating_sub(ticks);
    let clip_frames = 1 + ticks.div_ceil(step) + tail.div_ceil(step);
    let host = install_worker(d)?;
    let mut enc = video::Video::open(&part.0, &t, num(&t["view"]) as u8, size, fps, quality)?;
    let (mut frames, mut gpu, mut resources, mut end) = (0u64, None, J::Null, J::Null);
    lap("encoder_setup");
    for line in lines {
        let f: J = serde_json::from_str(&line?)?;
        if f["end"] == true {
            end = f;
            break;
        }
        lap("trace_frames");
        let r = scene.update(d, &mut assets, &f)?;
        // Avoid restarting the worker in the final second of the clip.
        // Topology changes always get a fresh worker.
        let recycle = starts
            .last()
            .is_some_and(|last| frames - last >= 60 * fps64)
            && clip_frames.saturating_sub(frames) > fps64;
        let restart = r.is_some() || gpu.is_none() || recycle;
        if let Some(r) = r {
            resources = r
        }
        lap("scene_updates");
        if restart {
            starts.push(frames);
            if profile {
                if let Some(g) = gpu.as_ref() {
                    worker_timings.record(g)?;
                }
            }
            drop(gpu.take());
            let g = start_worker(&host, d, level, runtime, game, &resources, out, size)
                .map_err(|e| format!("{level} frame {frames}: {e}"))?;
            gpu = Some(g);
        }
        lap("gpu_startup");
        poses.write(&mut assets, d, &f, &scene.entities)?;
        lap("poses");
        let g = gpu.as_mut().unwrap();
        let slot = (frames % 2) as usize;
        draw(g, d, &paths[slot]).map_err(|e| format!("{level} frame {frames}: {e}"))?;
        if frames > 0 {
            enc.push(slots[slot ^ 1].bytes())?;
        }
        lap("submit_and_encode_previous");
        g.until("DONE")
            .map_err(|e| format!("{level} frame {frames}: {e}"))?;
        lap("draw_wait_remaining");
        frames += 1;
    }
    if frames > 0 {
        enc.push(slots[((frames - 1) % 2) as usize].bytes())?;
    }
    if profile {
        if let Some(g) = gpu.as_ref() {
            worker_timings.record(g)?;
        }
    }
    drop(gpu);
    enc.finish()?;
    let trace_ok = c.wait()?;
    let video_frames = probe(&part.0, size, fps)?;
    if !trace_ok
        || end["frames"] != frames
        || end["won"] != t["won"]
        || end["target_level"] != t["target_level"]
        || end["ticks"] != t["ticks"]
        || end["render_ticks"] != t["render_ticks"]
        || end["standing_level"] != t["standing_level"]
        || end["visited"] != t["visited"]
        || end["settled"] != t["settled"]
        || expected.is_some_and(|_| frames != clip_frames)
        || video_frames != frames
    {
        return Err(format!("incomplete replay or video for {level}: trace_ok={trace_ok}, end={end}, expected ticks={} won={}, rendered frames={frames}, video frames={video_frames}", t["ticks"], t["won"]).into());
    }
    lap("finish_validate");
    let mut j = json!({"level":level,"ops":ops,"engine":identity,"fingerprint":fingerprint,"frames":frames,"ticks":end["ticks"],"render_ticks":end["render_ticks"],"won":end["won"],"target_level":end["target_level"],"settled":end["settled"],"width":size.width,"height":size.height,"fps":fps,"encoding":{"quality":quality},"seconds":frames as f64/f64::from(fps),"codec":"hevc","render_seconds":start.elapsed().as_secs_f64()});
    for key in ["standing_level", "visited"] {
        if let Some(value) = end.get(key) {
            j[key] = value.clone();
        }
    }
    if profile {
        j["profile"] = json!({"seconds":phases,"gpu_start_frames":starts,"worker_mean_ms":worker_timings.mean_ms()});
    }
    fs::rename(&part.0, out)?;
    atom(&meta, &j)?;
    Ok(j)
}
fn batch_inputs(paths: Vec<PathBuf>) -> Res<Vec<(String, PathBuf, Option<u64>)>> {
    let verified: std::collections::BTreeMap<String, u64> =
        if let Some(path) = env::var_os("VERIFIED_JSON") {
            let path = PathBuf::from(path);
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?
        } else {
            Default::default()
        };
    let cwd = env::current_dir()?;
    let mut levels = std::collections::BTreeSet::new();
    paths
        .into_iter()
        .map(|path| {
            let level = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| format!("cannot determine scene ID from {}", path.display()))?;
            if !crate::has_scene(level) {
                return Err(format!("unknown scene {level:?} from {}", path.display()).into());
            }
            if !path.is_file() {
                return Err(format!("not a solution file: {}", path.display()).into());
            }
            if !levels.insert(level.to_owned()) {
                return Err(format!("multiple input files for scene {level:?}").into());
            }
            Ok((
                level.to_owned(),
                cwd.join(&path),
                verified.get(level).copied(),
            ))
        })
        .collect()
}
pub fn run(args: Render) -> Res<()> {
    let output = args.output;
    let size = args.resolution.unwrap_or_default();
    let fps = args.fps.unwrap_or(DEFAULT_FPS);
    let quality = args.quality.unwrap_or(92);
    let format = output.as_deref().and_then(image::Format::from_path);
    if format.is_some() && args.quality.is_some() {
        return Err("--quality only applies to video output".into());
    }
    if format.is_some() && args.fps.is_some() {
        return Err("--fps only applies to video output".into());
    }
    if format.is_none() && (!size.width.is_multiple_of(2) || !size.height.is_multiple_of(2)) {
        return Err("HEVC video requires even width and height".into());
    }
    if !args.inputs.is_empty() {
        let mut all = batch_inputs(args.inputs)?;
        game()?;
        runtime()?;
        let dest = args.output_dir.unwrap();
        let jobs = args.jobs;
        fs::create_dir_all(&dest)?;
        let _batch_lock = lock(&dest.join(".batch.lock"), true)?;
        all.sort_by(|(l, _, t), (r, _, u)| {
            let order = |level: &str, ticks: Option<u64>| {
                (
                    !dest.join(format!("{level}.mp4")).exists(),
                    ticks.unwrap_or(0),
                )
            };
            order(l, *t).cmp(&order(r, *u)).then_with(|| l.cmp(r))
        });
        let count = all.len();
        all.reverse();
        let queue = Arc::new(Mutex::new(all));
        let index = Mutex::new(
            json!({"engine":source(),"width":size.width,"height":size.height,"fps":fps,"encoding":{"quality":quality},"total":count,"completed":{},"failed":{}}),
        );
        std::thread::scope(|s| {
            for _ in 0..jobs {
                let q = &queue;
                let dest = &dest;
                let index = &index;
                s.spawn(move || {
                    loop {
                        let Some((lv, route, expected)) = q.lock().unwrap().pop() else {
                            break;
                        };
                        let p = dest.join(format!("{lv}.mp4"));
                        let mut child = Command::new(env::current_exe().unwrap());
                        child
                            .args(["render", &lv, "-i"])
                            .arg(&route)
                            .args([
                                "--resolution",
                                &size.to_string(),
                                "--fps",
                                &fps.to_string(),
                                "--quality",
                                &quality.to_string(),
                                "-o",
                            ])
                            .arg(&p);
                        if let Some(ticks) = expected {
                            child.args(["--expected-ticks", &ticks.to_string()]);
                        }
                        let ok = child
                            .stdout(Stdio::null())
                            .status()
                            .is_ok_and(|s| s.success());
                        let mut i = index.lock().unwrap();
                        if ok {
                            let m: J = serde_json::from_slice(
                                &fs::read(p.with_extension("json")).unwrap(),
                            )
                            .unwrap();
                            i["completed"][&lv] =
                                json!({"file":format!("{lv}.mp4"),"frames":m["frames"],"won":m["won"],"target_level":m["target_level"]});
                        } else {
                            i["failed"][&lv] = json!("see renderer stderr")
                        }
                        atom(&dest.join("index.json"), &i).unwrap();
                        let mut names: Vec<_> =
                            i["completed"].as_object().unwrap().keys().collect();
                        names.sort();
                        let list = format!(
                            "#EXTM3U\n{}",
                            names
                                .iter()
                                .map(|l| format!("{l}.mp4\n"))
                                .collect::<String>()
                        );
                        fs::write(dest.join("videos.m3u8.tmp"), list)
                            .and_then(|_| {
                                fs::rename(dest.join("videos.m3u8.tmp"), dest.join("videos.m3u8"))
                            })
                            .unwrap();
                        eprintln!(
                            "{lv}: {}/{} ({} failed)",
                            names.len(),
                            count,
                            i["failed"].as_object().unwrap().len()
                        );
                    }
                });
            }
        });
        if index.into_inner()?["failed"]
            .as_object()
            .unwrap()
            .is_empty()
        {
            return Ok(());
        }
        return Err("some levels failed".into());
    }
    let level = args.level.as_deref().unwrap();
    if !crate::has_scene(level) {
        return Err(format!("unknown scene {level:?}").into());
    }
    let g = game()?;
    let runtime = runtime()?;
    let ops = String::from_utf8(args.input.read()?)?;
    let verified = args.expected_ticks;
    let out = env::current_dir()?.join(output.unwrap());
    fs::create_dir_all(out.parent().unwrap())?;
    let result = match format {
        Some(format) => image::render(level, &ops, &out, verified, &runtime, &g, format, size)?,
        None => movie(
            level, &ops, &out, verified, &runtime, &g, size, fps, quality,
        )?,
    };
    println!("{result}");
    Ok(())
}

#[cfg(test)]
mod worker_cache_tests {
    use super::*;

    #[test]
    fn independent_renders_share_only_an_intact_worker() {
        let root = Temp::new().unwrap();
        let first = root.0.join("first");
        let second = root.0.join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let cached = install_worker(&first).unwrap();
        let image = fs::read(&cached).unwrap();
        assert_eq!(install_worker(&second).unwrap(), cached);

        // Even a same-size file at the expected cache path must not be run.
        fs::write(&cached, vec![0; image.len()]).unwrap();
        let fallback = install_worker(&second).unwrap();
        assert_eq!(fallback, second.join("gpu-worker"));
        assert_eq!(fs::read(fallback).unwrap(), image);
    }
}
