use super::io::*;
use crate::cli::{Dimensions, GAME_FPS};
use serde_json::Value as J;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{CString, c_char, c_void},
    path::Path,
};
unsafe extern "C" {
    fn star_video_open(
        p: *const c_char,
        width: u32,
        height: u32,
        fps: u32,
        quality: u8,
    ) -> *mut c_void;
    fn star_video_frame(p: *mut c_void, b: *const u8, i: u32, l: *const c_char) -> i32;
    fn star_video_close(p: *mut c_void);
    fn star_video_finish(p: *mut c_void, n: u32) -> i32;
    fn star_video_probe(p: *const c_char, width: u32, height: u32, fps: u32) -> i64;
}
pub fn probe(p: &Path, size: Dimensions, fps: u32) -> Res<u64> {
    let p = CString::new(p.as_os_str().as_encoded_bytes())?;
    let n = unsafe { star_video_probe(p.as_ptr(), size.width, size.height, fps) };
    if n < 1 {
        return Err("invalid HEVC video track or duration".into());
    }
    Ok(n as u64)
}
pub struct Video {
    v: *mut c_void,
    n: u32,
    size: Dimensions,
    overlay_frames: u32,
    track: BTreeMap<u64, Vec<String>>,
    held: BTreeSet<String>,
    show: Vec<String>,
    until: u32,
}
impl Video {
    pub fn open(
        path: &Path,
        trace: &J,
        view: u8,
        size: Dimensions,
        fps: u32,
        quality: u8,
    ) -> Res<Self> {
        let path = CString::new(path.as_os_str().as_encoded_bytes())?;
        let mut track = BTreeMap::<u64, Vec<String>>::new();
        for row in vecj(&trace["keys"]) {
            let t = num(&row[0]).div_ceil(u64::from(GAME_FPS / fps));
            for k in vecj(&row[1]) {
                let k = k.as_str().ok_or("invalid input event")?;
                let keys = ["right", "up", "left", "down"];
                let label = if let Some(n) = keys.iter().position(|x| *x == k) {
                    ["→", "↑", "←", "↓"][(n + num(&trace["view"]) as usize + 4 - view as usize) % 4]
                } else {
                    match k {
                        "action" => "X",
                        "swap" => "C",
                        "dragon" => "F",
                        "take" => "V",
                        "undo" => "Z",
                        "wait" => "·",
                        x => x,
                    }
                };
                track.entry(t).or_default().push(label.into());
            }
        }
        let v = unsafe { star_video_open(path.as_ptr(), size.width, size.height, fps, quality) };
        if v.is_null() {
            return Err("cannot start HEVC encoder".into());
        }
        Ok(Self {
            v,
            size,
            overlay_frames: (2 * fps).div_ceil(5),
            n: 0,
            track,
            held: BTreeSet::new(),
            show: vec![],
            until: 0,
        })
    }
    pub fn push(&mut self, b: &[u8]) -> Res<()> {
        if b.len() != self.size.rgba_len() {
            return Err("invalid GPU readback length".into());
        }
        if self.n >= self.until {
            self.show.clear()
        }
        if let Some(events) = self.track.get(&(self.n as u64)) {
            let mut keys = vec![];
            for k in events {
                if let Some((key, dir)) = k.split_once(':') {
                    let key = match key {
                        "ctrl" => "Ctrl",
                        "alt" => "Alt",
                        _ => "Shift",
                    };
                    if dir == "down" {
                        self.held.insert(key.into());
                    } else {
                        self.held.remove(key);
                    }
                } else {
                    keys.push(k.clone())
                }
            }
            if !keys.is_empty() {
                self.show = keys;
                self.until = self.n + self.overlay_frames;
            }
        }
        let mut labels: Vec<_> = self.held.iter().cloned().collect();
        if self.show.len() > 6 {
            labels.push(format!("+{}", self.show.len() - 6));
        }
        labels.extend(
            self.show
                .iter()
                .skip(self.show.len().saturating_sub(6))
                .cloned(),
        );
        let labels = CString::new(labels.join(" "))?;
        if unsafe { star_video_frame(self.v, b.as_ptr(), self.n, labels.as_ptr()) } == 0 {
            return Err("HEVC frame failed".into());
        }
        self.n += 1;
        Ok(())
    }
    pub fn finish(&mut self) -> Res<()> {
        if unsafe { star_video_finish(self.v, self.n) } == 0 {
            return Err("HEVC finish failed".into());
        }
        Ok(())
    }
}
impl Drop for Video {
    fn drop(&mut self) {
        unsafe { star_video_close(self.v) }
    }
}
