use super::{
    assets, draw, frame_map, install_worker, io::*, key_trace, poses, snapshot, start_worker, trace,
};
use crate::cli::Dimensions;
use serde_json::{Value as J, json};
use std::{
    ffi::{CString, c_char},
    io::{BufRead, BufReader},
    path::Path,
};

unsafe extern "C" {
    fn star_image_write(
        path: *const c_char,
        rgba: *const u8,
        jpeg: i32,
        width: u32,
        height: u32,
    ) -> i32;
}

pub enum Format {
    Png,
    Jpeg,
}
impl Format {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        if ext.eq_ignore_ascii_case("png") {
            Some(Self::Png)
        } else if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
            Some(Self::Jpeg)
        } else {
            None
        }
    }
    pub fn codec(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }
    pub fn write(&self, path: &Path, rgba: &[u8], size: Dimensions) -> Res<()> {
        if rgba.len() != size.rgba_len() {
            return Err("invalid GPU readback length".into());
        }
        let path = CString::new(path.as_os_str().as_encoded_bytes())?;
        if unsafe {
            star_image_write(
                path.as_ptr(),
                rgba.as_ptr(),
                matches!(self, Self::Jpeg) as i32,
                size.width,
                size.height,
            )
        } == 0
        {
            return Err("image encoding or write failed".into());
        }
        Ok(())
    }
}

pub(super) fn render(
    level: &str,
    ops: &str,
    out: &Path,
    expected: Option<u64>,
    runtime: &Path,
    game: &Path,
    format: Format,
    size: Dimensions,
) -> Res<J> {
    let profile = std::env::var_os("SINKING_FRAME_PROFILE").is_some();
    let mut phases = std::collections::BTreeMap::new();
    let mut mark = std::time::Instant::now();
    let mut lap = |phase| {
        if profile {
            let now = std::time::Instant::now();
            *phases.entry(phase).or_insert(0.) += (now - mark).as_secs_f64();
            mark = now;
        }
    };
    crate::routes::parse(ops.as_bytes())?;
    let tmp = Temp::new()?;
    let d = &tmp.0;
    let mut c = trace(level, ops, "--movie-image-stream", None)?;
    let mut assets = assets::Assets::new(game, d, level)?;
    if level == "overworld" {
        assets.prepare_overworld_scenery()?;
    }
    lap("assets_load");
    let mut lines = BufReader::new(c.reader()).lines();
    let mut header: J = serde_json::from_str(&lines.next().ok_or("missing header")??)?;
    let mut first = header["frame"].take();
    if first["tick"] != 0 || !first["changes"].is_array() {
        return Err("missing initial screenshot sample".into());
    }
    let mut initial = json!({"level":level,"won":first["won"]});
    initial["initialVisualEvents"] = header["initialVisualEvents"].take();
    initial["entities"] = first["changes"].take();
    lap("trace_header");
    // With no inputs and no victory tail, the header contains every effect
    // the key pass would discover. Do not initialize the simulation twice.
    let t = if ops.is_empty() && expected.is_none() && initial["won"] == false {
        let names: Vec<_> = vecj(&initial["initialVisualEvents"])
            .iter()
            .map(|e| &e["name"])
            .collect();
        json!({"effectNames":names, "won":false, "ticks":0, "render_ticks":0, "settled":null})
    } else {
        key_trace(level, ops, "--movie-keys", expected)?
    };
    lap("key_trace");
    let setup = assets.setup(&initial, &t)?;
    let mut scene = snapshot::Scene::new(&initial, &setup);
    lap("assets_setup");
    let mut poses = poses::Poses::default();
    let (mut resources, mut end, mut last_tick, mut samples) = (None, J::Null, J::Null, 0u64);
    // Apply the same post-win updates as the video renderer; only the final
    // snapshot goes to the GPU, after any pending movement has settled.
    first["changes"] = initial["entities"].take();
    let rows = lines.map(|l| -> Res<J> { Ok(serde_json::from_str(&l?)?) });
    for sample in std::iter::once(Ok(first)).chain(rows) {
        let f = sample?;
        if f["end"] == true {
            end = f;
            break;
        }
        lap("trace_frames");
        if let Some(r) = scene.update(d, &mut assets, &f)? {
            resources = Some(r);
        }
        poses.write(&mut assets, d, &f, &scene.entities)?;
        lap("scene_updates");
        last_tick = f["tick"].clone();
        samples += 1;
    }
    if !c.wait()?
        || end["frames"] != samples
        || end["won"] != t["won"]
        || end["target_level"] != t["target_level"]
        || end["ticks"] != t["ticks"]
        || end["render_ticks"] != last_tick
        || end["render_ticks"] != t["render_ticks"]
        || end["settled"] != t["settled"]
    {
        return Err("incomplete replay".into());
    }

    lap("trace_end");
    let host = install_worker(d)?;
    let raw = d.join("final.rgba");
    let frame = frame_map::FrameMap::new(&raw, size)?;
    let mut gpu = start_worker(
        &host,
        d,
        level,
        runtime,
        game,
        &resources.ok_or("missing scene resources")?,
        out,
        size,
    )?;
    lap("gpu_startup");
    draw(&mut gpu, d, &raw)?;
    gpu.until("DONE")?;
    lap("draw");
    let mut worker_timings = WorkerTimings::default();
    if profile {
        worker_timings.record(&gpu)?;
    }
    drop(gpu);
    format.write(out, frame.bytes(), size)?;
    lap("encode");
    let mut result = json!({"level":level,"ops":ops,"engine":source(),"frames":1,"width":size.width,"height":size.height,"ticks":end["ticks"],"render_ticks":last_tick,"won":end["won"],"target_level":end["target_level"],"settled":end["settled"],"codec":format.codec()});
    if profile {
        result["profile"] = json!({"seconds":phases,"worker_mean_ms":worker_timings.mean_ms()});
    }
    Ok(result)
}
