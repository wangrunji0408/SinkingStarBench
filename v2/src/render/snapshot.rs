use super::io::*;
use serde_json::{Value as J, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
};
pub(super) const FIELDS: [&str; 18] = [
    "position.x",
    "position.y",
    "position.z",
    "scale.x",
    "scale.y",
    "scale.z",
    "orientation.x",
    "orientation.y",
    "orientation.z",
    "orientation.w",
    "tint_color_a.x",
    "tint_color_a.y",
    "tint_color_a.z",
    "tint_color_a.w",
    "tint_color_b.x",
    "tint_color_b.y",
    "tint_color_b.z",
    "tint_color_b.w",
];
#[derive(Default)]
struct TrailerGroups {
    guys: Vec<u64>,
    gates: Vec<u64>,
    gems: Vec<u64>,
    mechanisms: Vec<u64>,
}
impl TrailerGroups {
    fn from_entities(entities: &BTreeMap<u64, J>) -> Self {
        let mut result = Self::default();
        for (&id, e) in entities {
            if e["exists"] != true || e["dead"] != false {
                continue;
            }
            match e["type"].as_str() {
                Some("Guy") => result.guys.push(id),
                Some("Gate") => result.gates.push(id),
                Some("Gem") => result.gems.push(id),
                Some("Switch" | "Door" | "Checkpoint" | "Teleporter") => result.mechanisms.push(id),
                _ => (),
            }
        }
        result
    }
}
pub struct Scene {
    groups: TrailerGroups,
    transmuted: BTreeSet<u64>,
    pub entities: BTreeMap<u64, J>,
    initial: HashMap<u64, J>,
    auth: J,
    base: Vec<u8>,
    patch_overworld: bool,
    offsets: HashMap<u64, usize>,
    charm: J,
    bard: J,
    entry: J,
    active: Vec<J>,
    templates: HashSet<String>,
    ongoing: HashMap<(String, u64, u64), (u64, f64)>,
    serial: u64,
}
impl Scene {
    pub fn new(init: &J, setup: &J) -> Self {
        Self {
            entities: BTreeMap::new(),
            groups: TrailerGroups::default(),
            transmuted: BTreeSet::new(),
            initial: vecj(&init["entities"])
                .iter()
                .map(|e| (num(&e["id"]), e["f"].clone()))
                .collect(),
            auth: setup["authored"].clone(),
            base: vec![],
            patch_overworld: init["level"] == "overworld",
            offsets: HashMap::new(),
            charm: J::Null,
            bard: J::Null,
            entry: init["initialVisualEvents"].clone(),
            active: vec![],
            templates: vecj(&setup["templates"])
                .iter()
                .map(|v| v.as_str().unwrap().into())
                .collect(),
            ongoing: HashMap::new(),
            serial: 0,
        }
    }
    pub fn update(&mut self, dir: &Path, w: &mut super::assets::Assets, f: &J) -> Res<Option<J>> {
        let mut shape = self.base.is_empty() || self.bard != f["bardVisual"]["targets"];
        self.bard = f["bardVisual"]["targets"].clone();
        let first = self.base.is_empty();
        let now = val(&f["gameSeconds"]);
        let mut events = vec![];
        if first {
            events.extend(
                vecj(&self.entry)
                    .iter()
                    .filter(|e| num(&e["mode"]) == 0)
                    .cloned(),
            )
        }
        events.extend(vecj(&f["visualEvents"]).iter().cloned());
        for e in vecj(&f["changes"]) {
            let id = num(&e["id"]);
            if let Some(old) = self.entities.get(&id) {
                shape |= ["type", "exists", "dead"].iter().any(|&k| old[k] != e[k])
                    || ["entity_flags", "mesh", "materials"]
                        .iter()
                        .any(|&k| old["f"][k] != e["f"][k]);
                let a = &e["f"];
                let b = &old["f"];
                let effect = match e["type"].as_str().unwrap_or("") {
                    "Gate" if num(&a["silent"]) == 0 && a["open"] != b["open"] => Some(format!(
                        "Gate{}{}",
                        if num(&a["open"]) != 0 {
                            "Open"
                        } else {
                            "Closed"
                        },
                        if num(&a["is_monster_gate"]) != 0 {
                            "_Monster"
                        } else {
                            ""
                        }
                    )),
                    "Door" if num(&a["open"]) > num(&b["open"]) => Some("exit_open".into()),
                    "Teleporter"
                        if val(&a["enabled_t_target"]) > 0.
                            && val(&b["enabled_t_target"]) <= 0. =>
                    {
                        Some("EndgameTeleporterUnlocked".into())
                    }
                    "Level_Entry"
                        if val(&a["enabled_t_target"]) > 0.
                            && val(&b["enabled_t_target"]) <= 0. =>
                    {
                        Some(
                            if num(&a["tower_id"]) != 0 {
                                "LevelUnlocksTowers"
                            } else {
                                "LevelUnlocksLevels"
                            }
                            .into(),
                        )
                    }
                    "Overworld_Unlock" if num(&a["activated"]) > num(&b["activated"]) => {
                        Some("UnlockLevel".into())
                    }
                    "Guy"
                        if val(&b["velocity.z"]) < -2.
                            && val(&a["velocity.z"]).abs() < 1e-5
                            && e["dead"] == false =>
                    {
                        Some("LandingImpact".into())
                    }
                    "Monster" if old["dead"] == false && e["dead"] == true => {
                        Some("MonsterDissolve".into())
                    }
                    "Freezer" if old["exists"] == true && e["exists"] == false => {
                        Some("RemoveFreezer".into())
                    }
                    "Mirror" if old["exists"] == true && e["exists"] == false => Some(
                        if val(&e["p"][2]) < val(&old["p"][2]).min(0.) {
                            "MirrorSink"
                        } else {
                            "MirrorAsh"
                        }
                        .into(),
                    ),
                    _ => None,
                };
                if let Some(name) = effect {
                    events.push(json!({"name":name,"position":e["p"],"orientation":[a["orientation.x"],a["orientation.y"],a["orientation.z"],a["orientation.w"]],"attach":id,"time":now,"mode":if e["type"]=="Gate"{1}else{0}}));
                }
            } else if e["type"] != "Temporary_Splash" {
                shape = true
            }
            if e["exists"] == true
                && e["dead"] == false
                && val(&e["f"]["entity_params.druid_transmute_t"]) != 0.
            {
                self.transmuted.insert(id);
            } else {
                self.transmuted.remove(&id);
            }
            self.entities.insert(id, e.clone());
        }
        for id in vecj(&f["removed"]) {
            self.transmuted.remove(&num(id));
            if let Some(e) = self.entities.remove(&num(id)) {
                shape |= e["type"] != "Temporary_Splash";
            }
        }
        if shape {
            self.groups = TrailerGroups::from_entities(&self.entities);
        }
        self.active.retain(|e| val(&e["time"]) >= now - 8.);
        for mut e in events {
            self.serial += 1;
            e["id"] = self.serial.into();
            self.active.push(e);
        }
        let mut all = self.active.clone();
        let mut cont = vec![];
        for e in vecj(&self.entry).iter().filter(|e| num(&e["mode"]) == 2) {
            if let Some(owner) = self.entities.get(&num(&e["attach"])) {
                if owner["exists"] == true
                    && owner["dead"] == false
                    && num(&owner["f"]["activated"]) == 0
                    && num(&owner["f"]["found"]) == 0
                {
                    let mut ev = e.clone();
                    ev["position"] = owner["p"].clone();
                    cont.push(ev);
                }
            }
        }
        let mut add = |id: u64, name: &str, mode: u64, d: u64, z: f64| {
            if let Some(e) = self.entities.get(&id) {
                let mut pos = e["p"].clone();
                pos[2] = (val(&pos[2]) + z).into();
                cont.push(json!({"name":name,"position":pos,"orientation":[0,0,0,1],"attach":id,"mode":mode,"destination":d}));
            }
        };
        for id in &self.groups.gems {
            let a = &self.entities[id]["f"];
            if val(&a["on_t"]) * val(&a["room_on_t"]) > 0.01 {
                add(*id, "GemAura", 2, 0, 0.);
            }
        }
        for id in vecj(&f["bardVisual"]["bards"]) {
            add(num(id), "BardRadialParticles", 3, 0, 0.)
        }
        for e in vecj(&f["bardVisual"]["targets"]) {
            add(
                num(&e["id"]),
                "CharmedGuyTravelingNotes",
                4,
                num(&e["bard"]),
                1.4,
            )
        }
        let mut next = HashMap::new();
        for mut e in cont {
            let key = (
                e["name"].as_str().unwrap().to_string(),
                num(&e["attach"]),
                num(&e["destination"]),
            );
            let (id, t) = *self.ongoing.get(&key).unwrap_or_else(|| {
                self.serial += 1;
                next.entry(key.clone()).or_insert((self.serial, now))
            });
            next.insert(key, (id, t));
            e["id"] = id.into();
            e["time"] = t.into();
            all.push(e);
        }
        self.ongoing = next;
        let mut resources = None;
        if shape {
            let r = w.world(&self.entities, &self.initial, vecj(&self.bard))?;
            self.charm = r["charm"].clone();
            let path = dir.join("world.snapshot");
            let full = fs::read(&path)?;
            let n = u32::from_le_bytes(full[..4].try_into()?) as usize;
            self.offsets.clear();
            self.base.clear();
            if self.patch_overworld {
                // Keep the complete authored state for worker restarts. Only the
                // live replay entities and synthetic charm props can change.
                fs::rename(&path, dir.join("world.snapshot.seed"))?;
                put_u(&mut self.base, 0x3150574f);
                put_u(&mut self.base, 0);
                let charms: HashSet<u64> = self
                    .charm
                    .as_object()
                    .into_iter()
                    .flat_map(|m| m.keys())
                    .map(|k| k.parse())
                    .collect::<Result<_, _>>()?;
                for (i, row) in full[4..4 + 76 * n].chunks_exact(76).enumerate() {
                    let id = u32::from_le_bytes(row[..4].try_into()?) as u64;
                    if self.entities.contains_key(&id) || charms.contains(&id) {
                        put_u(&mut self.base, i as u64);
                        self.offsets.insert(id, self.base.len());
                        self.base.extend_from_slice(&row[4..]);
                    }
                }
                let count = ((self.base.len() - 8) / 76) as u32;
                self.base[4..8].copy_from_slice(&count.to_le_bytes());
            } else {
                self.base.extend_from_slice(&full[..4 + 76 * n]);
                for i in 0..n {
                    let o = 4 + i * 76;
                    self.offsets
                        .insert(u32::from_le_bytes(full[o..o + 4].try_into()?) as u64, o + 4);
                }
            }
            resources = Some(r);
        }
        for e in vecj(&f["changes"]) {
            let id = num(&e["id"]);
            let Some(&o) = self.offsets.get(&id) else {
                continue;
            };
            let live = &e["f"];
            let before = self.initial.get(&id);
            let auth = &self.auth[id.to_string()];
            for (j, k) in FIELDS.iter().enumerate() {
                let mut n = if auth.is_array() && before.is_some_and(|v| v[*k] == live[*k]) {
                    val(&auth[j])
                } else {
                    val(&live[*k])
                };
                if e["type"] == "Guy"
                    && before.is_some_and(|v| v["theta_current"] != live["theta_current"])
                    && (6..10).contains(&j)
                {
                    let a = val(&live["theta_current"]).to_radians() * 0.5;
                    n = [0., 0., a.sin(), a.cos()][j - 6];
                }
                self.base[o + j * 4..o + j * 4 + 4].copy_from_slice(&(n as f32).to_le_bytes());
            }
        }
        if let Some(map) = self.charm.as_object() {
            for (id, v) in map {
                if let (Some(&o), Some(e)) = (
                    self.offsets.get(&id.parse()?),
                    self.entities.get(&num(&v[0])),
                ) {
                    for j in 0..3 {
                        self.base[o + j * 4..o + j * 4 + 4].copy_from_slice(
                            &((val(&e["p"][j]) + val(&v[1][j])) as f32).to_le_bytes(),
                        );
                    }
                }
            }
        }
        let mut b = self.base.clone();
        self.trailer(&mut b, f, &all);
        fs::write(dir.join("world.snapshot"), b)?;
        Ok(resources)
    }
    fn trailer(&self, b: &mut Vec<u8>, f: &J, events: &[J]) {
        put_u(b, 0x53544154);
        put_u(b, num(&f["family"]));
        put_u(b, self.groups.guys.len() as u64);
        for id in &self.groups.guys {
            let e = &self.entities[id];
            put_u(b, num(&e["id"]));
            put_u(b, num(&e["f"]["turn_order_id"]))
        }
        for (key, tag) in [("mirages", 0x4d495252), ("beams", 0x4245414d)] {
            put_u(b, tag);
            put_u(b, vecj(&f[key]).len() as u64);
            for e in vecj(&f[key]) {
                let rec = b64(e["record"].as_str().unwrap());
                let seg = b64(e["segments"].as_str().unwrap());
                if key == "beams" {
                    put_u(b, num(&e["id"]));
                    put_u(b, num(&e["offset"]));
                } else {
                    b.extend(&rec)
                }
                put_u(b, num(&e["origin"]));
                put_u(b, (seg.len() / 36) as u64);
                if key == "beams" {
                    b.extend(rec)
                }
                b.extend(seg)
            }
        }
        for (kind, tag, ids) in [
            ("Gate", 0x47415445, &self.groups.gates),
            ("Guy", 0x41555241, &self.groups.guys),
            ("Gem", 0x47454d53, &self.groups.gems),
        ] {
            put_u(b, tag);
            put_u(b, ids.len() as u64);
            for id in ids {
                let e = &self.entities[id];
                put_u(b, num(&e["id"]));
                let a = &e["f"];
                match kind {
                    "Gate" => {
                        put_u(b, num(&a["open"]));
                        put_f(b, val(&a["open_t"]));
                    }
                    "Guy" => put_f(b, val(&a["protected_fade_t"])),
                    _ => put_f(b, val(&a["on_t"]) * val(&a["room_on_t"])),
                }
            }
        }
        let bard = &f["bardVisual"];
        put_u(b, 0x42415244);
        put_u(b, vecj(&bard["bards"]).len() as u64);
        put_u(b, vecj(&bard["targets"]).len() as u64);
        for e in vecj(&bard["bards"]) {
            put_u(b, num(e));
        }
        for e in vecj(&bard["targets"]) {
            put_u(b, num(&e["id"]));
            put_u(b, num(&e["bard"]))
        }
        let mechs = &self.groups.mechanisms;
        put_u(b, 0x4d454348);
        put_u(b, mechs.len() as u64);
        for id in mechs {
            let e = &self.entities[id];
            let k = match e["type"].as_str().unwrap() {
                "Switch" => ["pressed", "", "", ""],
                "Door" => ["open", "enabled_state", "glow_t_target", "broken_t"],
                "Checkpoint" => ["triggered", "", "", ""],
                _ => ["discovered", "", "enabled_t_target", ""],
            };
            put_u(b, num(&e["id"]));
            let a = &e["f"];
            put_u(b, num(&a[k[0]]));
            put_u(b, num(&a[k[1]]));
            put_f(b, val(&a[k[2]]));
            put_f(b, val(&a[k[3]]))
        }
        put_u(b, self.transmuted.len() as u64);
        for id in &self.transmuted {
            let e = &self.entities[id];
            put_u(b, num(&e["id"]));
            put_f(b, val(&e["f"]["entity_params.druid_transmute_t"]))
        }
        let ev: Vec<_> = events
            .iter()
            .filter(|e| self.templates.contains(e["name"].as_str().unwrap_or("")))
            .collect();
        put_u(b, 0x46585431);
        b.extend(val(&f["gameSeconds"]).to_le_bytes());
        put_u(b, ev.len() as u64);
        for e in ev {
            put_u(b, num(&e["id"]));
            b.extend(val(&e["time"]).to_le_bytes());
            for k in ["position", "orientation"] {
                for x in vecj(&e[k]) {
                    put_f(b, val(x));
                }
            }
            put_u(b, num(&e["attach"]));
            put_u(b, u64::from(e["group"] == true));
            put_u(b, num(&e["mode"]));
            put_u(b, num(&e["destination"]));
            let n = e["name"].as_str().unwrap();
            put_u(b, n.len() as u64);
            b.extend(n.as_bytes());
        }
    }
}
