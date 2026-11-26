pub mod anim;
mod catalog;
mod format;
mod material;
mod world;
use super::io::*;
use format::Package;
use serde_json::{Value as J, json};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use world::{E, World};
fn word(b: &mut Vec<u8>, n: usize) {
    b.extend((n as u32).to_le_bytes());
}
fn blob(b: &mut Vec<u8>, v: &[u8]) {
    word(b, v.len());
    b.extend(v);
}
#[derive(Default, Clone)]
struct Dependencies {
    meshes: BTreeSet<String>,
    textures: BTreeSet<String>,
    materials: BTreeSet<String>,
    pose_meshes: BTreeMap<String, bool>,
}
impl Dependencies {
    fn visit(&mut self, e: &E, known: &BTreeSet<String>) -> Res<()> {
        let mesh = e.name(31);
        if !mesh.is_empty() {
            self.meshes.insert(mesh.clone());
            *self.pose_meshes.entry(mesh).or_default() |= e.kind == "Guy";
        }
        scan(&e.tail, known, "compressed_mesh", &mut self.meshes);
        scan(&e.tail, known, "texture", &mut self.textures);
        let mut r = format::R(e.value(32));
        for _ in 0..r.q().unwrap_or(0) {
            self.materials.insert(r.long()?);
        }
        Ok(())
    }
}
struct Scenery {
    deps: Dependencies,
    snapshots: Vec<u8>,
}
pub struct Assets {
    scenery: Option<Scenery>,
    pub game: PathBuf,
    pub dir: PathBuf,
    pub anim: Package,
    world: World,
    fx: World,
    types: J,
    mat: material::Materials,
    fxtex: BTreeSet<String>,
    known: BTreeSet<String>,
    cache: BTreeMap<String, Vec<String>>,
}
impl Assets {
    pub fn new(game: &Path, dir: &Path, level: &str) -> Res<Self> {
        let p = Package::new(game.join("data/levels.package"))?;
        let world = World::load(&p, level)?;
        let fx = World::load(&p, "ingame_particles")?;
        let mat = material::build(game, dir)?;
        let known = fs::read_dir(game.join("data-pc"))?
            .map(|e| Ok(e?.file_name().to_string_lossy().into_owned()))
            .collect::<std::io::Result<_>>()?;
        fs::create_dir(dir.join("poses"))?;
        Ok(Self {
            scenery: None,
            game: game.into(),
            dir: dir.into(),
            anim: Package::new(game.join("data/animation.package"))?,
            world,
            fx,
            mat,
            types: serde_json::from_str(include_str!(concat!(
                env!("OUT_DIR"),
                "/render-types.json"
            )))?,
            fxtex: BTreeSet::new(),
            known,
            cache: BTreeMap::new(),
        })
    }
    // Run alongside the simulation's initial sample. Inanimate entities are
    // presentation-only; reuse this work only if every one survived unchanged.
    pub fn prepare_overworld_scenery(&mut self) -> Res<()> {
        let mut deps = Dependencies::default();
        let mut snapshots = Vec::new();
        for e in self.world.ents.iter().filter(|e| e.kind == "Inanimate") {
            deps.visit(e, &self.known)?;
            for i in world::indices() {
                snapshots.extend(e.float(i).to_le_bytes());
            }
        }
        for m in &deps.meshes {
            let (names, _) = format::mesh_info(&format::asset(&self.game, m, "compressed_mesh")?)
                .map_err(|e| format!("mesh {m}: {e}"))?;
            self.cache.insert(m.clone(), names);
        }
        self.scenery = Some(Scenery { deps, snapshots });
        Ok(())
    }
    fn groups(&self) -> BTreeMap<String, Vec<E>> {
        let mut groups = BTreeMap::new();
        let mut ids = BTreeMap::new();
        for e in self.fx.ents.iter().filter(|e| e.kind == "Group") {
            let name = e.name(11);
            ids.insert(e.id, name.clone());
            groups.insert(name, vec![e.clone()]);
        }
        for e in self.fx.ents.iter().filter(|e| e.kind == "Particle_Source") {
            if let Some(n) = ids.get(&(e.float(12).to_bits() as usize)) {
                groups.get_mut(n).unwrap().push(e.clone());
            }
        }
        groups
    }
    pub fn setup(&mut self, initial: &J, t: &J) -> Res<J> {
        let ids: HashSet<_> = vecj(&initial["entities"])
            .iter()
            .map(|e| num(&e["id"]) as usize)
            .collect();
        let mut authored = json!({});
        for e in &self.world.ents {
            if ids.contains(&e.id) {
                authored[e.id.to_string()] =
                    json!(world::indices().map(|i| e.float(i)).collect::<Vec<_>>());
            }
        }
        let mut names: BTreeSet<_> = vecj(&t["effectNames"])
            .iter()
            .filter_map(J::as_str)
            .collect();
        names.extend("CharmedGuyParticles BardRadialParticles CharmedGuyTravelingNotes GemAura GateOpen GateClosed GateOpen_Monster GateClosed_Monster UnlockLevel exit_open EndgameTeleporterUnlocked LevelUnlocksTowers LevelUnlocksLevels RemoveFreezer MonsterDissolve MirrorSink MirrorAsh LandingImpact".split_whitespace());
        let groups = self.groups();
        let mut records = vec![];
        let mut count = 0;
        for n in names {
            if let Some(rows) = groups.get(n) {
                for mut e in rows.clone() {
                    if e.kind == "Group" {
                        e.set(31, world::string(""));
                        e.set(32, world::strings([]));
                    }
                    scan(&e.tail, &self.known, "texture", &mut self.fxtex);
                    e.record(&mut records, &self.types)?;
                    count += 1;
                }
            }
        }
        let mut b = self.fx.base.to_le_bytes().to_vec();
        word(&mut b, count);
        b.extend(records);
        fs::write(self.dir.join("world.effects"), b)?;
        Ok(
            json!({"authored":authored,"templates":groups.iter().filter(|(_,v)|v.len()>1).map(|(n,_)|n).collect::<Vec<_>>()}),
        )
    }
    pub fn rig(&self, mesh: &str) -> Res<J> {
        anim::rig(&self.anim, mesh)
    }
    pub fn world(
        &mut self,
        live: &BTreeMap<u64, J>,
        initial: &HashMap<u64, J>,
        targets: &[J],
    ) -> Res<J> {
        let mut ents = vec![];
        for original in &self.world.ents {
            // All gameplay signs are in the initial trace except signs removed
            // by the retail map loader. Do not restore them from scenery.
            if original.kind == "Sign" && !initial.contains_key(&(original.id as u64)) {
                continue;
            }
            let mut e = Cow::Borrowed(original);
            if let Some(v) = live.get(&(e.id as u64)) {
                if v["type"] != e.kind {
                    continue;
                }
                if e.kind != "Settings" && (v["dead"] == true || v["exists"] == false) {
                    continue;
                }
                if let Some(before) = initial.get(&(e.id as u64)) {
                    e.to_mut().apply(v, before);
                }
            } else if e.kind != "Settings" && initial.contains_key(&(e.id as u64)) {
                continue;
            }
            ents.push(e);
        }
        let ids: BTreeSet<_> = ents.iter().map(|e| e.id).collect();
        for (&id, v) in live {
            let id = id as usize;
            let k = v["type"].as_str().ok_or("missing entity type")?;
            if ids.contains(&id)
                || v["dead"] == true
                || v["exists"] == false
                || matches!(k, "Settings" | "Level_State" | "Temporary_Splash")
            {
                continue;
            }
            let mut e = E {
                id,
                kind: k.into(),
                ver: *self
                    .world
                    .versions
                    .get(k)
                    .or_else(|| self.fx.versions.get(k))
                    .unwrap_or(&0),
                fields: BTreeMap::new(),
                source: None,
                tail: vec![],
            };
            e.apply(v, &J::Null);
            ents.push(Cow::Owned(e));
        }
        let mut charm = json!({});
        if !targets.is_empty() {
            let g = self.groups();
            let t = &g["CharmedGuyParticles"];
            let mut id = 0xfffffffe;
            for v in targets {
                while live.contains_key(&(id as u64)) || ids.contains(&id) {
                    id -= 1
                }
                let target = num(&v["id"]);
                let mut e = t[1].clone();
                e.id = id;
                e.editable().remove(&12);
                let off: [f32; 3] = std::array::from_fn(|i| {
                    e.float(i) - t[0].float(i) + if i == 2 { 1.4 } else { 0. }
                });
                for (i, x) in off.iter().enumerate() {
                    e.set(
                        i,
                        (val(&live[&target]["p"][i]) as f32 + x)
                            .to_le_bytes()
                            .into(),
                    );
                }
                ents.push(Cow::Owned(e));
                charm[id.to_string()] = json!([target, off]);
                id -= 1;
            }
        }
        let mut w = self.world.base.to_le_bytes().to_vec();
        word(&mut w, ents.len());
        let mut snap = vec![];
        word(&mut snap, ents.len());
        let scenery = self.scenery.as_ref().filter(|s| {
            let mut count = 0;
            for entity in &ents {
                if entity.kind == "Inanimate" {
                    if !matches!(entity, Cow::Borrowed(_)) {
                        return false;
                    }
                    count += 1;
                }
            }
            count * 72 == s.snapshots.len()
        });
        let mut deps = scenery.map(|s| s.deps.clone()).unwrap_or_default();
        let mut cached = scenery.map(|s| s.snapshots.chunks_exact(72));
        for e in ents {
            e.record(&mut w, &self.types)?;
            word(&mut snap, e.id);
            if e.kind == "Inanimate" {
                if let Some(rows) = &mut cached {
                    snap.extend_from_slice(rows.next().unwrap());
                    continue;
                }
            }
            for i in world::indices() {
                snap.extend(e.float(i).to_le_bytes());
            }
            deps.visit(&e, &self.known)?;
        }
        let Dependencies {
            mut meshes,
            textures: mut tex,
            materials: mut mats,
            pose_meshes,
        } = deps;
        tex.extend(
            include_str!("../../../gpu/textures.txt")
                .lines()
                .map(str::to_owned),
        );
        tex.extend(self.fxtex.iter().cloned());
        for (mesh, required) in pose_meshes {
            self.idle(&mesh, required)?;
        }
        fs::write(self.dir.join("world.bin"), w)?;
        fs::write(self.dir.join("world.snapshot"), snap)?;
        let mut b = vec![];
        let used = meshes.clone();
        meshes.extend(
            include_str!("../../../gpu/meshes.txt")
                .lines()
                .map(str::to_owned),
        );
        word(&mut b, meshes.len());
        for m in meshes {
            let p = self
                .game
                .join("data-pc")
                .join(format!("{m}.compressed_mesh"));
            if !self.cache.contains_key(&m) {
                let (names, _) =
                    format::mesh_info(&format::asset(&self.game, &m, "compressed_mesh")?)
                        .map_err(|e| format!("mesh {m}: {e}"))?;
                self.cache.insert(m.clone(), names);
            }
            let names = &self.cache[&m];
            blob(&mut b, m.as_bytes());
            catalog::external(&mut b, &p);
            word(&mut b, names.len());
            for n in names {
                blob(&mut b, n.as_bytes());
                if used.contains(&m) {
                    mats.insert(n.clone());
                }
            }
        }
        let eager = self
            .scenery
            .is_some()
            .then(|| tex.union(&self.mat.always_resident).cloned().collect());
        for n in mats {
            if let Some(ts) = self.mat.textures.get(&n) {
                tex.extend(ts.iter().cloned());
            }
        }
        fs::write(self.dir.join("meshes.bin"), b)?;
        catalog::textures(&self.game, &self.dir, tex, eager.as_ref())?;
        Ok(
            json!({"textures":self.dir.join("textures.bin"),"meshes":self.dir.join("meshes.bin"),"charm":charm}),
        )
    }
    fn idle(&self, m: &str, required: bool) -> Res<()> {
        let p = self.dir.join("poses").join(format!("{m}.bin"));
        if p.exists() {
            return Ok(());
        }
        let Some(c) = [anim::alias(m), m.trim_end_matches("_alt")]
            .iter()
            .map(|n| format!("{n}_active_idle_01"))
            .find(|c| self.anim.by_filename(&format!("{c}.anim")).is_some())
        else {
            if required {
                return Err(format!("missing idle clip for {m}").into());
            }
            return Ok(());
        };
        let a = anim::read(&self.anim, &self.game, m, &c)?;
        use super::poses::{matrix, mul};
        let mut global = vec![];
        for (i, p) in a.parents.iter().enumerate() {
            let mat = matrix(&a.samples[i]);
            global.push(if *p < 0 {
                mat
            } else {
                mul(&global[*p as usize], &mat)
            });
        }
        let mut b = vec![];
        word(&mut b, a.order.len());
        for (i, inv) in a.order.iter().zip(a.inv) {
            for f in &mul(&global[*i], &inv)[..12] {
                b.extend(f.to_le_bytes());
            }
        }
        fs::write(p, b)?;
        Ok(())
    }
}
fn scan(b: &[u8], known: &BTreeSet<String>, ext: &str, out: &mut BTreeSet<String>) {
    for raw in b
        .split(|c| !(32..127).contains(c))
        .filter(|s| (3..240).contains(&s.len()))
    {
        let n = std::str::from_utf8(raw).unwrap();
        if known.contains(&format!("{n}.{ext}")) {
            out.insert(n.into());
        }
    }
}
