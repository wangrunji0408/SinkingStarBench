// Read-only samples of the live game world. Simulation and victory stay in op()/advance().
use super::*;
use serde_json::{Map, Value as J, json};
use std::collections::{HashMap, HashSet, VecDeque};
thread_local! {static EFFECTS:std::cell::RefCell<Vec<J>>=const{std::cell::RefCell::new(Vec::new())};}
unsafe extern "C" {
    static observe_flag: *mut u8;
}
pub unsafe fn enable() {
    *observe_flag = 1;
}
fn drain() -> Vec<J> {
    EFFECTS.with_borrow_mut(std::mem::take)
}
#[unsafe(no_mangle)]
unsafe extern "C" fn star_particle_event(sp: usize, owner: usize, parent: usize, origin: usize) {
    if owner != W || origin == 0 {
        return;
    }
    let name = text(get(sp + 0x30));
    let ret = get::<usize>(sp);
    let position = get::<[f32; 3]>(origin);
    let ambient = matches!(
        ret,
        0x1403c665c | 0x1403c672c | 0x1403c67e1 | 0x1403c68b6 | 0x1403c6971
    );
    let parent = if ambient { origin } else { parent };
    let attach = if parent >= 0x100000
        && parent < 0x40000000
        && parent != W
        && get::<usize>(parent + layout::MANAGER) == W
    {
        get::<u32>(parent + layout::UID)
    } else {
        0
    };
    let mut orientation = get::<[f32; 4]>(get(sp + 0x28));
    if ret == 0x14028e119 && matches!(name.as_str(), "WandSuccess" | "WandFailure") {
        let caster = origin - 0xa0;
        let pos = get::<[f32; 3]>(caster);
        let dest = get::<[f32; 3]>(sp + 8 + 0x2f8);
        let (x, y) = (dest[0] - pos[0], dest[1] - pos[1]);
        let yaw = if (x.abs() + y.abs() - 1.).abs() < 0.0001 {
            y.atan2(x)
        } else {
            (get::<f32>(caster + 0x48) / 90.).round() * std::f32::consts::FRAC_PI_2
        };
        orientation = [0., 0., (yaw / 2.).sin(), (yaw / 2.).cos()];
    }
    EFFECTS.with_borrow_mut(|v|v.push(json!({"name":name,"position":position,"orientation":orientation,"attach":attach,"mode":if ambient&&attach!=0{2}else{0},"group":get::<u8>(sp+0x38)!=0,"time":get::<f64>(W+0x67d8)})));
}
struct Kind {
    name: String,
    fields: Vec<(String, usize, usize, bool)>,
    node: Option<usize>,
}
fn kinds() -> HashMap<usize, Kind> {
    let mut db = init::json("meta/layout");
    let ext: J = serde_json::from_reader(
        zstd::stream::Decoder::new(
            &include_bytes!(concat!(env!("OUT_DIR"), "/visual-types.zst"))[..],
        )
        .unwrap(),
    )
    .unwrap();
    db.as_object_mut()
        .unwrap()
        .extend(ext.as_object().unwrap().clone());
    let mut out = HashMap::new();
    for (name, s) in db.as_object().unwrap() {
        let mut fields = Vec::new();
        let all = s["fields"].as_object().unwrap();
        for (k, d) in all {
            let group = k.split('.').next().unwrap();
            if matches!(
                group,
                "position"
                    | "visual_position"
                    | "orientation"
                    | "scale"
                    | "tint_color_a"
                    | "tint_color_b"
            ) || (name == "Freezer" && k == "unfrozen")
                || (name == "Level_Entry" && (k == "solved_t_target" || group == "color_solved"))
                || matches!(
                    k.as_str(),
                    "theta_current"
                        | "theta_target"
                        | "velocity.z"
                        | "silent"
                        | "tower_id"
                        | "entity_flags"
                        | "turn_order_id"
                        | "open"
                        | "open_t"
                        | "pressed"
                        | "enabled_state"
                        | "glow_t_target"
                        | "broken_t"
                        | "is_monster_gate"
                        | "triggered"
                        | "enabled_t_target"
                        | "discovered"
                        | "activated"
                        | "found"
                        | "on_t"
                        | "room_on_t"
                        | "protected"
                        | "entity_params.druid_transmute_t"
                        | "emitted_fire_length"
                        | "status_flags"
                        | "skill_flags"
                        | "charmed_by"
                        | "magnitude"
                        | "thickness_scale"
                        | "elapsed"
                        | "duration"
                )
            {
                fields.push((
                    k.clone(),
                    d[0].as_u64().unwrap() as usize,
                    d[1].as_u64().unwrap() as usize,
                    d[2] == "float",
                ));
            }
        }
        out.insert(
            s["address"].as_u64().unwrap() as usize,
            Kind {
                name: name.clone(),
                fields,
                node: all
                    .get("animation_state.animation_state.node_index")
                    .map(|v| v[0].as_u64().unwrap() as usize),
            },
        );
    }
    out
}
unsafe fn text(p: usize) -> String {
    if p == 0 {
        return String::new();
    }
    let n = get::<usize>(p);
    if n == 0 {
        return String::new();
    }
    assert!(n <= 4096, "invalid asset string");
    let a = get::<usize>(p + 8);
    String::from_utf8_lossy(std::slice::from_raw_parts(a as *const u8, n)).into_owned()
}
unsafe fn bytes(p: usize, n: usize) -> String {
    let mut s = String::with_capacity(n.div_ceil(3) * 4);
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if n > 0 {
        for t in std::slice::from_raw_parts(p as *const u8, n).chunks(3) {
            let v = ((t[0] as usize) << 16)
                | ((t.get(1).copied().unwrap_or(0) as usize) << 8)
                | t.get(2).copied().unwrap_or(0) as usize;
            for (i, k) in [18, 12, 6, 0].into_iter().enumerate() {
                s.push(if i > t.len() {
                    '='
                } else {
                    alphabet[(v >> k) & 63] as char
                })
            }
        }
    }
    s
}
unsafe fn segment(p: usize, len: usize, origin: usize, seg: usize) -> J {
    let n = get::<usize>(p + seg);
    assert!(n <= 10000);
    let o = get::<usize>(p + origin);
    json!({"record":bytes(p,len),"segments":bytes(get(p+seg+8),n*36),"origin":if o==0{0}else{get::<u32>(o+layout::UID)}})
}
struct Reader {
    types: HashMap<usize, Kind>,
    prev: HashMap<u32, Vec<u64>>,
    fade: HashMap<u32, f32>,
    rocks: HashMap<u32, RockMotion>,
}

#[derive(Default)]
struct RockMotion {
    flip: f32,
    spin: f32,
}

fn rotate_pose([x, y, z, w]: [f32; 4], [a, b, c, d]: [f32; 4]) -> [f32; 4] {
    [
        w * a + x * d + y * c - z * b,
        w * b - x * c + y * d + z * a,
        w * c + x * b - y * a + z * d,
        w * d - x * a - y * b - z * c,
    ]
}

// Presentation-only part of the original Rock update (0x140419993).
// The simulation sets the flip flag and axis; the visual update eases in and out.
fn rock_flip(pos: &mut [f32; 3], q: [f32; 4], axis: [f32; 3], t: f32) -> [f32; 4] {
    let smooth = |x: f32| x * x * (3. - 2. * x);
    let t = smooth(smooth(t));
    let norm = axis.iter().map(|v| v * v).sum::<f32>().sqrt();
    let [ax, ay, az] = if norm < 0.5 {
        [1., 0., 0.]
    } else {
        axis.map(|v| v / norm)
    };
    let (s, w) = (t * std::f32::consts::FRAC_PI_2).sin_cos();
    let (x, y, z) = (ax * s, ay * s, az * s);
    // Rotate the stone around the raised pivot, then compose with its base pose.
    pos[0] += 0.36 * (w * y + z * x);
    pos[1] += 0.36 * (z * y - w * x);
    pos[2] += 0.57 * t - 0.36 * (x * x + y * y);
    rotate_pose([x, y, z, w], q)
}

impl Reader {
    unsafe fn sample(&mut self, tick: u32, quarters: u64, dt: f32) -> J {
        let mut changed = vec![];
        let mut removed: HashSet<_> = self.prev.keys().copied().collect();
        let mut tags = Map::new();
        let mut nodes = Map::new();
        let (mut beams, mut mirages, mut bards, mut targets) = (vec![], vec![], vec![], vec![]);
        let fam = if get::<usize>(W + layout::FAMILY_COUNT) > 0 {
            get::<u32>(
                get::<usize>(W + layout::FAMILY_IDS) + 4 * get::<usize>(W + layout::ACTIVE_FAMILY),
            )
        } else {
            0
        };
        let n = get::<usize>(W + 0x370);
        assert!(n < 100000);
        for i in 0..n {
            let p = get::<usize>(get::<usize>(W + 0x378) + 8 * i);
            let id = get::<u32>(p + layout::UID);
            let ty = get::<usize>(p + layout::TYPE);
            let Some(kind) = self.types.get(&ty) else {
                panic!("unknown live entity type {ty:#x}")
            };
            removed.remove(&id);
            let mesh = get::<usize>(p + layout::MESH);
            let graph = get::<usize>(p + layout::GRAPH);
            if let Some(offset) = kind.node.filter(|_| graph != 0) {
                let idx = get::<usize>(p + offset);
                if idx < get::<usize>(graph) {
                    let node = get::<usize>(get::<usize>(graph + 8) + 8 * idx);
                    if node != 0 && get::<usize>(node + 24) > 0 {
                        tags.insert(id.to_string(), json!(text(get(node + 32))));
                        nodes.insert(
                            id.to_string(),
                            json!({"name":text(node),"loop":get::<u8>(node+16)!=0}),
                        );
                    }
                }
            }
            let flags = get::<u32>(p + 0x28);
            let status = get::<u16>(p + 0x5c);
            let exists = flags & 0x2000000 == 0 || flags & 0x4000000 != 0;
            let dead = status & 1 != 0;
            let alive = exists && !dead;
            if alive {
                let offsets: &[usize] = match kind.name.as_str() {
                    "Promesst_Light" => &[0x360],
                    "Friendly_Dragon" => &[0x398, 0x4d8, 0x618, 0x758],
                    _ => &[],
                };
                for &off in offsets {
                    let mut b = segment(p + off, 320, 0x20, 0x60);
                    b["id"] = id.into();
                    b["offset"] = off.into();
                    beams.push(b);
                }
            }
            let mut fade = 0.;
            if kind.name == "Guy" {
                let protected = get::<u8>(p + 0x641) != 0;
                fade = (*self.fade.get(&id).unwrap_or(&0.)
                    + if protected { 2. * dt } else { -3. * dt })
                .clamp(0., 1.);
                self.fade.insert(id, fade);
                if alive && get::<u32>(p + 0x360) == fam && get::<u32>(p + 0x340) & 64 != 0 {
                    bards.push(id)
                }
            }
            if alive {
                let bard = get::<u32>(p + 0x11c);
                if bard != 0 {
                    targets.push((id, bard))
                }
            }
            let mut sig = vec![ty as u64, mesh as u64, fade.to_bits() as u64];
            let mut visual = get::<[f32; 3]>(p + 0xa0);
            let mut flip_pose = None;
            let mut splash_rng = None;
            if kind.name == "Rock" {
                let flags = get::<u16>(p + 0x332);
                if flags & 2 != 0 {
                    use std::f32::consts::{PI, TAU};
                    let motion = self.rocks.entry(id).or_default();
                    motion.flip = if flags & 8 != 0 && alive {
                        (motion.flip + 1.3 * dt).min(1.)
                    } else {
                        (motion.flip - 1.3 * dt).max(0.)
                    };
                    // Spin while skimming, then settle back to the base heading.
                    motion.spin = if flags & 4 != 0 && alive {
                        (motion.spin + 2. * TAU * dt + PI).rem_euclid(TAU) - PI
                    } else {
                        motion.spin.signum() * (motion.spin.abs() - 2. * TAU * dt).max(0.)
                    };
                    if motion.flip > 0. || motion.spin != 0. {
                        let (s, c) = (motion.spin * 0.5).sin_cos();
                        let q = rotate_pose([0., 0., s, c], get(p + 0x18));
                        flip_pose = Some(if motion.flip > 0. {
                            rock_flip(&mut visual, q, get(p + 0x3ac), motion.flip)
                        } else {
                            q
                        });
                    }
                } else {
                    self.rocks.remove(&id);
                }
            } else if kind.name == "Temporary_Splash" {
                splash_rng = Some(get::<[u64; 2]>(p + 0x340));
            }
            for (_, off, size, _) in &kind.fields {
                sig.push(match size {
                    1 => get::<u8>(p + off) as u64,
                    2 => get::<u16>(p + off) as u64,
                    4 => get::<u32>(p + off) as u64,
                    8 => get::<u64>(p + off),
                    _ => unreachable!(),
                });
            }
            if let Some(q) = flip_pose {
                sig.extend(q.into_iter().map(|v| v.to_bits() as u64));
            }
            if let Some(rng) = splash_rng {
                sig.extend(rng);
            }
            if self.prev.get(&id) == Some(&sig) {
                continue;
            }
            let mut f = Map::new();
            for ((k, _, _, float), &v) in kind.fields.iter().zip(&sig[3..]) {
                f.insert(
                    k.clone(),
                    if *float {
                        json!(f32::from_bits(v as u32))
                    } else {
                        v.into()
                    },
                );
            }
            self.prev.insert(id, sig);
            f.insert("mesh".into(), text(mesh).into());
            let pos = get::<[f32; 3]>(p);
            for (axis, v) in "xyz".chars().zip(visual) {
                f.insert(format!("position.{axis}"), v.into());
            }
            if let Some(q) = flip_pose {
                for (axis, v) in "xyzw".chars().zip(q) {
                    f.insert(format!("orientation.{axis}"), v.into());
                }
            }
            if let Some(rng) = splash_rng {
                f.insert("splash_random_state".into(), json!(rng));
            }
            if kind.name == "Guy" {
                f.insert("protected_fade_t".into(), fade.into());
            }
            // The shipping map selects a different mesh for each entry state.
            // Tinting a locked mesh cannot light up its missing star faces.
            if kind.name == "Level_Entry" {
                let solved = f["solved_t_target"].as_f64().unwrap_or(0.) > 0.;
                if solved || f["enabled_t_target"].as_f64().unwrap_or(0.) > 0. {
                    if let Some(mesh) = f["mesh"]
                        .as_str()
                        .and_then(|m| m.strip_suffix("_A"))
                        .and_then(|m| m.rsplit_once('_'))
                        .map(|(family, _)| family)
                        .filter(|family| {
                            matches!(*family, "GAME_OVER_Entry" | "GAME_OVER_Entry_Hero")
                        })
                    {
                        let state = if solved { "Solved" } else { "Available" };
                        f.insert("mesh".into(), format!("{mesh}_{state}_A").into());
                    }
                }
            }
            // Use the authored solved tint in the headless presentation; the native
            // graphics update that copies this tint is not run by the replay.
            if kind.name == "Level_Entry" && f["solved_t_target"].as_f64().unwrap_or(0.) > 0. {
                for axis in "xyzw".chars() {
                    let color = f[&format!("color_solved.{axis}")].clone();
                    f.insert(format!("tint_color_a.{axis}"), color);
                }
            }
            // A thawed freezer no longer contributes its ice mesh. Keep the
            // simulation entity (and its UID) intact for dependencies and undo.
            let exists = exists && !(kind.name == "Freezer" && f["unfrozen"].as_u64() == Some(1));
            let theta = f["theta_current"].clone();
            changed.push(json!({"id":id,"type":kind.name,"p":pos,"theta":theta,"dead":dead,"exists":exists,"f":f}));
        }
        for id in &removed {
            self.prev.remove(id);
            self.rocks.remove(id);
        }
        let blocks = get::<usize>(W + 0x4290);
        assert!(blocks <= 10000);
        for i in 0..blocks {
            let b = get::<usize>(get::<usize>(W + 0x4298) + 8 * i);
            for j in 0..20 {
                if get::<u8>(b + j) != 0 {
                    mirages.push(segment(b + 24 + j * 496, 496, 0xc8, 0x108));
                }
            }
        }
        let t: Vec<_> = targets
            .into_iter()
            .filter(|(_, b)| bards.contains(b))
            .map(|(id, b)| json!({"id":id,"bard":b}))
            .collect();
        json!({"tick":tick,"poseTick":quarters/4,"gameSeconds":quarters as f64/240.,"changes":changed,"removed":removed,"family":fam,"mirages":mirages,"beams":beams,"bardVisual":{"bards":bards,"targets":t},"tags":tags,"animationNodes":nodes,"won":won(),"visualEvents":drain()})
    }
}
const MAX_SETTLE_TICKS: u32 = 3 * crate::cli::GAME_FPS;

unsafe fn blocking_move_pending() -> bool {
    // The scheduler's gate before it dispatches the next queued input:
    // block until the transaction ends, unless it unblocks at physical commit.
    const BLOCKING_PLAYER: u32 = 0x8;
    const UNBLOCK_ON_COMMIT: u32 = 0x10000 | 0x2;
    let moves = W + layout::MOVE_TRANSACTIONS;
    let count = get::<usize>(moves);
    let data = get::<usize>(moves + 8);
    (0..count).any(|i| {
        let tx = get::<usize>(data + i * 8);
        let flags = get::<u32>(tx + layout::MOVE_TRANSACTION_FLAGS);
        flags & BLOCKING_PLAYER != 0 && flags & UNBLOCK_ON_COMMIT != UNBLOCK_ON_COMMIT
    })
}

pub unsafe fn emit(
    key: &str,
    route: &[routes::Key],
    mode: &str,
    fps: u32,
    mut send: impl FnMut(J) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use routes::Key::*;
    let step = crate::cli::GAME_FPS / fps;
    let keys = mode.ends_with("-keys");
    let movie = mode.starts_with("--movie-");
    let entry = drain();
    let mut effect_names = std::collections::BTreeSet::new();
    for e in &entry {
        effect_names.insert(e["name"].as_str().unwrap().to_owned());
    }
    let mut rd = Reader {
        types: if keys { HashMap::new() } else { kinds() },
        prev: HashMap::new(),
        fade: HashMap::new(),
        rocks: HashMap::new(),
    };
    let (mut tick, mut quarters, mut frames, mut ctrl, mut last, mut dt) =
        (0, 0, 0, false, 0, 0f32);
    let mut events = vec![];
    let mut queued = VecDeque::new();
    let mut visit_events = vec![];
    if !keys {
        let first = rd.sample(0, 0, 0.);
        if mode == "--movie-image-stream" {
            // The screenshot consumer uses tick zero for both scene setup and
            // the first update. Send its full entity set over the pipe once.
            let mut header = json!({"initialVisualEvents":entry});
            header["frame"] = first;
            send(header)?;
        } else {
            let initial = json!({"level":key,"entities":first["changes"],"activeFamily":first["family"],"mirages":first["mirages"],"beams":first["beams"],"bardVisual":first["bardVisual"],"won":first["won"],"gameSeconds":0,"initialVisualEvents":entry,"visualEvents":[],"provenance":{"backend":"star-replay","source_sha256":env!("STAR_SOURCE_SHA256")}});
            send(json!({"initial":initial,"view":view()}))?;
            send(first)?;
        }
        frames += 1;
    }
    let mut dirty = false;
    play_route(route, |step_event| {
        match step_event {
            ReplayStep::Frame(frame) => {
                let before = pending_input();
                advance(ctrl);
                tick = frame;
                quarters += if ctrl { 1 } else { 4 };
                dt += if ctrl { 0.25 } else { 1. } / 60.;
                if keys {
                    for e in drain() {
                        effect_names.insert(e["name"].as_str().unwrap().to_owned());
                    }
                    let left = pending_input();
                    for _ in 0..before.saturating_sub(left) {
                        if let Some(action) = queued.pop_front() {
                            events.push(json!([tick, [action]]))
                        }
                    }
                }
                if !keys && tick % step == 0 {
                    send(rd.sample(tick, quarters, dt))?;
                    last = tick;
                    dt = 0.;
                    frames += 1;
                    dirty = false;
                }
            }
            ReplayStep::Input(index, k) => {
                let entering = keys && matches!(k, Take) && crate::overworld::enabled();
                let before_visits = entering.then(crate::overworld::visit_count);
                let enqueued = op(k, &mut ctrl);
                if before_visits.is_some_and(|n| crate::overworld::visit_count() > n) {
                    visit_events.push(json!({"tick": tick, "op_index": index}));
                }
                dirty = true;
                if keys {
                    let name = match k {
                        Direction(d) => ["right", "up", "left", "down"][d as usize],
                        Action => "action",
                        Swap => "swap",
                        Take => "take",
                        Dragon => "dragon",
                        Undo => "undo",
                        Run(true) => "shift:down",
                        Run(false) => "shift:up",
                        Slow(true) => "ctrl:down",
                        Slow(false) => "ctrl:up",
                        Frames(_) => unreachable!(),
                    };
                    if enqueued {
                        queued.push_back(name)
                    } else {
                        events.push(json!([tick, [name]]))
                    }
                }
            }
            ReplayStep::Boundary => {}
        }
        Ok(())
    })?;
    // The renderers keep the original scheduler running after victory so motion
    // can settle without further input or loading another scene.
    let victory = won();
    let destination = target_level();
    let result_tick = tick;
    if !keys && (tick != last || dirty) {
        send(rd.sample(tick, quarters, dt))?;
        frames += 1;
    }
    let mut settled = None;
    if movie && victory {
        put(layout::GAME_DT, 1f32 / 60.);
        put(layout::FRAME_DT, 1f32 / 60.);
        // Inputs buffered beyond the winning move belong to no rendered action.
        put(W + layout::INPUT_QUEUE, 0usize);
        let mut still_blocked = blocking_move_pending();
        let mut elapsed = 0;
        for offset in 1..=MAX_SETTLE_TICKS {
            if !still_blocked {
                break;
            }
            reg(2, 0x100000);
            reg(3, W as u64);
            run_prepare();
            reg(5, W as u64);
            run_schedule();
            tick += 1;
            quarters += 4;
            elapsed += 1;
            still_blocked = blocking_move_pending();
            if keys {
                for e in drain() {
                    effect_names.insert(e["name"].as_str().unwrap().to_owned());
                }
            } else if offset % step == 0 || !still_blocked || offset == MAX_SETTLE_TICKS {
                send(rd.sample(tick, quarters, elapsed as f32 / 60.))?;
                elapsed = 0;
                frames += 1;
            }
        }
        settled = Some(!still_blocked);
    }
    if keys {
        let mut end = json!({"keys":events,"ticks":result_tick,"won":victory,"target_level":destination,"view":view(),"effectNames":effect_names});
        if crate::overworld::enabled() {
            end["visit_events"] = json!(visit_events);
        }
        if movie {
            end["render_ticks"] = tick.into();
            end["settled"] = json!(settled);
        }
        crate::overworld::report(&mut end, false);
        send(end)?;
    } else {
        let mut end = json!({"end":true,"ticks":result_tick,"frames":frames,"won":victory,"target_level":destination});
        if movie {
            end["render_ticks"] = tick.into();
            end["settled"] = json!(settled);
        }
        crate::overworld::report(&mut end, false);
        send(end)?;
    }
    Ok(())
}
