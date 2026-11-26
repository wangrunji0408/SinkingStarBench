// Original animation tracks are compiled once per clip. Sample and blend local
// joints here before composing parent transforms and applying inverse bind poses.
use super::io::*;
use serde_json::Value as J;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};
type Mat = [f32; 16];
type Local = [f32; 16]; // translation(3), xyzw(4), scale matrix(9)
struct Clip {
    frames: usize,
    fps: f64,
    parents: Vec<i32>,
    order: Vec<usize>,
    inv: Vec<Mat>,
    samples: Vec<Local>,
}
impl Clip {
    fn load(w: &mut super::assets::Assets, _dir: &Path, mesh: &str, name: &str) -> Res<Self> {
        let a = super::assets::anim::read(&w.anim, &w.game, mesh, name)?;
        let (frames, fps, parents, order, inv, samples) =
            (a.frames, a.fps as f64, a.parents, a.order, a.inv, a.samples);
        Ok(Self {
            frames,
            fps,
            parents,
            order,
            inv,
            samples,
        })
    }
}
fn mix(a: &Local, b: &Local, w: f32) -> Local {
    let sign = if (3..7).map(|i| a[i] * b[i]).sum::<f32>() < 0. {
        -1.
    } else {
        1.
    };
    let mut c = std::array::from_fn(|i| {
        a[i] + ((if (3..7).contains(&i) {
            sign * b[i]
        } else {
            b[i]
        }) - a[i])
            * w
    });
    let norm = c[3..7]
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt()
        .max(0.000001);
    for v in &mut c[3..7] {
        *v /= norm
    }
    c
}
pub(super) fn mul(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|i| (0..4).map(|k| a[i / 4 * 4 + k] * b[k * 4 + i % 4]).sum())
}
pub(super) fn matrix(a: &Local) -> Mat {
    let (x, y, z, w) = (a[3], a[4], a[5], a[6]);
    let r = [
        1. - 2. * (y * y + z * z),
        2. * (x * y - z * w),
        2. * (x * z + y * w),
        2. * (x * y + z * w),
        1. - 2. * (x * x + z * z),
        2. * (y * z - x * w),
        2. * (x * z - y * w),
        2. * (y * z + x * w),
        1. - 2. * (x * x + y * y),
    ];
    std::array::from_fn(|i| {
        if i == 15 {
            1.
        } else if i >= 12 {
            0.
        } else if i % 4 == 3 {
            a[i / 4] * 0.01
        } else {
            (0..3)
                .map(|k| r[i / 4 * 3 + k] * a[7 + k * 3 + i % 4])
                .sum()
        }
    })
}
#[derive(Default)]
struct Motion {
    mesh: String,
    node: String,
    tag: String,
    clip: String,
    start: f64,
    blend: f64,
    from: Vec<Local>,
    last: Vec<Local>,
}
#[derive(Default)]
pub struct Poses {
    rigs: HashMap<String, J>,
    clips: HashMap<(String, String), Clip>,
    state: HashMap<String, Motion>,
}
impl Poses {
    pub fn write(
        &mut self,
        w: &mut super::assets::Assets,
        dir: &Path,
        frame: &J,
        ents: &BTreeMap<u64, J>,
    ) -> Res<()> {
        let mut b = vec![0; 4];
        let mut records = 0u32;
        let tick = val(&frame["gameSeconds"]) * 60.;
        for (id, tag) in frame["tags"].as_object().ok_or("missing tags")? {
            let e = &ents[&id.parse()?];
            let mesh = e["f"]["mesh"].as_str().unwrap_or("");
            if mesh.is_empty() || e["exists"] != true || e["dead"] == true {
                continue;
            }
            if !self.rigs.contains_key(mesh) {
                self.rigs.insert(mesh.into(), w.rig(mesh)?);
            }
            let rig = &self.rigs[mesh];
            let tag = tag.as_str().unwrap();
            let node = frame["animationNodes"][id]["name"].as_str().unwrap_or(tag);
            let state = self.state.entry(id.clone()).or_default();
            if state.mesh != mesh || state.tag != tag || state.node != node {
                let choices = rig["choices"][tag]
                    .as_array()
                    .or_else(|| rig["choices"]["active"].as_array());
                let Some(cs) = choices else { continue };
                let sum: f64 = cs.iter().map(|e| val(&e[1])).sum();
                let h = format!("{id}:{mesh}:{node}:{tick}")
                    .bytes()
                    .fold(2166136261u32, |a, b| {
                        (a ^ (b as u32)).wrapping_mul(16777619)
                    });
                let mut draw = sum * (h as f64 / 4294967296.);
                let clip = cs
                    .iter()
                    .find(|c| {
                        draw -= val(&c[1]);
                        draw < 0.
                    })
                    .unwrap_or(&cs[0])[0]
                    .as_str()
                    .unwrap();
                state.from = if state.mesh == mesh {
                    std::mem::take(&mut state.last)
                } else {
                    vec![]
                };
                state.blend = rig["blends"][format!("{}>{node}", state.node)]
                    .as_f64()
                    .map(|v| v * 60.)
                    .unwrap_or(10.);
                if rig["shortBlends"] == true {
                    state.blend = state.blend.min(10.)
                }
                state.start = tick;
                state.clip = clip.into();
                state.tag = tag.into();
                state.node = node.into();
                state.mesh = mesh.into();
            }
            let key = (mesh.to_string(), state.clip.clone());
            if !self.clips.contains_key(&key) {
                if self.clips.len() >= 32 {
                    self.clips.clear()
                }
                self.clips
                    .insert(key.clone(), Clip::load(w, dir, mesh, &state.clip)?);
            }
            let c = &self.clips[&key];
            let age = tick - state.start;
            let mut phase = age * c.fps / 60.;
            if frame["animationNodes"][id]["loop"] == false {
                phase = phase.min((c.frames - 1) as f64)
            }
            let i = phase as usize % c.frames;
            let n = c.parents.len();
            let beta = if state.blend > 0. {
                ((age + 2.) / state.blend).min(1.) as f32
            } else {
                1.
            };
            let mut globals: Vec<Mat> = Vec::with_capacity(n);
            state.last.clear();
            for j in 0..n {
                let mut v = mix(
                    &c.samples[i * n + j],
                    &c.samples[((i + 1) % c.frames) * n + j],
                    phase.fract() as f32,
                );
                if beta < 1. && state.from.len() == n {
                    v = mix(&state.from[j], &v, beta * beta * (3. - 2. * beta))
                }
                state.last.push(v);
                let m = matrix(&v);
                globals.push(if c.parents[j] >= 0 {
                    mul(&globals[c.parents[j] as usize], &m)
                } else {
                    m
                });
            }
            put_u(&mut b, id.parse()?);
            put_u(&mut b, c.order.len() as u64);
            for (&j, inv) in c.order.iter().zip(&c.inv) {
                for f in &mul(&globals[j], inv)[..12] {
                    b.extend(f.to_le_bytes())
                }
            }
            records += 1;
        }
        b[..4].copy_from_slice(&records.to_le_bytes());
        put_u(&mut b, num(&frame["poseTick"]));
        let splashes: Vec<_> = ents
            .values()
            .filter(|e| {
                e["type"] == "Temporary_Splash" && e["exists"] == true && e["dead"] == false
            })
            .collect();
        put_u(&mut b, splashes.len() as u64);
        for e in splashes {
            for k in ["position.x", "position.y", "position.z"] {
                put_f(&mut b, val(&e["f"][k]));
            }
            for k in ["magnitude", "thickness_scale", "elapsed", "duration"] {
                put_f(&mut b, val(&e["f"][k]));
            }
            for v in vecj(&e["f"]["splash_random_state"]) {
                b.extend(num(v).to_le_bytes());
            }
        }
        b.extend(val(&frame["gameSeconds"]).to_le_bytes());
        fs::write(dir.join("world.snapshot.pose"), b)?;
        Ok(())
    }
}
