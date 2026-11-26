// Instant completion is opt-in. Positions and eligibility are sampled from
// the running scene, so an input blocked by terrain never earns a visit.
use super::*;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{cell::RefCell, collections::HashSet};
static ENABLED: AtomicBool = AtomicBool::new(false);
thread_local! { static STATE: RefCell<Option<Map>> = const { RefCell::new(None) }; }
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}
struct Gate {
    p: usize,
    id: u32,
    deps: Vec<u32>,
    any: bool,
    name: Option<String>,
}
struct Map {
    guy: usize,
    pos: usize,
    flags: usize,
    status: usize,
    family: usize,
    enabled: usize,
    enabled_now: usize,
    solved: usize,
    thaw: usize,
    gates: Vec<Gate>,
    done: HashSet<u32>,
    visits: Vec<String>,
}
pub unsafe fn init(ents: &[(&str, usize)], schema: &Value) {
    let off = |k: &str, f: &str| schema[k]["fields"][f][0].as_u64().unwrap() as usize;
    let mut gates = vec![];
    for &(k, p) in ents {
        if !matches!(k, "Level_Entry" | "Freezer") {
            continue;
        }
        let entry = k == "Level_Entry";
        let deps = (0..if entry { 3 } else { 4 })
            .map(|i| {
                if i == 0 {
                    "unlocked_by_id".into()
                } else {
                    format!("unlocked_by_id{}", i + 1)
                }
            })
            .map(|f| get::<u32>(p + off(k, &f)))
            .filter(|&id| id != 0)
            .collect();
        let name = entry.then(|| {
            let [n, a] = get::<[usize; 2]>(p + off(k, "level_name"));
            String::from_utf8(std::slice::from_raw_parts(a as *const u8, n).to_vec()).unwrap()
        });
        gates.push(Gate {
            p,
            id: get(p + layout::UID),
            deps,
            any: get::<u8>(p + off(k, "unlock_is_or_gate")) != 0,
            name,
        });
    }
    let k = "Level_Entry";
    STATE.with_borrow_mut(|s| {
        *s = Some(Map {
            guy: ents.iter().find(|e| e.0 == "Guy").unwrap().1,
            pos: off(k, "position.x"),
            flags: off(k, "entity_flags"),
            status: off("Guy", "status_flags"),
            family: off("Guy", "turn_order_id"),
            enabled: off(k, "enabled_t_target"),
            enabled_now: off(k, "enabled_t"),
            solved: off(k, "solved_t_target"),
            thaw: off("Freezer", "unfrozen"),
            gates,
            done: HashSet::new(),
            visits: vec![],
        })
    });
}
impl Map {
    unsafe fn active(&self, p: usize) -> bool {
        let f = get::<u64>(p + self.flags);
        f & ((1 << 21) | (1 << 29)) == 0 && (f & (1 << 25) == 0 || f & (1 << 26) != 0)
    }

    unsafe fn standing(&self, entering: bool) -> Option<&Gate> {
        let count = get::<usize>(W + layout::FAMILY_COUNT);
        let selected = get::<usize>(W + layout::ACTIVE_FAMILY);
        if selected >= count {
            return None;
        }
        let family = get::<u32>(get::<usize>(W + layout::FAMILY_IDS) + 4 * selected);
        let guy_type = get::<usize>(self.guy + layout::TYPE);
        let table = get::<usize>(W + 0x378);
        for i in 0..get::<usize>(W + 0x370) {
            let p = get::<usize>(table + 8 * i);
            if get::<usize>(p + layout::TYPE) != guy_type
                || !self.active(p)
                || get::<u32>(p + self.family) != family
                || get::<u16>(p + self.status) & 1 != 0
            {
                continue;
            }
            let pos = get::<[f32; 3]>(p + self.pos);
            if let Some(g) = self.gates.iter().find(|g| {
                g.name.is_some()
                    && self.active(g.p)
                    && get::<[f32; 3]>(g.p + self.pos) == pos
                    && (!entering
                        || (get::<f32>(g.p + self.enabled_now) > 0.
                            && g.name
                                .as_deref()
                                .is_some_and(|n| !n.is_empty() && has_scene(n))))
            }) {
                return Some(g);
            }
        }
        None
    }
}
pub unsafe fn advance(dt: f32) {
    if !enabled() {
        return;
    }
    STATE.with_borrow_mut(|s| {
        let m = s.as_mut().unwrap();
        // The headless scheduler omits the presentation update at 0x1404133a9.
        // Its live value is gameplay state too: the entry action at 0x14024883e
        // reads enabled_t, not the dependency system's enabled_t_target.
        for g in m.gates.iter().filter(|g| g.name.is_some()) {
            let target = get::<f32>(g.p + m.enabled);
            let live = get::<f32>(g.p + m.enabled_now);
            if live < target {
                put(g.p + m.enabled_now, (live + 7. * dt).min(target));
            } else if live > target {
                put(g.p + m.enabled_now, (live - 10. * dt).max(target));
            }
        }
    });
}
// Return true when the V press entered a level (including an already solved
// one), so it isn't also queued as a native take/interact action.
pub unsafe fn enter() -> bool {
    if !enabled() {
        return false;
    }
    STATE.with_borrow_mut(|s| {
        let m = s.as_mut().unwrap();
        let Some(g) = m.standing(true) else {
            return false;
        };
        if m.done.contains(&g.id) {
            return true;
        }
        put(g.p + m.solved, 1f32);
        let (id, name) = (g.id, g.name.clone().unwrap());
        m.done.insert(id);
        m.visits.push(name);
        // Instant completion satisfies only entry prerequisites. A Sign or a
        // story marker cannot be satisfied merely by walking over a puzzle.
        for g in &m.gates {
            if !g.deps.is_empty()
                && (if g.any {
                    g.deps.iter().any(|d| m.done.contains(d))
                } else {
                    g.deps.iter().all(|d| m.done.contains(d))
                })
            {
                if g.name.is_some() {
                    put(g.p + m.enabled, 1f32)
                } else {
                    put(g.p + m.thaw, 1u8)
                }
            }
        }
        true
    })
}
pub fn visit_count() -> usize {
    STATE.with_borrow(|s| s.as_ref().map_or(0, |m| m.visits.len()))
}
#[allow(dead_code)] // Only challenge scoring distinguishes locked from unvisited entries.
pub unsafe fn unlocked_levels() -> std::collections::BTreeSet<String> {
    STATE.with_borrow(|s| {
        s.iter()
            .flat_map(|m| {
                m.gates.iter().filter_map(|g| {
                    let name = g.name.as_deref()?;
                    (m.active(g.p)
                        && get::<f32>(g.p + m.enabled) > 0.
                        && !matches!(name, "intro" | "overworld")
                        && has_scene(name))
                    .then(|| name.to_owned())
                })
            })
            .collect()
    })
}
pub unsafe fn report(j: &mut Value, challenge: bool) {
    STATE.with_borrow(|s| {
        if let Some(m) = s {
            if let Some(name) = m.standing(false).and_then(|g| g.name.as_deref()) {
                j["standing_level"] = name.into();
            }
            if challenge {
                let mut seen = HashSet::new();
                let playable: Vec<_> = m
                    .visits
                    .iter()
                    .map(String::as_str)
                    .filter(|&name| !matches!(name, "intro" | "overworld") && seen.insert(name))
                    .collect();
                j["playable_levels"] = serde_json::json!(playable);
            } else if enabled() {
                j["visited"] = serde_json::json!(m.visits);
            }
        }
    });
}

// Sample live entities, including rocks created by transmutation. Authored
// positions alone cannot distinguish a held switch from the same empty tile.
pub unsafe fn inspect(radius: u8) -> Value {
    use serde_json::json;
    let schema = init::json("meta/layout");
    let selected = [
        "Guy",
        "Rock",
        "Mirror",
        "Tree",
        "Gate",
        "Switch",
        "Freezer",
        "Gem",
        "Gem_Slot",
        "Promesst_Light",
        "Monster",
        "Wardrobe",
        "Activator",
        "Spawner",
        "Teleporter",
        "Lily",
    ];
    let types: std::collections::HashMap<_, _> = selected
        .iter()
        .map(|&k| (schema[k]["address"].as_u64().unwrap() as usize, k))
        .collect();
    let off = |k: &str, f: &str| schema[k]["fields"][f][0].as_u64().unwrap() as usize;
    STATE.with_borrow(|s| {
        let m = s.as_ref().unwrap();
        let origin = get::<[f32; 3]>(m.guy + m.pos);
        let mut objects = vec![];
        let table = get::<usize>(W + 0x378);
        for i in 0..get::<usize>(W + 0x370) {
            let p = get::<usize>(table + 8 * i);
            let Some(&k) = types.get(&get::<usize>(p + layout::TYPE)) else {
                continue;
            };
            let pos = get::<[f32; 3]>(p + m.pos);
            if pos
                .iter()
                .zip(origin)
                .any(|(a, b)| (a - b).abs() > f32::from(radius))
            {
                continue;
            }
            let mut obj = json!({"uid":get::<u32>(p+layout::UID), "kind":k, "position":pos});
            let flags = get::<u64>(p + m.flags);
            obj["active"] = json!(
                flags & ((1 << 21) | (1 << 29)) == 0
                    && (flags & (1 << 25) == 0 || flags & (1 << 26) != 0)
            );
            let flag = match k {
                "Teleporter" => Some("discovered"),
                "Gate" => Some("open"),
                "Switch" => Some("pressed"),
                "Freezer" => Some("unfrozen"),
                "Gem_Slot" => Some("room_powered"),
                "Promesst_Light" => Some("powered"),
                _ => None,
            };
            if let Some(f) = flag {
                obj[f] = json!(get::<u8>(p + off(k, f)) != 0);
            }
            if k == "Teleporter" {
                obj["target_id"] = json!(get::<u32>(p + off(k, "target_id")));
            }
            if k == "Gem_Slot" {
                obj["power_flags"] = json!(get::<u32>(p + off(k, "flags")));
            }
            if k == "Gem" {
                obj["power_flags"] = json!(get::<u8>(p + off(k, "power_flags")));
            }
            if k == "Promesst_Light" {
                obj["power_kind"] = json!(get::<u64>(p + off(k, "color")));
            }
            if k == "Mirror" {
                obj["angle"] = json!(get::<f32>(p + off(k, "theta_target")));
            }
            if k == "Rock" {
                obj["properties"] = json!(get::<u16>(p + off(k, "properties")));
            }
            if matches!(k, "Rock" | "Monster") {
                obj["charmed_by"] = json!(get::<u32>(p + off(k, "charmed_by")));
            }
            objects.push(obj);
        }
        objects.sort_by_key(|o| o["uid"].as_u64());
        json!({"position":origin, "skill_flags":get::<u32>(m.guy+off("Guy","skill_flags")),
               "inventory_flags":get::<u32>(m.guy+off("Guy","inventory_flags")),
               "queued_inputs":get::<usize>(W+layout::INPUT_QUEUE), "objects":objects})
    })
}
